use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::models::cyan::Cyan;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanregistry::http::models::template_res::TemplateVersionRes;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use crate::cache::{Cache, CacheEntry};
use crate::client::CyanCoordinatorClient;
use crate::conflict_file_resolver::{
    ConflictFileResolverRegistry, FileConflictEntry, ResolverInstance, TemplateInfo,
};
use crate::fs::VirtualFileSystem;
use crate::operations::TemplateOperator;

use super::layerer::{DefaultVfsLayerer, ResolverAwareLayerer, VfsLayerer};
use super::resolver::{DependencyResolver, ResolvedDependency};
use super::state::CompositionState;

/// Composition operator for recursive template execution
pub struct CompositionOperator {
    template_operator: TemplateOperator,
    dependency_resolver: Box<dyn DependencyResolver>,
    vfs_layerer: Box<dyn VfsLayerer>,
    /// Optional client for resolver-aware layering
    client: Option<CyanCoordinatorClient>,
    /// File conflicts tracked during the last composition
    file_conflicts: Vec<FileConflictEntry>,
    /// Per-node output cache. Disabled by default; injected via [`Self::set_cache`].
    cache: Cache,
}

/// Owns the coordinator sessions a composition accumulates and releases them on any
/// early return (error or panic) while armed — closing the recurring composition-internal
/// session-leak class. The success paths call [`disarm`](Self::disarm) to hand the
/// accumulated ids to the caller (which then owns their release). When there is no client
/// (e.g. the test-only `CompositionOperator::new`), the guard still tracks the ids but
/// cannot release them — in that case there is no live coordinator session to leak.
///
/// The client is held by value (an `Arc`-sharing `Clone`), not by borrow, so the guard
/// does not borrow `&mut self` for its lifetime and a `Drop` release can run even though
/// `execute_composition` holds `&mut self` — matching the owned-descriptor RAII pattern
/// used by `SetupArtifactsGuard` in cyanprint.
struct CompositionSessionGuard {
    client: Option<CyanCoordinatorClient>,
    sessions: Vec<String>,
}

impl CompositionSessionGuard {
    fn new(client: Option<CyanCoordinatorClient>) -> Self {
        Self {
            client,
            sessions: Vec::new(),
        }
    }

    /// Record a coordinator session id allocated during a dependency's warm/bootstrap.
    fn push(&mut self, session_id: String) {
        self.sessions.push(session_id);
    }

    /// Hand the accumulated session ids to the caller on a legitimate terminal state
    /// (success or need_input). The guard no longer releases them on drop.
    fn disarm(mut self) -> Vec<String> {
        std::mem::take(&mut self.sessions)
    }
}

impl Drop for CompositionSessionGuard {
    fn drop(&mut self) {
        if let Some(client) = &self.client {
            for sid in &self.sessions {
                // Best-effort: a release hiccup must not mask the original error that
                // triggered the early return. Matches the closure-based teardown in
                // cyanprint (e.g. `cleanup_group_sessions`).
                let _ = client.try_cleanup(sid);
            }
        }
    }
}

impl CompositionOperator {
    pub fn new(
        template_operator: TemplateOperator,
        dependency_resolver: Box<dyn DependencyResolver>,
        vfs_layerer: Box<dyn VfsLayerer>,
    ) -> Self {
        Self {
            template_operator,
            dependency_resolver,
            vfs_layerer,
            client: None,
            file_conflicts: Vec::new(),
            cache: Cache::disabled(),
        }
    }

    /// Create a composition operator with resolver-aware layering
    pub fn with_client(
        template_operator: TemplateOperator,
        dependency_resolver: Box<dyn DependencyResolver>,
        client: CyanCoordinatorClient,
    ) -> Self {
        Self {
            template_operator,
            dependency_resolver,
            vfs_layerer: Box::new(DefaultVfsLayerer),
            client: Some(client),
            file_conflicts: Vec::new(),
            cache: Cache::disabled(),
        }
    }

    /// Inject the per-node output cache. Defaults to a disabled cache, so callers
    /// that want caching (create / update) call this with a resolved [`Cache`].
    pub fn set_cache(&mut self, cache: Cache) {
        self.cache = cache;
    }

