use chrono::{DateTime, Utc};
use cyanprompt::domain::models::answer::Answer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::conflict_file_resolver::FileConflictEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateHistoryEntry {
    pub version: i64,
    pub time: DateTime<Utc>,
    pub answers: HashMap<String, Answer>,
    pub deterministic_states: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateState {
    pub active: bool,
    pub history: Vec<TemplateHistoryEntry>,

    /// Normalized, path-only snapshot of the files this template produced on the
    /// most recent run. Empty for templates that produced nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CyanState {
    #[serde(flatten)]
    pub templates: HashMap<String, TemplateState>,

    /// Sorted, de-duplicated union of every active template's output paths — the
    /// path-only manifest of cyanprint-managed files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub managed_files: Vec<String>,

    /// File conflicts resolved during layering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_conflicts: Vec<FileConflictEntry>,
}

impl CyanState {
    /// Recompute the managed-files manifest wholesale from this run's per-template
    /// output paths, overwriting any prior values.
    ///
    /// `managed_by_template` is keyed by `"<user>/<template>"` and contains an
    /// entry only for templates that were active this run. Every template tracked
    /// in `self.templates` has its `files` set to its collected paths, or cleared
    /// to `[]` when absent (e.g. a deactivated template). The top-level
    /// `managed_files` becomes the sorted, de-duplicated union across all collected
    /// templates.
    pub fn set_managed_files(&mut self, managed_by_template: &HashMap<String, Vec<String>>) {
        for (key, ts) in self.templates.iter_mut() {
            ts.files = managed_by_template.get(key).cloned().unwrap_or_default();
        }

        let mut all: Vec<String> = managed_by_template.values().flatten().cloned().collect();
        all.sort();
        all.dedup();
        self.managed_files = all;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(active: bool, files: Vec<&str>) -> TemplateState {
        TemplateState {
            active,
            history: Vec::new(),
            files: files.into_iter().map(String::from).collect(),
        }
    }

    fn managed(pairs: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    v.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                )
            })
            .collect()
    }

    // AC1 (FR1): top-level managed_files is the sorted, de-duplicated union of
    // active templates' outputs.
    // AC2 (FR2): each template entry's files equals that template's own paths.
    #[test]
    fn union_and_per_template_snapshot() {
        let mut state = CyanState::default();
        state
            .templates
            .insert("alice/a".to_string(), template(true, vec![]));
        state
            .templates
            .insert("bob/b".to_string(), template(true, vec![]));

        state.set_managed_files(&managed(&[
            ("alice/a", &["a.txt", "shared.txt"]),
            ("bob/b", &["b.txt", "shared.txt"]),
        ]));

        assert_eq!(
            state.managed_files,
            vec![
                "a.txt".to_string(),
                "b.txt".to_string(),
                "shared.txt".to_string()
            ]
        );
        assert_eq!(
            state.templates["alice/a"].files,
            vec!["a.txt".to_string(), "shared.txt".to_string()]
        );
        assert_eq!(
            state.templates["bob/b"].files,
            vec!["b.txt".to_string(), "shared.txt".to_string()]
        );
    }

    // AC4 (FR4): a deactivated template (absent from managed_by_template)
    // contributes nothing to managed_files and has its files cleared.
    #[test]
    fn deactivated_template_cleared_and_excluded() {
        let mut state = CyanState::default();
        state
            .templates
            .insert("alice/a".to_string(), template(true, vec![]));
        // Deactivated template carries stale files from a prior run.
        state.templates.insert(
            "old/dead".to_string(),
            template(false, vec!["stale1.txt", "stale2.txt"]),
        );

        // Only the active template is collected this run.
        state.set_managed_files(&managed(&[("alice/a", &["a.txt"])]));

        assert_eq!(state.managed_files, vec!["a.txt".to_string()]);
        assert_eq!(state.templates["alice/a"].files, vec!["a.txt".to_string()]);
        // Deactivated template's files are cleared; no stale contribution.
        assert!(state.templates["old/dead"].files.is_empty());
    }

    // AC5 (FR5): re-running with an unchanged footprint yields identical lists;
    // re-running after a file is dropped removes it from both lists (no stale).
    #[test]
    fn recompute_is_idempotent_and_drops_stale() {
        let mut state = CyanState::default();
        state
            .templates
            .insert("alice/a".to_string(), template(true, vec![]));

        let run1 = managed(&[("alice/a", &["a.txt", "b.txt"])]);
        state.set_managed_files(&run1);
        let first_managed = state.managed_files.clone();
        let first_files = state.templates["alice/a"].files.clone();

        // Identical footprint → byte-identical lists.
        state.set_managed_files(&run1);
        assert_eq!(state.managed_files, first_managed);
        assert_eq!(state.templates["alice/a"].files, first_files);

        // Template drops b.txt → it disappears from both lists.
        state.set_managed_files(&managed(&[("alice/a", &["a.txt"])]));
        assert_eq!(state.managed_files, vec!["a.txt".to_string()]);
        assert_eq!(state.templates["alice/a"].files, vec!["a.txt".to_string()]);
    }

    // AC8 (FR8): a pre-existing state file WITHOUT the new fields deserializes
    // without error and round-trips.
    #[test]
    fn deserializes_legacy_state_without_new_fields() {
        let legacy = "\
alice/a:
  active: true
  history: []
";
        let state: CyanState = serde_yaml::from_str(legacy).expect("legacy state deserializes");
        assert!(state.managed_files.is_empty());
        assert!(state.templates["alice/a"].files.is_empty());
        assert!(state.templates["alice/a"].active);
    }

    // AC8 (FR8): serializing a state with empty lists omits managed_files / files.
    #[test]
    fn empty_lists_are_omitted_on_serialize() {
        let mut state = CyanState::default();
        state
            .templates
            .insert("alice/a".to_string(), template(true, vec![]));

        let yaml = serde_yaml::to_string(&state).expect("serializes");
        assert!(
            !yaml.contains("managed_files"),
            "empty managed_files must be omitted, got:\n{yaml}"
        );
        assert!(
            !yaml.contains("files:"),
            "empty per-template files must be omitted, got:\n{yaml}"
        );
        assert!(
            !yaml.contains("file_conflicts"),
            "empty file_conflicts must be omitted, got:\n{yaml}"
        );
    }

    // Round-trip with populated manifest preserves both lists.
    #[test]
    fn populated_manifest_round_trips() {
        let mut state = CyanState::default();
        state
            .templates
            .insert("alice/a".to_string(), template(true, vec![]));
        state.set_managed_files(&managed(&[("alice/a", &["a.txt", "shared.txt"])]));

        let yaml = serde_yaml::to_string(&state).expect("serializes");
        let back: CyanState = serde_yaml::from_str(&yaml).expect("round-trips");
        assert_eq!(back.managed_files, state.managed_files);
        assert_eq!(
            back.templates["alice/a"].files,
            state.templates["alice/a"].files
        );
    }

    // Flatten-collision guard: the top-level `managed_files` key never collides
    // with a template key, because template keys always contain '/'.
    #[test]
    fn managed_files_key_does_not_collide_with_template_keys() {
        let mut state = CyanState::default();
        state
            .templates
            .insert("alice/managed_files".to_string(), template(true, vec![]));
        state.set_managed_files(&managed(&[("alice/managed_files", &["x.txt"])]));

        let yaml = serde_yaml::to_string(&state).expect("serializes");
        let back: CyanState = serde_yaml::from_str(&yaml).expect("round-trips");
        // The bare top-level `managed_files` is the manifest, NOT mistaken for a template.
        assert_eq!(back.managed_files, vec!["x.txt".to_string()]);
        assert!(back.templates.contains_key("alice/managed_files"));
        assert_eq!(back.templates.len(), 1);
    }
}
