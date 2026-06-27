//! Composition-level cache integration tests (AC3–AC8).
//!
//! These exercise the real `execute_composition` wiring with a fake
//! `TemplateExecutor` that counts calls and produces deterministic tar.gz output,
//! plus the real `TarGzUnpacker`/`DefaultVfs`, so a cache hit genuinely skips
//! execution and replays byte-identical output and downstream state.

use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::models::cyan::Cyan;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::{
    TemplatePrincipalRes, TemplatePropertyRes, TemplateVersionPrincipalRes, TemplateVersionRes,
};

use flate2::Compression;
use flate2::write::GzEncoder;

use crate::cache::{Cache, CacheConfig};
use crate::fs::{DefaultVfs, DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker};
use crate::operations::TemplateOperator;
use crate::operations::composition::layerer::DefaultVfsLayerer;
use crate::operations::composition::operator::CompositionOperator;
use crate::operations::composition::resolver::{DependencyResolver, ResolvedDependency};
use crate::session::DefaultSessionIdGenerator;
use crate::template::{DefaultTemplateHistory, TemplateExecutor};

/// A record of one fake execution: which template ran and the answers it saw.
#[derive(Clone)]
struct Call {
    template_id: String,
    received_answers: HashMap<String, Answer>,
}

/// Fake executor: counts calls, records inputs, and emits deterministic tar.gz
/// output derived from the answers it received. Optionally fails, or derives an
/// extra answer (to exercise downstream-state replay).
struct CountingExecutor {
    calls: Arc<Mutex<Vec<Call>>>,
    fail: bool,
    /// (template_id, key, value) — when a node with `template_id` runs, it adds
    /// this derived answer to its Complete state (simulating Q&A derivation).
    derive: Option<(String, String, String)>,
}

impl CountingExecutor {
    fn new(calls: Arc<Mutex<Vec<Call>>>) -> Self {
        Self {
            calls,
            fail: false,
            derive: None,
        }
    }
    fn failing(calls: Arc<Mutex<Vec<Call>>>) -> Self {
        Self {
            calls,
            fail: true,
            derive: None,
        }
    }
    fn deriving(calls: Arc<Mutex<Vec<Call>>>, id: &str, k: &str, v: &str) -> Self {
        Self {
            calls,
            fail: false,
            derive: Some((id.to_string(), k.to_string(), v.to_string())),
        }
    }
}

fn make_targz(name: &str, content: &[u8]) -> Vec<u8> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut builder = tar::Builder::new(&mut enc);
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_cksum();
        builder.append_data(&mut header, name, content).unwrap();
        builder.finish().unwrap();
    }
    enc.finish().unwrap()
}

/// Render an answers map to a stable string (sorted) so output is deterministic.
fn render_answers(answers: &HashMap<String, Answer>) -> String {
    let mut pairs: Vec<(&String, String)> = answers
        .iter()
        .map(|(k, v)| {
            let rendered = match v {
                Answer::String(s) => s.clone(),
                Answer::Bool(b) => b.to_string(),
                Answer::StringArray(a) => a.join(","),
            };
            (k, rendered)
        })
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(";")
}

impl TemplateExecutor for CountingExecutor {
    fn execute_template(
        &self,
        template: &TemplateVersionRes,
        session_id: &str,
        answers: Option<&HashMap<String, Answer>>,
        _deterministic_states: Option<&HashMap<String, String>>,
    ) -> Result<(Vec<u8>, TemplateState, String), Box<dyn Error + Send>> {
        let received = answers.cloned().unwrap_or_default();
        self.calls.lock().unwrap().push(Call {
            template_id: template.principal.id.clone(),
            received_answers: received.clone(),
        });

        if self.fail {
            return Err(Box::new(std::io::Error::other("boom")) as Box<dyn Error + Send>);
        }

        // Output file encodes the template id + the answers it received, so a
        // cache hit (which replays the stored archive) is byte-for-byte identical
        // to a fresh run only when the inputs match.
        let body = format!("{}|{}", template.principal.id, render_answers(&received));
        let file_name = format!("{}.txt", template.principal.id);
        let archive = make_targz(&file_name, body.as_bytes());

        // Complete state: the received answers plus an optional derived answer.
        let mut complete_answers = received;
        if let Some((id, k, v)) = &self.derive {
            if id == &template.principal.id {
                complete_answers.insert(k.clone(), Answer::String(v.clone()));
            }
        }
        let state = TemplateState::Complete(
            Cyan {
                processors: Vec::new(),
                plugins: Vec::new(),
            },
            complete_answers,
        );
        Ok((archive, state, session_id.to_string()))
    }
}

