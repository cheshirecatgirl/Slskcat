//! Persistent recovery primitives for long-running transfers.
//!
//! This module intentionally stores only client recovery state. It does not
//! store credentials or protocol session data. A recovered transfer must still
//! reconnect through the normal Soulseek backend before it can continue.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::model::{Transfer, TransferId, TransferState};

/// A transfer snapshot safe to persist locally.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferSnapshot {
    pub id: TransferId,
    pub size: u64,
    pub state: TransferState,
    pub destination: PathBuf,
}

impl From<&Transfer> for TransferSnapshot {
    fn from(transfer: &Transfer) -> Self {
        Self {
            id: transfer.id.clone(),
            size: transfer.size,
            state: transfer.state.clone(),
            destination: transfer.destination.clone(),
        }
    }
}

/// Durably write recovery state, replacing whatever was there.
///
/// Writing to a temporary file and renaming means a crash never leaves a
/// half-written file where the previous recovery information used to be.
///
/// The rename alone is not enough, though: it is atomic with respect to the
/// directory entry, not to the bytes behind it. Without flushing first, a
/// power loss can publish a file whose contents never reached the disk — the
/// old state gone and the new state empty, which is the one outcome this
/// module exists to prevent. So the file is synced before the rename and the
/// directory after it.
///
/// # Errors
/// If the snapshots cannot be encoded, or the temporary file cannot be
/// written, flushed, or renamed over the destination.
pub fn save(path: &Path, transfers: &[TransferSnapshot]) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    let data = serde_json::to_vec_pretty(transfers)
        .map_err(|error| io::Error::other(error.to_string()))?;

    let mut file = fs::File::create(&temporary)?;
    file.write_all(&data)?;
    file.sync_all()?;
    drop(file);

    fs::rename(&temporary, path)?;

    // Best effort, and deliberately not fatal: the entry is already renamed,
    // and a filesystem that will not hand out a directory handle is not a
    // reason to report a save that succeeded as having failed.
    if let Some(Ok(directory)) = path.parent().map(fs::File::open) {
        let _ = directory.sync_all();
    }
    Ok(())
}

/// Load previously known transfers. Missing state is treated as a first run.
///
/// # Errors
/// If the file exists but cannot be read or parsed. A missing file is not an
/// error — it is what a first run looks like.
pub fn load(path: &Path) -> io::Result<Vec<TransferSnapshot>> {
    // Asking the error rather than asking `exists()` first: the two-step form
    // is a race, and this is the same shape `settings::load` already uses.
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    serde_json::from_slice(&data).map_err(|error| io::Error::other(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_recovery_file_is_empty() {
        let path =
            std::env::temp_dir().join(format!("slskcat-missing-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);

        assert!(
            load(&path)
                .expect("missing state should be valid")
                .is_empty()
        );
    }

    #[test]
    fn saved_snapshots_round_trip() {
        let path =
            std::env::temp_dir().join(format!("slskcat-recovery-{}.json", std::process::id()));
        let snapshot = TransferSnapshot {
            id: TransferId::new("peer", "album/file.flac"),
            size: 123,
            state: TransferState::Paused {
                transferred: 50,
                total: 123,
            },
            destination: PathBuf::from("Downloads/file.flac"),
        };

        save(&path, std::slice::from_ref(&snapshot)).expect("save should work");
        let loaded = load(&path).expect("load should work");
        let _ = fs::remove_file(&path);

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, snapshot.id);
        assert_eq!(loaded[0].size, snapshot.size);
        assert_eq!(
            loaded[0].state, snapshot.state,
            "the state must survive too"
        );
    }

    #[test]
    fn saving_replaces_the_previous_state_and_leaves_no_scratch_file() {
        let path =
            std::env::temp_dir().join(format!("slskcat-replace-{}.json", std::process::id()));
        let snapshot = |path: &str| TransferSnapshot {
            id: TransferId::new("peer", path),
            size: 1,
            state: TransferState::Completed,
            destination: PathBuf::from(path),
        };

        save(&path, &[snapshot("first.flac")]).expect("first save");
        save(&path, &[snapshot("second.flac")]).expect("second save");
        let loaded = load(&path).expect("load should work");

        let temporary = path.with_extension("tmp");
        let stray = temporary.exists();
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&temporary);

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id.path, "second.flac", "the newer state wins");
        assert!(!stray, "the temporary file must not outlive the rename");
    }

    #[test]
    fn a_corrupt_recovery_file_is_an_error_rather_than_a_panic() {
        // Half a JSON array is what an older, non-durable write could leave
        // behind. Losing the resume points is survivable; crashing at startup
        // because of them is not.
        let path =
            std::env::temp_dir().join(format!("slskcat-corrupt-{}.json", std::process::id()));
        fs::write(&path, b"[{\"id\":{\"username\":\"peer\"").unwrap();

        let result = load(&path);
        let _ = fs::remove_file(&path);
        assert!(result.is_err(), "a truncated file must be reported");
    }
}