    /// Build a resolver registry from template response data
    fn build_resolver_registry(
        dependencies: &[ResolvedDependency],
    ) -> ConflictFileResolverRegistry {
        let mut registry = ConflictFileResolverRegistry::new();

        for dep in dependencies {
            let template = &dep.template;
            let template_id = template.principal.id.clone();
            let resolvers: Vec<ResolverInstance> = template
                .resolvers
                .iter()
                .map(|r| ResolverInstance {
                    id: r.id.clone(),
                    docker_ref: r.docker_reference.clone(),
                    docker_tag: r.docker_tag.clone(),
                    config: r.config.clone(),
                    file_patterns: r.files.clone(),
                })
                .collect();

            if !resolvers.is_empty() {
                registry.register(template_id, resolvers);
            }
        }

        registry
    }

    /// Build template info list for layerer from template response data
    fn build_template_infos(dependencies: &[ResolvedDependency]) -> Vec<TemplateInfo> {
        dependencies
            .iter()
            .enumerate()
            .map(|(idx, dep)| TemplateInfo {
                template_id: dep.template.principal.id.clone(),
                template_version: dep.template.principal.version,
                layer: idx as i32,
            })
            .collect()
    }

    /// Get file conflicts from the last composition
    pub fn get_file_conflicts(&self) -> &[FileConflictEntry] {
        &self.file_conflicts
    }

    /// Cache hit/total summary across this operator's lifetime, or `None` when the
    /// cache is disabled. Backs [`Self::print_cache_summary`] (the only external
    /// caller) and the crate's composition tests; not part of the public API. (FR15)
    pub(crate) fn cache_summary(&self) -> Option<(usize, usize)> {
        if self.cache.enabled() {
            Some((self.cache.hits(), self.cache.total()))
        } else {
            None
        }
    }

    /// Print the one-line `N/M nodes served from cache` summary when caching is
    /// enabled. Shared by every command path that drives an operator (create,
    /// update, try group). (FR15, L6)
    pub fn print_cache_summary(&self) {
        if let Some((hits, total)) = self.cache_summary() {
            println!("♻️  Cache: {hits}/{total} nodes served from cache");
        }
    }