/// A resolver that returns a fixed dependency list, ignoring its input template.
struct FixedResolver {
    nodes: Vec<(TemplateVersionRes, HashMap<String, Answer>)>,
}

impl DependencyResolver for FixedResolver {
    fn resolve_dependencies(
        &self,
        _template: &TemplateVersionRes,
    ) -> Result<Vec<ResolvedDependency>, Box<dyn Error + Send>> {
        Ok(self
            .nodes
            .iter()
            .map(|(t, p)| ResolvedDependency {
                template: t.clone(),
                preset_answers: p.clone(),
            })
            .collect())
    }
}

fn template(id: &str, version: i64, has_props: bool) -> TemplateVersionRes {
    TemplateVersionRes {
        principal: TemplateVersionPrincipalRes {
            id: id.to_string(),
            version,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            description: "d".to_string(),
            properties: has_props.then_some(TemplatePropertyRes {
                blob_docker_reference: "b".to_string(),
                blob_docker_tag: "latest".to_string(),
                template_docker_reference: "t".to_string(),
                template_docker_tag: "latest".to_string(),
            }),
        },
        template: TemplatePrincipalRes {
            id: id.to_string(),
            name: id.to_string(),
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

fn dummy_registry() -> Rc<CyanRegistryClient> {
    Rc::new(CyanRegistryClient {
        endpoint: String::new(),
        version: "1.0".to_string(),
        client: Rc::new(reqwest::blocking::Client::new()),
    })
}

fn build_operator(
    executor: CountingExecutor,
    nodes: Vec<(TemplateVersionRes, HashMap<String, Answer>)>,
    cache: Cache,
) -> CompositionOperator {
    let vfs = Box::new(DefaultVfs::new(
        Box::new(TarGzUnpacker),
        Box::new(DiskFileLoader),
        Box::new(GitLikeMerger::new(false, 50)),
        Box::new(DiskFileWriter),
    ));
    let template_operator = TemplateOperator::new(
        Box::new(DefaultSessionIdGenerator),
        Box::new(executor),
        Box::new(DefaultTemplateHistory::new()),
        vfs,
        dummy_registry(),
    );
    let resolver = Box::new(FixedResolver { nodes });
    let mut op = CompositionOperator::new(template_operator, resolver, Box::new(DefaultVfsLayerer));
    op.set_cache(cache);
    op
}

fn enabled_cache(dir: &std::path::Path) -> Cache {
    Cache::new(CacheConfig {
        enabled: true,
        dir: dir.to_path_buf(),
        debug: false,
    })
}

fn vfs_files(vfs: &crate::fs::VirtualFileSystem) -> Vec<(PathBuf, Vec<u8>)> {
    let mut paths = vfs.get_paths();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let content = vfs.get_file(&p).cloned().unwrap_or_default();
            (p, content)
        })
        .collect()
}

fn answers_of(pairs: &[(&str, &str)]) -> HashMap<String, Answer> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), Answer::String(v.to_string())))
        .collect()
}

// AC3: a second execution of a node with identical inputs performs 0 executor
// calls and yields a byte-identical VFS; downstream state matches the fresh run.
#[test]
fn cache_hit_skips_execution_and_is_byte_identical() {
    let tmp = tempfile::tempdir().unwrap();
    let cache_dir = tmp.path().join("cyanprint");
    let answers = answers_of(&[("name", "alice")]);
    let states = HashMap::new();
    let node = template("tmpl-a", 1, true);

    // First run: miss -> executes once, stores.
    let calls1 = Arc::new(Mutex::new(Vec::new()));
    let mut op1 = build_operator(
        CountingExecutor::new(calls1.clone()),
        vec![(node.clone(), HashMap::new())],
        enabled_cache(&cache_dir),
    );
    let (vfs1, state1, _s, _c) = op1
        .execute_template(&node, &answers, &states, false)
        .unwrap();
    assert_eq!(
        calls1.lock().unwrap().len(),
        1,
        "first run executes the node"
    );

    // Second run: hit -> 0 executor calls, identical VFS and state.
    let calls2 = Arc::new(Mutex::new(Vec::new()));
    let mut op2 = build_operator(
        CountingExecutor::new(calls2.clone()),
        vec![(node.clone(), HashMap::new())],
        enabled_cache(&cache_dir),
    );
    let (vfs2, state2, _s, _c) = op2
        .execute_template(&node, &answers, &states, false)
        .unwrap();
    assert_eq!(
        calls2.lock().unwrap().len(),
        0,
        "second run must perform 0 executor calls (cache hit)"
    );
    assert_eq!(
        vfs_files(&vfs1),
        vfs_files(&vfs2),
        "VFS must be byte-identical"
    );
    assert_eq!(
        state1.shared_answers, state2.shared_answers,
        "downstream shared_answers must match the fresh run"
    );
    // FR15 / H2: the second run served a genuine hit, so the summary reports
    // 1/1 served (the hit is counted only after the entry was actually used).
    assert_eq!(
        op2.cache_summary(),
        Some((1, 1)),
        "a served hit must be reported as 1/1 in the summary"
    );
}

