//! Cache file system helpers: paths, timestamps and atomic writes.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};

use super::Cache;

impl Cache {
    pub(super) fn cover_path(&self, url: &str) -> PathBuf {
        let digest = format!("{:x}", md5::compute(url.as_bytes()));
        self.root.join("covers").join(format!("{digest}.img"))
    }

    pub(super) fn lyrics_path(&self, bvid: &str, cid: i64) -> PathBuf {
        self.root.join("lyrics").join(format!("{bvid}_{cid}.json"))
    }

    pub(super) fn view_path(&self, bvid: &str) -> PathBuf {
        self.root.join("view").join(format!("{bvid}.json"))
    }
}
pub(super) fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub(super) fn touch(path: &Path) {
    if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("缓存路径缺少父目录")?;
    std::fs::create_dir_all(parent).context("创建缓存目录失败")?;
    let suffix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let tmp = path.with_extension(format!("tmp.{}.{suffix}", std::process::id()));
    std::fs::write(&tmp, bytes).context("写入缓存临时文件失败")?;
    std::fs::rename(&tmp, path).context("提交缓存文件失败")
}
