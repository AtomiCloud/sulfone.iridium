//! Per-node template-execution output cache (content-addressed).
//!
//! [`Cache`] is the facade the composition operator consults: it owns the
//! resolved [`CacheConfig`] and a hit/miss counter for the end-of-run summary,
//! and no-ops entirely when disabled (FR6, FR15). Keying lives in [`key`] and
//! the on-disk store in [`store`].

pub mod key;
pub mod store;

use std::cell::Cell;
use std::path::PathBuf;

use cyanregistry::http::models::template_res::TemplateVersionRes;

pub use key::compute_key;
pub use store::{CacheEntry, CacheStore};

/// Environment variable that forces caching off when set to a truthy value.
pub const ENV_NO_CACHE: &str = "CYANPRINT_NO_CACHE";
/// Environment variable that overrides the cache directory.
pub const ENV_CACHE_DIR: &str = "CYANPRINT_CACHE";

/// Resolved cache configuration.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub enabled: bool,
    pub dir: PathBuf,
    /// When true, per-node HIT/MISS lines are logged (gated on the CLI `--debug`).
    pub debug: bool,
}

impl CacheConfig {
    /// Resolve the effective configuration from CLI overrides + environment.
    ///
    /// Precedence for *enabled*: `--no-output-cache` flag OR `CYANPRINT_NO_CACHE` (truthy)
    /// disables the cache (FR6). Precedence for *dir*: delegated to
    /// [`resolve_cache_dir`] — `--cache-dir` flag → `CYANPRINT_CACHE` env → the
    /// OS-standard cache dir (via the `directories` crate) joined with `cyanprint`
    /// (FR7).
    pub fn resolve(no_cache_flag: bool, cache_dir_flag: Option<PathBuf>, debug: bool) -> Self {
        let env_no_cache = std::env::var(ENV_NO_CACHE)
            .ok()
            .map(|v| is_truthy(&v))
            .unwrap_or(false);
        let enabled = !(no_cache_flag || env_no_cache);
        let dir = resolve_cache_dir(cache_dir_flag);
        Self {
            enabled,
            dir,
            debug,
        }
    }

    /// A disabled config (used by paths that must never cache, e.g. `test`).
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            dir: resolve_cache_dir(None),
            debug: false,
        }
    }
}

/// Resolve only the cache directory (used by the `cyanprint cache` command,
/// which needs the dir regardless of whether caching is enabled). (FR7)
///
/// Precedence: `--cache-dir` flag → `CYANPRINT_CACHE` env → the platform cache
/// dir (from the `directories` crate) joined with `cyanprint`. The `directories`
/// crate resolves the OS-standard cache location: `$XDG_CACHE_HOME` or
/// `~/.cache` on Linux, `~/Library/Caches` on macOS, `%LOCALAPPDATA%` on
/// Windows. If it cannot determine a home directory, fall back to a relative
/// `.cache/cyanprint`.
pub fn resolve_cache_dir(cache_dir_flag: Option<PathBuf>) -> PathBuf {
    if let Some(dir) = cache_dir_flag {
        return dir;
    }
    if let Some(dir) = std::env::var_os(ENV_CACHE_DIR) {
        return PathBuf::from(dir);
    }
    if let Some(base_dirs) = directories::BaseDirs::new() {
        return base_dirs.cache_dir().join("cyanprint");
    }
    // Unlikely fallback when no home directory can be determined.
    PathBuf::from(".cache").join("cyanprint")
}

