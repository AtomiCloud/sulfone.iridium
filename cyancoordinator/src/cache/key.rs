//! Content-addressed cache key derivation for a single template node execution.
//!
//! The key is a SHA-256 digest (lowercase hex) over a *canonical* serialization of
//! everything that can change a node's output: the template identity + published
//! version, the node's full effective input (the preset-merged answers and the
//! shared deterministic states), the node's pinned plugins/processors, and a
//! format-version marker. Canonicalization uses `BTreeMap`-ordered serde so the
//! key is stable regardless of map ordering. Raw registry names never appear in
//! the returned digest, so the key is safe to use as a filesystem path. (FR5, FR11)

use std::collections::BTreeMap;

use cyanprompt::domain::models::answer::Answer;
use cyanregistry::http::models::template_res::TemplateVersionRes;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Bump this whenever the cached value layout or key composition changes, so old
/// entries can never be mistaken for new ones. (FR5)
const CACHE_FORMAT_VERSION: u32 = 1;

/// Stable identity of a pinned plugin/processor: the pieces that pin its behavior.
#[derive(Serialize)]
struct PinnedArtifact<'a> {
    id: &'a str,
    version: i64,
    docker_reference: &'a str,
    docker_tag: &'a str,
}

/// The template's own execution image, included in the key as defense-in-depth:
/// even though a published `id`+`version` is immutable, pinning the actual
/// Docker reference/tag makes a registry-image swap impossible to serve stale.
/// (L4) None for group/dev templates that carry no properties.
#[derive(Serialize)]
struct TemplateImages<'a> {
    blob_docker_reference: &'a str,
    blob_docker_tag: &'a str,
    template_docker_reference: &'a str,
    template_docker_tag: &'a str,
}

/// The canonical key material. Field order here is the serialization order
/// (serde_json preserves struct field declaration order); maps are `BTreeMap` so
/// their entries serialize in sorted key order regardless of insertion order.
///
/// # Deliberately excluded (load-bearing invariant)
/// `templates`, `resolvers`, and `commands` from `TemplateVersionRes` are NOT in
/// the key. This is safe because of how `execute_composition` consumes them:
/// - `templates` (nested/sub-template deps) are each a separate node with their
///   own key, resolved via `resolve_dependencies` and executed in their own loop
///   iteration — nesting does not change THIS node's archive output.
/// - `resolvers` only affect the layering/merge step, which FR10 guarantees
///   ALWAYS re-runs on the (cached or fresh) node outputs after the loop — so
///   cached vs fresh produce identical merged results regardless of resolvers.
/// - `commands` are post-execution metadata collected for later shell execution;
///   they never influence the archive or contributed state.
///
/// Because a change to any of these cannot change a node's cached output,
/// omitting them can never serve stale data. (FR10)
#[derive(Serialize)]
struct CacheKeyMaterial<'a> {
    format_version: u32,
    template_id: &'a str,
    template_version: i64,
    images: Option<TemplateImages<'a>>,
    answers: BTreeMap<&'a str, &'a Answer>,
    deterministic_states: BTreeMap<&'a str, &'a str>,
    plugins: Vec<PinnedArtifact<'a>>,
    processors: Vec<PinnedArtifact<'a>>,
}

/// Map a list of pinned plugin/processor versions into the canonical
/// `PinnedArtifact` form and sort it by (id, version) so ordering does not
/// affect the key. (L7)
fn pinned_artifacts<'a, T>(
    items: &'a [T],
    extract: impl Fn(&'a T) -> (&'a str, i64, &'a str, &'a str),
) -> Vec<PinnedArtifact<'a>> {
    let mut out: Vec<PinnedArtifact> = items
        .iter()
        .map(|p| {
            let (id, version, docker_reference, docker_tag) = extract(p);
            PinnedArtifact {
                id,
                version,
                docker_reference,
                docker_tag,
            }
        })
        .collect();
    out.sort_by(|a, b| (a.id, a.version).cmp(&(b.id, b.version)));
    out
}

