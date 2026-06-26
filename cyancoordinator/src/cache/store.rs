//! Content-addressed store for node-execution outputs, keyed by the hex digest
//! from [`super::key`].
//!
//! Each entry holds the node's output archive bytes plus the answers it would
//! contribute downstream. Entries are framed with a length + SHA-256 content
//! checksum header (FR12); a corrupt or truncated entry reads back as a miss
//! rather than an error (FR8). Writes are atomic: a temp file in the same dir
//! (created `0600`), `sync_all`, `rename`, then a parent fsync; any write error
//! is swallowed so a cache fault never aborts a run (FR8, FR9).

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use cyanprompt::domain::models::answer::Answer;
use sha2::{Digest, Sha256};

/// The value stored for one node: its output archive and the answers it merges
/// into shared state downstream (the `TemplateState::Complete` answers map). (FR12)
#[derive(Debug, Clone, PartialEq)]
pub struct CacheEntry {
    pub archive: Vec<u8>,
    pub state: HashMap<String, Answer>,
}

/// On-disk layout (all integers little-endian):
///   [0..32)            sha256(payload)
///   [32..40)           payload length (u64)
///   [40..]             payload
/// where payload =
///   [0..8)             archive length (u64)
///   [8..8+alen)        archive bytes
///   [8+alen..]         state JSON (serde_json of HashMap<String, Answer>)
const CHECKSUM_LEN: usize = 32;
const LEN_FIELD: usize = 8;
const HEADER_LEN: usize = CHECKSUM_LEN + LEN_FIELD;

/// A content-addressed store rooted at a directory.
pub struct CacheStore {
    dir: PathBuf,
}

