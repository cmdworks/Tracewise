use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

pub const MAX_DIFF_SIZE: u64 = 5 * 1024 * 1024; // 5 MB

/// Computes the 40-character lowercase hexadecimal git object SHA-1 hash of the bytes.
/// Format: sha1("blob <len>\0" + data).
pub fn hash_bytes(data: &[u8]) -> String {
    let header = format!("blob {}\0", data.len());
    let mut hasher = Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// The file's bytes and git blob hash. A file larger than `max_file` is hashed in 64 KB pieces and returns no bytes
/// (hash-only, like `BlobStore::store` treats it), so a huge or sparse file never has to fit in memory (Y17, N05).
pub fn read_or_hash(path: &Path, size: u64, max_file: u64) -> io::Result<(Vec<u8>, String)> {
    if size <= max_file {
        let bytes = fs::read(path)?;
        let hash = hash_bytes(&bytes);
        return Ok((bytes, hash));
    }
    use std::io::Read;
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {size}\0").as_bytes());
    let mut file = fs::File::open(path)?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok((Vec::new(), format!("{:x}", hasher.finalize())))
}

/// Parses size strings like "1KB", "2MB", "500B", "1024" into bytes.
pub fn parse_size_bytes(s: &str) -> Option<u64> {
    let trimmed = s.trim().trim_matches('"').trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();
    if let Some(num) = lower.strip_suffix("kb") {
        num.trim().parse::<u64>().ok().map(|n| n * 1024)
    } else if let Some(num) = lower.strip_suffix("k") {
        num.trim().parse::<u64>().ok().map(|n| n * 1024)
    } else if let Some(num) = lower.strip_suffix("mb") {
        num.trim().parse::<u64>().ok().map(|n| n * 1024 * 1024)
    } else if let Some(num) = lower.strip_suffix("m") {
        num.trim().parse::<u64>().ok().map(|n| n * 1024 * 1024)
    } else if let Some(num) = lower.strip_suffix("gb") {
        num.trim().parse::<u64>().ok().map(|n| n * 1024 * 1024 * 1024)
    } else if let Some(num) = lower.strip_suffix("g") {
        num.trim().parse::<u64>().ok().map(|n| n * 1024 * 1024 * 1024)
    } else if let Some(num) = lower.strip_suffix("b") {
        num.trim().parse::<u64>().ok()
    } else {
        lower.parse::<u64>().ok()
    }
}

/// The first value of `[storage] <key>` found in the config of `data_dir` or the three folders above it (the repository).
fn find_storage_value(data_dir: &Path, key: &str) -> Option<String> {
    data_dir.ancestors().take(4).find_map(|dir| crate::reasons::config_value(dir, "storage", key))
}

pub fn find_max_file(data_dir: &Path) -> Option<u64> {
    data_dir.ancestors().take(4).find_map(|dir| crate::reasons::config_value(dir, "storage", "max_file").and_then(|v| parse_size_bytes(&v)))
}

/// `[storage] secrets = "store"` keeps files that hold a known secret like any other text (default: hash only). Set once per
/// process by `BlobStore::new`; every scan, hook and command runs in its own process for one repository.
static STORE_SECRETS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `[durability] level = "full"` makes every write wait for the disk: blob files and their folder, the `chain-head` file and
/// SQLite (`synchronous=FULL`). Default `normal` (`full` as default failed the scale tests); with `normal` a power cut can lose the newest records (a crash of Tracewise itself cannot).
/// Set once per process by `BlobStore::new` and `Database::open`, like `STORE_SECRETS`.
static FULL_DURABILITY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Reads `[durability] level` for the data folder (same walk-up as `[storage]`), remembers it and returns whether it is `full`.
pub(crate) fn load_durability(data_dir: &Path) -> bool {
    let full = data_dir
        .ancestors()
        .take(4)
        .find_map(|dir| crate::reasons::config_value(dir, "durability", "level"))
        .is_none_or(|v| v.trim() != "normal");
    FULL_DURABILITY.store(full, std::sync::atomic::Ordering::Relaxed);
    full
}

/// Set when a file was fsynced and the drive's cache has not been flushed since (see `flush_if_pending`).
static UNFLUSHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Hands `path` (a file or a folder) to the drive, when the level is `full`. The cache flush that makes it safe against a power
/// cut is done once per batch by `flush_if_pending`, not once per file: that is what keeps a 20,000-file burst fast.
pub(crate) fn sync_if_full(path: &Path) -> io::Result<()> {
    if FULL_DURABILITY.load(std::sync::atomic::Ordering::Relaxed) {
        tracewise_platform::private::fsync(path)?;
        UNFLUSHED.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

/// One drive-cache flush (`F_FULLFSYNC`) for everything fsynced since the last one. Called before a change record commits, so a
/// record never becomes durable before its blobs.
pub(crate) fn flush_if_pending(dir: &Path) -> io::Result<()> {
    flush_marked(&UNFLUSHED, dir).map(|_| ())
}

/// Flushes `dir` when `mark` is set and clears it; says whether it flushed.
fn flush_marked(mark: &std::sync::atomic::AtomicBool, dir: &Path) -> io::Result<bool> {
    let pending = mark.swap(false, std::sync::atomic::Ordering::Relaxed);
    if pending {
        fs::File::open(dir)?.sync_all()?;
    }
    Ok(pending)
}

pub(crate) fn holds_secret(data: &[u8]) -> bool {
    !STORE_SECRETS.load(std::sync::atomic::Ordering::Relaxed) && crate::share::contains_secret(data)
}

/// Content that is recorded by hash only: binary (a NUL byte), over 5 MB, or holding a known secret (a key must not
/// land in `.git/tracewise/objects`). Undo and restore then say "recorded by hash only", as for any binary file.
pub fn is_binary_or_large(data: &[u8]) -> bool {
    data.len() as u64 > MAX_DIFF_SIZE || data.contains(&0) || holds_secret(data)
}

/// Looks `hash` (40 hex digits) up in every `*.idx` of `pack_dir` without starting git. `None` when an index is
/// not the version 2 format (or cannot be read), so the caller can ask git instead. A pack folder with no index
/// answers `Some(false)` at once; that is the common case of a fresh repository.
fn pack_indexes_contain(pack_dir: &Path, hash: &str) -> Option<bool> {
    use std::io::{Read, Seek, SeekFrom};
    let want: Vec<u8> = (0..hash.len() / 2).map(|i| u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16)).collect::<Result<_, _>>().ok()?;
    if want.len() != 20 {
        return None;
    }
    for entry in fs::read_dir(pack_dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("idx") {
            continue;
        }
        let mut f = fs::File::open(&path).ok()?;
        let mut head = [0u8; 8 + 1024];
        f.read_exact(&mut head).ok()?;
        if head[..8] != [0xff, b't', b'O', b'c', 0, 0, 0, 2] {
            return None;
        }
        let fan = |i: usize| u32::from_be_bytes(head[8 + i * 4..12 + i * 4].try_into().unwrap()) as u64;
        let b = want[0] as usize;
        let (mut lo, mut hi) = (if b == 0 { 0 } else { fan(b - 1) }, fan(b));
        let names = 8 + 1024u64;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let mut name = [0u8; 20];
            f.seek(SeekFrom::Start(names + mid * 20)).ok()?;
            f.read_exact(&mut name).ok()?;
            match name.as_slice().cmp(want.as_slice()) {
                std::cmp::Ordering::Equal => return Some(true),
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
    }
    Some(false)
}

/// Blob store located at `<git common dir>/tracewise/objects/` with git alternates.
#[derive(Debug, Clone)]
pub struct BlobStore {
    root: PathBuf,
    git_objects_dir: Option<PathBuf>,
    max_file: u64,
}

impl BlobStore {
    pub fn new(data_dir: &Path) -> Self {
        let root = data_dir.join("objects");
        let git_objects_dir = if let Some(parent) = data_dir.parent() {
            let candidate = parent.join("objects");
            if candidate.is_dir() || data_dir.file_name() == Some(std::ffi::OsStr::new("tracewise")) {
                Some(candidate)
            } else {
                None
            }
        } else {
            None
        };
        let max_file = find_max_file(data_dir).unwrap_or(MAX_DIFF_SIZE);
        if find_storage_value(data_dir, "secrets").is_some_and(|v| v.trim() == "store") {
            STORE_SECRETS.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        load_durability(data_dir);
        Self::with_options(root, git_objects_dir, max_file)
    }

    pub fn with_git_objects(root: PathBuf, git_objects_dir: Option<PathBuf>) -> Self {
        Self::with_options(root, git_objects_dir, MAX_DIFF_SIZE)
    }

    pub fn with_options(root: PathBuf, git_objects_dir: Option<PathBuf>, max_file: u64) -> Self {
        let store = Self { root, git_objects_dir, max_file };
        if !store.root.exists() {
            // a brand-new store is already in the git format: mark it, or the first command after `init` reads every object
            // to look for the old zstd format (13 s with 100,000 files)
            if tracewise_platform::private::create_dir_all(&store.root).is_ok() {
                let _ = tracewise_platform::private::write(store.root.join(".migrated_git"), b"migrated\n");
            }
        }
        store.ensure_alternates();
        store
    }

    pub fn max_file(&self) -> u64 {
        self.max_file
    }

    pub fn is_binary_or_large(&self, data: &[u8]) -> bool {
        data.len() as u64 > self.max_file || data.contains(&0) || holds_secret(data)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn git_objects_dir(&self) -> Option<&Path> {
        self.git_objects_dir.as_deref()
    }

    pub fn ensure_alternates(&self) {
        if let Some(ref git_objs) = self.git_objects_dir {
            let alternates_path = self.root.join("info").join("alternates");
            let target_line = format!("{}\n", git_objs.display());
            let needs_write = match fs::read_to_string(&alternates_path) {
                Ok(content) => !content.lines().any(|l| l.trim() == git_objs.to_str().unwrap_or("")),
                Err(_) => true,
            };
            if needs_write {
                if let Some(parent) = alternates_path.parent() {
                    let _ = tracewise_platform::private::create_dir_all(parent);
                }
                let _ = tracewise_platform::private::write(&alternates_path, target_line.as_bytes());
            }
        }
    }

    fn blob_path(&self, hash: &str) -> PathBuf {
        let (prefix, rest) = hash.split_at(2.min(hash.len()));
        self.root.join(prefix).join(rest)
    }

    fn real_git_cmd() -> std::process::Command {
        let git_path = if let Ok(real) = std::env::var("TRACEWISE_REAL_GIT") {
            PathBuf::from(real)
        } else if Path::new("/usr/bin/git").is_file() {
            PathBuf::from("/usr/bin/git")
        } else {
            PathBuf::from("git")
        };
        let mut cmd = std::process::Command::new(git_path);
        cmd.env("TRACEWISE_GIT_DEPTH", "1");
        cmd
    }

    /// `git` aimed at the project's repository whatever the current directory is: git itself can only read the
    /// project's packs when it knows which repository it is looking at. Tracewise's own objects are an alternate.
    fn git_on_objects(&self) -> std::process::Command {
        let mut cmd = Self::real_git_cmd();
        match self.git_objects_dir.as_ref().and_then(|o| o.parent()) {
            Some(git_dir) => {
                cmd.arg("--git-dir").arg(git_dir);
                cmd.env("GIT_ALTERNATE_OBJECT_DIRECTORIES", &self.root);
            }
            None => {
                cmd.env("GIT_OBJECT_DIRECTORY", &self.root);
            }
        }
        cmd
    }

    /// Checks if a blob already exists in the project's git objects (loose or pack).
    pub fn exists_in_git(&self, hash: &str) -> bool {
        let (prefix, rest) = hash.split_at(2.min(hash.len()));
        if let Some(ref git_objs) = self.git_objects_dir {
            if git_objs.join(prefix).join(rest).is_file() {
                return true;
            }
            let pack_dir = git_objs.join("pack");
            if pack_dir.is_dir() {
                match pack_indexes_contain(&pack_dir, hash) {
                    Some(found) if !git_objs.join("info").join("alternates").exists() => return found,
                    _ => {
                        // Unknown index format, or objects may live in an alternate: ask git.
                        let mut cmd = self.git_on_objects();
                        cmd.args(["cat-file", "-e", hash]);
                        if let Ok(status) = cmd.status() {
                            if status.success() {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Stores the raw bytes as a git blob.
    /// Skips storing if content is binary or large, or if already in git objects or tracewise objects.
    /// Returns the git object id.
    pub fn store(&self, data: &[u8]) -> io::Result<String> {
        let hash = hash_bytes(data);
        if self.is_binary_or_large(data) {
            return Ok(hash);
        }

        // 1. Skip if already in project's git objects
        if self.exists_in_git(&hash) {
            return Ok(hash);
        }

        // 2. Skip if already in our own objects
        let path = self.blob_path(&hash);
        if path.is_file() {
            return Ok(hash);
        }

        if let Some(parent) = path.parent() {
            tracewise_platform::private::create_dir_all(parent)?;
        }

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        let header = format!("blob {}\0", data.len());
        encoder.write_all(header.as_bytes())?;
        encoder.write_all(data)?;
        let compressed = encoder.finish()?;

        let tmp_path = path.with_extension(format!("{}.tmp", ulid::Ulid::new()));
        tracewise_platform::private::write(&tmp_path, &compressed)?;
        sync_if_full(&tmp_path)?;
        fs::rename(&tmp_path, &path)?;
        if let Some(parent) = path.parent() {
            sync_if_full(parent)?;
            // a new two-letter folder is an entry of the objects folder, which must reach the disk too
            if let Some(root) = parent.parent() {
                sync_if_full(root)?;
            }
        }
        Ok(hash)
    }

    /// Reads and decompresses the blob by its hash.
    pub fn read(&self, hash: &str) -> io::Result<Vec<u8>> {
        let (prefix, rest) = hash.split_at(2.min(hash.len()));
        let tracewise_loose = self.root.join(prefix).join(rest);
        if tracewise_loose.is_file() {
            return Self::read_loose(&tracewise_loose);
        }

        if let Some(ref git_objs) = self.git_objects_dir {
            let git_loose = git_objs.join(prefix).join(rest);
            if git_loose.is_file() {
                return Self::read_loose(&git_loose);
            }
        }

        // Check alternates listed in info/alternates
        let alternates_path = self.root.join("info").join("alternates");
        if alternates_path.is_file() {
            if let Ok(content) = fs::read_to_string(&alternates_path) {
                for line in content.lines() {
                    let alt = Path::new(line.trim());
                    let alt_loose = alt.join(prefix).join(rest);
                    if alt_loose.is_file() {
                        return Self::read_loose(&alt_loose);
                    }
                }
            }
        }

        // Fallback: check git cat-file (e.g. from packs)
        self.read_via_git(hash)
    }

    fn read_loose(path: &Path) -> io::Result<Vec<u8>> {
        let compressed = fs::read(path)?;
        // Check if legacy zstd (magic 0x28, 0xB5, 0x2F, 0xFD)
        if compressed.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
            return zstd::decode_all(&compressed[..]);
        }

        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;

        if let Some(nul_pos) = decompressed.iter().position(|&b| b == 0) {
            Ok(decompressed[nul_pos + 1..].to_vec())
        } else {
            Ok(decompressed)
        }
    }

    fn read_via_git(&self, hash: &str) -> io::Result<Vec<u8>> {
        let mut cmd = self.git_on_objects();
        cmd.args(["cat-file", "-p", hash]);
        let output = cmd.output()?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, format!("git object {hash} not found")))
        }
    }

    /// The hashes that are in none of Tracewise's, the project's or an alternate's object folders, nor in a pack.
    /// Loose files are looked up directly; the rest go to one `git cat-file --batch-check`.
    pub fn missing(&self, hashes: &[String]) -> Vec<String> {
        let loose = |dir: &Path, h: &str| dir.join(&h[..2.min(h.len())]).join(&h[2.min(h.len())..]).is_file();
        let unresolved: Vec<&String> =
            hashes.iter().filter(|h| !(loose(&self.root, h) || self.git_objects_dir.as_deref().is_some_and(|g| loose(g, h)))).collect();
        if unresolved.is_empty() {
            return Vec::new();
        }
        let spawned = self
            .git_on_objects()
            .args(["cat-file", "--batch-check"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = spawned else { return unresolved.into_iter().cloned().collect() };
        let mut stdin = child.stdin.take().expect("piped");
        let input: String = unresolved.iter().map(|h| format!("{h}\n")).collect();
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
        let out = child.wait_with_output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        let _ = writer.join();
        // one answer line per input line, in order; "<hash> missing" for an absent object
        let answers: Vec<&str> = out.lines().collect();
        unresolved
            .iter()
            .enumerate()
            .filter(|(i, _)| answers.get(*i).is_none_or(|a| a.ends_with(" missing")))
            .map(|(_, h)| (*h).clone())
            .collect()
    }

    /// Calculates the disk usage of the objects directory in bytes.
    pub fn du(&self) -> u64 {
        Self::dir_size(&self.root)
    }

    fn dir_size(dir: &Path) -> u64 {
        let mut total = 0;
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    if let Ok(m) = p.metadata() {
                        total += m.len();
                    }
                } else if p.is_dir() {
                    total += Self::dir_size(&p);
                }
            }
        }
        total
    }

    /// Packs loose objects in `<data>/objects` into delta packs using git pack-objects.
    pub fn gc(&self) -> io::Result<GcReport> {
        let bytes_before = self.du();

        // 1. Collect all loose object hashes
        let mut loose_hashes = Vec::new();
        let mut loose_paths = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if dir_name.len() == 2 && dir_name.chars().all(|c| c.is_ascii_hexdigit()) {
                        if let Ok(sub_entries) = fs::read_dir(&path) {
                            for sub in sub_entries.flatten() {
                                let sub_path = sub.path();
                                if sub_path.is_file() {
                                    let file_name = sub_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                                    if file_name.len() == 38 && file_name.chars().all(|c| c.is_ascii_hexdigit()) {
                                        let full_hash = format!("{dir_name}{file_name}");
                                        loose_hashes.push(full_hash);
                                        loose_paths.push(sub_path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let packed_count = loose_hashes.len();
        if packed_count == 0 {
            return Ok(GcReport { packed: 0, bytes_before, bytes_after: bytes_before, commands_pruned: None });
        }

        // 2. Ensure pack dir exists
        let pack_dir = self.root.join("pack");
        tracewise_platform::private::create_dir_all(&pack_dir)?;

        // 3. Run git pack-objects
        let pack_prefix = pack_dir.join("pack");
        let mut cmd = Self::real_git_cmd();
        cmd.args(["pack-objects", "--delta-base-offset", pack_prefix.to_str().unwrap()]);
        cmd.env("GIT_OBJECT_DIRECTORY", &self.root);
        if let Some(ref git_objs) = self.git_objects_dir {
            cmd.env("GIT_ALTERNATE_OBJECT_DIRECTORIES", git_objs);
        }
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            for h in &loose_hashes {
                stdin.write_all(h.as_bytes())?;
                stdin.write_all(b"\n")?;
            }
        }

        let output = child.wait_with_output()?;
        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            return Err(io::Error::other(format!("git pack-objects failed: {err_msg}")));
        }

        // 4. Remove loose objects that were packed
        for p in loose_paths {
            let _ = fs::remove_file(&p);
            if let Some(parent) = p.parent() {
                let _ = fs::remove_dir(parent);
            }
        }

        let bytes_after = self.du();

        Ok(GcReport { packed: packed_count, bytes_before, bytes_after, commands_pruned: None })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GcReport {
    pub packed: usize,
    pub bytes_before: u64,
    pub bytes_after: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands_pruned: Option<usize>,
}

#[cfg(test)]
#[path = "test_blob.rs"]
mod tests;