/// Compute the content-addressed cache key for a node execution.
///
/// `answers` is the node's full effective input (preset-merged-with-inherited
/// answers — i.e. the `template_answers` map `execute_composition` assembles).
/// `deterministic_states` is the shared deterministic-state map. Both are
/// canonicalized so that reordering them does not change the key. Returns the
/// lowercase-hex SHA-256 digest.
pub fn compute_key(
    template: &TemplateVersionRes,
    answers: &std::collections::HashMap<String, Answer>,
    deterministic_states: &std::collections::HashMap<String, String>,
) -> String {
    let answers_sorted: BTreeMap<&str, &Answer> =
        answers.iter().map(|(k, v)| (k.as_str(), v)).collect();
    let states_sorted: BTreeMap<&str, &str> = deterministic_states
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let plugins = pinned_artifacts(&template.plugins, |p| {
        (&p.id, p.version, &p.docker_reference, &p.docker_tag)
    });
    let processors = pinned_artifacts(&template.processors, |p| {
        (&p.id, p.version, &p.docker_reference, &p.docker_tag)
    });

    // The template's own execution image (None for group/dev templates).
    let images = template
        .principal
        .properties
        .as_ref()
        .map(|p| TemplateImages {
            blob_docker_reference: &p.blob_docker_reference,
            blob_docker_tag: &p.blob_docker_tag,
            template_docker_reference: &p.template_docker_reference,
            template_docker_tag: &p.template_docker_tag,
        });

    let material = CacheKeyMaterial {
        format_version: CACHE_FORMAT_VERSION,
        template_id: &template.principal.id,
        template_version: template.principal.version,
        images,
        answers: answers_sorted,
        deterministic_states: states_sorted,
        plugins,
        processors,
    };

    // serde_json over the canonical material is deterministic: struct fields in
    // declaration order, BTreeMaps in sorted order, Vecs explicitly sorted.
    let canonical =
        serde_json::to_vec(&material).expect("cache key material is always serializable");

    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    let digest = hasher.finalize();
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cyanregistry::http::models::plugin_res::PluginVersionPrincipalRes;
    use cyanregistry::http::models::processor_res::ProcessorVersionPrincipalRes;
    use cyanregistry::http::models::template_res::{
        TemplatePrincipalRes, TemplatePropertyRes, TemplateVersionPrincipalRes, TemplateVersionRes,
    };
    use std::collections::HashMap;

    fn template(id: &str, version: i64) -> TemplateVersionRes {
        TemplateVersionRes {
            principal: TemplateVersionPrincipalRes {
                id: id.to_string(),
                version,
                created_at: "2025-01-01T00:00:00Z".to_string(),
                description: "test".to_string(),
                properties: Some(TemplatePropertyRes {
                    blob_docker_reference: "blob".to_string(),
                    blob_docker_tag: "latest".to_string(),
                    template_docker_reference: "tmpl".to_string(),
                    template_docker_tag: "latest".to_string(),
                }),
            },
            template: TemplatePrincipalRes {
                id: id.to_string(),
                name: "t".to_string(),
                project: "p".to_string(),
                source: "s".to_string(),
                email: "e".to_string(),
                tags: vec![],
                description: "d".to_string(),
                readme: "r".to_string(),
                user_id: "u".to_string(),
            },
            plugins: vec![],
            processors: vec![],
            templates: vec![],
            resolvers: vec![],
            commands: vec![],
        }
    }

    fn plugin(id: &str, version: i64) -> PluginVersionPrincipalRes {
        PluginVersionPrincipalRes {
            id: id.to_string(),
            version,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            description: "d".to_string(),
            docker_reference: "ref".to_string(),
            docker_tag: "tag".to_string(),
        }
    }

    fn processor(id: &str, version: i64) -> ProcessorVersionPrincipalRes {
        ProcessorVersionPrincipalRes {
            id: id.to_string(),
            version,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            description: "d".to_string(),
            docker_reference: "ref".to_string(),
            docker_tag: "tag".to_string(),
        }
    }

    fn answers(pairs: &[(&str, &str)]) -> HashMap<String, Answer> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Answer::String(v.to_string())))
            .collect()
    }

    // AC1: same inputs -> same key.
    #[test]
    fn same_inputs_same_key() {
        let t = template("tid", 3);
        let a = answers(&[("x", "1"), ("y", "2")]);
        let s: HashMap<String, String> = [("s1".to_string(), "v1".to_string())].into();
        assert_eq!(compute_key(&t, &a, &s), compute_key(&t, &a, &s));
    }

    // AC1: reordered answer/state maps -> same key (canonicalization).
    #[test]
    fn reordered_maps_same_key() {
        let t = template("tid", 3);
        // Two HashMaps built in different insertion order.
        let mut a1 = HashMap::new();
        a1.insert("a".to_string(), Answer::String("1".to_string()));
        a1.insert("b".to_string(), Answer::String("2".to_string()));
        a1.insert("c".to_string(), Answer::String("3".to_string()));
        let mut a2 = HashMap::new();
        a2.insert("c".to_string(), Answer::String("3".to_string()));
        a2.insert("b".to_string(), Answer::String("2".to_string()));
        a2.insert("a".to_string(), Answer::String("1".to_string()));

        let mut s1 = HashMap::new();
        s1.insert("k1".to_string(), "x".to_string());
        s1.insert("k2".to_string(), "y".to_string());
        let mut s2 = HashMap::new();
        s2.insert("k2".to_string(), "y".to_string());
        s2.insert("k1".to_string(), "x".to_string());

        assert_eq!(compute_key(&t, &a1, &s1), compute_key(&t, &a2, &s2));
    }

    // AC1: a change to version -> different key.
    #[test]
    fn version_change_differs() {
        let a = answers(&[("x", "1")]);
        let s = HashMap::new();
        assert_ne!(
            compute_key(&template("tid", 1), &a, &s),
            compute_key(&template("tid", 2), &a, &s)
        );
    }

    // AC1: a change to an answer -> different key.
    #[test]
    fn answer_change_differs() {
        let t = template("tid", 1);
        let s = HashMap::new();
        assert_ne!(
            compute_key(&t, &answers(&[("x", "1")]), &s),
            compute_key(&t, &answers(&[("x", "2")]), &s)
        );
    }

    // AC1: a change to a deterministic state -> different key.
    #[test]
    fn state_change_differs() {
        let t = template("tid", 1);
        let a = answers(&[("x", "1")]);
        let s1: HashMap<String, String> = [("s".to_string(), "a".to_string())].into();
        let s2: HashMap<String, String> = [("s".to_string(), "b".to_string())].into();
        assert_ne!(compute_key(&t, &a, &s1), compute_key(&t, &a, &s2));
    }

    // AC1: pinned plugins/processors are part of the key (and order-independent).
    #[test]
    fn pinned_artifacts_affect_key_but_order_does_not() {
        let a = answers(&[("x", "1")]);
        let s = HashMap::new();

        let mut base = template("tid", 1);
        let mut with_plugin = template("tid", 1);
        with_plugin.plugins = vec![plugin("p1", 1)];
        assert_ne!(
            compute_key(&base, &a, &s),
            compute_key(&with_plugin, &a, &s),
            "adding a pinned plugin must change the key"
        );

        // Same plugins in different order -> same key.
        base.plugins = vec![plugin("p1", 1), plugin("p2", 2)];
        let mut reordered = template("tid", 1);
        reordered.plugins = vec![plugin("p2", 2), plugin("p1", 1)];
        assert_eq!(compute_key(&base, &a, &s), compute_key(&reordered, &a, &s));

        let mut with_processor = template("tid", 1);
        with_processor.processors = vec![processor("proc", 1)];
        assert_ne!(
            compute_key(&template("tid", 1), &a, &s),
            compute_key(&with_processor, &a, &s),
            "adding a pinned processor must change the key"
        );
    }

    // FR11: the key is lowercase hex, fixed length, no path separators.
    #[test]
    fn key_is_safe_hex_digest() {
        let key = compute_key(
            &template("../etc/passwd", 1),
            &answers(&[]),
            &HashMap::new(),
        );
        assert_eq!(key.len(), 64, "sha-256 hex is 64 chars");
        assert!(
            key.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()),
            "key must be lowercase hex with no path-escaping characters"
        );
    }

    // L4: the template's own docker image is part of the key, so a registry
    // image swap (same id+version, different reference/tag) is never served stale.
    #[test]
    fn template_image_affects_key() {
        let a = answers(&[("x", "1")]);
        let s = HashMap::new();

        let t1 = template("tid", 1);
        let mut t2 = template("tid", 1);
        t2.principal
            .properties
            .as_mut()
            .unwrap()
            .template_docker_tag = "v2".to_string();
        assert_ne!(
            compute_key(&t1, &a, &s),
            compute_key(&t2, &a, &s),
            "a changed docker image must change the key"
        );

        // A template with no properties (group/dev) keys fine and is distinct.
        let mut group = template("tid", 1);
        group.principal.properties = None;
        assert_ne!(
            compute_key(&t1, &a, &s),
            compute_key(&group, &a, &s),
            "a group (no images) must differ from a template with images"
        );
    }
}
