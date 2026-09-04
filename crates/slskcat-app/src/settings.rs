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

use slskcat_core::model::{Config, Credentials};
use slskcat_core::proxy::Proxy;

use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, Manager};

/// Identifies our entry in the OS credential store.
const KEYCHAIN_SERVICE: &str = "cat.slsk.client";
const FILE: &str = "settings.json";

/// What the interface reads and writes.
///
/// `password` is never part of the JSON on disk; it is filled in from the
/// credential store on load and routed back to it on save.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub username: String,
    /// Carried between the interface and the credential store, and removed by
    /// `for_disk` before anything is written.
    ///
    /// It has to take part in serialisation. This struct is also the payload
    /// of the `connect` command, so `#[serde(skip)]` here did not just keep
    /// the password off disk: it dropped the typed password on the way into
    /// Rust — every sign-in went out with an empty one — and omitted the field
    /// on the way back, leaving the form bound to `undefined`. Keeping the
    /// secret off disk is the job of `for_disk`, which is the only place that
    /// writes the file.
    pub password: String,
    pub remember_password: bool,
    pub download_dir: PathBuf,
    pub shared_dirs: Vec<PathBuf>,
    pub upload_slots: usize,
    /// Downloads allowed to run at once.
    pub download_slots: usize,
    /// Route the session through a proxy. `None` connects directly.
    ///
    /// The password travels with it rather than going to the credential store:
    /// a proxy password is a property of a network route, not of the person,
    /// and keying it in the store beside the account password would make one
    /// look like the other.
    pub proxy: Option<Proxy>,
    pub search_timeout_secs: u64,
    /// Interface scale as a percentage. 100 is the designed size.
    pub ui_scale: u32,
    /// Rooms to rejoin on sign-in, so the list is there before the server
    /// answers. The server remembers nothing between sessions.
    pub rooms: Vec<String>,
    /// People kept across sessions, in the order they were added.
    ///
    /// The network has no friends list of its own — there is no server message
    /// for one — so this is entirely local, which also means it never tells
    /// anyone who you are interested in.
    pub friends: Vec<String>,
    /// Every account this machine knows about, most recently used first.
    ///
    /// Names only. The credential store is already keyed by username, so the
    /// passwords sit beside each other without any further arrangement — what
    /// was missing was any record that the other accounts exist.
    pub accounts: Vec<String>,
    /// Standing wishes. Persisted because a wish is only useful across
    /// sessions — the point is that it keeps looking after you stop.
    pub wishlist: Vec<String>,
    /// False until the credential store has been reached successfully, so the
    /// interface can explain why a password was not remembered.
    ///
    /// A fact about this run rather than a preference, so `for_disk` clears it
    /// and `load` recomputes it. Serialised for the same reason `password` is:
    /// the interface reads it off the command's reply.
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
            download_slots: config.download_slots,
            proxy: None,
            search_timeout_secs: config.search_timeout.as_secs(),
            ui_scale: 100,
            rooms: Vec::new(),
            friends: Vec::new(),
            accounts: Vec::new(),
            wishlist: Vec::new(),
            keychain_available: false,
        }
    }
}

impl Settings {
    /// Record the current account as the most recently used one.
    ///
    /// Called on save rather than on sign-in so the list only ever names
    /// accounts the user actually configured — a typo in the form that never
    /// reaches the server still gets saved, but a name never typed does not
    /// appear at all.
    fn remember_account(&mut self) {
        if self.username.is_empty() {
            return;
        }
        self.accounts.retain(|name| name != &self.username);
        self.accounts.insert(0, self.username.clone());
    }

    /// The same settings with nothing secret and nothing transient in them,
    /// which is what `settings.json` gets.
    ///
    /// Redacting at the point of writing, rather than by leaving the field out
    /// of the type's serialisation, keeps the guarantee where the file is
    /// actually produced — and leaves the field free to travel over IPC, which
    /// is the whole reason the interface can send a password at all.
    #[must_use]
    fn for_disk(&self) -> Self {
        Self {
            password: String::new(),
            keychain_available: false,
            ..self.clone()
        }
    }

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
            download_slots: self.download_slots,
            proxy: self.proxy.clone(),
            // Filled in by `connect`, which is the only place that knows where
            // this application keeps its files.
            state_file: None,
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
    restrict(&dir);
    Ok(dir.join(FILE))
}

