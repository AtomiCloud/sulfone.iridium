use std::collections::HashMap;
use std::path::PathBuf;

use super::layerer::{DefaultVfsLayerer, VfsLayerer};
use super::resolver::serde_json_value_to_answer;
use super::state::CompositionState;
use crate::fs::VirtualFileSystem;
use cyanprompt::domain::models::answer::Answer;

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that VFS layerer correctly implements LWW semantics
    #[test]
    fn test_vfs_layerer_lww_semantics() {
        let layerer = DefaultVfsLayerer;

        // Create first VFS with file A and B
        let mut vfs1 = VirtualFileSystem::new();
        vfs1.add_file(PathBuf::from("file_a.txt"), b"content_a_v1".to_vec());
        vfs1.add_file(PathBuf::from("file_b.txt"), b"content_b_v1".to_vec());

        // Create second VFS with file B (different content) and C
        let mut vfs2 = VirtualFileSystem::new();
        vfs2.add_file(PathBuf::from("file_b.txt"), b"content_b_v2".to_vec());
        vfs2.add_file(PathBuf::from("file_c.txt"), b"content_c_v2".to_vec());

        // Layer merge - vfs2 should overwrite vfs1 for overlapping files
        let merged = layerer
            .layer_merge(&[vfs1, vfs2])
            .expect("Layer merge should succeed");

        // Check that file_a from vfs1 is preserved
        assert_eq!(
            merged.get_file(&PathBuf::from("file_a.txt")),
            Some(&b"content_a_v1".to_vec()),
            "file_a should come from first VFS"
        );

        // Check that file_b from vfs2 overwrites vfs1 (LWW)
        assert_eq!(
            merged.get_file(&PathBuf::from("file_b.txt")),
            Some(&b"content_b_v2".to_vec()),
            "file_b should come from second VFS (LWW)"
        );

        // Check that file_c from vfs2 is present
        assert_eq!(
            merged.get_file(&PathBuf::from("file_c.txt")),
            Some(&b"content_c_v2".to_vec()),
            "file_c should come from second VFS"
        );
    }

    /// Test that VFS layerer handles empty input
    #[test]
    fn test_vfs_layerer_empty() {
        let layerer = DefaultVfsLayerer;

        // Empty input should return empty VFS
        let result = layerer.layer_merge(&[]);
        assert!(
            result.is_ok(),
            "Layer merge with empty input should succeed"
        );

        let merged = result.unwrap();
        assert!(
            merged.get_paths().is_empty(),
            "Empty input should produce empty VFS"
        );
    }

    /// Test that VFS layerer handles single VFS
    #[test]
    fn test_vfs_layerer_single() {
        let layerer = DefaultVfsLayerer;

        let mut vfs = VirtualFileSystem::new();
        vfs.add_file(PathBuf::from("file.txt"), b"content".to_vec());

        let merged = layerer
            .layer_merge(&[vfs.clone()])
            .expect("Layer merge should succeed");

        assert_eq!(
            merged.get_file(&PathBuf::from("file.txt")),
            Some(&b"content".to_vec()),
            "Single VFS should be returned as-is"
        );
    }

    /// Test that VFS layerer handles three VFS with correct LWW ordering
    #[test]
    fn test_vfs_layerer_three_vfs() {
        let layerer = DefaultVfsLayerer;

        // First VFS: file.txt = "v1"
        let mut vfs1 = VirtualFileSystem::new();
        vfs1.add_file(PathBuf::from("file.txt"), b"v1".to_vec());

        // Second VFS: file.txt = "v2"
        let mut vfs2 = VirtualFileSystem::new();
        vfs2.add_file(PathBuf::from("file.txt"), b"v2".to_vec());

        // Third VFS: file.txt = "v3"
        let mut vfs3 = VirtualFileSystem::new();
        vfs3.add_file(PathBuf::from("file.txt"), b"v3".to_vec());

        // Layer merge - last writer wins
        let merged = layerer
            .layer_merge(&[vfs1, vfs2, vfs3])
            .expect("Layer merge should succeed");

        assert_eq!(
            merged.get_file(&PathBuf::from("file.txt")),
            Some(&b"v3".to_vec()),
            "Last VFS should win (LWW)"
        );
    }

    /// Test that VirtualFileSystem clone works correctly
    #[test]
    fn test_vfs_clone() {
        let mut original = VirtualFileSystem::new();
        original.add_file(PathBuf::from("file.txt"), b"content".to_vec());

        let cloned = original.clone();

        // Modify original to ensure clone is independent
        original.add_file(PathBuf::from("file.txt"), b"modified".to_vec());

        // Cloned should still have original content
        assert_eq!(
            cloned.get_file(&PathBuf::from("file.txt")),
            Some(&b"content".to_vec()),
            "Clone should be independent of original"
        );
    }

    // =========================================================================
    // Batch VFS Layering Tests (v2 unified flow)
    // =========================================================================

    /// Test that layering with overlapping files from multiple templates
    /// correctly implements LWW semantics (later templates win)
    #[test]
    fn test_batch_layering_lww_with_overlapping_files() {
        let layerer = DefaultVfsLayerer;

        // Create three templates that all modify the same file
        let mut template1 = VirtualFileSystem::new();
        template1.add_file(PathBuf::from("shared.txt"), b"from template 1".to_vec());

        let mut template2 = VirtualFileSystem::new();
        template2.add_file(PathBuf::from("shared.txt"), b"from template 2".to_vec());

        let mut template3 = VirtualFileSystem::new();
        template3.add_file(PathBuf::from("shared.txt"), b"from template 3".to_vec());

        // Layer in order - template3 should win for shared.txt
        let layered = layerer
            .layer_merge(&[template1, template2, template3])
            .expect("Layer merge should succeed");

        assert_eq!(
            layered.get_file(&PathBuf::from("shared.txt")),
            Some(&b"from template 3".to_vec()),
            "Last template should win for overlapping files (LWW)"
        );
    }

    /// Test that empty VFS collections are handled correctly
    #[test]
    fn test_empty_vfs_in_collection() {
        let layerer = DefaultVfsLayerer;

        // Create a collection with empty VFS
        let empty_vfs = VirtualFileSystem::new();
        let mut non_empty_vfs = VirtualFileSystem::new();
        non_empty_vfs.add_file(PathBuf::from("file.txt"), b"content".to_vec());

        // Layer with empty first
        let layered1 = layerer
            .layer_merge(&[empty_vfs.clone(), non_empty_vfs.clone()])
            .expect("Layer merge should succeed");

        assert_eq!(
            layered1.get_file(&PathBuf::from("file.txt")),
            Some(&b"content".to_vec()),
            "Non-empty VFS should be layered on top of empty"
        );

        // Layer with empty last
        let layered2 = layerer
            .layer_merge(&[non_empty_vfs.clone(), empty_vfs])
            .expect("Layer merge should succeed");

        assert_eq!(
            layered2.get_file(&PathBuf::from("file.txt")),
            Some(&b"content".to_vec()),
            "Empty VFS should not remove existing files"
        );
    }

    /// Test that CompositionState can be merged from multiple collections
    #[test]
    fn test_composition_state_merging() {
        let mut state1 = CompositionState::new();
        state1
            .shared_answers
            .insert("key1".to_string(), Answer::String("value1".to_string()));

        let mut state2 = CompositionState::new();
        state2
            .shared_answers
            .insert("key2".to_string(), Answer::String("value2".to_string()));
        state2
            .shared_deterministic_states
            .insert("state1".to_string(), "state_value1".to_string());

        // Simulate merging states (as done in batch_create_for_existing_project)
        let mut merged_state = CompositionState::new();
        for (key, value) in &state1.shared_answers {
            merged_state
                .shared_answers
                .insert(key.clone(), value.clone());
        }
        for (key, value) in &state2.shared_answers {
            merged_state
                .shared_answers
                .insert(key.clone(), value.clone());
        }
        for (key, value) in &state2.shared_deterministic_states {
            merged_state
                .shared_deterministic_states
                .insert(key.clone(), value.clone());
        }

        assert_eq!(
            merged_state.shared_answers.len(),
            2,
            "Should have 2 answers"
        );
        assert_eq!(
            merged_state.shared_deterministic_states.len(),
            1,
            "Should have 1 state"
        );
    }

    /// Test batch VFS layering simulation for the unified v2 flow.
    /// This simulates collecting prev_vfs_list and curr_vfs_list,
    /// then layering them separately.
    #[test]
    fn test_batch_vfs_layering_simulation_v2() {
        let layerer = DefaultVfsLayerer;

        // Simulate prev specs: Template A (time=1), Template B (time=2)
        let mut prev_a = VirtualFileSystem::new();
        prev_a.add_file(PathBuf::from("config.yaml"), b"version: 1".to_vec());

        let mut prev_b = VirtualFileSystem::new();
        prev_b.add_file(PathBuf::from("readme.md"), b"# Project v1".to_vec());

        // prev_vfs_list = [prev_a, prev_b]
        let prev_vfs_list = vec![prev_a, prev_b];

        // Simulate curr specs: Template A upgraded (time=1), Template B upgraded (time=2), Template C new (time=3)
        let mut curr_a = VirtualFileSystem::new();
        curr_a.add_file(PathBuf::from("config.yaml"), b"version: 2".to_vec());

        let mut curr_b = VirtualFileSystem::new();
        curr_b.add_file(PathBuf::from("readme.md"), b"# Project v2".to_vec());
        curr_b.add_file(
            PathBuf::from("config.yaml"),
            b"# override from template2".to_vec(),
        );

        let mut curr_c = VirtualFileSystem::new();
        curr_c.add_file(PathBuf::from("new_file.txt"), b"new content".to_vec());

        // curr_vfs_list = [curr_a, curr_b, curr_c]
        let curr_vfs_list = vec![curr_a, curr_b, curr_c];

        // Layer prev VFS outputs
        let layered_prev = layerer
            .layer_merge(&prev_vfs_list)
            .expect("Layer prev should succeed");

        // Layer curr VFS outputs (LWW semantics)
        let layered_curr = layerer
            .layer_merge(&curr_vfs_list)
            .expect("Layer curr should succeed");

        // Verify prev outputs
        assert_eq!(
            layered_prev.get_file(&PathBuf::from("config.yaml")),
            Some(&b"version: 1".to_vec()),
            "Prev config.yaml should be from template A"
        );
        assert_eq!(
            layered_prev.get_file(&PathBuf::from("readme.md")),
            Some(&b"# Project v1".to_vec()),
            "Prev readme.md should be from template B"
        );

        // Verify curr outputs (LWW - template B's config.yaml should win over template A's)
        assert_eq!(
            layered_curr.get_file(&PathBuf::from("config.yaml")),
            Some(&b"# override from template2".to_vec()),
            "Curr config.yaml should be from template B (LWW)"
        );
        assert_eq!(
            layered_curr.get_file(&PathBuf::from("readme.md")),
            Some(&b"# Project v2".to_vec()),
            "Curr readme.md should be from template B"
        );
        assert_eq!(
            layered_curr.get_file(&PathBuf::from("new_file.txt")),
            Some(&b"new content".to_vec()),
            "Curr new_file.txt should be from template C"
        );
    }

    /// Test that non-upgraded templates correctly participate in LWW layering (v2 flow).
    /// This is the CRITICAL bug fix test: when Template A is upgraded but Template B is not,
    /// Template B's VFS should still be included in the curr_vfs_list to preserve LWW semantics.
    #[test]
    fn test_non_upgraded_template_lww_layering_v2() {
        let layerer = DefaultVfsLayerer;

        // Scenario: Two templates both generate "shared.txt" but with different content
        // Template A (time=1, earlier): generates shared.txt = "from A"
        // Template B (time=2, later): generates shared.txt = "from B" (should win via LWW)

        // Template A is being UPGRADED (appears in both prev and curr lists)
        let mut prev_a = VirtualFileSystem::new();
        prev_a.add_file(PathBuf::from("shared.txt"), b"from A v1".to_vec());

        let mut curr_a = VirtualFileSystem::new();
        curr_a.add_file(PathBuf::from("shared.txt"), b"from A v2".to_vec());
        curr_a.add_file(PathBuf::from("a_only.txt"), b"only in A".to_vec());

        // Template B is NOT being upgraded (appears only in curr list, not prev)
        // It was added AFTER Template A, so its content should win via LWW
        let mut curr_b = VirtualFileSystem::new();
        curr_b.add_file(PathBuf::from("shared.txt"), b"from B v1".to_vec());
        curr_b.add_file(PathBuf::from("b_only.txt"), b"only in B".to_vec());

        // prev_vfs_list = [prev_a] (only upgraded templates have prev)
        let prev_vfs_list = vec![prev_a];

        // curr_vfs_list = [curr_a, curr_b] (BOTH upgraded and non-upgraded templates)
        // This is the key fix: ALL templates contribute to LWW layering
        let curr_vfs_list = vec![curr_a, curr_b];

        // Layer prev VFS
        let layered_prev = layerer
            .layer_merge(&prev_vfs_list)
            .expect("Layer prev should succeed");

        // Layer curr VFS - LWW semantics apply
        let layered_curr = layerer
            .layer_merge(&curr_vfs_list)
            .expect("Layer curr should succeed");

        // Verify prev only has A's content (B wasn't upgraded so no prev)
        assert_eq!(
            layered_prev.get_file(&PathBuf::from("shared.txt")),
            Some(&b"from A v1".to_vec()),
            "Prev shared.txt should be from template A only"
        );

        // Verify curr has correct LWW behavior:
        // - shared.txt should come from B (later in the layering order)
        // - Both a_only.txt and b_only.txt should be present
        assert_eq!(
            layered_curr.get_file(&PathBuf::from("shared.txt")),
            Some(&b"from B v1".to_vec()),
            "Curr shared.txt should be from template B (LWW - B was added later)"
        );
        assert_eq!(
            layered_curr.get_file(&PathBuf::from("a_only.txt")),
            Some(&b"only in A".to_vec()),
            "a_only.txt from template A should be present"
        );
        assert_eq!(
            layered_curr.get_file(&PathBuf::from("b_only.txt")),
            Some(&b"only in B".to_vec()),
            "b_only.txt from template B should be present"
        );
    }

    /// Test LWW ordering with MIXED upgrade/non-upgrade templates in interleaved time order (v2 flow).
    /// This is the REGRESSION test for the bug where the orchestrator would collect:
    ///   [all upgraded templates first] + [all non-upgraded templates second]
    /// instead of preserving the original time-based order.
    ///
    /// Scenario:
    /// - Template A (time=1, NON-UPGRADE): generates shared.txt = "A"
    /// - Template B (time=2, UPGRADE): generates shared.txt = "B"
    /// - Template C (time=3, NON-UPGRADE): generates shared.txt = "C" (should win)
    ///
    /// Correct LWW order: [A, B, C] → C wins
    /// Old buggy order: [B, A, C] or [B, C, A] → wrong winner
    #[test]
    fn test_mixed_upgrade_nonupgrade_ordering_lww_v2() {
        let layerer = DefaultVfsLayerer;

        // Template A (time=1, NON-UPGRADE - only in curr list, not prev)
        let mut curr_a = VirtualFileSystem::new();
        curr_a.add_file(
            PathBuf::from("shared.txt"),
            b"from A (oldest, non-upgrade)".to_vec(),
        );
        curr_a.add_file(PathBuf::from("a_only.txt"), b"A unique file".to_vec());

        // Template B (time=2, UPGRADE - in both prev and curr lists)
        let mut prev_b = VirtualFileSystem::new();
        prev_b.add_file(PathBuf::from("shared.txt"), b"from B v1".to_vec());

        let mut curr_b = VirtualFileSystem::new();
        curr_b.add_file(
            PathBuf::from("shared.txt"),
            b"from B (middle, upgrade)".to_vec(),
        );
        curr_b.add_file(PathBuf::from("b_only.txt"), b"B unique file".to_vec());

        // Template C (time=3, NON-UPGRADE - only in curr list, not prev)
        let mut curr_c = VirtualFileSystem::new();
        curr_c.add_file(
            PathBuf::from("shared.txt"),
            b"from C (newest, non-upgrade)".to_vec(),
        );
        curr_c.add_file(PathBuf::from("c_only.txt"), b"C unique file".to_vec());

        // CRITICAL: Collect curr_vfs_list in TIME ORDER [A, B, C], NOT by upgrade status
        // The old bug would produce [B, A, C] or [B, C, A]
        let curr_vfs_list_in_time_order: Vec<_> = vec![curr_a, curr_b, curr_c];

        // Layer curr VFS - LWW semantics apply
        let layered_curr = layerer
            .layer_merge(&curr_vfs_list_in_time_order)
            .expect("Layer curr should succeed");

        // Verify C wins for shared.txt (newest in time order)
        assert_eq!(
            layered_curr.get_file(&PathBuf::from("shared.txt")),
            Some(&b"from C (newest, non-upgrade)".to_vec()),
            "shared.txt should be from template C (LWW - C is newest in time order)"
        );

        // All unique files should be present
        assert_eq!(
            layered_curr.get_file(&PathBuf::from("a_only.txt")),
            Some(&b"A unique file".to_vec()),
            "a_only.txt from template A should be present"
        );
        assert_eq!(
            layered_curr.get_file(&PathBuf::from("b_only.txt")),
            Some(&b"B unique file".to_vec()),
            "b_only.txt from template B should be present"
        );
        assert_eq!(
            layered_curr.get_file(&PathBuf::from("c_only.txt")),
            Some(&b"C unique file".to_vec()),
            "c_only.txt from template C should be present"
        );

        // Now test what would happen with the BUGGY ordering [B first, then A and C]
        // This demonstrates why the fix is important
        let mut buggy_a = VirtualFileSystem::new();
        buggy_a.add_file(
            PathBuf::from("shared.txt"),
            b"from A (oldest, non-upgrade)".to_vec(),
        );

        let mut buggy_b = VirtualFileSystem::new();
        buggy_b.add_file(
            PathBuf::from("shared.txt"),
            b"from B (middle, upgrade)".to_vec(),
        );

        let mut buggy_c = VirtualFileSystem::new();
        buggy_c.add_file(
            PathBuf::from("shared.txt"),
            b"from C (newest, non-upgrade)".to_vec(),
        );

        // Buggy ordering: [B, C, A] - upgrades first, then non-upgrades
        let buggy_order_swapped: Vec<_> = vec![
            buggy_b, // Upgrade (time=2) first
            buggy_c, // Non-upgrade (time=3) second - should be after A
            buggy_a, // Non-upgrade (time=1) last - WRONG!
        ];

        let layered_buggy_swapped = layerer
            .layer_merge(&buggy_order_swapped)
            .expect("Layer buggy swapped should succeed");

        // This shows A would incorrectly win with the swapped buggy order
        assert_eq!(
            layered_buggy_swapped.get_file(&PathBuf::from("shared.txt")),
            Some(&b"from A (oldest, non-upgrade)".to_vec()),
            "With buggy ordering [B, C, A], A would incorrectly win (not LWW!)"
        );

        // The correct order [A, B, C] produces C as winner (LWW correct)
        // This is what the fix ensures
    }

    // =========================================================================
    // serde_json_value_to_answer Tests
    // =========================================================================

    /// Test that serde_json_value_to_answer correctly converts String values
    #[test]
    fn test_serde_json_value_to_answer_string() {
        let json_value = serde_json::json!("hello world");
        let result = serde_json_value_to_answer(&json_value);
        assert!(
            result.is_some(),
            "String value should convert to Some(Answer)"
        );
        if let Answer::String(s) = result.unwrap() {
            assert_eq!(
                s, "hello world",
                "String value should convert to Answer::String"
            );
        } else {
            panic!("Expected Answer::String, got something else");
        }
    }

    /// Test that serde_json_value_to_answer correctly converts Bool values
    #[test]
    fn test_serde_json_value_to_answer_bool() {
        let json_true = serde_json::json!(true);
        let result_true = serde_json_value_to_answer(&json_true);
        assert!(
            result_true.is_some(),
            "Bool true should convert to Some(Answer)"
        );
        if let Answer::Bool(b) = result_true.unwrap() {
            assert!(b, "Bool true should convert to Answer::Bool(true)");
        } else {
            panic!("Expected Answer::Bool(true), got something else");
        }

        let json_false = serde_json::json!(false);
        let result_false = serde_json_value_to_answer(&json_false);
        assert!(
            result_false.is_some(),
            "Bool false should convert to Some(Answer)"
        );
        if let Answer::Bool(b) = result_false.unwrap() {
            assert!(!b, "Bool false should convert to Answer::Bool(false)");
        } else {
            panic!("Expected Answer::Bool(false), got something else");
        }
    }

    /// Test that serde_json_value_to_answer correctly converts StringArray values
    #[test]
    fn test_serde_json_value_to_answer_string_array() {
        let json_array = serde_json::json!(["a", "b", "c"]);
        let result = serde_json_value_to_answer(&json_array);
        assert!(
            result.is_some(),
            "String array should convert to Some(Answer)"
        );
        if let Answer::StringArray(arr) = result.unwrap() {
            assert_eq!(
                arr,
                &["a", "b", "c"],
                "String array should convert to Answer::StringArray"
            );
        } else {
            panic!("Expected Answer::StringArray, got something else");
        }
    }

    /// Test that serde_json_value_to_answer returns None for Number values
    #[test]
    fn test_serde_json_value_to_answer_number() {
        let json_number = serde_json::json!(42);
        let result = serde_json_value_to_answer(&json_number);
        assert!(result.is_none(), "Number value should return None");

        let json_float = serde_json::json!(42.5);
        let result_float = serde_json_value_to_answer(&json_float);
        assert!(result_float.is_none(), "Float value should return None");
    }

    /// Test that serde_json_value_to_answer returns None for Null values
    #[test]
    fn test_serde_json_value_to_answer_null() {
        let json_null = serde_json::json!(null);
        let result = serde_json_value_to_answer(&json_null);
        assert!(result.is_none(), "Null value should return None");
    }

    /// Test that serde_json_value_to_answer returns None for mixed arrays (non-strings)
    #[test]
    fn test_serde_json_value_to_answer_mixed_array() {
        let json_mixed = serde_json::json!(["a", 1, "b"]);
        let result = serde_json_value_to_answer(&json_mixed);
        assert!(result.is_none(), "Mixed array should return None");
    }

    /// Test that serde_json_value_to_answer returns None for object values
    #[test]
    fn test_serde_json_value_to_answer_object() {
        let json_obj = serde_json::json!({"key": "value"});
        let result = serde_json_value_to_answer(&json_obj);
        assert!(result.is_none(), "Object value should return None");
    }

    // =========================================================================
    // Preset Answer Injection Tests
    //
    // NOTE: the former tests here (`test_preset_answer_fills_gap`,
    // `test_user_answer_wins_over_preset`, `test_preset_answers_isolated_per_template`,
    // `test_empty_preset_answers`) RE-IMPLEMENTED the merge logic inline instead of
    // calling production `execute_composition`. That made them pass vacuously even after
    // an earlier namespacing refactor inverted the real precedence. They have been replaced by
    // real-pipeline tests in the "Headless composition-boundary tests" section below
    // (`caller_answer_wins_over_preset_in_pipeline`, `preset_fills_gap_in_pipeline`,
    // `preset_answers_isolated_per_dependency_in_pipeline`) which drive the actual
    // execute_composition → executor boundary via a capturing fake executor.
    // =========================================================================

    // =========================================================================
    // DefaultDependencyResolver Mock Tests (spec 2.9)
    // =========================================================================

    use std::collections::HashSet;
    use std::rc::Rc;

    use super::super::operator::CompositionOperator;
    use super::super::resolver::{
        DefaultDependencyResolver, DependencyResolver, ResolvedDependency,
        flatten_dependencies_with_fetcher,
    };
    use cyanregistry::http::models::template_res::{
        TemplatePrincipalRes, TemplatePropertyRes, TemplateVersionPrincipalRes, TemplateVersionRes,
        TemplateVersionTemplateRefRes,
    };

    /// Helper to build a minimal TemplateVersionRes for testing
    fn make_template_version(
        id: &str,
        name: &str,
        version: i64,
        templates: Vec<TemplateVersionTemplateRefRes>,
    ) -> TemplateVersionRes {
        use cyanregistry::http::models::plugin_res::PluginVersionPrincipalRes;
        TemplateVersionRes {
            principal: TemplateVersionPrincipalRes {
                id: id.to_string(),
                version,
                created_at: "2025-01-01T00:00:00Z".to_string(),
                description: "test".to_string(),
                properties: Some(TemplatePropertyRes {
                    blob_docker_reference: "test".to_string(),
                    blob_docker_tag: "latest".to_string(),
                    template_docker_reference: "test".to_string(),
                    template_docker_tag: "latest".to_string(),
                }),
            },
            template: TemplatePrincipalRes {
                id: id.to_string(),
                name: name.to_string(),
                project: "test-project".to_string(),
                source: "local".to_string(),
                email: "test@test.com".to_string(),
                tags: vec![],
                description: "test".to_string(),
                readme: "".to_string(),
                user_id: "user1".to_string(),
            },
            plugins: vec![PluginVersionPrincipalRes {
                id: "plugin1".to_string(),
                version: 1,
                created_at: "2025-01-01T00:00:00Z".to_string(),
                description: "test".to_string(),
                docker_reference: "test".to_string(),
                docker_tag: "latest".to_string(),
            }],
            processors: vec![],
            templates,
            resolvers: vec![],
            commands: vec![],
        }
    }

    /// Mock DependencyResolver that simulates preset answer extraction from dependency refs
    struct MockDependencyResolver {
        responses: HashMap<String, (TemplateVersionRes, HashMap<String, Answer>)>,
    }

    impl MockDependencyResolver {
        fn new() -> Self {
            Self {
                responses: HashMap::new(),
            }
        }

        fn add_response(
            &mut self,
            template_id: &str,
            template: TemplateVersionRes,
            preset_answers: HashMap<String, Answer>,
        ) {
            self.responses
                .insert(template_id.to_string(), (template, preset_answers));
        }
    }

    impl DependencyResolver for MockDependencyResolver {
        fn resolve_dependencies(
            &self,
            template: &TemplateVersionRes,
        ) -> Result<Vec<ResolvedDependency>, Box<dyn std::error::Error + Send>> {
            let mut resolved = Vec::new();

            // Simulate what DefaultDependencyResolver does: iterate templates field,
            // extract preset_answers, create ResolvedDependency
            for dep_ref in &template.templates {
                let preset_answers: HashMap<String, Answer> = dep_ref
                    .preset_answers
                    .iter()
                    .filter_map(|(key, value)| {
                        serde_json_value_to_answer(value).map(|answer| (key.clone(), answer))
                    })
                    .collect();

                // In a real resolver, we'd fetch from registry. For mock, look up from responses.
                if let Some((dep_template, _)) = self.responses.get(&dep_ref.id) {
                    resolved.push(ResolvedDependency {
                        template: dep_template.clone(),
                        preset_answers,
                    });
                } else {
                    // If no explicit response, create a minimal template
                    resolved.push(ResolvedDependency {
                        template: make_template_version(
                            &dep_ref.id,
                            "mock-dep",
                            dep_ref.version,
                            vec![],
                        ),
                        preset_answers,
                    });
                }
            }

            // Add root template (no preset answers)
            resolved.push(ResolvedDependency {
                template: template.clone(),
                preset_answers: HashMap::new(),
            });

            Ok(resolved)
        }
    }

    /// Test that MockDependencyResolver correctly extracts preset_answers from dependency refs
    /// and carries them through as ResolvedDependency
    #[test]
    fn test_resolver_carries_preset_answers_through() {
        let mut dep1_preset = HashMap::new();
        dep1_preset.insert("db_host".to_string(), serde_json::json!("localhost"));
        dep1_preset.insert("db_port".to_string(), serde_json::json!("5432"));

        let mut dep2_preset = HashMap::new();
        dep2_preset.insert(
            "cache_url".to_string(),
            serde_json::json!("redis://localhost:6379"),
        );

        let dep1_ref = TemplateVersionTemplateRefRes {
            id: "dep-1".to_string(),
            version: 1,
            preset_answers: dep1_preset,
        };
        let dep2_ref = TemplateVersionTemplateRefRes {
            id: "dep-2".to_string(),
            version: 1,
            preset_answers: dep2_preset,
        };

        let root = make_template_version("root", "root-template", 1, vec![dep1_ref, dep2_ref]);
        let dep1_template = make_template_version("dep-1", "dep-1-template", 1, vec![]);
        let dep2_template = make_template_version("dep-2", "dep-2-template", 1, vec![]);

        let mut resolver = MockDependencyResolver::new();
        resolver.add_response("dep-1", dep1_template, HashMap::new());
        resolver.add_response("dep-2", dep2_template, HashMap::new());

        let result = resolver
            .resolve_dependencies(&root)
            .expect("resolve should succeed");

        // Should have 3 entries: dep-1, dep-2, root
        assert_eq!(result.len(), 3, "Should have 3 resolved dependencies");

        // Verify dep-1 has its preset answers
        let dep1 = &result[0];
        assert_eq!(dep1.template.principal.id, "dep-1");
        assert_eq!(
            dep1.preset_answers.len(),
            2,
            "dep-1 should have 2 preset answers"
        );

        if let Some(Answer::String(s)) = dep1.preset_answers.get("db_host") {
            assert_eq!(s, "localhost", "dep-1 db_host preset should be 'localhost'");
        } else {
            panic!("Expected Answer::String for db_host");
        }
        if let Some(Answer::String(s)) = dep1.preset_answers.get("db_port") {
            assert_eq!(s, "5432", "dep-1 db_port preset should be '5432'");
        } else {
            panic!("Expected Answer::String for db_port");
        }

        // Verify dep-2 has its preset answers
        let dep2 = &result[1];
        assert_eq!(dep2.template.principal.id, "dep-2");
        assert_eq!(
            dep2.preset_answers.len(),
            1,
            "dep-2 should have 1 preset answer"
        );

        if let Some(Answer::String(s)) = dep2.preset_answers.get("cache_url") {
            assert_eq!(
                s, "redis://localhost:6379",
                "dep-2 cache_url preset should be correct"
            );
        } else {
            panic!("Expected Answer::String for cache_url");
        }

        // Verify root has no preset answers
        let root_dep = &result[2];
        assert_eq!(root_dep.template.principal.id, "root");
        assert!(
            root_dep.preset_answers.is_empty(),
            "Root template should have no preset answers"
        );
    }

    /// Test that preset answer conversion from TemplateVersionTemplateRefRes handles all supported types
    #[test]
    fn test_preset_answer_type_conversion_from_template_ref() {
        let mut preset = HashMap::new();
        preset.insert("string_key".to_string(), serde_json::json!("string_value"));
        preset.insert("bool_key".to_string(), serde_json::json!(true));
        preset.insert("array_key".to_string(), serde_json::json!(["a", "b", "c"]));
        preset.insert("number_key".to_string(), serde_json::json!(42)); // Should be skipped
        preset.insert("null_key".to_string(), serde_json::json!(null)); // Should be skipped

        let dep_ref = TemplateVersionTemplateRefRes {
            id: "dep-with-mixed".to_string(),
            version: 1,
            preset_answers: preset,
        };

        // Simulate the extraction logic from DefaultDependencyResolver.flatten_dependencies
        let converted: HashMap<String, Answer> = dep_ref
            .preset_answers
            .iter()
            .filter_map(|(key, value)| {
                serde_json_value_to_answer(value).map(|answer| (key.clone(), answer))
            })
            .collect();

        assert_eq!(
            converted.len(),
            3,
            "Only String, Bool, and StringArray should be converted; Number and Null skipped"
        );

        if let Some(Answer::String(s)) = converted.get("string_key") {
            assert_eq!(s, "string_value");
        } else {
            panic!("Expected Answer::String for string_key");
        }

        if let Some(Answer::Bool(b)) = converted.get("bool_key") {
            assert!(*b);
        } else {
            panic!("Expected Answer::Bool for bool_key");
        }

        if let Some(Answer::StringArray(arr)) = converted.get("array_key") {
            assert_eq!(arr, &["a", "b", "c"]);
        } else {
            panic!("Expected Answer::StringArray for array_key");
        }

        assert!(
            !converted.contains_key("number_key"),
            "Number should be filtered out"
        );
        assert!(
            !converted.contains_key("null_key"),
            "Null should be filtered out"
        );
    }

    // =========================================================================
    // DefaultDependencyResolver real-implementation tests
    // =========================================================================

    /// Test that DefaultDependencyResolver::flatten_with_fetcher correctly extracts
    /// preset_answers from TemplateVersionTemplateRefRes during traversal.
    /// This exercises the actual flatten_dependencies code path with a mock fetcher.
    #[test]
    fn test_default_resolver_extracts_preset_answers_from_dep_refs() {
        // Build test templates
        let mut dep1_preset = HashMap::new();
        dep1_preset.insert("db_host".to_string(), serde_json::json!("localhost"));
        dep1_preset.insert("db_port".to_string(), serde_json::json!("5432"));

        let mut dep2_preset = HashMap::new();
        dep2_preset.insert(
            "cache_url".to_string(),
            serde_json::json!("redis://localhost:6379"),
        );

        let dep1_ref = TemplateVersionTemplateRefRes {
            id: "dep-1".to_string(),
            version: 1,
            preset_answers: dep1_preset,
        };
        let dep2_ref = TemplateVersionTemplateRefRes {
            id: "dep-2".to_string(),
            version: 1,
            preset_answers: dep2_preset,
        };

        let root = make_template_version("root", "root-template", 1, vec![dep1_ref, dep2_ref]);
        let dep1_template = make_template_version("dep-1", "dep-1-template", 1, vec![]);
        let dep2_template = make_template_version("dep-2", "dep-2-template", 1, vec![]);

        // Create a mock fetcher that returns templates based on ID
        // Wrap templates in Rc so the closure can own its data (dyn Fn requires 'static)
        let templates: Rc<HashMap<String, TemplateVersionRes>> = Rc::new(HashMap::from([
            ("dep-1".to_string(), dep1_template),
            ("dep-2".to_string(), dep2_template),
        ]));

        let mut visited = HashSet::new();
        let result = flatten_dependencies_with_fetcher(
            &root,
            &mut visited,
            Rc::new(move |id| match templates.get(&id).cloned() {
                Some(t) => Ok(t),
                None => Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Template not found: {id}"),
                )) as Box<dyn std::error::Error + Send>),
            }),
        )
        .expect("flatten_dependencies_with_fetcher should succeed");

        // Should have 2 entries: dep-1 and dep-2 (root is added by resolve_dependencies)
        assert_eq!(result.len(), 2, "Should have 2 resolved dependencies");

        // Verify dep-1 has its preset answers
        let dep1 = &result[0];
        assert_eq!(dep1.template.principal.id, "dep-1");
        assert_eq!(
            dep1.preset_answers.len(),
            2,
            "dep-1 should have 2 preset answers"
        );

        if let Some(Answer::String(s)) = dep1.preset_answers.get("db_host") {
            assert_eq!(s, "localhost", "dep-1 db_host preset should be 'localhost'");
        } else {
            panic!("Expected Answer::String for db_host");
        }
        if let Some(Answer::String(s)) = dep1.preset_answers.get("db_port") {
            assert_eq!(s, "5432", "dep-1 db_port preset should be '5432'");
        } else {
            panic!("Expected Answer::String for db_port");
        }

        // Verify dep-2 has its preset answers
        let dep2 = &result[1];
        assert_eq!(dep2.template.principal.id, "dep-2");
        assert_eq!(
            dep2.preset_answers.len(),
            1,
            "dep-2 should have 1 preset answer"
        );

        if let Some(Answer::String(s)) = dep2.preset_answers.get("cache_url") {
            assert_eq!(
                s, "redis://localhost:6379",
                "dep-2 cache_url preset should be correct"
            );
        } else {
            panic!("Expected Answer::String for cache_url");
        }
    }

    /// Test that DefaultDependencyResolver::flatten_with_fetcher correctly handles
    /// nested dependency trees (post-order traversal).
    #[test]
    fn test_default_resolver_nested_dependency_traversal() {
        // Build: root -> [A -> [B], C]
        // Expected post-order: B, A, C

        let b_ref = TemplateVersionTemplateRefRes {
            id: "B".to_string(),
            version: 1,
            preset_answers: HashMap::new(),
        };
        let a_ref = TemplateVersionTemplateRefRes {
            id: "A".to_string(),
            version: 1,
            preset_answers: HashMap::new(),
        };
        let c_ref = TemplateVersionTemplateRefRes {
            id: "C".to_string(),
            version: 1,
            preset_answers: HashMap::new(),
        };

        let root = make_template_version("root", "root", 1, vec![a_ref, c_ref]);
        let template_a = make_template_version("A", "A", 1, vec![b_ref]);
        let template_b = make_template_version("B", "B", 1, vec![]);
        let template_c = make_template_version("C", "C", 1, vec![]);

        // Wrap templates in Rc so the closure can own its data (dyn Fn requires 'static)
        let templates: Rc<HashMap<String, TemplateVersionRes>> = Rc::new(HashMap::from([
            ("A".to_string(), template_a),
            ("B".to_string(), template_b),
            ("C".to_string(), template_c),
        ]));

        let mut visited = HashSet::new();
        let result = flatten_dependencies_with_fetcher(
            &root,
            &mut visited,
            Rc::new(move |id| match templates.get(&id).cloned() {
                Some(t) => Ok(t),
                None => Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Template not found: {id}"),
                )) as Box<dyn std::error::Error + Send>),
            }),
        )
        .expect("flatten_dependencies_with_fetcher should succeed");

        // Should have 3 entries: B, A, C (post-order)
        assert_eq!(result.len(), 3, "Should have 3 resolved dependencies");

        // Verify post-order: B first (leaf), then A (parent of B), then C
        assert_eq!(
            result[0].template.principal.id, "B",
            "First should be B (leaf, post-order)"
        );
        assert_eq!(
            result[1].template.principal.id, "A",
            "Second should be A (parent of B)"
        );
        assert_eq!(result[2].template.principal.id, "C", "Third should be C");
    }

    /// Test that DefaultDependencyResolver::flatten_with_fetcher correctly merges
    /// preset_answers when the same template is referenced multiple times.
    #[test]
    fn test_default_resolver_merges_duplicate_template_preset_answers() {
        // Build: root -> [B (preset: {key1: "first"}), B (preset: {key2: "second"})]
        // Both dep_refs point to template B but with different preset_answers.
        // Expected: B should appear once with BOTH preset_answers merged.

        let mut preset1 = HashMap::new();
        preset1.insert("key1".to_string(), serde_json::json!("first"));

        let mut preset2 = HashMap::new();
        preset2.insert("key2".to_string(), serde_json::json!("second"));

        let b_ref_1 = TemplateVersionTemplateRefRes {
            id: "B".to_string(),
            version: 1,
            preset_answers: preset1,
        };
        let b_ref_2 = TemplateVersionTemplateRefRes {
            id: "B".to_string(),
            version: 1,
            preset_answers: preset2,
        };

        // Note: The root template's templates list has TWO refs to B
        let root = make_template_version("root", "root", 1, vec![b_ref_1, b_ref_2]);
        let template_b = make_template_version("B", "B", 1, vec![]);

        // Wrap templates in Rc so the closure can own its data (dyn Fn requires 'static)
        let templates: Rc<HashMap<String, TemplateVersionRes>> =
            Rc::new(HashMap::from([("B".to_string(), template_b)]));

        let mut visited = HashSet::new();
        let result = flatten_dependencies_with_fetcher(
            &root,
            &mut visited,
            Rc::new(move |id| match templates.get(&id).cloned() {
                Some(t) => Ok(t),
                None => Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Template not found: {id}"),
                )) as Box<dyn std::error::Error + Send>),
            }),
        )
        .expect("flatten_dependencies_with_fetcher should succeed");

        // Should have 1 entry (B appears once, with merged preset_answers)
        assert_eq!(
            result.len(),
            1,
            "B should appear once (duplicate merged, not skipped)"
        );

        // Verify B has BOTH preset_answers merged
        let b = &result[0];
        assert_eq!(b.template.principal.id, "B");
        assert_eq!(
            b.preset_answers.len(),
            2,
            "B should have 2 preset_answers (merged from both refs)"
        );

        // key1 should come from first ref
        if let Some(Answer::String(s)) = b.preset_answers.get("key1") {
            assert_eq!(s, "first", "key1 should be 'first' from first ref");
        } else {
            panic!("Expected Answer::String for key1");
        }

        // key2 should come from second ref
        if let Some(Answer::String(s)) = b.preset_answers.get("key2") {
            assert_eq!(s, "second", "key2 should be 'second' from second ref");
        } else {
            panic!("Expected Answer::String for key2");
        }
    }

    /// Test that DefaultDependencyResolver::flatten_with_fetcher correctly merges
    /// preset_answers for the cross-branch diamond case.
    ///
    /// Diamond scenario: root → [A, B]; A → X with preset {key1: "first"}; B → X with preset {key2: "second"}
    ///
    /// The resolver must share one `flattened` accumulator across branches: when processing
    /// A's branch X is added to it, so when processing B's branch the merge check finds X
    /// and merges {key2} into it. A per-branch (local, empty) `flattened` would miss X on
    /// B's branch and silently skip it via `visited`, dropping the {key2} preset.
    #[test]
    fn test_default_resolver_merges_cross_branch_diamond_preset_answers() {
        // Build: root -> [A, B]; A -> X with {key1: "first"}; B -> X with {key2: "second"}
        // Diamond:     A
        //             / \
        //            X   B
        //             \ /
        //             root

        let mut x_preset_from_a = HashMap::new();
        x_preset_from_a.insert("key1".to_string(), serde_json::json!("first"));

        let mut x_preset_from_b = HashMap::new();
        x_preset_from_b.insert("key2".to_string(), serde_json::json!("second"));

        let x_ref_in_a = TemplateVersionTemplateRefRes {
            id: "X".to_string(),
            version: 1,
            preset_answers: x_preset_from_a,
        };
        let x_ref_in_b = TemplateVersionTemplateRefRes {
            id: "X".to_string(),
            version: 1,
            preset_answers: x_preset_from_b,
        };

        let a_ref = TemplateVersionTemplateRefRes {
            id: "A".to_string(),
            version: 1,
            preset_answers: HashMap::new(),
        };
        let b_ref = TemplateVersionTemplateRefRes {
            id: "B".to_string(),
            version: 1,
            preset_answers: HashMap::new(),
        };

        // root has deps A and B (sorted: A, B)
        let root = make_template_version("root", "root", 1, vec![a_ref, b_ref]);
        // A has dep X
        let template_a = make_template_version("A", "A", 1, vec![x_ref_in_a]);
        // B has dep X
        let template_b = make_template_version("B", "B", 1, vec![x_ref_in_b]);
        // X has no deps
        let template_x = make_template_version("X", "X", 1, vec![]);

        let templates: Rc<HashMap<String, TemplateVersionRes>> = Rc::new(HashMap::from([
            ("A".to_string(), template_a),
            ("B".to_string(), template_b),
            ("X".to_string(), template_x),
        ]));

        let mut visited = HashSet::new();
        let result = flatten_dependencies_with_fetcher(
            &root,
            &mut visited,
            Rc::new(move |id| match templates.get(&id).cloned() {
                Some(t) => Ok(t),
                None => Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Template not found: {id}"),
                )) as Box<dyn std::error::Error + Send>),
            }),
        )
        .expect("flatten_dependencies_with_fetcher should succeed");

        // Should have 3 entries: X, A, B (post-order: X first, then A, then B)
        assert_eq!(
            result.len(),
            3,
            "Should have 3 resolved dependencies (X, A, B)"
        );

        // Find X in the result
        let x_dep = result
            .iter()
            .find(|d| d.template.principal.id == "X")
            .expect("X should be in the result");

        // CRITICAL: X should have BOTH preset_answers merged (from A's and B's references)
        assert_eq!(
            x_dep.preset_answers.len(),
            2,
            "X should have 2 preset_answers (merged from both A's and B's references)"
        );

        // key1 should come from A's reference to X
        if let Some(Answer::String(s)) = x_dep.preset_answers.get("key1") {
            assert_eq!(s, "first", "key1 should be 'first' from A's reference");
        } else {
            panic!("Expected Answer::String for key1");
        }

        // key2 should come from B's reference to X
        if let Some(Answer::String(s)) = x_dep.preset_answers.get("key2") {
            assert_eq!(s, "second", "key2 should be 'second' from B's reference");
        } else {
            panic!("Expected Answer::String for key2");
        }
    }

    /// Test that DefaultDependencyResolver::flatten_with_fetcher correctly handles
    /// cyclic dependencies by skipping already-visited templates.
    #[test]
    fn test_default_resolver_skips_cyclic_dependencies() {
        // Build: A -> [B -> [A]] (cyclic: A references B, B references A)
        // flatten_dependencies_with_fetcher(&template_b, ...):
        //   template_b's deps: [A] -> process A -> visited={A} -> recurse into A's deps: [B]
        //   -> process B -> visited={A,B} -> recurse into B's deps: [A] -> A already visited, skip
        //   -> push B -> push A -> result: [B, A]

        let a_ref = TemplateVersionTemplateRefRes {
            id: "A".to_string(),
            version: 1,
            preset_answers: HashMap::new(),
        };
        let b_ref = TemplateVersionTemplateRefRes {
            id: "B".to_string(),
            version: 1,
            preset_answers: HashMap::new(),
        };

        // A references B, B references A (cyclic)
        let template_a = make_template_version("A", "A", 1, vec![b_ref]);
        let template_b = make_template_version("B", "B", 1, vec![a_ref]);

        // Wrap templates in Rc so the closure can own its data (dyn Fn requires 'static)
        // Clone template_b since we need to pass &template_b to flatten_dependencies_with_fetcher
        let templates: Rc<HashMap<String, TemplateVersionRes>> = Rc::new(HashMap::from([
            ("A".to_string(), template_a),
            ("B".to_string(), template_b.clone()),
        ]));

        let mut visited = HashSet::new();
        let result = flatten_dependencies_with_fetcher(
            &template_b,
            &mut visited,
            Rc::new(move |id| match templates.get(&id).cloned() {
                Some(t) => Ok(t),
                None => Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Template not found: {id}"),
                )) as Box<dyn std::error::Error + Send>),
            }),
        )
        .expect("flatten_dependencies_with_fetcher should succeed");

        // Should have 2 entries: B, A (post-order traversal)
        assert_eq!(result.len(), 2, "Should have 2 entries (cycle prevented)");
        assert_eq!(
            result[0].template.principal.id, "B",
            "First should be B (leaf in traversal, pushed before A)"
        );
        assert_eq!(
            result[1].template.principal.id, "A",
            "Second should be A (pushed after recursion into its deps returns)"
        );
    }

    /// Test that DefaultDependencyResolver::resolve_dependencies_with_fetcher correctly
    /// handles cyclic graphs where the root appears in its own dependency chain.
    /// The root should appear exactly once in the result (not twice).
    #[test]
    fn test_resolve_dependencies_prevents_root_duplicate_on_cycle() {
        // Build: A -> [B -> [A]] (A is root, A depends on B, B depends on A)
        // resolve_dependencies should:
        //   1. Mark A as visited BEFORE traversal
        //   2. Process B (dep of A)
        //   3. Try to process A (dep of B) but skip since A is already visited
        //   4. Append root A at the end
        // Expected result: [B, A] with A appearing exactly once

        let b_ref = TemplateVersionTemplateRefRes {
            id: "B".to_string(),
            version: 1,
            preset_answers: HashMap::new(),
        };
        let a_ref = TemplateVersionTemplateRefRes {
            id: "A".to_string(),
            version: 1,
            preset_answers: HashMap::new(),
        };

        // A depends on B, B depends on A (cycle back to root)
        let template_a = make_template_version("A", "A", 1, vec![b_ref]);
        let template_b = make_template_version("B", "B", 1, vec![a_ref]);

        let templates: Rc<HashMap<String, TemplateVersionRes>> = Rc::new(HashMap::from([
            ("A".to_string(), template_a.clone()),
            ("B".to_string(), template_b),
        ]));

        let result = DefaultDependencyResolver::resolve_dependencies_with_fetcher(
            &template_a,
            Rc::new(move |id| match templates.get(&id).cloned() {
                Some(t) => Ok(t),
                None => Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Template not found: {id}"),
                )) as Box<dyn std::error::Error + Send>),
            }),
        )
        .expect("resolve_dependencies_with_fetcher should succeed");

        // Should have exactly 2 entries: B (dependency), A (root)
        // A should appear exactly once, not twice (once from recursion, once as root)
        assert_eq!(
            result.len(),
            2,
            "Should have 2 entries (root appears exactly once)"
        );

        // Verify B is first (dependency processed before root append)
        assert_eq!(
            result[0].template.principal.id, "B",
            "First should be B (dependency of root A)"
        );

        // Verify A is second (root appended at the end) and appears exactly once
        assert_eq!(
            result[1].template.principal.id, "A",
            "Second should be A (root, appended at end)"
        );

        // Verify A appears only once by counting occurrences
        let a_count = result
            .iter()
            .filter(|d| d.template.principal.id == "A")
            .count();
        assert_eq!(
            a_count, 1,
            "Root A should appear exactly once, not twice (cycle prevented by pre-marking root as visited)"
        );
    }

    // =========================================================================
    // collect_commands Tests (spec 2)
    // =========================================================================

    /// Helper to build a minimal TemplateVersionRes with commands
    fn make_template_with_commands(
        id: &str,
        name: &str,
        commands: Vec<String>,
    ) -> TemplateVersionRes {
        TemplateVersionRes {
            principal: TemplateVersionPrincipalRes {
                id: id.to_string(),
                version: 1,
                created_at: "2025-01-01T00:00:00Z".to_string(),
                description: "test".to_string(),
                properties: Some(TemplatePropertyRes {
                    blob_docker_reference: "test".to_string(),
                    blob_docker_tag: "latest".to_string(),
                    template_docker_reference: "test".to_string(),
                    template_docker_tag: "latest".to_string(),
                }),
            },
            template: TemplatePrincipalRes {
                id: id.to_string(),
                name: name.to_string(),
                project: "test-project".to_string(),
                source: "local".to_string(),
                email: "test@test.com".to_string(),
                tags: vec![],
                description: "test".to_string(),
                readme: "".to_string(),
                user_id: "user1".to_string(),
            },
            plugins: vec![],
            processors: vec![],
            templates: vec![],
            resolvers: vec![],
            commands,
        }
    }

    /// Test that collect_commands returns empty vec for empty dependency list
    #[test]
    fn test_collect_commands_empty_list() {
        let deps: Vec<ResolvedDependency> = vec![];
        let result = CompositionOperator::collect_commands(&deps);
        assert!(
            result.is_empty(),
            "Empty dependency list should return empty commands"
        );
    }

    /// Test that collect_commands returns commands from single template
    #[test]
    fn test_collect_commands_single_template() {
        let template = make_template_with_commands("t1", "template1", vec!["build".to_string()]);
        let deps = vec![ResolvedDependency {
            template,
            preset_answers: HashMap::new(),
        }];
        let result = CompositionOperator::collect_commands(&deps);
        assert_eq!(result.len(), 1, "Should have 1 command");
        assert_eq!(result[0], "build");
    }

    /// Test that collect_commands returns commands from multiple templates in order
    #[test]
    fn test_collect_commands_multiple_templates_in_order() {
        let template1 = make_template_with_commands("t1", "template1", vec!["build".to_string()]);
        let template2 = make_template_with_commands(
            "t2",
            "template2",
            vec!["test".to_string(), "lint".to_string()],
        );
        let template3 = make_template_with_commands("t3", "template3", vec!["deploy".to_string()]);

        let deps = vec![
            ResolvedDependency {
                template: template1,
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: template2,
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: template3,
                preset_answers: HashMap::new(),
            },
        ];

        let result = CompositionOperator::collect_commands(&deps);
        assert_eq!(result.len(), 4, "Should have 4 commands");
        assert_eq!(result[0], "build");
        assert_eq!(result[1], "test");
        assert_eq!(result[2], "lint");
        assert_eq!(result[3], "deploy");
    }

    /// Test that collect_commands skips templates with empty commands
    #[test]
    fn test_collect_commands_skips_empty_commands() {
        let template1 = make_template_with_commands("t1", "template1", vec!["build".to_string()]);
        let template2 = make_template_with_commands("t2", "template2", vec![]); // No commands
        let template3 = make_template_with_commands("t3", "template3", vec!["deploy".to_string()]);

        let deps = vec![
            ResolvedDependency {
                template: template1,
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: template2,
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: template3,
                preset_answers: HashMap::new(),
            },
        ];

        let result = CompositionOperator::collect_commands(&deps);
        assert_eq!(result.len(), 2, "Should have 2 commands (skipping empty)");
        assert_eq!(result[0], "build");
        assert_eq!(result[1], "deploy");
    }

    /// Test that collect_commands handles mix of empty and non-empty commands
    #[test]
    fn test_collect_commands_mixed_empty_and_non_empty() {
        let template1 = make_template_with_commands("t1", "template1", vec![]);
        let template2 = make_template_with_commands("t2", "template2", vec!["test".to_string()]);
        let template3 = make_template_with_commands("t3", "template3", vec![]);
        let template4 = make_template_with_commands(
            "t4",
            "template4",
            vec!["package".to_string(), "upload".to_string()],
        );

        let deps = vec![
            ResolvedDependency {
                template: template1,
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: template2,
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: template3,
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: template4,
                preset_answers: HashMap::new(),
            },
        ];

        let result = CompositionOperator::collect_commands(&deps);
        assert_eq!(result.len(), 3, "Should have 3 commands total");
        assert_eq!(result[0], "test");
        assert_eq!(result[1], "package");
        assert_eq!(result[2], "upload");
    }

    /// Test that collect_commands maintains post-order (dep order)
    #[test]
    fn test_collect_commands_maintains_post_order() {
        // Post-order means: dependencies first (in reverse dependency order), then root
        // Build: root -> [A, B]; A -> [C]]
        // Expected post-order: C, A, B (dependencies processed before their parents)
        // Commands should be collected in this order

        let template_c = make_template_with_commands("C", "child", vec!["cmd_c".to_string()]);
        let template_a = make_template_with_commands("A", "parent_a", vec!["cmd_a".to_string()]);
        let template_b = make_template_with_commands("B", "parent_b", vec!["cmd_b".to_string()]);
        let template_root =
            make_template_with_commands("root", "root", vec!["cmd_root".to_string()]);

        // Simulate post-order: C first, then A, then B, then root
        let deps = vec![
            ResolvedDependency {
                template: template_c,
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: template_a,
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: template_b,
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: template_root,
                preset_answers: HashMap::new(),
            },
        ];

        let result = CompositionOperator::collect_commands(&deps);
        assert_eq!(result.len(), 4, "Should have 4 commands");
        assert_eq!(result[0], "cmd_c");
        assert_eq!(result[1], "cmd_a");
        assert_eq!(result[2], "cmd_b");
        assert_eq!(result[3], "cmd_root");
    }

    // =========================================================================
    // collect_commands_from_templates Tests
    // =========================================================================

    /// Test that collect_commands_from_templates returns empty for empty list
    #[test]
    fn test_collect_commands_from_templates_empty() {
        let templates: Vec<TemplateVersionRes> = vec![];
        let result = CompositionOperator::collect_commands_from_templates(&templates);
        assert!(result.is_empty());
    }

    /// Test that collect_commands_from_templates collects from single template
    #[test]
    fn test_collect_commands_from_templates_single() {
        let template = make_template_with_commands("t1", "template1", vec!["build".to_string()]);
        let result = CompositionOperator::collect_commands_from_templates(&[template]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "build");
    }

    /// Test that collect_commands_from_templates skips empty commands
    #[test]
    fn test_collect_commands_from_templates_skips_empty() {
        let t1 = make_template_with_commands("t1", "template1", vec![]);
        let t2 = make_template_with_commands("t2", "template2", vec!["test".to_string()]);
        let t3 = make_template_with_commands(
            "t3",
            "template3",
            vec!["deploy".to_string(), "lint".to_string()],
        );
        let result = CompositionOperator::collect_commands_from_templates(&[t1, t2, t3]);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "test");
        assert_eq!(result[1], "deploy");
        assert_eq!(result[2], "lint");
    }

    // =========================================================================
    // Headless composition-boundary tests (NeedInput surfacing + per-template id namespacing)
    //
    // These drive the REAL execute_template → execute_composition pipeline with a
    // fake TemplateExecutor stubbed at the trait boundary, closing the prior coverage gap
    // where every headless test called engine.start_headless directly and never
    // exercised unpack_archive / the composition operator.
    // =========================================================================

    use crate::fs::{DefaultVfs, DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker};
    use crate::operations::TemplateOperator;
    use crate::session::DefaultSessionIdGenerator;
    use crate::template::{DefaultTemplateHistory, TemplateExecutor};
    use cyanprompt::domain::models::cyan::Cyan;
    use cyanprompt::domain::models::question::{Question, QuestionTrait, TextQuestion};
    use cyanprompt::domain::services::template::states::TemplateState;
    use cyanregistry::http::client::CyanRegistryClient;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A minimal valid (empty) tar.gz archive — a single zero-length entry header plus
    /// the two end-of-archive zero blocks, then gzip-framed. Producing real output lets
    /// the "complete" path's `unpack_archive` succeed.
    fn empty_tar_gz() -> Vec<u8> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;
        // 512-byte zero block repeated twice = end-of-archive marker (empty archive).
        let zero_block = [0u8; 512];
        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(&zero_block).unwrap();
        gz.write_all(&zero_block).unwrap();
        gz.finish().unwrap()
    }

    /// A scripted fake `TemplateExecutor` that returns canned outcomes per call. Used to
    /// drive the real composition pipeline without a live coordinator.
    struct StubExecutor {
        /// Canned results returned in order, one per execute_template call. Each is
        /// (archive_data, template_state, session_id).
        scripted: std::sync::Mutex<Vec<(Vec<u8>, TemplateState, String)>>,
        calls: Arc<AtomicUsize>,
    }

    impl TemplateExecutor for StubExecutor {
        fn execute_template(
            &self,
            _template: &TemplateVersionRes,
            _session_id: &str,
            _answers: Option<&HashMap<String, Answer>>,
            _deterministic_states: Option<&HashMap<String, String>>,
        ) -> Result<(Vec<u8>, TemplateState, String), Box<dyn std::error::Error + Send>> {
            let idx = self.calls.fetch_add(1, Ordering::SeqCst);
            let guard = self.scripted.lock().unwrap();
            if idx < guard.len() {
                let (archive, state, sid) = guard[idx].clone();
                Ok((archive, state, sid))
            } else {
                // Default: complete with an empty archive if the script runs out.
                Ok((
                    empty_tar_gz(),
                    TemplateState::Complete(
                        Cyan {
                            processors: vec![],
                            plugins: vec![],
                        },
                        HashMap::new(),
                    ),
                    format!("session-{idx}"),
                ))
            }
        }
    }

    /// Build a `CompositionOperator` wired to a fake executor + real DefaultVfs (whose
    /// TarGzUnpacker errors on an EMPTY byte stream — exactly the empty-archive NeedInput scenario). The
    /// registry client is never used by execute_composition (dependencies are pre-built).
    fn operator_with_executor(executor: StubExecutor) -> CompositionOperator {
        let registry = Rc::new(CyanRegistryClient {
            endpoint: "http://unused.invalid".to_string(),
            version: "1.0".to_string(),
            client: Rc::new(reqwest::blocking::Client::new()),
        });
        let unpacker = Box::new(TarGzUnpacker);
        let loader = Box::new(DiskFileLoader);
        let merger = Box::new(GitLikeMerger::new(false, 50));
        let writer = Box::new(DiskFileWriter);
        let template_operator = TemplateOperator::new(
            Box::new(DefaultSessionIdGenerator),
            Box::new(executor),
            Box::new(DefaultTemplateHistory::new()),
            Box::new(DefaultVfs::new(unpacker, loader, merger, writer)),
            registry,
        );
        let dependency_resolver: Box<dyn DependencyResolver> =
            Box::new(MockDependencyResolver::new());
        CompositionOperator::new(
            template_operator,
            dependency_resolver,
            Box::new(DefaultVfsLayerer),
        )
    }

    /// Like [`operator_with_executor`] but accepts an arbitrary `dyn TemplateExecutor`,
    /// so a test can wire a bespoke capturing/branching executor into the REAL composition
    /// pipeline (the only way to observe per-dependency answer routing).
    fn operator_with_dyn_executor(executor: Box<dyn TemplateExecutor>) -> CompositionOperator {
        let registry = Rc::new(CyanRegistryClient {
            endpoint: "http://unused.invalid".to_string(),
            version: "1.0".to_string(),
            client: Rc::new(reqwest::blocking::Client::new()),
        });
        let unpacker = Box::new(TarGzUnpacker);
        let loader = Box::new(DiskFileLoader);
        let merger = Box::new(GitLikeMerger::new(false, 50));
        let writer = Box::new(DiskFileWriter);
        let template_operator = TemplateOperator::new(
            Box::new(DefaultSessionIdGenerator),
            executor,
            Box::new(DefaultTemplateHistory::new()),
            Box::new(DefaultVfs::new(unpacker, loader, merger, writer)),
            registry,
        );
        let dependency_resolver: Box<dyn DependencyResolver> =
            Box::new(MockDependencyResolver::new());
        CompositionOperator::new(
            template_operator,
            dependency_resolver,
            Box::new(DefaultVfsLayerer),
        )
    }

    // A NeedInput template state MUST be surfaced (need_input set, no error) even
    // though the executor returns an EMPTY archive for it. Before the fix, execute_composition
    // called unpack_archive(Vec::new()) BEFORE checking need_input → "unexpected end of file"
    // error envelope (exit 1) instead of need_input (exit 2).
    #[test]
    fn need_input_is_surfaced_without_unpacking_empty_archive() {
        let question = Question::Text(TextQuestion {
            message: "Name?".to_string(),
            default: None,
            desc: None,
            initial: None,
            id: "name".to_string(),
        });
        let need_state = TemplateState::NeedInput(question, HashMap::new());
        // Empty archive is exactly what DefaultTemplateExecutor returns on NeedInput.
        let executor = StubExecutor {
            scripted: std::sync::Mutex::new(vec![(Vec::new(), need_state, "s1".to_string())]),
            calls: Arc::new(AtomicUsize::new(0)),
        };
        let mut operator = operator_with_executor(executor);

        let template = make_template_version("root", "root", 1, vec![]);
        let deps = vec![ResolvedDependency {
            template: template.clone(),
            preset_answers: HashMap::new(),
        }];
        let shared_state = CompositionState::new();
        let (vfs, final_state, session_ids) = operator
            .execute_composition(&deps, &shared_state, true)
            .expect("NeedInput must not error via unpack_archive");

        // The NeedInput question surfaced: exit-2 path, not exit-1.
        assert!(
            final_state.need_input.is_some(),
            "NeedInput must be surfaced, not turned into an unpack error"
        );
        assert_eq!(
            final_state.need_input.as_ref().unwrap().id(),
            "name",
            "single-template (no namespace): id stays raw"
        );
        // Single dependency → no namespace applied, question id is raw.
        assert_eq!(session_ids.len(), 1);
        // No files (empty VFS discarded on the NeedInput path).
        assert!(vfs.get_paths().is_empty());
    }

    // Regression: an EMPTY archive fed to a COMPLETE path still errors (the fix did
    // not weaken the happy path's unpack). Only NeedInput skips unpacking.
    #[test]
    fn empty_archive_on_complete_path_still_errors() {
        let complete = TemplateState::Complete(
            Cyan {
                processors: vec![],
                plugins: vec![],
            },
            HashMap::new(),
        );
        let executor = StubExecutor {
            scripted: std::sync::Mutex::new(vec![(Vec::new(), complete, "s1".to_string())]),
            calls: Arc::new(AtomicUsize::new(0)),
        };
        let mut operator = operator_with_executor(executor);

        let template = make_template_version("root", "root", 1, vec![]);
        let deps = vec![ResolvedDependency {
            template: template.clone(),
            preset_answers: HashMap::new(),
        }];
        let result = operator.execute_composition(&deps, &CompositionState::new(), true);
        assert!(
            result.is_err(),
            "an empty archive on the COMPLETE path must still error (unpack is only skipped for NeedInput)"
        );
    }

    // When TWO dependencies both ask a question with the SAME raw id ("name"),
    // the composition namespaces surfaced ids by template id so they do not collide, and
    // routes caller-supplied namespaced answers to the correct dependency.
    #[test]
    fn multi_template_namespaces_same_id_across_dependencies() {
        // Dependency A asks "name", dependency B also asks "name". Both finalizing once
        // their own "name" is present (each sees only its own answer thanks to routing).
        let dep_a = make_template_version("frontend", "frontend", 1, vec![]);
        let dep_b = make_template_version("backend", "backend", 1, vec![]);
        let deps = vec![
            ResolvedDependency {
                template: dep_a,
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: dep_b,
                preset_answers: HashMap::new(),
            },
        ];

        let question = Question::Text(TextQuestion {
            message: "Name?".to_string(),
            default: None,
            desc: None,
            initial: None,
            id: "name".to_string(),
        });

        // First dependency (frontend) returns NeedInput("name") with an empty archive.
        // The surfaced id MUST be namespaced as "frontend/name".
        let need_state = TemplateState::NeedInput(question.clone(), HashMap::new());
        let executor = StubExecutor {
            scripted: std::sync::Mutex::new(vec![(
                Vec::new(),
                need_state,
                "s-frontend".to_string(),
            )]),
            calls: Arc::new(AtomicUsize::new(0)),
        };
        let mut operator = operator_with_executor(executor);
        let shared_state = CompositionState::new();
        let (_vfs, final_state, _session_ids) = operator
            .execute_composition(&deps, &shared_state, true)
            .expect("NeedInput must surface, not error");

        let need = final_state
            .need_input
            .as_ref()
            .expect("NeedInput must be surfaced");
        assert_eq!(
            need.id(),
            "frontend/name",
            "multi-template composition must namespace the surfaced id by template id"
        );

        // Routing: a caller-supplied answer under "backend/name" must reach dependency B's
        // "name" (prefix stripped), and NOT dependency A's. Verify by feeding both deps a
        // Complete outcome and asserting each received only its own "name" answer.
        // Capture what answers each dep saw via a shared log.
        let seen: Arc<std::sync::Mutex<Vec<HashMap<String, Answer>>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));

        struct RoutingExecutor {
            seen: Arc<std::sync::Mutex<Vec<HashMap<String, Answer>>>>,
        }
        impl TemplateExecutor for RoutingExecutor {
            fn execute_template(
                &self,
                _template: &TemplateVersionRes,
                _session_id: &str,
                answers: Option<&HashMap<String, Answer>>,
                _deterministic_states: Option<&HashMap<String, String>>,
            ) -> Result<(Vec<u8>, TemplateState, String), Box<dyn std::error::Error + Send>>
            {
                let ans = answers.cloned().unwrap_or_default();
                self.seen.lock().unwrap().push(ans.clone());
                Ok((
                    empty_tar_gz(),
                    TemplateState::Complete(
                        Cyan {
                            processors: vec![],
                            plugins: vec![],
                        },
                        ans,
                    ),
                    "s".to_string(),
                ))
            }
        }

        let mut operator2 = {
            let registry = Rc::new(CyanRegistryClient {
                endpoint: "http://unused.invalid".to_string(),
                version: "1.0".to_string(),
                client: Rc::new(reqwest::blocking::Client::new()),
            });
            let unpacker = Box::new(TarGzUnpacker);
            let loader = Box::new(DiskFileLoader);
            let merger = Box::new(GitLikeMerger::new(false, 50));
            let writer = Box::new(DiskFileWriter);
            let template_operator = TemplateOperator::new(
                Box::new(DefaultSessionIdGenerator),
                Box::new(RoutingExecutor {
                    seen: Arc::clone(&seen),
                }),
                Box::new(DefaultTemplateHistory::new()),
                Box::new(DefaultVfs::new(unpacker, loader, merger, writer)),
                registry,
            );
            CompositionOperator::new(
                template_operator,
                Box::new(MockDependencyResolver::new()),
                Box::new(DefaultVfsLayerer),
            )
        };

        // Caller supplies namespaced answers for both dependencies.
        let mut shared = CompositionState::new();
        shared.shared_answers.insert(
            "frontend/name".to_string(),
            Answer::String("fe-val".to_string()),
        );
        shared.shared_answers.insert(
            "backend/name".to_string(),
            Answer::String("be-val".to_string()),
        );
        let (_vfs, final_state, _session_ids) = operator2
            .execute_composition(&deps, &shared, true)
            .expect("composition must complete");

        // Both deps completed (no NeedInput).
        assert!(
            final_state.need_input.is_none(),
            "all namespaced answers supplied → both deps complete"
        );

        // Each dependency saw ONLY its own raw "name" (namespace stripped on routing).
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "two dependencies executed");
        assert!(
            matches!(seen[0].get("name"), Some(Answer::String(s)) if s == "fe-val"),
            "frontend dep receives frontend/name → name"
        );
        assert!(
            matches!(seen[1].get("name"), Some(Answer::String(s)) if s == "be-val"),
            "backend dep receives backend/name → name"
        );
        // And the shared_answers (after Complete merge) are namespaced and distinct.
        assert!(
            matches!(final_state.shared_answers.get("frontend/name"), Some(Answer::String(s)) if s == "fe-val")
        );
        assert!(
            matches!(final_state.shared_answers.get("backend/name"), Some(Answer::String(s)) if s == "be-val")
        );
        // No raw "name" leaked into shared answers (would collide).
        assert!(
            !final_state.shared_answers.contains_key("name"),
            "raw id must not leak into the namespaced shared answer map"
        );
    }

    // Regression: a SINGLE dependency keeps raw question ids (no namespace) — the
    // common create/update path is unchanged. This guards against over-namespacing.
    #[test]
    fn single_template_keeps_raw_question_ids() {
        let question = Question::Text(TextQuestion {
            message: "Name?".to_string(),
            default: None,
            desc: None,
            initial: None,
            id: "name".to_string(),
        });
        let need_state = TemplateState::NeedInput(question, HashMap::new());
        let executor = StubExecutor {
            scripted: std::sync::Mutex::new(vec![(Vec::new(), need_state, "s1".to_string())]),
            calls: Arc::new(AtomicUsize::new(0)),
        };
        let mut operator = operator_with_executor(executor);
        let template = make_template_version("root", "root", 1, vec![]);
        let deps = vec![ResolvedDependency {
            template,
            preset_answers: HashMap::new(),
        }];
        let (_vfs, final_state, _session_ids) = operator
            .execute_composition(&deps, &CompositionState::new(), true)
            .expect("NeedInput must surface");
        let need = final_state.need_input.as_ref().unwrap();
        assert_eq!(
            need.id(),
            "name",
            "single dependency: raw id, no namespace prefix"
        );
    }

    // Partial-scope convergence in a multi-template `try group` headless walk: the caller
    // answers ONLY one of two same-named dependencies (`frontend/name` but not
    // `backend/name`). The answered dependency must complete, the unanswered one must
    // surface `NeedInput` namespaced as `backend/name`, and supplying the missing answer
    // walks both to completion. This is the most common real-world `try group` headless
    // path; it lacked an explicit regression test in Loop 1. (Loop-1 review issue #4.)
    #[test]
    fn partial_scope_one_dep_answered_other_surfaces_namespaced_need_input() {
        let deps = vec![
            ResolvedDependency {
                template: make_template_version("frontend", "frontend", 1, vec![]),
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: make_template_version("backend", "backend", 1, vec![]),
                preset_answers: HashMap::new(),
            },
        ];

        let seen: Arc<std::sync::Mutex<Vec<HashMap<String, Answer>>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));

        // Completes once it received a `name` answer, else surfaces NeedInput("name").
        // Records the answer map each dependency saw so routing is observable.
        struct ScopeExecutor {
            seen: Arc<std::sync::Mutex<Vec<HashMap<String, Answer>>>>,
        }
        impl TemplateExecutor for ScopeExecutor {
            fn execute_template(
                &self,
                _template: &TemplateVersionRes,
                _session_id: &str,
                answers: Option<&HashMap<String, Answer>>,
                _deterministic_states: Option<&HashMap<String, String>>,
            ) -> Result<(Vec<u8>, TemplateState, String), Box<dyn std::error::Error + Send>>
            {
                let ans = answers.cloned().unwrap_or_default();
                self.seen.lock().unwrap().push(ans.clone());
                if ans.contains_key("name") {
                    Ok((
                        empty_tar_gz(),
                        TemplateState::Complete(
                            Cyan {
                                processors: vec![],
                                plugins: vec![],
                            },
                            ans,
                        ),
                        "s".to_string(),
                    ))
                } else {
                    let question = Question::Text(TextQuestion {
                        message: "Name?".to_string(),
                        default: None,
                        desc: None,
                        initial: None,
                        id: "name".to_string(),
                    });
                    Ok((
                        Vec::new(),
                        TemplateState::NeedInput(question, HashMap::new()),
                        "s".to_string(),
                    ))
                }
            }
        }

        let mut operator = operator_with_dyn_executor(Box::new(ScopeExecutor {
            seen: Arc::clone(&seen),
        }));

        // Caller answers ONLY frontend/name. frontend completes; backend asks again.
        let mut shared = CompositionState::new();
        shared.shared_answers.insert(
            "frontend/name".to_string(),
            Answer::String("fe".to_string()),
        );
        let (_vfs, final_state, _session_ids) = operator
            .execute_composition(&deps, &shared, true)
            .expect("composition must surface NeedInput, not error");

        {
            let seen = seen.lock().unwrap();
            assert_eq!(seen.len(), 2, "both dependencies executed");
            assert!(
                matches!(seen[0].get("name"), Some(Answer::String(s)) if s == "fe"),
                "frontend received its scoped frontend/name, stripped to raw name"
            );
            assert!(
                !seen[1].contains_key("name"),
                "backend must NOT receive frontend's scoped answer"
            );
        }
        assert_eq!(
            final_state.need_input.as_ref().unwrap().id(),
            "backend/name",
            "unanswered dependency's question is namespaced by its template id"
        );

        // Convergence: supplying backend/name too walks both dependencies to completion.
        shared
            .shared_answers
            .insert("backend/name".to_string(), Answer::String("be".to_string()));
        let (_vfs, final_state2, _session_ids) = operator
            .execute_composition(&deps, &shared, true)
            .expect("both deps complete once both answers supplied");
        assert!(
            final_state2.need_input.is_none(),
            "all answers supplied → both deps complete (convergence)"
        );
    }

    // A GLOBAL answer whose raw id legitimately contains a `/` (an e2e-style id like
    // `cyane2e/template1/name`) must reach EVERY dependency in a multi-template
    // composition — NOT be silently dropped or misrouted. The Loop-1 `contains('/')`
    // discriminator classified such a key as "scoped" and, matching no dependency's
    // namespace, skipped it for all of them. Classifying against the KNOWN namespace set
    // routes it correctly as global. (Loop-1 review issues #1 and #5.)
    #[test]
    fn global_slash_id_routed_to_every_dependency() {
        let deps = vec![
            ResolvedDependency {
                template: make_template_version("frontend", "frontend", 1, vec![]),
                preset_answers: HashMap::new(),
            },
            ResolvedDependency {
                template: make_template_version("backend", "backend", 1, vec![]),
                preset_answers: HashMap::new(),
            },
        ];

        let seen: Arc<std::sync::Mutex<Vec<HashMap<String, Answer>>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        struct CaptureExecutor {
            seen: Arc<std::sync::Mutex<Vec<HashMap<String, Answer>>>>,
        }
        impl TemplateExecutor for CaptureExecutor {
            fn execute_template(
                &self,
                _template: &TemplateVersionRes,
                _session_id: &str,
                answers: Option<&HashMap<String, Answer>>,
                _deterministic_states: Option<&HashMap<String, String>>,
            ) -> Result<(Vec<u8>, TemplateState, String), Box<dyn std::error::Error + Send>>
            {
                let ans = answers.cloned().unwrap_or_default();
                self.seen.lock().unwrap().push(ans.clone());
                Ok((
                    empty_tar_gz(),
                    TemplateState::Complete(
                        Cyan {
                            processors: vec![],
                            plugins: vec![],
                        },
                        ans,
                    ),
                    "s".to_string(),
                ))
            }
        }
        let mut operator = operator_with_dyn_executor(Box::new(CaptureExecutor {
            seen: Arc::clone(&seen),
        }));

        // A scoped answer (frontend/name) PLUS a GLOBAL answer whose id contains a slash.
        let mut shared = CompositionState::new();
        shared.shared_answers.insert(
            "frontend/name".to_string(),
            Answer::String("fe".to_string()),
        );
        shared.shared_answers.insert(
            "cyane2e/template1/name".to_string(),
            Answer::String("glob".to_string()),
        );
        let (_vfs, final_state, _session_ids) = operator
            .execute_composition(&deps, &shared, true)
            .expect("composition must complete");

        assert!(
            final_state.need_input.is_none(),
            "both dependencies have their answers → complete"
        );

        let seen = seen.lock().unwrap();
        // frontend received its scoped `name` AND the slash-containing global verbatim.
        assert!(
            matches!(seen[0].get("name"), Some(Answer::String(s)) if s == "fe"),
            "frontend receives its scoped frontend/name → stripped to raw name"
        );
        assert!(
            matches!(seen[0].get("cyane2e/template1/name"), Some(Answer::String(s)) if s == "glob"),
            "frontend receives the slash-containing GLOBAL answer verbatim (not dropped)"
        );
        // backend received the global (reaches every dep) but NOT frontend's scoped answer.
        assert!(
            matches!(seen[1].get("cyane2e/template1/name"), Some(Answer::String(s)) if s == "glob"),
            "backend also receives the slash-containing global answer"
        );
        assert!(
            !seen[1].contains_key("name"),
            "backend must not receive frontend's scoped answer"
        );
    }

    // =========================================================================
    // Caller-vs-preset answer precedence (REAL pipeline, not inline sim)
    //
    // These replace the four removed inline-simulation tests. They drive the actual
    // execute_composition → executor boundary with a capturing fake executor and assert
    // the answer map the executor RECEIVES, so a precedence inversion in production code
    // (like the single-dependency namespacing regression) is caught instead of masked.
    // =========================================================================

    /// A fake executor that records the answer map it is handed for each dependency, then
    /// completes with an empty archive. Lets a test assert what answers the composition
    /// actually routed to the template.
    struct CapturingExecutor {
        seen: Arc<std::sync::Mutex<Vec<HashMap<String, Answer>>>>,
    }
    impl TemplateExecutor for CapturingExecutor {
        fn execute_template(
            &self,
            _template: &TemplateVersionRes,
            _session_id: &str,
            answers: Option<&HashMap<String, Answer>>,
            _deterministic_states: Option<&HashMap<String, String>>,
        ) -> Result<(Vec<u8>, TemplateState, String), Box<dyn std::error::Error + Send>> {
            self.seen
                .lock()
                .unwrap()
                .push(answers.cloned().unwrap_or_default());
            Ok((
                empty_tar_gz(),
                TemplateState::Complete(
                    Cyan {
                        processors: vec![],
                        plugins: vec![],
                    },
                    HashMap::new(),
                ),
                "s".to_string(),
            ))
        }
    }

    fn operator_with_boxed_executor(executor: Box<dyn TemplateExecutor>) -> CompositionOperator {
        let registry = Rc::new(CyanRegistryClient {
            endpoint: "http://unused.invalid".to_string(),
            version: "1.0".to_string(),
            client: Rc::new(reqwest::blocking::Client::new()),
        });
        let template_operator = TemplateOperator::new(
            Box::new(DefaultSessionIdGenerator),
            executor,
            Box::new(DefaultTemplateHistory::new()),
            Box::new(DefaultVfs::new(
                Box::new(TarGzUnpacker),
                Box::new(DiskFileLoader),
                Box::new(GitLikeMerger::new(false, 50)),
                Box::new(DiskFileWriter),
            )),
            registry,
        );
        CompositionOperator::new(
            template_operator,
            Box::new(MockDependencyResolver::new()),
            Box::new(DefaultVfsLayerer),
        )
    }

    // When the caller supplies an answer for the SAME id as a dependency preset,
    // the CALLER wins. (Single dependency → no namespace, so the raw id matches directly.)
    #[test]
    fn caller_answer_wins_over_preset_in_pipeline() {
        let seen: Arc<std::sync::Mutex<Vec<HashMap<String, Answer>>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut operator = operator_with_boxed_executor(Box::new(CapturingExecutor {
            seen: Arc::clone(&seen),
        }));

        let deps = vec![ResolvedDependency {
            template: make_template_version("root", "root", 1, vec![]),
            preset_answers: HashMap::from([(
                "db_host".to_string(),
                Answer::String("preset-host".to_string()),
            )]),
        }];
        let mut shared = CompositionState::new();
        shared.shared_answers.insert(
            "db_host".to_string(),
            Answer::String("user-host".to_string()),
        );

        operator
            .execute_composition(&deps, &shared, true)
            .expect("composition completes");

        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1, "one dependency executed");
        assert!(
            matches!(seen[0].get("db_host"), Some(Answer::String(s)) if s == "user-host"),
            "caller-supplied answer must WIN over the preset, got {:?}",
            seen[0].get("db_host")
        );
    }

    // A preset fills a gap only when the caller did NOT supply that id.
    #[test]
    fn preset_fills_gap_in_pipeline() {
        let seen: Arc<std::sync::Mutex<Vec<HashMap<String, Answer>>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut operator = operator_with_boxed_executor(Box::new(CapturingExecutor {
            seen: Arc::clone(&seen),
        }));

        let deps = vec![ResolvedDependency {
            template: make_template_version("root", "root", 1, vec![]),
            preset_answers: HashMap::from([(
                "db_host".to_string(),
                Answer::String("preset-host".to_string()),
            )]),
        }];
        let mut shared = CompositionState::new();
        shared
            .shared_answers
            .insert("app_name".to_string(), Answer::String("myapp".to_string()));

        operator
            .execute_composition(&deps, &shared, true)
            .expect("composition completes");

        let seen = seen.lock().unwrap();
        assert!(
            matches!(seen[0].get("db_host"), Some(Answer::String(s)) if s == "preset-host"),
            "preset fills the gap when the caller did not supply the id"
        );
        assert!(
            matches!(seen[0].get("app_name"), Some(Answer::String(s)) if s == "myapp"),
            "caller-supplied unrelated answer is preserved"
        );
    }

    // A dependency's preset does not leak to a sibling dependency.
    #[test]
    fn preset_answers_isolated_per_dependency_in_pipeline() {
        let seen: Arc<std::sync::Mutex<Vec<HashMap<String, Answer>>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut operator = operator_with_boxed_executor(Box::new(CapturingExecutor {
            seen: Arc::clone(&seen),
        }));

        // Dep A carries a preset; dep B carries none.
        let deps = vec![
            ResolvedDependency {
                template: make_template_version("dep_a", "dep_a", 1, vec![]),
                preset_answers: HashMap::from([(
                    "a_secret".to_string(),
                    Answer::String("s1".to_string()),
                )]),
            },
            ResolvedDependency {
                template: make_template_version("dep_b", "dep_b", 1, vec![]),
                preset_answers: HashMap::new(),
            },
        ];

        operator
            .execute_composition(&deps, &CompositionState::new(), true)
            .expect("composition completes");

        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "both dependencies executed");
        assert!(
            seen[0].contains_key("a_secret"),
            "dep A receives its own preset"
        );
        assert!(
            !seen[1].contains_key("a_secret"),
            "dep B must NOT receive dep A's preset (isolation)"
        );
    }
}