fn is_truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Whether a node may be cached. Local/dev/synthetic templates are never cached
/// so a changed local source is never served stale. (FR14)
///
/// A node is cacheable only when it has a published immutable version: real
/// Docker properties present, a positive version, and a non-synthetic id (not
/// the `10ca1…` / `local-…` ids produced by `build_synthetic_template`).
pub fn is_cacheable(template: &TemplateVersionRes) -> bool {
    if template.principal.properties.is_none() {
        return false; // dev mode / group template — no execution artifacts
    }
    if template.principal.version <= 0 {
        return false; // unpublished / synthetic
    }
    let id = &template.principal.id;
    if id.starts_with("10ca1") || id.starts_with("local-") {
        // Synthetic local template ids produced by `build_synthetic_template`
        // (try_cmd.rs). `10ca1` is the synthetic-id prefix; `local-` the dev
        // prefix. A real published template whose registry id happened to start
        // with `10ca1` would be excluded here too — that is a fail-safe (it would
        // re-execute, never serve stale), and the only `10ca1` producer is
        // synthetic ids, so the false-negative risk is vanishing. (FR14, L19)
        return false;
    }
    true
}

/// The cache facade held by the composition operator.
pub struct Cache {
    config: CacheConfig,
    store: CacheStore,
    hits: Cell<usize>,
    total: Cell<usize>,
}

impl Cache {
    pub fn new(config: CacheConfig) -> Self {
        let store = CacheStore::new(config.dir.clone());
        Self {
            config,
            store,
            hits: Cell::new(0),
            total: Cell::new(0),
        }
    }

    /// A disabled cache that never reads or writes.
    pub fn disabled() -> Self {
        Self::new(CacheConfig::disabled())
    }

    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn debug(&self) -> bool {
        self.config.debug
    }

    /// Look up a node's cached output. Counts the node toward the summary total
    /// (an *attempt*), but does NOT count a hit yet — the caller must confirm the
    /// entry was actually served by calling [`Self::record_hit`] after the cached
    /// archive unpacks and its state is replayed. This keeps the `N/M served from
    /// cache` summary honest: a poisoned entry that unpacks to garbage and falls
    /// back to live execution is reported as a miss, not a hit. (FR6, FR14, FR15)
    ///
    /// Returns `None` (and counts nothing) when disabled or the node is not
    /// cacheable.
    pub fn lookup(&self, template: &TemplateVersionRes, key: &str) -> Option<CacheEntry> {
        if !self.config.enabled || !is_cacheable(template) {
            return None;
        }
        self.total.set(self.total.get() + 1);
        self.store.get(key)
    }

    /// Record that a looked-up entry was actually served from cache. Paired with
    /// [`Self::lookup`]: `lookup` increments the *attempt* total, and this
    /// increments the *hit* total only when the cached output was successfully
    /// consumed. (FR15)
    pub fn record_hit(&self) {
        self.hits.set(self.hits.get() + 1);
    }

    /// Store a node's output. No-ops when disabled or not cacheable. (FR6, FR14)
    pub fn store(&self, template: &TemplateVersionRes, key: &str, entry: &CacheEntry) {
        if !self.config.enabled || !is_cacheable(template) {
            return;
        }
        self.store.put(key, entry);
    }

    /// Remove a single entry. Used to self-heal a poisoned entry whose archive
    /// can't be unpacked, so a subsequent run re-executes instead of aborting. (FR8)
    pub fn evict(&self, key: &str) {
        self.store.remove(key);
    }

    pub fn hits(&self) -> usize {
        self.hits.get()
    }

