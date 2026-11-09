//! Optional encryption of shared batches. A project key (32 random bytes) is kept in the keyring of the
//! data folder (`share-keys.json`, mode 0600), never in the repository. With `[share] encrypt = true`, `trail push` encrypts every file of
//! a batch except `batch.json` (XChaCha20-Poly1305, a fresh random nonce in front of each file) and then signs the encrypted bytes, so a
//! machine without the key can still check the batch. `fetch` and `rebuild` decrypt into a scratch folder and import from there.
//! `batch.json` stays readable: it names the key id and holds the time range the import order needs.
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const KEYS_FILE: &str = "share-keys.json";
const NONCE_LEN: usize = 24;

/// All keys this clone knows, by id, oldest first. New batches use `current`.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Keyring {
    pub current: Option<String>,
    pub keys: Vec<(String, String)>,
}

fn bad(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return None;
    }
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()).collect()
}

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(KEYS_FILE)
}

pub fn load(data_dir: &Path) -> Keyring {
    fs::read_to_string(path(data_dir)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(data_dir: &Path, ring: &Keyring) -> io::Result<()> {
    tracewise_platform::private::write(path(data_dir), serde_json::to_string_pretty(ring).unwrap_or_default())
}

fn random_key() -> io::Result<[u8; 32]> {
    let mut k = [0u8; 32];
    getrandom::getrandom(&mut k).map_err(|e| io::Error::other(format!("no random numbers: {e}")))?;
    Ok(k)
}

/// The id of a key: the first 8 bytes of its blake3 hash, in hex (names the key without revealing it).
pub fn key_id(key_hex: &str) -> String {
    blake3::hash(key_hex.as_bytes()).to_hex()[..16].to_string()
}

/// Makes a new key and makes it the current one. Returns its id.
pub fn add_new_key(data_dir: &Path) -> io::Result<String> {
    let key = hex(&random_key()?);
    let id = key_id(&key);
    let mut ring = load(data_dir);
    ring.keys.push((id.clone(), key));
    ring.current = Some(id.clone());
    save(data_dir, &ring)?;
    Ok(id)
}

/// The current key's id, making the first key when there is none.
pub fn ensure_key(data_dir: &Path) -> io::Result<String> {
    match load(data_dir).current {
        Some(id) => Ok(id),
        None => add_new_key(data_dir),
    }
}

const EXPORT_PREFIX: &str = "trailkey1";

/// The string that carries a key to another machine: `trailkey1:<id>:<64 hex>`. It is the key itself: a secret.
pub fn export_key(data_dir: &Path, id: Option<&str>) -> io::Result<String> {
    let ring = load(data_dir);
    let id = id.map(String::from).or(ring.current.clone()).ok_or_else(|| bad("this clone has no key"))?;
    let key = ring.keys.iter().find(|(k, _)| *k == id).ok_or_else(|| bad(format!("no key {id} in this clone")))?;
    Ok(format!("{EXPORT_PREFIX}:{}:{}", key.0, key.1))
}

/// Adds an exported key to the keyring (a key already there is left alone). It becomes the current key when the keyring was
/// empty or `make_current` is set. Returns its id.
pub fn import_key(data_dir: &Path, text: &str, make_current: bool) -> io::Result<String> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    let [prefix, id, key] = parts[..] else { return Err(bad("not an exported key (expected trailkey1:<id>:<key>)")) };
    if prefix != EXPORT_PREFIX || key.len() != 64 || unhex(key).is_none() || key_id(key) != id {
        return Err(bad("not a valid exported key: the id does not match the key"));
    }
    let mut ring = load(data_dir);
    if !ring.keys.iter().any(|(k, _)| k == id) {
        ring.keys.push((id.to_string(), key.to_string()));
    }
    if make_current || ring.current.is_none() {
        ring.current = Some(id.to_string());
    }
    save(data_dir, &ring)?;
    Ok(id.to_string())
}

/// Notes that the key `id` was exported at least once (`trail doctor` warns when encryption is on and it never was).
pub fn note_exported(data_dir: &Path, id: &str) {
    let _ = tracewise_platform::private::write(data_dir.join("share-key-exported"), id);
}

pub fn was_exported(data_dir: &Path, id: &str) -> bool {
    fs::read_to_string(data_dir.join("share-key-exported")).is_ok_and(|t| t.trim() == id)
}

fn key_by_id(ring: &Keyring, id: &str) -> Option<[u8; 32]> {
    let hexed = &ring.keys.iter().find(|(k, _)| k == id)?.1;
    unhex(hexed)?.try_into().ok()
}

fn seal(key: &[u8; 32], plain: &[u8]) -> io::Result<Vec<u8>> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce).map_err(|e| io::Error::other(format!("no random numbers: {e}")))?;
    let ct =
        XChaCha20Poly1305::new(Key::from_slice(key)).encrypt(XNonce::from_slice(&nonce), plain).map_err(|_| bad("encryption failed"))?;
    Ok([nonce.to_vec(), ct].concat())
}