// AC5: a cache hit propagates a derived answer (one not in the input map) to a
// downstream node, so the downstream output/state is identical to a non-cached run.
#[test]
fn cache_hit_replays_downstream_state() {
    let tmp = tempfile::tempdir().unwrap();
    let cache_dir = tmp.path().join("cyanprint");
    let answers = answers_of(&[("seed", "1")]);
    let states = HashMap::new();
    let a = template("tmpl-a", 1, true);
    let b = template("tmpl-b", 1, true);

    // Phase 1: run with A only (A derives `secret`). Caches A.
    let calls1 = Arc::new(Mutex::new(Vec::new()));
    let mut op1 = build_operator(
        CountingExecutor::deriving(calls1.clone(), "tmpl-a", "secret", "xyz"),
        vec![(a.clone(), HashMap::new())],
        enabled_cache(&cache_dir),
    );
    op1.execute_template(&a, &answers, &states, false).unwrap();

    // Phase 2: run with [A, B]. A hits (no call) and replays `secret`; B misses
    // and must receive `secret` via the replayed downstream state.
    let calls2 = Arc::new(Mutex::new(Vec::new()));
    let mut op2 = build_operator(
        CountingExecutor::deriving(calls2.clone(), "tmpl-a", "secret", "xyz"),
        vec![(a.clone(), HashMap::new()), (b.clone(), HashMap::new())],
        enabled_cache(&cache_dir),
    );
    let (_vfs, state, _s, _c) = op2.execute_template(&a, &answers, &states, false).unwrap();

    let calls = calls2.lock().unwrap();
    assert_eq!(calls.len(), 1, "only B should execute (A is a cache hit)");
    assert_eq!(calls[0].template_id, "tmpl-b");
    assert_eq!(
        calls[0].received_answers.get("secret"),
        Some(&Answer::String("xyz".to_string())),
        "B must receive A's derived answer via downstream-state replay"
    );
    assert_eq!(
        state.shared_answers.get("secret"),
        Some(&Answer::String("xyz".to_string())),
        "final state must contain the replayed derived answer"
    );
}

// AC4: a second composition with unchanged inputs makes 0 executor calls
// (baseline reuse); changing one sub-template re-executes only that node.
#[test]
fn update_reuses_baseline_and_reexecutes_only_changed_node() {
    let tmp = tempfile::tempdir().unwrap();
    let cache_dir = tmp.path().join("cyanprint");
    let answers = answers_of(&[("k", "v")]);
    let states = HashMap::new();
    let nodes = |b_ver: i64| {
        vec![
            (template("node-a", 1, true), HashMap::new()),
            (template("node-b", b_ver, true), HashMap::new()),
            (template("node-c", 1, true), HashMap::new()),
        ]
    };

    // Run 1: all miss -> 3 calls, all cached.
    let calls1 = Arc::new(Mutex::new(Vec::new()));
    let mut op1 = build_operator(
        CountingExecutor::new(calls1.clone()),
        nodes(1),
        enabled_cache(&cache_dir),
    );
    let root = template("root", 1, true);
    op1.execute_template(&root, &answers, &states, false)
        .unwrap();
    assert_eq!(calls1.lock().unwrap().len(), 3);

    // Run 2: identical inputs -> 0 calls (full baseline reuse).
    let calls2 = Arc::new(Mutex::new(Vec::new()));
    let mut op2 = build_operator(
        CountingExecutor::new(calls2.clone()),
        nodes(1),
        enabled_cache(&cache_dir),
    );
    op2.execute_template(&root, &answers, &states, false)
        .unwrap();
    assert_eq!(
        calls2.lock().unwrap().len(),
        0,
        "unchanged baseline must make 0 executor calls"
    );

    // Run 3: bump node-b's version -> only node-b re-executes.
    let calls3 = Arc::new(Mutex::new(Vec::new()));
    let mut op3 = build_operator(
        CountingExecutor::new(calls3.clone()),
        nodes(2),
        enabled_cache(&cache_dir),
    );
    op3.execute_template(&root, &answers, &states, false)
        .unwrap();
    let c3 = calls3.lock().unwrap();
    assert_eq!(c3.len(), 1, "only the changed node re-executes");
    assert_eq!(c3[0].template_id, "node-b");
}