    pub fn total(&self) -> usize {
        self.total.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cyanregistry::http::models::template_res::{
        TemplatePrincipalRes, TemplatePropertyRes, TemplateVersionPrincipalRes, TemplateVersionRes,
    };
    use std::collections::HashMap;

    fn make_template(id: &str, version: i64, has_props: bool) -> TemplateVersionRes {
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
                name: "n".to_string(),
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

    // AC8 / FR14: dev-mode (no properties), synthetic ids, and version<=0 are non-cacheable.
    #[test]
    fn is_cacheable_rejects_local_and_dev() {
        assert!(is_cacheable(&make_template("real-id", 3, true)));

        assert!(
            !is_cacheable(&make_template("real-id", 3, false)),
            "dev mode (no properties) must not cache"
        );
        assert!(
            !is_cacheable(&make_template("real-id", 0, true)),
            "version <= 0 must not cache"
        );
        assert!(
            !is_cacheable(&make_template("local-abc", 5, true)),
            "local- synthetic id must not cache"
        );
        assert!(
            !is_cacheable(&make_template("10ca1deadbeef", 5, true)),
            "10ca1 synthetic id must not cache"
        );
    }

    // AC7: a disabled cache never reads or writes, and lookup returns None.
    #[test]
    fn disabled_cache_is_inert() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = CacheConfig {
            enabled: false,
            dir: dir.path().join("cyanprint"),
            debug: false,
        };
        let cache = Cache::new(cfg);
        let t = make_template("real-id", 3, true);
        let entry = CacheEntry {
            archive: b"x".to_vec(),
            state: HashMap::new(),
        };
        cache.store(&t, "k", &entry);
        assert!(cache.lookup(&t, "k").is_none());
        assert!(
            !dir.path().join("cyanprint").exists(),
            "disabled cache must not create its directory"
        );
    }

    // AC7: an enabled cache stores and serves a hit, and counts attempts/hits.
    // The hit counter is only incremented via `record_hit`, after the caller
    // confirms the entry was actually served (not just looked up). (H2)
    #[test]
    fn enabled_cache_hit_and_counters() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = CacheConfig {
            enabled: true,
            dir: dir.path().join("cyanprint"),
            debug: false,
        };
        let cache = Cache::new(cfg);
        let t = make_template("real-id", 3, true);
        let entry = CacheEntry {
            archive: b"hello".to_vec(),
            state: HashMap::new(),
        };
        // A valid 64-char lowercase-hex key (the only shape the store accepts).
        let k = "deadbeef".repeat(8);
        // First lookup: a miss. Total counts the attempt; hits stays 0.
        assert!(cache.lookup(&t, &k).is_none(), "first lookup is a miss");
        assert_eq!(cache.total(), 1, "miss still counts as an attempt");
        assert_eq!(cache.hits(), 0, "a miss is not a hit");
        cache.store(&t, &k, &entry);
        // Second lookup: a hit. Total counts the attempt; hits only rises once
        // the caller confirms the entry was served.
        let got = cache.lookup(&t, &k).expect("second lookup is a hit");
        assert_eq!(got, entry);
        assert_eq!(cache.total(), 2);
        assert_eq!(
            cache.hits(),
            0,
            "lookup must not count a hit until record_hit is called"
        );
        cache.record_hit();
        assert_eq!(cache.hits(), 1, "record_hit counts the served entry");
    }

    // FR6: CYANPRINT_NO_CACHE / --no-output-cache disable; flag wins over env-off.
    #[test]
    fn resolve_respects_no_cache_flag() {
        // `resolve` reads CYANPRINT_NO_CACHE, so scope it for this test: a truthy
        // value in the ambient environment would otherwise flip `enabled` and make
        // the assertions non-deterministic. Resolve both configs while it is
        // cleared, restore the original value, then assert.
        let prev = std::env::var_os(ENV_NO_CACHE);
        std::env::remove_var(ENV_NO_CACHE);

        let disabled = CacheConfig::resolve(true, Some(PathBuf::from("/tmp/x")), false);
        let enabled = CacheConfig::resolve(false, Some(PathBuf::from("/tmp/x")), false);

        match prev {
            Some(v) => std::env::set_var(ENV_NO_CACHE, v),
            None => std::env::remove_var(ENV_NO_CACHE),
        }

        assert!(!disabled.enabled);
        assert!(enabled.enabled);
        assert_eq!(enabled.dir, PathBuf::from("/tmp/x"));
    }

    #[test]
    fn truthy_parsing() {
        for v in ["1", "true", "TRUE", "yes", "on", " true "] {
            assert!(is_truthy(v), "{v} should be truthy");
        }
        for v in ["0", "false", "no", "", "off"] {
            assert!(!is_truthy(v), "{v} should be falsy");
        }
    }
}