impl CacheStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    /// Create the cache directory (owner-only `0700`) if it does not exist. (FR9)
    fn ensure_dir(&self) -> std::io::Result<()> {
        if self.dir.exists() {
            return Ok(());
        }
        fs::create_dir_all(&self.dir)?;
        set_dir_perms_0700(&self.dir)?;
        Ok(())
    }

    fn entry_path(&self, key: &str) -> PathBuf {
        self.dir.join(key)
    }

    fn encode(entry: &CacheEntry) -> Vec<u8> {
        let state_json = serde_json::to_vec(&entry.state).unwrap_or_default();
        let mut payload = Vec::with_capacity(LEN_FIELD + entry.archive.len() + state_json.len());
        payload.extend_from_slice(&(entry.archive.len() as u64).to_le_bytes());
        payload.extend_from_slice(&entry.archive);
        payload.extend_from_slice(&state_json);

        let mut hasher = Sha256::new();
        hasher.update(&payload);
        let checksum = hasher.finalize();

        let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
        out.extend_from_slice(&checksum);
        out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    /// Decode and self-verify a raw entry file. Returns `None` on any
    /// corruption (bad checksum, wrong length, truncation, decode failure). (FR8)
    fn decode(bytes: &[u8]) -> Option<CacheEntry> {
        if bytes.len() < HEADER_LEN {
            return None;
        }
        let checksum = &bytes[0..CHECKSUM_LEN];
        let payload_len =
            u64::from_le_bytes(bytes[CHECKSUM_LEN..HEADER_LEN].try_into().ok()?) as usize;
        let payload = bytes.get(HEADER_LEN..)?;
        if payload.len() != payload_len {
            return None;
        }

        let mut hasher = Sha256::new();
        hasher.update(payload);
        if hasher.finalize().as_slice() != checksum {
            return None;
        }

        if payload.len() < LEN_FIELD {
            return None;
        }
        let archive_len = u64::from_le_bytes(payload[0..LEN_FIELD].try_into().ok()?) as usize;
        let archive_end = LEN_FIELD.checked_add(archive_len)?;
        let archive = payload.get(LEN_FIELD..archive_end)?.to_vec();
        let state_bytes = payload.get(archive_end..)?;
        let state: HashMap<String, Answer> = serde_json::from_slice(state_bytes).ok()?;

        Some(CacheEntry { archive, state })
    }

    /// Look up an entry by key. Any IO error / checksum mismatch / decode error
    /// is treated as a miss (`None`), never an error. (FR8)
    pub fn get(&self, key: &str) -> Option<CacheEntry> {
        let path = self.entry_path(key);
        let mut file = fs::File::open(&path).ok()?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).ok()?;
        Self::decode(&bytes)
    }

    /// Store an entry under its key. Best-effort: any error (ENOSPC/EXDEV/EACCES,
    /// etc.) is logged and swallowed so the run continues. (FR8, FR9)
    pub fn put(&self, key: &str, entry: &CacheEntry) {
        if let Err(e) = self.put_inner(key, entry) {
            tracing::debug!("cache put for {key} failed (non-fatal): {e}");
        }
    }

    /// Remove a single entry. No-op if it is absent; errors are swallowed so a
    /// cache fault never aborts a run. Used to self-heal a poisoned entry whose
    /// archive can't be unpacked. (FR8)
    pub fn remove(&self, key: &str) {
        if let Err(e) = fs::remove_file(self.entry_path(key)) {
            // NotFound is expected (nothing to evict); other errors are non-fatal.
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::debug!("cache remove for {key} failed (non-fatal): {e}");
            }
        }
    }

    fn put_inner(&self, key: &str, entry: &CacheEntry) -> std::io::Result<()> {
        self.ensure_dir()?;
        let encoded = Self::encode(entry);

        // Temp file in the SAME dir so the rename is atomic (same filesystem). (FR8)
        let mut tmp = tempfile::NamedTempFile::new_in(&self.dir)?;
        set_file_perms_0600(tmp.path())?;
        tmp.write_all(&encoded)?;
        tmp.flush()?;
        tmp.as_file().sync_all()?;

        let dest = self.entry_path(key);
        tmp.persist(&dest)
            .map_err(|e| std::io::Error::other(format!("persist failed: {e}")))?;

        // fsync the parent dir so the rename is durable.
        if let Ok(dir) = fs::File::open(&self.dir) {
            let _ = dir.sync_all();
        }
        Ok(())
    }

    /// Remove every entry, leaving the (empty) cache directory in place. (FR15)
    ///
    /// Only entries whose name is a lowercase-hex digest (the cache's own naming
    /// scheme) are removed, so a cache dir that has been pointed at (or shares
    /// with) an unrelated directory never loses non-cache files. A safety guard:
    /// it refuses to descend into a directory it did not create.
    pub fn clear(&self) -> std::io::Result<()> {
        if !self.dir.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if is_cache_entry_name(entry.file_name()) {
                // Cache entries are plain files (atomic rename target), never dirs.
                if path.is_file() {
                    fs::remove_file(&path)?;
                }
            }
        }
        Ok(())
    }

    /// Total size in bytes of all entries under the cache dir. (FR15)
    ///
    /// Only counts real cache entries (lowercase-hex-digest-named files), matching
    /// the content guard [`clear`] uses, so an orphaned `.tmp*` temp from a crash
    /// mid-write is not double-counted here while being unreaped by [`clear`].
    ///
    /// [`clear`]: CacheStore::clear
    pub fn size(&self) -> u64 {
        let mut total = 0u64;
        if let Ok(read) = fs::read_dir(&self.dir) {
            for entry in read.flatten() {
                if is_cache_entry_name(entry.file_name()) {
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_file() {
                            total += meta.len();
                        }
                    }
                }
            }
        }
        total
    }
}