    /// Execute a composition of templates (recursive dependencies)
    pub(crate) fn execute_composition(
        &mut self,
        dependencies: &[ResolvedDependency],
        initial_shared_state: &CompositionState,
        headless: bool,
    ) -> Result<(VirtualFileSystem, CompositionState, Vec<String>), Box<dyn Error + Send>> {
        let mut shared_state = initial_shared_state.clone();
        let mut vfs_outputs = Vec::new();
        // Coordinator sessions are allocated during each dependency's warm/bootstrap
        // (recorded as `actual_session_id`) and are normally returned to the caller for
        // release. But several fallible steps run AFTER a session is recorded and BEFORE
        // this vector reaches the caller — `update_from_template_state` (the need_input
        // branch and the completed branch), `unpack_archive`, and resolver/default VFS
        // layering. A `?` from any of them discards the locally accumulated sessions,
        // leaking the Boron sessions until server-side expiry. The guard releases every
        // accumulated session on ANY early return while armed; the success paths
        // `disarm()` it and move the ids out. This is the RAII fix for the recurring
        // composition-internal session-leak class — one mechanism covers every current
        // and future `?` between acquisition and return, instead of per-path patching.
        let mut sessions = CompositionSessionGuard::new(self.client.clone());

        // Clear previous conflicts
        self.file_conflicts.clear();

        // A composition of more than one dependency can contain two templates
        // asking a question with the same raw id; namespace their ids by template id to
        // avoid collisions in the shared answer map. A single dependency has no such
        // risk, so its ids stay raw (existing create/update behavior unchanged).
        let multi_template = dependencies.len() > 1;
        // Every dependency's template id is a potential `{ns}/…` namespace prefix a
        // caller-supplied answer can be scoped under. Classifying a shared-answer key
        // against this KNOWN set (rather than by the brittle "contains a `/`" shape) is
        // what lets a raw question id that legitimately contains `/` (e.g. an e2e fixture
        // id like `cyane2e/template1/name`) be routed as the GLOBAL answer it is, instead
        // of being silently dropped or misrouted. Every namespace id is guaranteed `/`-free
        // by the guard below, so a `{ns}/` prefix matches at most one namespace.
        let namespaces: Vec<String> = dependencies
            .iter()
            .map(|d| d.template.principal.id.clone())
            .collect();

        for dep in dependencies {
            let template = &dep.template;

            // Check if template has execution artifacts (properties field)
            if template.principal.properties.is_none() {
                cprogress!(
                    headless,
                    "⏭️ Skipping template: {}/{} (v{}) - no execution artifacts (group template)",
                    template.template.name,
                    template.template.name, // TODO: Need username
                    template.principal.version
                );
                // Update execution order tracking even for skipped templates
                shared_state
                    .execution_order
                    .push(template.principal.id.clone());
                continue;
            }

            // Generate session for this template
            let session_id = self.template_operator.session_id_generator.generate();

            let template_id = template.principal.id.clone();
            // When more than one template composes, two dependencies can ask a
            // question with the SAME raw id (e.g. `name`); routing them through one
            // flat `shared_answers` map would collide. Namespace per dependency by its
            // template id: caller-supplied answers live under `{template_id}/{raw_id}`,
            // so they are scoped to the dependency that owns them. For a single
            // dependency (the common create/update case) there is no collision risk, so
            // ids stay raw and existing behavior is preserved exactly.
            //
            // The `{template_id}/{raw_id}` key + first-`/` `strip_namespace` is only
            // unambiguous when the template id itself has no `/`. A template id with a
            // slash would let two distinct (template, raw-id) pairs map to one key, so
            // fail fast rather than route an answer to the wrong dependency. Template ids
            // are coordinator-assigned and non-slash in practice; this is the guard.
            if multi_template && template_id.contains('/') {
                return Err(Box::new(std::io::Error::other(format!(
                    "cannot namespace a multi-template composition: template id '{template_id}' \
                     contains '/' which is the namespace separator"
                ))) as Box<dyn Error + Send>);
            }
            let namespace = multi_template.then_some(template_id.as_str());

            // Build this dependency's answer map. Caller-supplied answers MUST win
            // over presets, so seed the map from the caller's answers FIRST, then let
            // presets fill only the gaps via `or_insert`. Seeding from presets first and
            // letting the caller `or_insert` would invert precedence — presets would
            // silently win, which also changes non-headless group behavior and breaks the
            // interactive-unchanged guarantee.
            //
            // Answer routing in a multi-template (namespaced) composition has three
            // classes, decided by classifying the key against the KNOWN dependency
            // namespaces — NOT by whether the key merely contains a `/`. A raw question id
            // may legitimately contain `/` (e.g. an e2e fixture id `cyane2e/template1/name`),
            // so the `/`-shape test used previously would have classified such a GLOBAL
            // answer as "scoped" and, matching no dependency's namespace, dropped it from
            // every dependency. Instead:
            // - A key belonging to NO namespace (`classify_namespace` → `None`) is a GLOBAL
            //   answer: it applies to every dependency verbatim. This preserves the
            //   pre-namespacing behavior (each dep received the full flat `shared_answers`
            //   map), so a shared answer still reaches all sub-templates and a node's cache
            //   key is unchanged whether it runs standalone or inside a composition.
            // - A key scoped to THIS dependency (`{template_id}/{raw}`) is stripped to its
            //   raw id and OVERRIDES the global of the same raw id, so a caller can target
            //   one dependency's question without colliding with a sibling's same-named one.
            // - A key scoped to a SIBLING dependency is skipped entirely for this dep.
            // In a single-template composition the flat shared map applies directly.
            let mut template_answers: HashMap<String, Answer> = HashMap::new();
            match namespace {
                Some(ns) => {
                    // Global answers first (belong to no namespace), so a dependency-scoped
                    // answer below can override a global of the same raw id.
                    for (key, answer) in &shared_state.shared_answers {
                        if classify_namespace(key, &namespaces).is_none() {
                            template_answers.insert(key.clone(), answer.clone());
                        }
                    }
                    // This dependency's scoped answers override the globals (raw id restored).
                    for (key, answer) in &shared_state.shared_answers {
                        if classify_namespace(key, &namespaces) == Some(ns) {
                            let raw_key = strip_namespace(ns, key)
                                .expect("key classified under this namespace");
                            template_answers.insert(raw_key, answer.clone());
                        }
                    }
                }
                None => {
                    for (key, answer) in &shared_state.shared_answers {
                        template_answers.insert(key.clone(), answer.clone());
                    }
                }
            }
            // Preset answers (raw ids declared by the parent) fill gaps ONLY — a
            // caller-supplied answer for the same id, inserted above, is left untouched.
            for (key, answer) in &dep.preset_answers {
                template_answers
                    .entry(key.clone())
                    .or_insert_with(|| answer.clone());
            }

            // Compute the content-addressed key only when the node can be cached,
            // so a disabled / non-cacheable run pays nothing for keying. (FR5, L5)
            let cache_key = if self.cache.enabled() && crate::cache::is_cacheable(template) {
                Some(crate::cache::compute_key(
                    template,
                    &template_answers,
                    &shared_state.shared_deterministic_states,
                ))
            } else {
                None
            };

            // CACHE HIT: skip Docker, replay the cached output + downstream state.
            // (FR1, FR2, FR4, FR12) — all cache touchpoints use match/if let, never
            // `?`, so a cache fault degrades to execution rather than failing.
            let mut served_from_cache = false;
            if let Some(key) = cache_key.as_ref() {
                if let Some(entry) = self.cache.lookup(template, key) {
                    if self.cache.debug() {
                        println!(
                            "  ♻️  cache HIT for {} (key {}…)",
                            template.template.name,
                            &key[..key.len().min(12)]
                        );
                    }
                    // Unpack the cached archive. If it fails to unpack (the framing
                    // checksummed OK, but the inner archive is bad) we do NOT abort:
                    // treat it as a miss, evict the poisoned entry so it self-heals,
                    // and fall through to live execution. (FR8, C1)
                    match self.template_operator.vfs.unpack_archive(entry.archive) {
                        Ok(vfs) => {
                            vfs_outputs.push(vfs);

                            // Replay the contributed downstream state: feed the
                            // cached answers through the same merge path so later
                            // nodes' template_answers and the saved
                            // .cyan_state.yaml are identical to a fresh run. (FR12)
                            // The cache stores raw (un-namespaced) Complete answers,
                            // so route the replay through `namespace_template_state`
                            // exactly like the live completed path below — keeping a
                            // cache hit and a fresh run byte-identical in a namespaced
                            // multi-template composition too.
                            let replay_state = TemplateState::Complete(
                                Cyan {
                                    processors: Vec::new(),
                                    plugins: Vec::new(),
                                },
                                entry.state,
                            );
                            let namespaced = namespace_template_state(
                                &replay_state,
                                namespace,
                                &initial_shared_state.shared_answers,
                            );
                            shared_state.update_from_template_state(
                                &namespaced,
                                template.principal.id.clone(),
                            )?;
                            served_from_cache = true;
                            // The entry was genuinely served: count it now, AFTER
                            // the archive unpacked and state replayed, so a
                            // poisoned entry that fell back to execution is not
                            // reported as a served hit. (FR15, H2)
                            self.cache.record_hit();
                        }
                        Err(e) => {
                            tracing::debug!(
                                "cache hit for {} unpacked invalid, evicting and re-executing: {e}",
                                template.principal.id
                            );
                            self.cache.evict(key);
                            // Do NOT record a hit: `lookup` already counted this
                            // node toward the *attempt* total, but it was NOT
                            // served from cache. Fall through to live execution.
                        }
                    }
                } else if self.cache.debug() {
                    println!(
                        "  🔍 cache MISS for {} (key {}…)",
                        template.template.name,
                        &key[..key.len().min(12)]
                    );
                }
            }

            if !served_from_cache {
                // CACHE MISS: execute as today. (FR3) The "Executing" line lives
                // here (not before the cache check) so a cache HIT is never
                // reported as an execution; a non-cached run still emits it for
                // every node. Routed through `cprogress!` so under headless it is
                // suppressed (the single-JSON-on-stdout contract); interactive is
                // unchanged.
                cprogress!(
                    headless,
                    "🚀 Executing template: {}/{} (v{})",
                    template.template.name,
                    template.template.name, // TODO: Need username
                    template.principal.version
                );

                let (archive_data, template_state, actual_session_id) =
                    self.template_operator.template_executor.execute_template(
                        template,
                        &session_id,
                        Some(&template_answers),
                        Some(&shared_state.shared_deterministic_states),
                    )?;

                // Track session for cleanup regardless of outcome (the session was
                // created during warm/bootstrap even when Q&A stops early).
                if !actual_session_id.is_empty() {
                    sessions.push(actual_session_id);
                }

                // Headless: a template reached an unanswered question (NeedInput). The
                // executor returns an EMPTY archive in this case — it must NOT be
                // unpacked (an empty byte stream is an invalid gzip stream, so
                // `unpack_archive` would error). Detect the terminal NeedInput state
                // directly from `template_state` BEFORE unpacking, record the question
                // in shared state (namespaced in a multi-template composition), and
                // short-circuit: stop executing remaining templates and skip layering
                // (the partial VFS outputs are discarded by the caller, which surfaces
                // the question and stops before any files are written). A NeedInput is
                // never cached — only a Complete result is stored below — so there is no
                // cache interaction on this path.
                let need_input = matches!(
                    template_state,
                    cyanprompt::domain::services::template::states::TemplateState::NeedInput(_, _)
                );
                if need_input {
                    let namespaced = namespace_template_state(
                        &template_state,
                        namespace,
                        &initial_shared_state.shared_answers,
                    );
                    shared_state
                        .update_from_template_state(&namespaced, template.principal.id.clone())?;
                    // need_input is a legitimate terminal state, not an error: the caller
                    // owns the session release from here, so disarm the guard and hand the
                    // accumulated ids back. (`update_from_template_state` above can still
                    // `?` — in that case the guard stays armed and releases the sessions.)
                    let all_session_ids = sessions.disarm();
                    return Ok((VirtualFileSystem::new(), shared_state, all_session_ids));
                }

                // Validate the archive can actually unpack BEFORE caching it, so a
                // bad (Ok((invalid_bytes, Complete(..)))) result is never persisted.
                // (FR13, C1) The to-store copy is taken only when this node will be
                // cached, so a disabled / non-cacheable run never clones the full
                // archive bytes. (L16) `unpack_archive` consumes the bytes by value,
                // so the clone must precede the move.
                let to_store = cache_key.as_ref().map(|_| archive_data.clone());
                let vfs = self.template_operator.vfs.unpack_archive(archive_data)?;
                vfs_outputs.push(vfs);

                // Store ONLY a non-interactive Complete result, and only after the
                // archive has unpacked cleanly. The raw (un-namespaced) answers are
                // stored so a later replay can re-namespace per consuming dependency.
                // QnA panics upstream, Err short-circuits via `?` above, and a NeedInput
                // already returned, so failures/partials are never cached. (FR13)
                if let (Some(key), TemplateState::Complete(_, ref answers)) =
                    (cache_key.as_ref(), &template_state)
                {
                    let entry = CacheEntry {
                        archive: to_store.expect("to_store is Some iff cache_key is Some"),
                        state: answers.clone(),
                    };
                    self.cache.store(template, key, &entry);
                }

                // Update shared state with results (caller-targeted answers namespaced
                // in a multi-template composition so they do not collide with another
                // dependency's; global/derived answers stay raw and propagate). The
                // "was this caller-targeted?" decision uses the immutable initial caller
                // input, not the evolving accumulator — see `namespace_template_state`.
                let namespaced = namespace_template_state(
                    &template_state,
                    namespace,
                    &initial_shared_state.shared_answers,
                );
                shared_state
                    .update_from_template_state(&namespaced, template.principal.id.clone())?;
            }
        }

        // Layer all VFS outputs
        let layered_vfs = if vfs_outputs.is_empty() {
            // No templates produced output (all were group templates)
            cprogress!(
                headless,
                "ℹ️ No execution artifacts produced - all templates were group templates"
            );
            VirtualFileSystem::new()
        } else if let Some(ref client) = self.client {
            // Use resolver-aware layering
            // Vertical layering: collect resolvers from ALL templates in dependency tree
            let registry = Self::build_resolver_registry(dependencies);
            let template_infos = Self::build_template_infos(dependencies);
            let layerer =
                ResolverAwareLayerer::new(registry, template_infos, client.clone(), headless);

            let result = layerer.layer_merge(&vfs_outputs)?;

            // Track conflicts for state writing
            self.file_conflicts = layerer.get_conflicts();

            result
        } else {
            // Use default layering (LWW)
            self.vfs_layerer.layer_merge(&vfs_outputs)?
        };

        Ok((layered_vfs, shared_state, sessions.disarm()))
    }