fn open(key: &[u8; 32], sealed: &[u8]) -> io::Result<Vec<u8>> {
    if sealed.len() < NONCE_LEN {
        return Err(bad("an encrypted file is too short"));
    }
    let (nonce, ct) = sealed.split_at(NONCE_LEN);
    XChaCha20Poly1305::new(Key::from_slice(key))
        .decrypt(XNonce::from_slice(nonce), ct)
        .map_err(|_| bad("could not decrypt a file of the batch: the key is wrong or the file was changed"))
}

fn files_of(dir: &Path) -> io::Result<Vec<PathBuf>> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
        for e in fs::read_dir(dir)? {
            let e = e?;
            if e.file_type()?.is_dir() {
                walk(&e.path(), out)?;
            } else {
                out.push(e.path());
            }
        }
        Ok(())
    }
    let mut v = Vec::new();
    walk(dir, &mut v)?;
    v.sort();
    Ok(v)
}

/// Encrypts every file of the batch folder except `batch.json`, in place. Blob files are renamed `blob-<n>` so their names (a hash of
/// the plain content) do not leak. Returns the key id used.
pub fn encrypt_batch(data_dir: &Path, dir: &Path) -> io::Result<String> {
    // a push never makes a key of its own: two machines would then encrypt with different keys
    let id = load(data_dir).current.ok_or_else(|| {
        io::Error::other("encryption is on but this clone has no key; run `trail share init --encrypt` here, or import the project's key")
    })?;
    let key = key_by_id(&load(data_dir), &id).ok_or_else(|| bad("the current key is unreadable"))?;
    let blobs = dir.join("blobs");
    let mut n = 0;
    for f in files_of(dir)? {
        if f.file_name().is_some_and(|n| n == "batch.json") {
            continue;
        }
        let sealed = seal(&key, &fs::read(&f)?)?;
        let target = if f.starts_with(&blobs) {
            n += 1;
            blobs.join(format!("blob-{n:06}"))
        } else {
            f.clone()
        };
        fs::write(&target, sealed)?;
        if target != f {
            fs::remove_file(&f)?;
        }
    }
    Ok(id)
}

/// Decrypts a batch folder into `into` (made fresh). Refuses with a plain message when the key is missing.
pub fn decrypt_batch(data_dir: &Path, dir: &Path, key_id: &str, into: &Path) -> io::Result<()> {
    let key = key_by_id(&load(data_dir), key_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("this history is encrypted (key {key_id}); import the key with `trail share key import` first"),
        )
    })?;
    let _ = fs::remove_dir_all(into);
    for f in files_of(dir)? {
        let rel = f.strip_prefix(dir).map_err(|_| bad("path outside the batch"))?;
        let dest = into.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = fs::read(&f)?;
        fs::write(&dest, if rel == Path::new("batch.json") || rel == Path::new("signature.json") { bytes } else { open(&key, &bytes)? })?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "test_crypt.rs"]
mod tests;
