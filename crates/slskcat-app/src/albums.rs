//! The library as releases rather than as files.
//!
//! A folder listing answers "what is on disk". This answers "what do I have",
//! which is a different question and the one a music player is for. It comes
//! from the tags inside the files, falling back to the folder when they are
//! missing — a library with no tags at all still groups into the releases it
//! was downloaded as, because that is what the folders already are.
//!
//! Reading tags means opening every file, so this is deliberately a separate
//! command from the plain listing: nothing pays for it until the album view is
//! actually asked for.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lofty::file::TaggedFileExt;
use lofty::prelude::{Accessor, ItemKey};
use lofty::probe::Probe;

use crate::library::LocalRoot;

/// One track, with whatever its tags say about it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub path: String,
    /// The file name, which is what shows when there is no title tag.
    pub name: String,
    pub size: u64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub number: Option<u32>,
    pub disc: Option<u32>,
    pub seconds: Option<u64>,
}

/// A release, as the grid shows it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    /// Stable across a rescan, so an open album stays open.
    pub key: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    /// Absolute path to a cover, extracted or found beside the files.
    pub cover: Option<String>,
    /// Whether these came from the download folder rather than a shared one.
    pub downloads: bool,
    pub tracks: Vec<Track>,
}

/// Names a picture beside the music, in the order worth preferring.
const COVER_NAMES: [&str; 6] = [
    "cover.jpg",
    "cover.png",
    "folder.jpg",
    "folder.png",
    "front.jpg",
    "front.png",
];

/// Extensions worth opening. Anything else is not going to have audio tags.
const TAGGED: [&str; 11] = [
    "flac", "mp3", "m4a", "aac", "ogg", "oga", "opus", "wav", "aiff", "aif", "wv",
];

/// Group the files under these roots into releases, reading tags as it goes.
///
/// `covers` is where extracted artwork is written. It has to be a directory the
/// asset protocol is allowed to read, or the pictures will not load.
#[must_use]
pub fn gather(roots: &[LocalRoot], covers: &Path) -> Vec<Album> {
    let _ = std::fs::create_dir_all(covers);
    let mut grouped: HashMap<String, Album> = HashMap::new();
    // One picture per folder, whether it came from a tag or from disk.
    let mut seen_folders: HashMap<PathBuf, Option<String>> = HashMap::new();

    for root in roots {
        for file in &root.files {
            let path = Path::new(&file.path);
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_lowercase)
                .unwrap_or_default();
            if !TAGGED.contains(&extension.as_str()) {
                continue;
            }

            let tags = read(path);
            let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();

            // The release this belongs to. Tags first; a folder name is the
            // fallback because a download folder is already one release deep.
            let album = tags.album.clone().unwrap_or_else(|| {
                folder.file_name().map_or_else(
                    || root.path.clone(),
                    |name| name.to_string_lossy().into_owned(),
                )
            });
            let artist = tags
                .album_artist
                .clone()
                .or_else(|| tags.artist.clone())
                .unwrap_or_else(|| "Unknown artist".to_owned());

            let key = format!("{artist}\u{0}{album}");
            let entry = grouped.entry(key.clone()).or_insert_with(|| Album {
                key,
                title: album,
                artist,
                year: tags.year,
                cover: None,
                downloads: root.downloads,
                tracks: Vec::new(),
            });
            if entry.year.is_none() {
                entry.year = tags.year;
            }
            if entry.cover.is_none() {
                let found = seen_folders
                    .entry(folder.clone())
                    .or_insert_with(|| cover_for(path, &folder, covers))
                    .clone();
                entry.cover = found;
            }
            entry.tracks.push(Track {
                path: file.path.clone(),
                name: file.name.clone(),
                size: file.size,
                title: tags.title,
                artist: tags.artist,
                number: tags.number,
                disc: tags.disc,
                seconds: tags.seconds,
            });
        }
    }

    let mut albums: Vec<Album> = grouped.into_values().collect();
    for album in &mut albums {
        album.tracks.sort_by(|a, b| {
            (a.disc.unwrap_or(1), a.number.unwrap_or(0), &a.name).cmp(&(
                b.disc.unwrap_or(1),
                b.number.unwrap_or(0),
                &b.name,
            ))
        });
    }
    albums.sort_by(|a, b| {
        (
            a.artist.to_lowercase(),
            a.year.unwrap_or(0),
            a.title.to_lowercase(),
        )
            .cmp(&(
                b.artist.to_lowercase(),
                b.year.unwrap_or(0),
                b.title.to_lowercase(),
            ))
    });
    albums
}

/// What one file's tags say. Every field is optional because every field is.
#[derive(Default)]
struct Tags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    album_artist: Option<String>,
    number: Option<u32>,
    disc: Option<u32>,
    year: Option<u32>,
    seconds: Option<u64>,
}