    /// Get a reference to the VFS operations
    pub fn get_vfs(&self) -> &dyn crate::fs::Vfs {
        self.template_operator.vfs.as_ref()
    }

    /// Get a reference to the template history
    pub fn get_template_history(&self) -> &dyn crate::template::TemplateHistory {
        self.template_operator.template_history.as_ref()
    }

    // =========================================================================
    // Unified Batch Processing Methods (v2/v3 spec)
    // =========================================================================

    /// Execute a single template spec and return VFS + final state + session IDs + commands.
    /// This is the core primitive - pure function, no side effects.
    /// Dependencies are resolved in post-order and layered internally.
    /// Returns the final CompositionState which contains answers after Q&A,
    /// and commands collected from all resolved dependencies in post-order.
    #[allow(clippy::type_complexity)]
    pub fn execute_template(
        &mut self,
        template: &cyanregistry::http::models::template_res::TemplateVersionRes,
        answers: &HashMap<String, Answer>,
        deterministic_states: &HashMap<String, String>,
        headless: bool,
    ) -> Result<
        (
            VirtualFileSystem,
            CompositionState,
            Vec<String>,
            Vec<String>,
        ),
        Box<dyn Error + Send>,
    > {
        let dependencies = self.dependency_resolver.resolve_dependencies(template)?;

        let shared_state = CompositionState {
            shared_answers: answers.clone(),
            shared_deterministic_states: deterministic_states.clone(),
            execution_order: Vec::new(),
            need_input: None,
        };

        let (vfs, final_state, session_ids) =
            self.execute_composition(&dependencies, &shared_state, headless)?;
        let commands = Self::collect_commands(&dependencies);
        Ok((vfs, final_state, session_ids, commands))
    }

