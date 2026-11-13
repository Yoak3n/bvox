//! Offline cache for the account's favorite folders and their contents.
//!
//! The favorite API is the only source of the library list, so a cold start
//! without network would otherwise leave the app with no entry point at all.
//! These small JSON files keep the last successful response per account so
//! the folder list and previously opened playlists stay browsable offline.
//! Playback itself still needs the audio in the media cache.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::api::{FavoriteFolder, VideoItem};

const CACHE_DIR_NAME: &str = "playlist_cache";

#[derive(Serialize, Deserialize)]
struct CachedFolders {
    folders: Vec<FavoriteFolder>,
}

#[derive(Serialize, Deserialize)]
struct CachedItems {
    items: Vec<VideoItem>,
}

fn cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(CACHE_DIR_NAME)
}

fn folders_path(data_dir: &Path, mid: i64) -> PathBuf {
    cache_dir(data_dir).join(format!("{mid}.json"))
}

fn items_path(data_dir: &Path, mid: i64, media_id: i64) -> PathBuf {
    cache_dir(data_dir).join(format!("{mid}-{media_id}.json"))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("创建歌单缓存目录失败")?;
    }
    let bytes = serde_json::to_vec(value).context("序列化歌单缓存失败")?;
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, bytes).context("写入歌单缓存失败")?;
    std::fs::rename(&tmp, path).context("提交歌单缓存失败")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save_folders(data_dir: &Path, mid: i64, folders: &[FavoriteFolder]) -> Result<()> {
    write_json(
        &folders_path(data_dir, mid),
        &CachedFolders {
            folders: folders.to_vec(),
        },
    )
}

pub fn load_folders(data_dir: &Path, mid: i64) -> Option<Vec<FavoriteFolder>> {
    read_json::<CachedFolders>(&folders_path(data_dir, mid)).map(|cached| cached.folders)
}

pub fn save_items(
    data_dir: &Path,
    mid: i64,
    media_id: i64,
    items: &[VideoItem],
) -> Result<()> {
    write_json(
        &items_path(data_dir, mid, media_id),
        &CachedItems {
            items: items.to_vec(),
        },
    )
}

pub fn load_items(data_dir: &Path, mid: i64, media_id: i64) -> Option<Vec<VideoItem>> {
    read_json::<CachedItems>(&items_path(data_dir, mid, media_id)).map(|cached| cached.items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "bvox-playlists-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ))
    }

    fn folder(id: i64, title: &str) -> FavoriteFolder {
        FavoriteFolder {
            id,
            title: title.to_string(),
            count: 0,
        }
    }

    fn item(bvid: &str) -> VideoItem {
        VideoItem {
            aid: 0,
            bvid: bvid.to_string(),
            title: bvid.to_string(),
            author: String::new(),
            duration: "0:00".to_string(),
            pic: None,
        }
    }

    #[test]
    fn folders_and_items_roundtrip_per_account() {
        let root = temp_root();
        save_folders(&root, 42, &[folder(1, "A")]).unwrap();
        save_folders(&root, 43, &[folder(2, "B")]).unwrap();

        assert_eq!(load_folders(&root, 42).unwrap()[0].title, "A");
        assert_eq!(load_folders(&root, 43).unwrap()[0].title, "B");
        assert!(load_folders(&root, 44).is_none());

        save_items(&root, 42, 7, &[item("BV1")]).unwrap();
        assert_eq!(load_items(&root, 42, 7).unwrap()[0].bvid, "BV1");
        assert!(load_items(&root, 43, 7).is_none());

        let _ = std::fs::remove_dir_all(root);
    }
}
