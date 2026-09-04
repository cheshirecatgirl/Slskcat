//! What is already in the download folder.
//!
//! Search results carry no hash — the protocol has no field for one — so
//! "already have this" cannot be answered by content. Name and size is what
//! every other client uses and what this uses: an exact filename, extension
//! included, and an exact byte count. Two different files agreeing on both is
//! possible and is a miss this accepts; the alternative is reading every byte
//! of the folder on every search, to answer a question the network never asked.

use std::path::Path;

/// One finished file, as the interface needs to recognise it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Downloaded {
    /// File name, lowercased, extension included.
    pub name: String,
    pub size: u64,
}

/// A file in one of this machine's own folders, for the library view.
///
/// Carries the full path as well as the name, because unlike the "already
/// downloaded" index this is a list to open things from, not a set to compare
/// against.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFile {
    /// Absolute path, which is what a player is handed.
    pub path: String,
    /// The folder it sits in, relative to the root it was found under.
    pub folder: String,
    pub name: String,
    pub size: u64,
}

/// One of this machine's folders and what is in it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalRoot {
    pub path: String,
    /// Whether this is where downloads land, as opposed to a shared folder.
    pub downloads: bool,
    pub files: Vec<LocalFile>,
}

/// Everything under `dir`, with paths, for the library view.
#[must_use]
pub fn list(dir: &Path, downloads: bool) -> LocalRoot {
    let mut files = Vec::new();
    collect(dir, dir, 0, &mut files);
    files.sort_by(|a, b| (&a.folder, &a.name).cmp(&(&b.folder, &b.name)));
    LocalRoot {
        path: dir.to_string_lossy().into_owned(),
        downloads,
        files,
    }
}

fn collect(root: &Path, dir: &Path, depth: usize, found: &mut Vec<LocalFile>) {
    if depth > MAX_DEPTH || found.len() >= MAX_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if found.len() >= MAX_FILES {
            return;
        }
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect(root, &path, depth + 1, found);
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Ok(meta) = entry.metadata() else { continue };
        let folder = path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .map(|rest| rest.to_string_lossy().into_owned())
            .unwrap_or_default();
        found.push(LocalFile {
            path: path.to_string_lossy().into_owned(),
            folder,
            name: name.to_owned(),
            size: meta.len(),
        });
    }
}

/// How deep to walk. Downloads land in the folder or one level under it, in a
/// directory named for the release; deeper is somebody's own filing.
const MAX_DEPTH: usize = 4;
/// A ceiling on a folder that has been in use for years.
const MAX_FILES: usize = 100_000;

/// Every file under `dir`, as name and size.
///
/// Unreadable directories are skipped rather than failing the walk: a download
/// folder with one bad subdirectory should still recognise everything else.
#[must_use]
pub fn scan(dir: &Path) -> Vec<Downloaded> {
    let mut found = Vec::new();
    walk(dir, 0, &mut found);
    found
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<Downloaded>) {
    if depth > MAX_DEPTH || found.len() >= MAX_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if found.len() >= MAX_FILES {
            return;
        }
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            walk(&path, depth + 1, found);
        } else if kind.is_file() {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let name = name.to_lowercase();
            // A part-file is not a finished download, and matching one would
            // claim a file the user does not yet have. Read as an extension
            // rather than a suffix so a file genuinely called `x.incomplete`
            // is judged the same way whatever its case.
            let unfinished = Path::new(&name)
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| matches!(ext, "part" | "incomplete"));
            if unfinished {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            found.push(Downloaded {
                name,
                size: meta.len(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("slskcat-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn files_are_found_by_lowercased_name_and_real_size() {
        let dir = temp("scan");
        std::fs::write(dir.join("Track One.FLAC"), b"1234567890").unwrap();
        std::fs::create_dir_all(dir.join("Album")).unwrap();
        std::fs::write(dir.join("Album/Track Two.flac"), b"12345").unwrap();

        let mut found = scan(&dir);
        found.sort_by(|a, b| a.name.cmp(&b.name));
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "track one.flac");
        assert_eq!(found[0].size, 10);
        assert_eq!(found[1].name, "track two.flac", "and one level down");
        assert_eq!(found[1].size, 5);
    }

    #[test]
    fn a_part_file_is_not_a_download() {
        let dir = temp("part");
        std::fs::write(dir.join("Half.flac.part"), b"12").unwrap();
        let found = scan(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(found.is_empty(), "an unfinished file must not count as had");
    }

    #[test]
    fn the_library_lists_paths_and_the_folder_each_sits_in() {
        let dir = temp("list");
        std::fs::create_dir_all(dir.join("Album")).unwrap();
        std::fs::write(dir.join("loose.flac"), b"12345").unwrap();
        std::fs::write(dir.join("Album/track.flac"), b"12").unwrap();

        let root = list(&dir, true);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(root.downloads);
        assert_eq!(root.files.len(), 2);
        // Sorted by folder then name, so the root's own files come first.
        assert_eq!(root.files[0].folder, "");
        assert_eq!(root.files[0].name, "loose.flac");
        assert_eq!(root.files[1].folder, "Album");
        assert_eq!(root.files[1].size, 2);
        assert!(
            root.files[1].path.ends_with("track.flac"),
            "the full path is what a player is handed"
        );
    }

    #[test]
    fn a_missing_folder_is_empty_rather_than_an_error() {
        assert!(scan(std::path::Path::new("/nowhere/at/all")).is_empty());
    }
}