    /// Layer merge a list of VFS into one (LWW semantics).
    pub fn layer_merge(
        &self,
        vfs_list: &[VirtualFileSystem],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.vfs_layerer.layer_merge(vfs_list)
    }

    /// Horizontal layering with resolver support.
    /// Collects resolvers ONLY from root templates being merged (not from dependencies).
    /// This is used when merging multiple independent templates in batch processing.
    pub fn layer_merge_with_resolvers(
        &mut self,
        vfs_list: &[VirtualFileSystem],
        root_templates: &[cyanregistry::http::models::template_res::TemplateVersionRes],
        client: &CyanCoordinatorClient,
        headless: bool,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        if vfs_list.is_empty() {
            return Ok(VirtualFileSystem::new());
        }

        if vfs_list.len() == 1 {
            return Ok(vfs_list[0].clone());
        }

        // Convert root templates to ResolvedDependency for helper functions
        // (preset_answers are not applicable for horizontal layering)
        let root_dependencies: Vec<ResolvedDependency> = root_templates
            .iter()
            .map(|t| ResolvedDependency {
                template: t.clone(),
                preset_answers: HashMap::new(),
            })
            .collect();

        // Build resolver registry from ONLY root templates (horizontal layering scope)
        let registry = Self::build_resolver_registry(&root_dependencies);
        let template_infos = Self::build_template_infos(&root_dependencies);

        // Use resolver-aware layerer
        let layerer = ResolverAwareLayerer::new(registry, template_infos, client.clone(), headless);
        let result = layerer.layer_merge(vfs_list)?;

        // Track conflicts for state writing
        self.file_conflicts = layerer.get_conflicts();

        Ok(result)
    }

