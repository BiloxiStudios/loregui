//! `host_store::probe` — round-trip writability check for a local store directory.
//!
//! Writes a small probe file under `.loregui-host/` within the store directory,
//! reads it back, verifies the bytes match, then deletes it. This is the
//! local-FS equivalent of a connectivity / storage-backend validation check.

use serde::{Deserialize, Serialize};

use crate::error::{LoreError, Result};

/// Arguments for [`probe`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProbeArgs {
    /// Path to the local host-store directory to probe.
    pub store_dir: String,
}

/// Arguments accepted by the probe (always empty — probe is a side-effect check).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProbeResult {}

const PROBE_FILE: &str = ".loregui-host/probe.tmp";
const PROBE_PAYLOAD: &[u8] = b"lore-vm host-store probe";

/// Perform a write → read → delete round-trip to verify store-directory writability.
///
/// 1. Creates `.loregui-host/` under the store directory (idempotent).
/// 2. Writes a small probe file.
/// 3. Reads it back and verifies the bytes match.
/// 4. Deletes the probe file (even on mismatch, to avoid leaving garbage).
///
/// This is called by the wizard's "Validate Connectivity" step when the
/// user has chosen the **Local** filesystem backend. For S3/remote backends,
/// the existing `storage::open` + `put`/`get` path is used instead.
pub fn probe(args: ProbeArgs) -> Result<ProbeResult> {
    let store_dir = args.store_dir.trim();
    if store_dir.is_empty() {
        return Err(LoreError::CommandFailed(
            "a local storage path is required".into(),
        ));
    }

    let base = std::path::PathBuf::from(store_dir);
    let probe_dir = base.join(".loregui-host");
    std::fs::create_dir_all(&probe_dir).map_err(|e| {
        LoreError::CommandFailed(format!(
            "could not create probe directory {}: {e}",
            base.display()
        ))
    })?;

    let probe_path = base.join(PROBE_FILE);

    // Write.
    std::fs::write(&probe_path, PROBE_PAYLOAD).map_err(|e| {
        LoreError::CommandFailed(format!(
            "store directory is not writable ({}): {e}",
            base.display()
        ))
    })?;

    // Read back.
    let read_back = std::fs::read(&probe_path).map_err(|e| {
        LoreError::CommandFailed(format!("could not read back probe file: {e}"))
    })?;

    // Always tidy up before asserting, so a mismatch still removes the probe.
    let _ = std::fs::remove_file(&probe_path);

    if read_back != PROBE_PAYLOAD {
        return Err(LoreError::CommandFailed(
            "store directory round-trip mismatch — storage may be unreliable".into(),
        ));
    }

    Ok(ProbeResult {})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_deserialise() {
        let args: ProbeArgs =
            serde_json::from_str(r#"{"store_dir":"/tmp/test-store"}"#).expect("deserialise");
        assert_eq!(args.store_dir, "/tmp/test-store");
    }

    #[test]
    fn args_default_is_empty() {
        let args = ProbeArgs::default();
        assert!(args.store_dir.is_empty());
    }

    #[test]
    fn probe_rejects_empty_path() {
        let err = probe(ProbeArgs {
            store_dir: String::new(),
        })
        .unwrap_err();
        match err {
            LoreError::CommandFailed(msg) => {
                assert!(msg.contains("required"), "got: {msg}");
            }
            other => panic!("expected CommandFailed, got: {other:?}"),
        }
    }

    #[test]
    fn probe_roundtrip_succeeds() {
        let temp = std::env::temp_dir();
        let store = temp.join(format!("lore-vm-probe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&store);
        std::fs::create_dir_all(&store).expect("create store dir");

        let result = probe(ProbeArgs {
            store_dir: store.to_string_lossy().into_owned(),
        })
        .expect("probe should succeed");

        // Probe file should be cleaned up.
        let probe_path = store.join(PROBE_FILE);
        assert!(
            !probe_path.exists(),
            "probe file should be deleted after round-trip"
        );

        let _ = std::fs::remove_dir_all(&store);
        drop(result); // unused but silence warning
    }

    #[test]
    fn probe_fails_on_readonly_dir() {
        let temp = std::env::temp_dir();
        let store = temp.join(format!("lore-vm-probe-readonly-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&store);
        std::fs::create_dir_all(&store).expect("create store dir");

        // Make it read-only (Unix-only; on Windows this is a no-op for dirs).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&store).unwrap().permissions();
            perms.set_mode(0o555);
            std::fs::set_permissions(&store, perms).unwrap();
        }

        let result = probe(ProbeArgs {
            store_dir: store.to_string_lossy().into_owned(),
        });

        // Restore permissions so we can clean up.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&store).unwrap().permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&store, perms);
        }

        let _ = std::fs::remove_dir_all(&store);

        // On Unix this should fail; on Windows it may succeed (no-op).
        #[cfg(unix)]
        assert!(result.is_err(), "probe should fail on read-only dir");
    }

    #[test]
    fn result_serialises() {
        let result = ProbeResult {};
        let json = serde_json::to_string(&result).expect("serialise");
        assert_eq!(json, "{}");
    }

    #[test]
    fn result_deserialises() {
        let json = "{}";
        let result: ProbeResult = serde_json::from_str(json).expect("deserialise");
        drop(result); // struct is empty but deserialises cleanly
    }
}