// AC6: a node whose execution errors leaves no entry under its key; the next run
// re-executes (nothing was cached).
#[test]
fn errors_are_never_cached() {
    let tmp = tempfile::tempdir().unwrap();
    let cache_dir = tmp.path().join("cyanprint");
    let answers = answers_of(&[("k", "v")]);
    let states = HashMap::new();
    let node = template("tmpl-err", 1, true);

    // First run errors out.
    let calls1 = Arc::new(Mutex::new(Vec::new()));
    let mut op1 = build_operator(
        CountingExecutor::failing(calls1.clone()),
        vec![(node.clone(), HashMap::new())],
        enabled_cache(&cache_dir),
    );
    assert!(
        op1.execute_template(&node, &answers, &states, false)
            .is_err()
    );

    // Nothing was stored.
    let store = crate::cache::CacheStore::new(cache_dir.clone());
    assert_eq!(store.size(), 0, "a failed node must not be cached");

    // Second run with a working executor re-executes (miss).
    let calls2 = Arc::new(Mutex::new(Vec::new()));
    let mut op2 = build_operator(
        CountingExecutor::new(calls2.clone()),
        vec![(node.clone(), HashMap::new())],
        enabled_cache(&cache_dir),
    );
    op2.execute_template(&node, &answers, &states, false)
        .unwrap();
    assert_eq!(
        calls2.lock().unwrap().len(),
        1,
        "next run re-executes because nothing was cached"
    );
}