    /// 3-way merge: (base, local, incoming) -> merged.
    pub fn merge(
        &self,
        base: &VirtualFileSystem,
        local: &VirtualFileSystem,
        incoming: &VirtualFileSystem,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.template_operator.vfs.merge(base, local, incoming)
    }

    /// Load local files from target directory.
    pub fn load_local_files(
        &self,
        target_dir: &Path,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.template_operator.vfs.load_local_files(target_dir, &[])
    }

    /// Write VFS to disk.
    pub fn write_to_disk(
        &self,
        target_dir: &Path,
        vfs: &VirtualFileSystem,
    ) -> Result<(), Box<dyn Error + Send>> {
        self.template_operator.vfs.write_to_disk(target_dir, vfs)
    }

    /// Delete files that were present before merge but absent after merge.
    pub fn cleanup_deleted_files(
        &self,
        target_dir: &Path,
        local_vfs: &VirtualFileSystem,
        merged_vfs: &VirtualFileSystem,
    ) -> Result<Vec<std::path::PathBuf>, Box<dyn Error + Send>> {
        self.template_operator
            .vfs
            .cleanup_deleted_files(target_dir, local_vfs, merged_vfs)
    }

    // =========================================================================
    // Command Collection (for post-composition execution)
    // =========================================================================

    /// Collect commands from resolved dependencies in post-order.
    /// Iterates over dependencies (already in post-order from resolve_dependencies),
    /// collects non-empty commands from each template, and flattens into a single vec.
    pub fn collect_commands(dependencies: &[ResolvedDependency]) -> Vec<String> {
        let mut commands = Vec::new();
        for dep in dependencies {
            let template_commands = &dep.template.commands;
            if !template_commands.is_empty() {
                commands.extend(template_commands.iter().cloned());
            }
        }
        commands
    }