#[cfg(unix)]
fn set_dir_perms_0700(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_dir_perms_0700(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_file_perms_0600(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_file_perms_0600(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// True when `name` looks like a cache entry: a SHA-256 hex digest (64 lowercase
/// hex chars), the only names this store ever writes. Used by `clear()` as a
/// content guard so it never deletes unrelated files.
fn is_cache_entry_name(name: std::ffi::OsString) -> bool {
    let s = match name.to_str() {
        Some(s) => s,
        None => return false,
    };
    s.len() == 64
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> CacheEntry {
        let mut state = HashMap::new();
        state.insert("answer".to_string(), Answer::String("v".to_string()));
        state.insert("flag".to_string(), Answer::Bool(true));
        CacheEntry {
            archive: b"\x00\x01\x02 some archive bytes \xff".to_vec(),
            state,
        }
    }

    // AC2: put then get returns identical (archive, state).
    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().join("cyanprint"));
        let e = entry();
        store.put("deadbeef", &e);
        let got = store.get("deadbeef").expect("entry should be present");
        assert_eq!(got, e);
    }

    // AC2: flipping a byte in the stored file -> get returns None (miss), no error.
    #[test]
    fn corruption_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().join("cyanprint"));
        let e = entry();
        store.put("key1", &e);

        let path = store.entry_path("key1");
        let mut bytes = fs::read(&path).unwrap();
        // Flip a byte well inside the payload (past the header).
        let idx = bytes.len() - 1;
        bytes[idx] ^= 0xff;
        fs::write(&path, &bytes).unwrap();

        assert!(
            store.get("key1").is_none(),
            "a corrupted entry must read back as a miss"
        );
    }

    // AC2: a truncated entry is also a miss.
    #[test]
    fn truncation_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().join("cyanprint"));
        store.put("k", &entry());
        let path = store.entry_path("k");
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        assert!(store.get("k").is_none());
    }

    // AC2: a missing entry is a miss, not an error.
    #[test]
    fn missing_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().join("cyanprint"));
        assert!(store.get("never-written").is_none());
    }

    // AC9: clear empties the cache; size reflects contents. Uses real hex-digest
    // keys (the only names clear() will touch once the content guard is in place).
    #[test]
    fn clear_and_size() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().join("cyanprint"));
        assert_eq!(store.size(), 0);
        let k1 = "deadbeef".repeat(8); // 64-char hex
        let k2 = "cafebabe".repeat(8); // 64-char hex
        store.put(&k1, &entry());
        store.put(&k2, &entry());
        assert!(store.size() > 0, "size should be non-zero after writes");
        store.clear().unwrap();
        assert_eq!(store.size(), 0, "size should be 0 after clear");
        assert!(store.path().exists(), "clear keeps the directory");
    }

    // L3 / safety: clear() is a content guard — it removes only hex-digest-named
    // entries and leaves unrelated files alone (a footgun if pointed at $HOME).
    #[test]
    fn clear_only_removes_cache_entries() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().join("cyanprint"));
        let key = "deadbeef".repeat(8);
        store.put(&key, &entry());
        // An unrelated file the cache did not create.
        let intruder = store.path().join("README.md");
        fs::create_dir_all(store.path()).unwrap();
        fs::write(&intruder, "do not delete me").unwrap();

        store.clear().unwrap();
        assert!(!store.entry_path(&key).exists(), "cache entry removed");
        assert!(
            intruder.exists(),
            "clear() must not delete non-cache files (content guard)"
        );
    }

    // L13: size() applies the same hex-digest content guard as clear(), so an
    // orphaned .tmp* temp left by a crash mid-write is not counted.
    #[test]
    fn size_ignores_orphaned_temps() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().join("cyanprint"));
        let key = "deadbeef".repeat(8);
        store.put(&key, &entry());

        // Simulate a crash mid-write: a leftover temp file the store never finished
        // persisting (NamedTempFile uses a `.tmp*`-ish prefix; here any non-hex name).
        fs::create_dir_all(store.path()).unwrap();
        fs::write(store.path().join(".tmpABCD"), b"orphaned bytes").unwrap();

        let entry_size = fs::metadata(store.entry_path(&key)).unwrap().len();
        assert_eq!(
            store.size(),
            entry_size,
            "size() must count only hex-named cache entries, not orphaned temps"
        );
    }

    // FR8 self-heal: remove() deletes an entry and is a no-op when absent.
    #[test]
    fn remove_evicts_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().join("cyanprint"));
        let key = "deadbeef".repeat(8);
        store.put(&key, &entry());
        assert!(store.get(&key).is_some());
        store.remove(&key);
        assert!(store.get(&key).is_none());
        // Removing again must not error.
        store.remove(&key);
    }

    // NFC1: dir is 0700, entry files 0600, set at creation (unix only).
    #[cfg(unix)]
    #[test]
    fn permissions_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = CacheStore::new(dir.path().join("cyanprint"));
        store.put("k", &entry());

        let dir_mode = fs::metadata(store.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "cache dir must be 0700");

        let file_mode = fs::metadata(store.entry_path("k"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600, "cache entry files must be 0600");
    }

    // FR8: put to an unwritable parent does not panic (best-effort, swallowed).
    #[cfg(unix)]
    #[test]
    fn put_failure_is_swallowed() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        // Make the parent read-only so create_dir_all of the child fails.
        let ro_parent = dir.path().join("ro");
        fs::create_dir(&ro_parent).unwrap();
        fs::set_permissions(&ro_parent, fs::Permissions::from_mode(0o500)).unwrap();

        let store = CacheStore::new(ro_parent.join("cyanprint"));
        // Must not panic.
        store.put("k", &entry());
        assert!(store.get("k").is_none());

        // Restore perms so tempdir cleanup succeeds.
        fs::set_permissions(&ro_parent, fs::Permissions::from_mode(0o700)).unwrap();
    }
}
