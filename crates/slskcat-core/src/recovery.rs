//! Persistent recovery primitives for long-running transfers.
//!
//! This module intentionally stores only client recovery state. It does not
//! store credentials or protocol session data. A recovered transfer must still
//! reconnect through the normal Soulseek backend before it can continue.

use serde::{Deserialize, Serialize};
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

/// Atomically write recovery state.
///
/// The temporary file prevents a power loss from leaving a half-written JSON
/// file that destroys the previous recovery information.
///
/// # Errors
/// If the snapshots cannot be encoded, or the temporary file cannot be written
/// or renamed over the destination.
pub fn save(path: &Path, transfers: &[TransferSnapshot]) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    let data = serde_json::to_vec_pretty(transfers)
        .map_err(|error| io::Error::other(error.to_string()))?;
    fs::write(&temporary, data)?;
    fs::rename(temporary, path)
}

/// Load previously known transfers. Missing state is treated as a first run.
///
/// # Errors
/// If the file exists but cannot be read or parsed. A missing file is not an
/// error — it is what a first run looks like.
pub fn load(path: &Path) -> io::Result<Vec<TransferSnapshot>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read(path)?;
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
    }
}