    /// Collect commands from template version responses.
    /// Used by batch_process where commands are collected from raw template results.
    pub fn collect_commands_from_templates(templates: &[TemplateVersionRes]) -> Vec<String> {
        let mut commands = Vec::new();
        for template in templates {
            if !template.commands.is_empty() {
                commands.extend(template.commands.iter().cloned());
            }
        }
        commands
    }
}

// ===========================================================================
// Per-template question-id namespacing
// ===========================================================================

/// If `namespaced_key` is of the form `{namespace}/{rest}`, return `rest`; otherwise
/// `None` (the key does not belong to this namespace). Used to route a caller-supplied
/// namespaced answer back to its dependency's raw question id.
fn strip_namespace(namespace: &str, namespaced_key: &str) -> Option<String> {
    let prefix = format!("{namespace}/");
    namespaced_key
        .strip_prefix(&prefix)
        .map(|rest| rest.to_string())
}

/// If `key` is scoped under one of the known dependency namespaces — i.e. it begins with
/// `{ns}/` for some `ns` in `namespaces` — return that namespace; otherwise return `None`
/// (the key is GLOBAL: it belongs to no dependency).
///
/// This is the unambiguous replacement for the old "does the key contain a `/`?" routing
/// test. A raw question id may itself legitimately contain `/` (e.g. an e2e fixture id
/// `cyane2e/template1/name`); such a key matches no `{ns}/` prefix here, so it is correctly
/// classified as global and routed to every dependency instead of being silently dropped.
/// The match is unambiguous because every namespace id is guaranteed `/`-free by the guard
/// in `execute_composition` (a `{ns}/` prefix can match at most one `/`-free `ns`).
fn classify_namespace<'a>(key: &str, namespaces: &'a [String]) -> Option<&'a str> {
    namespaces
        .iter()
        .find_map(|ns| strip_namespace(ns, key).is_some().then_some(ns.as_str()))
}

/// Return a copy of `state` whose question ids and (caller-targeted) answer keys are
/// prefixed with `{namespace}/` when a namespace is supplied, so each dependency's
/// questions/answers are scoped and cannot collide with another dependency's. When
/// `namespace` is `None` (single-template composition) the state is returned with raw
/// ids unchanged.
///
/// Two terminal states are rewritten:
/// - `NeedInput`: the unanswered `Question`'s id is ALWAYS namespaced as `{namespace}/id`,
///   so the surfaced need_input envelope asks for `{namespace}/id` and the caller knows
///   which dependency to answer.
/// - `Complete`: an answer key `k` is namespaced to `{namespace}/k` ONLY when the caller
///   actually targeted it via a `{namespace}/k` entry in `supplied_answers`. Global
///   (un-namespaced) caller answers and answers DERIVED by the dependency (not supplied
///   by the caller) stay raw, so they remain shared across sibling dependencies — this
///   preserves the pre-namespacing flat-shared-state behavior (and the cache feature's
///   cross-dependency derived-answer propagation) while still isolating answers the
///   caller deliberately scoped to one dependency.
///
/// `supplied_answers` MUST be the immutable caller-supplied input snapshot (the
/// `initial_shared_state.shared_answers` captured before the dependency loop), NOT the
/// live `shared_state.shared_answers` accumulator. The accumulator also holds answers
/// DERIVED by earlier dependencies, so using it here would let an earlier dependency's
/// derived key shaped like `{later_ns}/{raw}` spuriously re-key a later dependency's raw
/// answer as caller-scoped. Seeding downstream templates still uses the accumulator (so
/// derived answers propagate); only this "was it caller-targeted?" decision needs the
/// original input.
fn namespace_template_state(
    state: &cyanprompt::domain::services::template::states::TemplateState,
    namespace: Option<&str>,
    supplied_answers: &HashMap<String, Answer>,
) -> cyanprompt::domain::services::template::states::TemplateState {
    use cyanprompt::domain::services::template::states::TemplateState;

    let Some(ns) = namespace else {
        return state.clone();
    };

    match state {
        TemplateState::NeedInput(question, det) => {
            TemplateState::NeedInput(rename_question(question, ns), det.clone())
        }
        TemplateState::Complete(cyan, answers) => {
            let namespaced: HashMap<String, Answer> = answers
                .iter()
                .map(|(k, v)| {
                    let scoped = format!("{ns}/{k}");
                    if supplied_answers.contains_key(&scoped) {
                        // Caller targeted this dependency's answer explicitly — keep it
                        // scoped so it cannot collide with a sibling's same-named answer.
                        (scoped, v.clone())
                    } else {
                        // Global or derived answer — stays raw so it propagates to
                        // sibling dependencies (flat-shared-state / cache replay).
                        (k.clone(), v.clone())
                    }
                })
                .collect();
            TemplateState::Complete(cyan.clone(), namespaced)
        }
        // QnA is never terminal; Err carries no ids. Clone unchanged.
        other => other.clone(),
    }
}