/// Restrict a path to its owner on Unix.
///
/// The settings file names the account and every shared directory — enough to
/// profile what someone has and where it lives. On a shared machine the
/// default mode would leave that readable by any other local user, so both the
/// directory and the file are narrowed to the owner.
///
/// Best effort: a filesystem that cannot represent the mode (a mounted
/// network share, for instance) is not a reason to refuse to save settings.
#[cfg(unix)]
fn restrict(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    let mode = if metadata.is_dir() { 0o700 } else { 0o600 };
    let mut permissions = metadata.permissions();
    if permissions.mode() & 0o777 != mode {
        permissions.set_mode(mode);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

/// Windows and macOS place the config directory under the user profile, which
/// is already owner-scoped, so there is nothing to narrow.
#[cfg(not(unix))]
fn restrict(_path: &std::path::Path) {}

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
        matches!(
            entry.delete_credential(),
            Ok(()) | Err(keyring::Error::NoEntry)
        )
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
    let mut settings = settings.clone();
    settings.remember_account();

    let file = path(app)?;
    let text = serde_json::to_string_pretty(&settings.for_disk())
        .map_err(|error| format!("Could not encode settings: {error}"))?;
    std::fs::write(&file, text)
        .map_err(|error| format!("Could not write {}: {error}", file.display()))?;
    restrict(&file);

    let stored = write_password(
        &settings.username,
        &settings.password,
        settings.remember_password,
    );

    let mut saved = settings;
    saved.keychain_available = stored || !saved.remember_password;
    Ok(saved)
}

/// Switch the stored settings to another known account.
///
/// Everything that is a preference — shared directories, slots, wishlist —
/// belongs to the machine rather than the account and stays as it is. Only the
/// identity changes, and the password comes from the credential store under
/// the new name.
///
/// # Errors
/// If the settings cannot be read or written.
pub fn switch(app: &AppHandle, username: &str) -> Result<Settings, String> {
    let mut settings = load(app)?;
    if settings.username == username {
        return Ok(settings);
    }
    username.clone_into(&mut settings.username);

    let (password, available) = read_password(username);
    settings.password = password;
    settings.keychain_available = available;
    // Nothing stored for this account means it has to be typed again, which
    // is not the same as choosing not to remember it.
    settings.remember_password = !settings.password.is_empty();

    save(app, &settings)
}

/// Forget an account: remove it from the list and delete its password.
///
/// # Errors
/// If the settings cannot be read or written.
pub fn forget(app: &AppHandle, username: &str) -> Result<Settings, String> {
    let mut settings = load(app)?;
    settings.accounts.retain(|name| name != username);

    // Best effort, and deliberately after the list has been updated: an
    // account the user asked to forget must stop being offered even if the
    // credential store cannot be reached to clear the secret.
    let _ = write_password(username, "", false);

    if settings.username == username {
        // The one being forgotten is the current one, so fall back to whatever
        // is left, or to nothing rather than a name that no longer exists.
        settings.username = settings.accounts.first().cloned().unwrap_or_default();
        let (password, available) = read_password(&settings.username);
        settings.password = password;
        settings.keychain_available = available;
        settings.remember_password = !settings.password.is_empty();
    }
    save(app, &settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_signed_in_settings() -> Settings {
        Settings {
            username: "listener".into(),
            password: "hunter2".into(),
            remember_password: true,
            keychain_available: true,
            ..Settings::default()
        }
    }

    #[test]
    fn the_password_is_never_written_to_the_settings_file() {
        let json = serde_json::to_string(&a_signed_in_settings().for_disk()).unwrap();
        assert!(
            !json.contains("hunter2"),
            "the password must not reach disk: {json}"
        );
        assert!(json.contains("listener"), "the username should be stored");
    }

    #[test]
    fn the_interface_payload_carries_the_password_and_the_keychain_verdict() {
        // The counterpart to the test above, and the reason `password` is not
        // `#[serde(skip)]`: this same struct crosses the command boundary, so
        // skipping the field signed in with an empty password and left the
        // sign-in form bound to `undefined`.
        let sent = serde_json::to_value(a_signed_in_settings()).unwrap();
        assert_eq!(sent["password"], "hunter2");
        assert_eq!(sent["keychainAvailable"], true);

        let received: Settings = serde_json::from_value(sent).unwrap();
        assert_eq!(
            received.password, "hunter2",
            "a password typed into the interface must survive the trip into Rust"
        );
        assert_eq!(
            received.to_config().credentials.password,
            "hunter2",
            "and must be the one the session signs in with"
        );
    }

    #[test]
    fn a_settings_file_never_dictates_whether_the_keychain_works() {
        // The file records preferences; whether the store answered is a fact
        // about this run, so it is cleared on the way out and recomputed on
        // the way in.
        let json = serde_json::to_string(&a_signed_in_settings().for_disk()).unwrap();
        let restored: Settings = serde_json::from_str(&json).unwrap();
        assert!(!restored.keychain_available);
        assert!(restored.remember_password, "preferences do survive");
    }

    #[cfg(unix)]
    #[test]
    fn saved_files_are_readable_only_by_their_owner() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("slskcat-perm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        std::fs::write(&file, "{}").unwrap();

        restrict(&dir);
        restrict(&file);

        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode(&file),
            0o600,
            "the settings file must not be group- or world-readable"
        );
        assert_eq!(mode(&dir), 0o700, "nor the directory holding it");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Exercises the real OS credential store, so it needs a session keyring.
    /// Ignored by default — CI has none, and the degradation path is what runs
    /// there. To run it:
    ///
    /// ```text
    /// dbus-run-session -- sh -c '
    ///   echo -n "" | gnome-keyring-daemon --unlock --components=secrets
    ///   cargo test -p slskcat-app -- --ignored --test-threads=1'
    /// ```
    #[test]
    #[ignore = "needs a session keyring; see the doc comment for how to run it"]
    fn the_password_round_trips_through_the_credential_store() {
        let user = format!("slskcat-test-{}", std::process::id());

        // Nothing stored yet: the store is reachable, the entry is absent.
        let (password, available) = read_password(&user);
        assert!(
            available,
            "the credential store should be reachable in this test"
        );
        assert!(password.is_empty(), "no entry has been written yet");

        assert!(
            write_password(&user, "correct horse battery", true),
            "storing should succeed"
        );
        let (password, available) = read_password(&user);
        assert!(available);
        assert_eq!(
            password, "correct horse battery",
            "what went in must come back"
        );

        // Forgetting removes it, and reading then reports reachable-but-empty
        // rather than failing.
        assert!(write_password(&user, "", false), "clearing should succeed");
        let (password, available) = read_password(&user);
        assert!(available);
        assert!(password.is_empty(), "the entry should be gone");

        // Clearing again is not an error: the desired end state already holds.
        assert!(write_password(&user, "", false));
    }

    #[test]
    #[ignore = "needs a session keyring; see the doc comment above"]
    fn declining_to_remember_stores_nothing() {
        let user = format!("slskcat-test-decline-{}", std::process::id());
        write_password(&user, "should-not-persist", false);

        let (password, _) = read_password(&user);
        assert!(
            password.is_empty(),
            "a password must not be stored when not asked for"
        );
    }

    #[test]
    fn an_empty_username_never_touches_the_credential_store() {
        // Guards the early return: an entry keyed on "" would be shared by
        // every user of the machine.
        assert!(!write_password("", "secret", true));
        let (password, available) = read_password("");
        assert!(password.is_empty());
        assert!(!available);
    }

    #[test]
    fn saving_puts_the_current_account_at_the_front_without_duplicating_it() {
        let mut settings = Settings {
            username: "second".into(),
            accounts: vec!["first".into(), "second".into()],
            ..Settings::default()
        };
        settings.remember_account();
        assert_eq!(settings.accounts, vec!["second", "first"]);

        // Again, and it is still one entry — the list is a set with an order,
        // and re-saving the same account must not grow it.
        settings.remember_account();
        assert_eq!(settings.accounts, vec!["second", "first"]);
    }

    #[test]
    fn an_empty_username_is_not_an_account() {
        let mut settings = Settings::default();
        settings.remember_account();
        assert!(
            settings.accounts.is_empty(),
            "a blank name must never reach the switcher"
        );
    }

    #[test]
    fn a_half_typed_proxy_is_treated_as_no_proxy() {
        // Someone part-way through the form must not have every connection
        // fail because the host box is still empty.
        let settings = Settings {
            proxy: Some(Proxy {
                kind: slskcat_core::proxy::ProxyKind::Socks5,
                host: String::new(),
                port: 1080,
                username: String::new(),
                password: String::new(),
            }),
            ..Settings::default()
        };
        assert!(settings.to_config().proxy.is_none());
    }

    #[test]
    fn a_complete_proxy_reaches_the_session() {
        let proxy = Proxy {
            kind: slskcat_core::proxy::ProxyKind::Http,
            host: "127.0.0.1".into(),
            port: 8080,
            username: "user".into(),
            password: "pass".into(),
        };
        let settings = Settings {
            proxy: Some(proxy.clone()),
            ..Settings::default()
        };
        assert_eq!(settings.to_config().proxy, Some(proxy));
    }

    #[test]
    fn a_wishlist_survives_a_round_trip_through_the_settings_file() {
        let settings = Settings {
            wishlist: vec!["boards of canada".into(), "coil — musick".into()],
            ..Settings::default()
        };
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.wishlist, settings.wishlist);
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
        let settings = Settings {
            download_dir: PathBuf::new(),
            ..Settings::default()
        };
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