// AC7: `--no-output-cache` (disabled cache) forces execution and skips store; an
// unwritable cache dir still completes the run (degrades, no abort).
#[test]
fn bypass_and_nonfatal_fallback() {
    let answers = answers_of(&[("k", "v")]);
    let states = HashMap::new();
    let node = template("tmpl-a", 1, true);

    // Bypass: disabled cache -> executes every run, never serves a hit.
    let tmp = tempfile::tempdir().unwrap();
    let cache_dir = tmp.path().join("cyanprint");
    let disabled = || {
        Cache::new(CacheConfig {
            enabled: false,
            dir: cache_dir.clone(),
            debug: false,
        })
    };
    let calls1 = Arc::new(Mutex::new(Vec::new()));
    let mut op1 = build_operator(
        CountingExecutor::new(calls1.clone()),
        vec![(node.clone(), HashMap::new())],
        disabled(),
    );
    op1.execute_template(&node, &answers, &states, false)
        .unwrap();
    let calls2 = Arc::new(Mutex::new(Vec::new()));
    let mut op2 = build_operator(
        CountingExecutor::new(calls2.clone()),
        vec![(node.clone(), HashMap::new())],
        disabled(),
    );
    op2.execute_template(&node, &answers, &states, false)
        .unwrap();
    assert_eq!(calls1.lock().unwrap().len(), 1);
    assert_eq!(
        calls2.lock().unwrap().len(),
        1,
        "disabled cache must never serve a hit"
    );
    assert!(
        !cache_dir.exists(),
        "disabled cache must not create its directory"
    );

    // Non-fatal fallback: an unwritable cache dir still completes the run.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let ro = tempfile::tempdir().unwrap();
        let ro_parent = ro.path().join("ro");
        std::fs::create_dir(&ro_parent).unwrap();
        std::fs::set_permissions(&ro_parent, std::fs::Permissions::from_mode(0o500)).unwrap();

        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut op = build_operator(
            CountingExecutor::new(calls.clone()),
            vec![(node.clone(), HashMap::new())],
            enabled_cache(&ro_parent.join("cyanprint")),
        );
        let result = op.execute_template(&node, &answers, &states, false);
        assert!(
            result.is_ok(),
            "an unwritable cache dir must not abort the run"
        );
        assert_eq!(calls.lock().unwrap().len(), 1, "node still executes");

        std::fs::set_permissions(&ro_parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
}

// AC8 / FR14: a local/dev template (version <= 0 with a synthetic id) is never
// cached, so it re-executes on every run even with the cache enabled.
#[test]
fn dev_mode_local_template_not_cached() {
    let tmp = tempfile::tempdir().unwrap();
    let cache_dir = tmp.path().join("cyanprint");
    let answers = answers_of(&[("k", "v")]);
    let states = HashMap::new();
    // Synthetic local node as produced by `build_synthetic_template`: version 0,
    // `local-` id, properties present.
    let node = template("local-abc123", 0, true);

    let calls1 = Arc::new(Mutex::new(Vec::new()));
    let mut op1 = build_operator(
        CountingExecutor::new(calls1.clone()),
        vec![(node.clone(), HashMap::new())],
        enabled_cache(&cache_dir),
    );
    op1.execute_template(&node, &answers, &states, false)
        .unwrap();

    let calls2 = Arc::new(Mutex::new(Vec::new()));
    let mut op2 = build_operator(
        CountingExecutor::new(calls2.clone()),
        vec![(node.clone(), HashMap::new())],
        enabled_cache(&cache_dir),
    );
    op2.execute_template(&node, &answers, &states, false)
        .unwrap();

    assert_eq!(calls1.lock().unwrap().len(), 1);
    assert_eq!(
        calls2.lock().unwrap().len(),
        1,
        "a local/dev template must re-execute (never cached)"
    );
    let store = crate::cache::CacheStore::new(cache_dir);
    assert_eq!(store.size(), 0, "local/dev output must never be stored");
}

// C1: a cache entry whose framing checksums OK but whose inner archive is
// garbage (can't unpack) does NOT abort the run — the hit falls back to live
// execution, the poisoned entry is evicted, and the re-execution re-caches a
// valid one. The next run then serves a clean hit.
#[test]
fn poisoned_entry_self_heals() {
    let tmp = tempfile::tempdir().unwrap();
    let cache_dir = tmp.path().join("cyanprint");
    let answers = answers_of(&[("k", "v")]);
    let states = HashMap::new();
    let node = template("tmpl-poison", 1, true);

    // Seed a poisoned entry under the exact key the node would compute. The
    // framing is valid (put succeeds) but the archive bytes are not a real
    // tar.gz, so unpack_archive fails.
    let poison_key = crate::cache::compute_key(&node, &answers, &states);
    let store = crate::cache::CacheStore::new(cache_dir.clone());
    store.put(
        &poison_key,
        &crate::cache::CacheEntry {
            archive: b"not a real archive".to_vec(),
            state: HashMap::new(),
        },
    );
    assert!(store.get(&poison_key).is_some(), "poisoned entry seeded");

    // Run: the poisoned hit must NOT abort; it falls back to execution (1 call)
    // and evicts + replaces the bad entry.
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut op = build_operator(
        CountingExecutor::new(calls.clone()),
        vec![(node.clone(), HashMap::new())],
        enabled_cache(&cache_dir),
    );
    let result = op.execute_template(&node, &answers, &states, false);
    assert!(
        result.is_ok(),
        "a poisoned cache entry must not abort the run (degrade to miss)"
    );
    assert_eq!(
        calls.lock().unwrap().len(),
        1,
        "the poisoned hit must fall back to live execution"
    );
    // FR15 / H2: the poisoned entry was looked up but NOT served, so the
    // summary reports 0/1 served (not 1/1). The hit counter self-heals
    // alongside the entry: only a genuinely-served entry counts as a hit.
    assert_eq!(
        op.cache_summary(),
        Some((0, 1)),
        "a poisoned hit that fell back to execution must not count as served"
    );

    // The poisoned bytes were evicted and replaced with a valid (unpackable) entry.
    let replaced = store.get(&poison_key).expect("valid entry re-cached");
    let checker = crate::fs::DefaultVfs::new(
        Box::new(crate::fs::TarGzUnpacker),
        Box::new(crate::fs::DiskFileLoader),
        Box::new(crate::fs::GitLikeMerger::new(false, 50)),
        Box::new(crate::fs::DiskFileWriter),
    );
    crate::fs::Vfs::unpack_archive(&checker, replaced.archive)
        .expect("re-cached archive must be valid (unpackable)");

    // Next run serves a clean hit: 0 executor calls.
    let calls2 = Arc::new(Mutex::new(Vec::new()));
    let mut op2 = build_operator(
        CountingExecutor::new(calls2.clone()),
        vec![(node.clone(), HashMap::new())],
        enabled_cache(&cache_dir),
    );
    op2.execute_template(&node, &answers, &states, false)
        .unwrap();
    assert_eq!(
        calls2.lock().unwrap().len(),
        0,
        "after self-heal, the next run serves a clean cache hit"
    );
}
