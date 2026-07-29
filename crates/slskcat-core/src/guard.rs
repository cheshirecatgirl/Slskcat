//! Share-path safety.
//!
//! The defining failure of consumer P2P clients was not a protocol flaw — it
//! was people sharing a folder they did not mean to. Whole home directories
//! went onto the network: tax returns, private keys, medical letters. The
//! clients let it happen because a share was just a path, and nothing ever
//! looked at what the path was.
//!
//! So every directory is classified before it can be offered to the network,
//! and the classification is enforced in the core rather than the interface.
//! A bug in the UI, or anything else driving the core, still cannot put
//! `~/.ssh` on a public network.

use std::path::{Component, Path, PathBuf};

/// What sharing a directory would expose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareRisk {
    /// Nothing unusual.
    Safe,
    /// Holds personal files by convention. Allowed, but the user should be
    /// asked whether they meant it.
    Sensitive(&'static str),
    /// Would expose credentials, system files, or everything at once. Never
    /// shared, whatever the caller asks for.
    Refused(&'static str),
}

impl ShareRisk {
    /// Whether the core will offer this directory to peers.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        !matches!(self, Self::Refused(_))
    }

    /// The explanation, if there is one to give.
    #[must_use]
    pub const fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Safe => None,
            Self::Sensitive(reason) | Self::Refused(reason) => Some(*reason),
        }
    }
}

/// Directories that would expose the system or another application's data.
#[cfg(unix)]
const SYSTEM_ROOTS: &[&str] = &[
    "/bin", "/boot", "/dev", "/etc", "/lib", "/lib32", "/lib64", "/opt", "/proc", "/root", "/sbin",
    "/sys", "/usr", "/var",
];

#[cfg(not(unix))]
const SYSTEM_ROOTS: &[&str] = &[
    r"C:\Windows",
    r"C:\Program Files",
    r"C:\Program Files (x86)",
    r"C:\ProgramData",
];

/// Folders that conventionally hold personal documents rather than media.
const PERSONAL: &[&str] = &["documents", "desktop", "pictures", "photos", "videos"];

/// The user's home directory, if the environment names one.
fn home() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(key).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}

/// Classify a directory that is about to be shared.
///
/// The check is on the path as written; it does not follow symlinks, so a link
/// into a refused location is not caught here. That is a known limit rather
/// than an oversight — resolving every share would need filesystem access this
/// function deliberately avoids, and the destructive cases are all reachable
/// by path.
#[must_use]
pub fn assess_share_path(path: &Path) -> ShareRisk {
    assess_against(path, home().as_deref())
}

/// The classification proper, with the home directory passed in.
///
/// Separated so tests can pin a home directory without mutating the process
/// environment — which `unsafe` in this workspace forbids, and which would
/// make the tests order-dependent besides.
fn assess_against(path: &Path, home: Option<&Path>) -> ShareRisk {
    // A relative path is ambiguous about what it would actually expose.
    if !path.is_absolute() {
        return ShareRisk::Refused("Only absolute paths can be shared.");
    }

    // The filesystem root, or a drive letter on its own.
    if path.parent().is_none() {
        return ShareRisk::Refused("Sharing the whole filesystem is never allowed.");
    }

    // Any hidden component: ~/.ssh, ~/.gnupg, ~/.config and everything under
    // them. This is where keys, tokens and session cookies live.
    if path.components().any(|component| {
        matches!(component, Component::Normal(name)
            if name.to_string_lossy().starts_with('.'))
    }) {
        return ShareRisk::Refused("Hidden folders can hold keys and credentials.");
    }

    for root in SYSTEM_ROOTS {
        if path.starts_with(root) {
            return ShareRisk::Refused("System folders cannot be shared.");
        }
    }

    if let Some(home) = home {
        if path == home {
            return ShareRisk::Refused("Sharing your whole home folder would expose everything in it.");
        }
        // One level below home, matched by name: ~/Documents and friends.
        if path.parent() == Some(home)
            && path
                .file_name()
                .is_some_and(|name| PERSONAL.contains(&name.to_string_lossy().to_lowercase().as_str()))
        {
            return ShareRisk::Sensitive("This folder usually holds personal files, not music.");
        }
    }

    ShareRisk::Safe
}

/// Split a set of directories into those that may be shared and those refused,
/// each refusal paired with its reason.
#[must_use]
pub fn partition(paths: Vec<PathBuf>) -> (Vec<PathBuf>, Vec<(PathBuf, &'static str)>) {
    let mut allowed = Vec::with_capacity(paths.len());
    let mut refused = Vec::new();
    for path in paths {
        match assess_share_path(&path) {
            ShareRisk::Refused(reason) => refused.push((path, reason)),
            ShareRisk::Safe | ShareRisk::Sensitive(_) => allowed.push(path),
        }
    }
    (allowed, refused)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Classify against a fixed home directory.
    fn check(path: &str) -> ShareRisk {
        assess_against(Path::new(path), Some(Path::new("/home/listener")))
    }

    #[test]
    fn ordinary_music_folders_are_safe() {
        assert_eq!(check("/home/listener/Music"), ShareRisk::Safe);
        assert_eq!(check("/home/listener/Music/FLAC/Ambient"), ShareRisk::Safe);
        assert_eq!(check("/media/archive/rips"), ShareRisk::Safe);
    }

    #[test]
    fn the_whole_home_folder_is_refused() {
        let risk = check("/home/listener");
        assert!(!risk.is_allowed(), "sharing all of $HOME is the classic leak");
        assert!(risk.reason().is_some());
    }

    #[test]
    fn hidden_folders_are_refused_anywhere_in_the_path() {
        for path in [
            "/home/listener/.ssh",
            "/home/listener/.gnupg",
            "/home/listener/.config/slskcat",
            "/home/listener/Music/.private/stash",
        ] {
            assert!(
                !check(path).is_allowed(),
                "{path} holds credentials or was hidden on purpose"
            );
        }
    }

    #[test]
    fn the_filesystem_root_is_refused() {
        assert!(!check("/").is_allowed());
    }

    #[cfg(unix)]
    #[test]
    fn system_folders_are_refused() {
        for path in ["/etc", "/etc/ssl/private", "/usr/share", "/var/log", "/root"] {
            assert!(!check(path).is_allowed(), "{path} is not the user's to share");
        }
    }

    #[test]
    fn relative_paths_are_refused_as_ambiguous() {
        assert!(!check("Music").is_allowed());
        assert!(!check("../../etc").is_allowed());
    }

    #[test]
    fn personal_folders_are_allowed_but_flagged() {
        let risk = check("/home/listener/Documents");
        assert!(risk.is_allowed(), "the user may genuinely mean it");
        assert!(matches!(risk, ShareRisk::Sensitive(_)), "but they should be asked");

        // Case should not be a way around the check.
        assert!(matches!(check("/home/listener/desktop"), ShareRisk::Sensitive(_)));
        // A music folder nested under one of them is not itself the
        // conventional personal folder.
        assert_eq!(check("/home/listener/Documents/Music"), ShareRisk::Safe);
    }

    #[test]
    fn partition_keeps_the_allowed_and_explains_the_rest() {
        // Uses the real home, so the paths chosen here are refused or safe
        // regardless of where that points.
        let (allowed, refused) = partition(vec![
            PathBuf::from("/media/archive/rips"),
            PathBuf::from("/media/archive/.hidden"),
            PathBuf::from("/etc"),
        ]);
        assert_eq!(allowed, vec![PathBuf::from("/media/archive/rips")]);
        assert_eq!(refused.len(), 2);
        assert!(refused.iter().all(|(_, reason)| !reason.is_empty()));
    }
}
