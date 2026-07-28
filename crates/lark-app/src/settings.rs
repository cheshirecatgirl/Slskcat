//! Persisted preferences.
//!
//! Two stores, deliberately:
//!
//! - everything ordinary goes to `settings.json` in the platform config
//!   directory;
//! - the password goes to the OS credential store — Keychain, Credential
//!   Manager, or the D-Bus Secret Service — and never to disk in the clear.
//!
//! The credential store can be genuinely unavailable (a headless Linux box
//! with no session keyring is the usual case), so every call through it
//! degrades to "no remembered password" rather than failing the operation.
//! Losing a saved password is an inconvenience; refusing to start is not
//! an acceptable response to it.

use lark_core::model::{Config, Credentials};

use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, Manager};

/// Identifies our entry in the OS credential store.
const KEYCHAIN_SERVICE: &str = "dev.lark.client";
const FILE: &str = "settings.json";

/// What the interface reads and writes.
///
/// `password` is never part of the JSON on disk; it is filled in from the
/// credential store on load and routed back to it on save.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub username: String,
    /// Not serialised. Carried in memory between the interface and the
    /// credential store.
    #[serde(skip)]
    pub password: String,
    pub remember_password: bool,
    pub download_dir: PathBuf,
    pub shared_dirs: Vec<PathBuf>,
    pub upload_slots: usize,
    pub search_timeout_secs: u64,
    /// False until the credential store has been reached successfully, so the
    /// interface can explain why a password was not remembered.
    #[serde(skip)]
    pub keychain_available: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let config = Config::default();
        Self {
            username: String::new(),
            password: String::new(),
            remember_password: false,
            download_dir: config.download_dir,
            shared_dirs: Vec::new(),
            upload_slots: config.upload_slots,
            search_timeout_secs: config.search_timeout.as_secs(),
            keychain_available: false,
        }
    }
}

impl Settings {
    /// The core configuration these settings describe.
    #[must_use]
    pub fn to_config(&self) -> Config {
        Config {
            credentials: Credentials {
                username: self.username.clone(),
                password: self.password.clone(),
            },
            download_dir: self.download_dir.clone(),
            shared_dirs: self.shared_dirs.clone(),
            upload_slots: self.upload_slots,
            search_timeout: Duration::from_secs(self.search_timeout_secs),
        }
        // The core repairs anything blank; doing it here too means the
        // interface sees the same values the session will actually use.
        .normalized()
    }
}

/// Where `settings.json` lives, creating the directory if needed.
fn path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("No config directory available: {error}"))?;
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("Could not create {}: {error}", dir.display()))?;
    Ok(dir.join(FILE))
}

/// Read the credential store, treating every failure as "nothing saved".
fn read_password(username: &str) -> (String, bool) {
    if username.is_empty() {
        return (String::new(), false);
    }
    match keyring::Entry::new(KEYCHAIN_SERVICE, username) {
        Ok(entry) => match entry.get_password() {
            Ok(password) => (password, true),
            // A missing entry still proves the store is reachable, which is
            // what `keychain_available` reports.
            Err(keyring::Error::NoEntry) => (String::new(), true),
            Err(_) => (String::new(), false),
        },
        Err(_) => (String::new(), false),
    }
}

/// Write or clear the credential store. Returns whether it could be reached.
fn write_password(username: &str, password: &str, remember: bool) -> bool {
    if username.is_empty() {
        return false;
    }
    let Ok(entry) = keyring::Entry::new(KEYCHAIN_SERVICE, username) else {
        return false;
    };
    if remember && !password.is_empty() {
        entry.set_password(password).is_ok()
    } else {
        // Forgetting is the point here, so an entry that was never there is
        // the desired end state rather than an error.
        matches!(entry.delete_credential(), Ok(()) | Err(keyring::Error::NoEntry))
    }
}

/// Load settings, falling back to defaults when nothing is stored yet.
///
/// A corrupt or unreadable file is reported through `Err` so the interface can
/// say so, rather than silently resetting preferences the user set.
///
/// # Errors
/// If the config directory cannot be determined or created, or the settings
/// file exists but cannot be read or parsed. A missing file is not an error.
pub fn load(app: &AppHandle) -> Result<Settings, String> {
    let file = path(app)?;
    let mut settings = match std::fs::read_to_string(&file) {
        Ok(text) => serde_json::from_str::<Settings>(&text)
            .map_err(|error| format!("{} is not readable: {error}", file.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Settings::default(),
        Err(error) => return Err(format!("Could not read {}: {error}", file.display())),
    };

    if settings.remember_password {
        let (password, available) = read_password(&settings.username);
        settings.password = password;
        settings.keychain_available = available;
    } else {
        settings.keychain_available = true;
    }
    Ok(settings)
}

/// Persist settings, routing the password to the credential store.
///
/// Returns the settings as stored, with `keychain_available` reflecting
/// whether the password could actually be kept.
///
/// # Errors
/// If the config directory cannot be determined or created, or the settings
/// file cannot be encoded or written. A credential store that cannot be
/// reached is reported through `keychain_available`, not as an error.
pub fn save(app: &AppHandle, settings: &Settings) -> Result<Settings, String> {
    let file = path(app)?;
    let text = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Could not encode settings: {error}"))?;
    std::fs::write(&file, text)
        .map_err(|error| format!("Could not write {}: {error}", file.display()))?;

    let stored = write_password(&settings.username, &settings.password, settings.remember_password);

    let mut saved = settings.clone();
    saved.keychain_available = stored || !settings.remember_password;
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_password_is_never_written_to_the_settings_file() {
        let settings = Settings {
            username: "listener".into(),
            password: "hunter2".into(),
            remember_password: true,
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(!json.contains("hunter2"), "the password must not reach disk: {json}");
        assert!(json.contains("listener"), "the username should be stored");
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        // A settings file written by an older build must still load.
        let settings: Settings = serde_json::from_str(r#"{"username":"listener"}"#).unwrap();
        assert_eq!(settings.username, "listener");
        assert_eq!(settings.upload_slots, Settings::default().upload_slots);
        assert!(!settings.download_dir.as_os_str().is_empty());
    }

    #[test]
    fn a_blank_download_directory_is_repaired_on_the_way_to_a_config() {
        let settings = Settings { download_dir: PathBuf::new(), ..Settings::default() };
        assert!(
            !settings.to_config().download_dir.as_os_str().is_empty(),
            "downloads must never be pointed at an empty path"
        );
    }

    #[test]
    fn settings_become_the_config_the_session_uses() {
        let settings = Settings {
            username: "listener".into(),
            password: "secret".into(),
            upload_slots: 5,
            search_timeout_secs: 20,
            ..Settings::default()
        };
        let config = settings.to_config();
        assert_eq!(config.credentials.username, "listener");
        assert_eq!(config.credentials.password, "secret");
        assert_eq!(config.upload_slots, 5);
        assert_eq!(config.search_timeout, Duration::from_secs(20));
    }
}
