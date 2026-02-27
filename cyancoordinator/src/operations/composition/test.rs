use std::path::PathBuf;

use super::layerer::{DefaultVfsLayerer, VfsLayerer};
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
}