/// Clone `question` with its `id` prefixed by `{namespace}/`. `Question` is an enum over
/// per-kind structs each carrying a `String` id, so each variant's id is rewritten.
fn rename_question(
    question: &cyanprompt::domain::models::question::Question,
    namespace: &str,
) -> cyanprompt::domain::models::question::Question {
    use cyanprompt::domain::models::question::{
        CheckboxQuestion, ConfirmQuestion, DateQuestion, PasswordQuestion, Question,
        SelectQuestion, TextQuestion,
    };

    let prefixed = |id: &str| format!("{namespace}/{id}");
    match question {
        Question::Confirm(q) => Question::Confirm(ConfirmQuestion {
            id: prefixed(&q.id),
            ..q.clone()
        }),
        Question::Date(q) => Question::Date(DateQuestion {
            id: prefixed(&q.id),
            ..q.clone()
        }),
        Question::Checkbox(q) => Question::Checkbox(CheckboxQuestion {
            id: prefixed(&q.id),
            ..q.clone()
        }),
        Question::Password(q) => Question::Password(PasswordQuestion {
            id: prefixed(&q.id),
            ..q.clone()
        }),
        Question::Text(q) => Question::Text(TextQuestion {
            id: prefixed(&q.id),
            ..q.clone()
        }),
        Question::Select(q) => Question::Select(SelectQuestion {
            id: prefixed(&q.id),
            ..q.clone()
        }),
    }
}

#[cfg(test)]
mod session_guard_tests {
    use super::*;

    /// `disarm` returns every recorded session id, in order, to the caller — the
    /// legitimate-terminal-state path. This is what the success and need_input arms rely
    /// on to hand the ids back so the CLI layer (the caller) owns their release.
    #[test]
    fn disarm_returns_recorded_sessions_in_order() {
        let mut guard = CompositionSessionGuard::new(None);
        guard.push("s1".to_string());
        guard.push("s2".to_string());
        let ids = guard.disarm();
        assert_eq!(ids, vec!["s1".to_string(), "s2".to_string()]);
    }

    /// A guard that never had a session pushed disarms to an empty vec — no spurious
    /// release and no surprise for the caller (e.g. a composition of only group
    /// templates, which produce no sessions).
    #[test]
    fn disarm_on_empty_guard_is_empty() {
        let guard = CompositionSessionGuard::new(None);
        assert!(guard.disarm().is_empty());
    }

    /// With no client the guard cannot release sessions (there is no live coordinator to
    /// leak against — the test-only `CompositionOperator::new` path). `disarm` still
    /// returns the recorded ids so the bookkeeping contract holds regardless of whether
    /// a release path is wired.
    #[test]
    fn guard_without_client_still_tracks_sessions() {
        let mut guard = CompositionSessionGuard::new(None);
        guard.push("orphan".to_string());
        assert_eq!(guard.disarm(), vec!["orphan".to_string()]);
    }
}
