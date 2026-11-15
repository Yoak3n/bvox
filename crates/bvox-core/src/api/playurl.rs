//! `/x/player/playurl` DASH response models.

use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct PlayUrlResponse {
    pub(super) code: i64,
    pub(super) message: Option<String>,
    pub(super) data: Option<PlayUrlData>,
}

#[derive(Deserialize)]
pub(super) struct PlayUrlData {
    pub(super) dash: Option<Dash>,
}

#[derive(Deserialize)]
pub(super) struct Dash {
    pub(super) audio: Option<Vec<AudioTrack>>,
    pub(super) dolby: Option<Dolby>,
    pub(super) flac: Option<Flac>,
}

#[derive(Deserialize)]
pub(super) struct Dolby {
    pub(super) audio: Option<Vec<AudioTrack>>,
}

#[derive(Deserialize)]
pub(super) struct Flac {
    pub(super) audio: Option<AudioTrack>,
}

#[derive(Deserialize)]
pub(super) struct AudioTrack {
    pub(super) id: i64,
    #[serde(default)]
    pub(super) bandwidth: i64,
    #[serde(default, rename = "baseUrl")]
    pub(super) base_url: Option<String>,
    #[serde(default, rename = "backupUrl")]
    pub(super) backup_url: Vec<String>,
}
