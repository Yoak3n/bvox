//! Domain models shared by the API, cache and state layers.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoPage {
    pub cid: i64,
    pub duration: i64,
    pub part: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoDetail {
    pub bvid: String,
    pub aid: i64,
    pub cid: i64,
    pub title: String,
    pub author: String,
    pub duration: i64,
    pub pages: Vec<VideoPage>,
    pub pic: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, uniffi::Record)]
pub struct LyricLine {
    pub from: f64,
    /// Reserved for karaoke-style highlighting; the UI currently keys off `from`.
    #[allow(dead_code)]
    pub to: f64,
    pub text: String,
}
