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
pub fn save(path: &Path, transfers: &[TransferSnapshot]) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    let data = serde_json::to_vec_pretty(transfers)
        .map_err(|error| io::Error::other(error.to_string()))?;
    fs::write(&temporary, data)?;
    fs::rename(temporary, path)
}

/// Load previously known transfers. Missing state is treated as a first run.
pub fn load(path: &Path) -> io::Result<Vec<TransferSnapshot>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read(path)?;
    serde_json::from_slice(&data)
        .map_err(|error| io::Error::other(error.to_string()))
}