fn read(path: &Path) -> Tags {
    let Ok(file) = Probe::open(path).and_then(Probe::read) else {
        return Tags::default();
    };
    let mut tags = Tags {
        seconds: Some(
            lofty::file::AudioFile::properties(&file)
                .duration()
                .as_secs(),
        ),
        ..Tags::default()
    };
    let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) else {
        return tags;
    };
    tags.title = tag
        .title()
        .map(|t| t.trim().to_owned())
        .filter(|s| non_empty(s));
    tags.artist = tag
        .artist()
        .map(|t| t.trim().to_owned())
        .filter(|s| non_empty(s));
    tags.album = tag
        .album()
        .map(|t| t.trim().to_owned())
        .filter(|s| non_empty(s));
    tags.album_artist = tag
        .get_string(ItemKey::AlbumArtist)
        .map(|t| t.trim().to_owned())
        .filter(|s| non_empty(s));
    tags.number = tag.track();
    tags.disc = tag.disk();
    // `date` covers both `RecordingDate` and a plain `Year`, which is the
    // difference between a Vorbis comment and an ID3 frame.
    tags.year = tag.date().map(|date| u32::from(date.year));
    tags
}

fn non_empty(s: &str) -> bool {
    !s.is_empty()
}

/// A cover for this folder: the picture inside the first file that has one,
/// or a picture sitting beside the music.
fn cover_for(file: &Path, folder: &Path, covers: &Path) -> Option<String> {
    if let Some(extracted) = embedded(file, covers) {
        return Some(extracted);
    }
    for name in COVER_NAMES {
        let beside = folder.join(name);
        if beside.is_file() {
            return Some(beside.to_string_lossy().into_owned());
        }
    }
    None
}

fn embedded(file: &Path, covers: &Path) -> Option<String> {
    let tagged = Probe::open(file).ok()?.read().ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let picture = tag.pictures().first()?;
    let bytes = picture.data();
    if bytes.is_empty() {
        return None;
    }

    // Written out rather than sent inline: a grid of covers as data URIs is
    // megabytes of JSON across the bridge every time the view opens, and the
    // webview can read a file it has been given the scope for.
    let extension = match picture.mime_type().map(lofty::picture::MimeType::as_str) {
        Some("image/png") => "png",
        Some("image/webp") => "webp",
        _ => "jpg",
    };
    let out = covers.join(format!("{:016x}.{extension}", fingerprint(bytes)));
    if !out.exists() && std::fs::write(&out, bytes).is_err() {
        return None;
    }
    Some(out.to_string_lossy().into_owned())
}

/// FNV-1a over the picture, so the same artwork is written once however many
/// files carry it. Not a security hash and not asked to be one.
fn fingerprint(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::LocalFile;

    fn file(path: &str, folder: &str, name: &str) -> LocalFile {
        LocalFile {
            path: path.to_owned(),
            folder: folder.to_owned(),
            name: name.to_owned(),
            size: 10,
        }
    }

    #[test]
    fn untagged_files_group_by_the_folder_they_sit_in() {
        let dir = std::env::temp_dir().join(format!("slskcat-albums-{}", std::process::id()));
        let album = dir.join("Some Release");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&album).unwrap();
        // Not real audio, so every tag read fails: the fallback is the point.
        std::fs::write(album.join("a.flac"), b"not audio").unwrap();
        std::fs::write(album.join("b.flac"), b"not audio").unwrap();

        let root = LocalRoot {
            path: dir.to_string_lossy().into_owned(),
            downloads: true,
            files: vec![
                file(
                    album.join("a.flac").to_str().unwrap(),
                    "Some Release",
                    "a.flac",
                ),
                file(
                    album.join("b.flac").to_str().unwrap(),
                    "Some Release",
                    "b.flac",
                ),
            ],
        };
        let albums = gather(&[root], &dir.join("covers"));
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(albums.len(), 1, "one folder is one release");
        assert_eq!(albums[0].title, "Some Release");
        assert_eq!(albums[0].artist, "Unknown artist");
        assert_eq!(albums[0].tracks.len(), 2);
    }

    #[test]
    fn files_that_cannot_hold_tags_are_left_out() {
        let root = LocalRoot {
            path: "/x".to_owned(),
            downloads: false,
            files: vec![file("/x/cover.jpg", "", "cover.jpg")],
        };
        assert!(gather(&[root], Path::new("/tmp/slskcat-none")).is_empty());
    }

    #[test]
    fn the_same_picture_is_written_once_however_many_files_carry_it() {
        assert_eq!(fingerprint(b"abc"), fingerprint(b"abc"));
        assert_ne!(fingerprint(b"abc"), fingerprint(b"abd"));
    }
}
