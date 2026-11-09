//! Signed batches. Each device has one ed25519 key; `push` signs the batch, `fetch` and `rebuild` check it.
//!
//! The private seed lives in `<Tracewise home>/device.key` (mode 0600) and is never copied or pushed. `signature.json` in a batch holds the
//! public key and the signature over a digest of every other file of the batch (names and bytes). The first key seen for a device is
//! pinned in `device-keys.json` (trust on first use); a different key for a pinned device, a bad signature, or an unsigned batch from a
//! pinned device is refused. Batches from before signing existed are accepted until the device signs once.
//! shortcut: the seed is a protected file, not the Keychain; move it there when a Keychain binding is added.
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SIGNATURE_FILE: &str = "signature.json";
const PINS_FILE: &str = "device-keys.json";

#[derive(Serialize, Deserialize)]
struct Signed {
    key: String,
    sig: String,
}

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

fn key_path() -> PathBuf {
    crate::get_trail_home().join("device.key")
}

/// This device's key, created on first use.
fn signing_key() -> io::Result<SigningKey> {
    let path = key_path();
    if let Some(seed) = fs::read_to_string(&path).ok().and_then(|t| hex_to::<32>(t.trim())) {
        return Ok(SigningKey::from_bytes(&seed));
    }
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|e| io::Error::other(format!("no random numbers: {e}")))?;
    tracewise_platform::private::create_dir_all(crate::get_trail_home())?;
    tracewise_platform::private::write(&path, hex(&seed))?;
    Ok(SigningKey::from_bytes(&seed))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn hex_to<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != N * 2 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// blake3 over the relative name and bytes of every file of the batch except the signature, in name order.
fn digest(dir: &Path) -> io::Result<[u8; 32]> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> io::Result<()> {
        for e in fs::read_dir(dir)? {
            let e = e?;
            if e.file_type()?.is_dir() {
                walk(root, &e.path(), out)?;
            } else if let Ok(rel) = e.path().strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
        Ok(())
    }
    let mut names = Vec::new();
    walk(dir, dir, &mut names)?;
    names.retain(|n| n != SIGNATURE_FILE);
    names.sort();
    let mut h = blake3::Hasher::new();
    for n in names {
        let bytes = fs::read(dir.join(&n))?;
        h.update(&(n.len() as u64).to_le_bytes()).update(n.as_bytes()).update(&(bytes.len() as u64).to_le_bytes()).update(&bytes);
    }
    Ok(*h.finalize().as_bytes())
}

/// The pinned public keys, device id → hex.
pub fn pinned(data_dir: &Path) -> BTreeMap<String, String> {
    fs::read_to_string(data_dir.join(PINS_FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_pins(data_dir: &Path, pins: &BTreeMap<String, String>) -> io::Result<()> {
    tracewise_platform::private::write(data_dir.join(PINS_FILE), serde_json::to_string_pretty(pins).unwrap_or_default())
}

/// Forgets a device's pinned key (after a deliberate key change). Returns whether there was one.
pub fn forget(data_dir: &Path, device: &str) -> io::Result<bool> {
    let mut pins = pinned(data_dir);
    let had = pins.remove(device).is_some();
    if had {
        save_pins(data_dir, &pins)?;
    }
    Ok(had)
}

/// Signs the batch folder as `device` and pins this device's own key.
pub fn sign_batch(data_dir: &Path, dir: &Path, device: &str) -> io::Result<()> {
    let key = signing_key()?;
    let public = hex(key.verifying_key().as_bytes());
    let mut pins = pinned(data_dir);
    match pins.get(device) {
        Some(p) if *p != public => {
            return Err(bad(format!(
                "this device's signing key differs from the one pinned for '{device}'; run `trail share keys --forget {device}` if you replaced it on purpose"
            )))
        }
        Some(_) => {}
        None => {
            pins.insert(device.to_string(), public.clone());
            save_pins(data_dir, &pins)?;
        }
    }
    let sig = hex(&key.sign(&digest(dir)?).to_bytes());
    fs::write(dir.join(SIGNATURE_FILE), serde_json::to_string(&Signed { key: public, sig }).unwrap_or_default())
}

/// Checks one local batch folder (device read from its `batch.json`). Pins the key of a device seen for the first time.
pub fn verify_batch(data_dir: &Path, dir: &Path) -> io::Result<()> {
    let device = fs::read_to_string(dir.join("batch.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<super::BatchMeta>(&t).ok())
        .map(|m| m.device)
        .ok_or_else(|| bad(format!("{}: no readable batch.json", dir.display())))?;
    let mut pins = pinned(data_dir);
    let Ok(text) = fs::read_to_string(dir.join(SIGNATURE_FILE)) else {
        return if pins.contains_key(&device) {
            Err(bad(format!("batch {} from '{device}' is unsigned, but that device signs its batches", dir.display())))
        } else {
            Ok(()) // from before signing existed
        };
    };
    let s: Signed = serde_json::from_str(&text).map_err(|e| bad(format!("{}: bad signature file: {e}", dir.display())))?;
    let key_bytes = hex_to::<32>(&s.key).ok_or_else(|| bad("bad public key".into()))?;
    let sig_bytes = hex_to::<64>(&s.sig).ok_or_else(|| bad("bad signature".into()))?;
    let key = VerifyingKey::from_bytes(&key_bytes).map_err(|e| bad(format!("bad public key: {e}")))?;
    key.verify(&digest(dir)?, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| bad(format!("batch {} from '{device}' does not match its signature; it was changed after signing", dir.display())))?;
    match pins.get(&device) {
        Some(p) if *p != s.key => Err(bad(format!(
            "batch {} is signed with a different key than the one pinned for '{device}'; refused. If the device really got a new key, run `trail share keys --forget {device}`",
            dir.display()
        ))),
        Some(_) => Ok(()),
        None => {
            pins.insert(device, s.key);
            save_pins(data_dir, &pins)
        }
    }
}

#[cfg(test)]
#[path = "test_sign.rs"]
mod tests;
