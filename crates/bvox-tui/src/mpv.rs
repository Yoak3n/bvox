//! Thin mpv control layer over the JSON IPC socket.
//!
//! mpv owns decoding, buffering and seeking; the TUI only sends commands and
//! polls playback properties. This keeps the Rust side independent of codecs.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::json;
use tokio::io::BufReader;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::process::{Child, Command};
use tokio::time::timeout;

use bvox_core::api::{REFERER, UA};
use bvox_core::player::{PlayerConfig, PlayerState};

mod ipc;
mod player;

pub use player::MpvPlayer;

pub struct Mpv {
    child: Option<Child>,
    reader: Option<BufReader<OwnedReadHalf>>,
    writer: Option<OwnedWriteHalf>,
    socket: PathBuf,
    request_id: u64,
    /// Stream hints for the next (or current) load; used when spawning mpv.
    config: PlayerConfig,
}

impl Mpv {
    pub fn new() -> Self {
        let socket = std::env::temp_dir().join(format!("bvox-mpv-{}.sock", std::process::id()));
        Self {
            child: None,
            reader: None,
            writer: None,
            socket,
            request_id: 0,
            config: PlayerConfig {
                user_agent: UA.to_string(),
                referer: REFERER.to_string(),
                title: String::new(),
                artist: String::new(),
                cover_url: None,
                start_position: 0.0,
            },
        }
    }

    pub fn is_started(&self) -> bool {
        self.child.is_some()
    }

    pub async fn load(&mut self, url: &str, config: &PlayerConfig) -> Result<()> {
        self.config = config.clone();
        match self.try_load(url).await {
            Ok(()) => Ok(()),
            Err(first_error) => {
                // The mpv process may have exited or its socket may be stale.
                self.reset();
                self.try_load(url)
                    .await
                    .with_context(|| format!("mpv 播放失败（首次尝试: {first_error:#}）"))
            }
        }
    }

    async fn try_load(&mut self, url: &str) -> Result<()> {
        self.ensure_started().await?;
        // `start` applies to the next loadfile only; every load sets it so a
        // resume position cannot leak into the following track. mpv 0.41+
        // rejects JSON numbers for time options over IPC, so send the
        // seconds as a plain time string.
        self.command(json!({
            "command": ["set_property", "start", format!("{}", self.config.start_position.max(0.0))]
        }))
        .await?;
        self.command(json!({"command": ["loadfile", url]})).await?;
        self.command(json!({"command": ["set_property", "pause", false]}))
            .await?;
        Ok(())
    }

    pub async fn toggle_pause(&mut self) -> Result<bool> {
        let paused = self
            .get("pause")
            .await
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let next = !paused;
        self.command(json!({"command": ["set_property", "pause", next]}))
            .await?;
        Ok(next)
    }

    pub async fn seek(&mut self, seconds: f64) -> Result<()> {
        self.command(json!({"command": ["seek", seconds, "relative"]}))
            .await?;
        Ok(())
    }

    pub async fn seek_to(&mut self, seconds: f64) -> Result<()> {
        self.command(json!({"command": ["seek", seconds, "absolute"]}))
            .await?;
        Ok(())
    }

    pub async fn set_volume(&mut self, volume: f64) -> Result<()> {
        self.command(json!({"command": ["set_property", "volume", volume]}))
            .await?;
        Ok(())
    }

    pub async fn poll_state(&mut self) -> Result<Option<PlayerState>> {
        if self.child.is_none() {
            return Ok(None);
        }

        let position = self.number("time-pos").await.unwrap_or(0.0);
        let duration = self.number("duration").await.unwrap_or(0.0);
        let paused = self
            .get("pause")
            .await
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let volume = self.number("volume").await.unwrap_or(100.0);
        let eof = self
            .get("eof-reached")
            .await
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let idle = self
            .get("idle-active")
            .await
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

        Ok(Some(PlayerState {
            position,
            duration,
            paused,
            volume,
            eof,
            idle,
        }))
    }

    /// Stop playback and make sure the mpv process is gone.
    ///
    /// This is the normal exit path. `kill_on_drop` and the Linux parent-death
    /// signal still cover crashes and SIGKILL, where this method cannot run.
    pub async fn quit(&mut self) {
        if self.child.is_some() {
            // mpv usually exits on `quit`, but it may close the socket before
            // replying, so this is best effort with a short timeout.
            let _ = timeout(Duration::from_millis(300), async {
                self.command(json!({"command": ["quit"]})).await
            })
            .await;

            if let Some(mut child) = self.child.take() {
                if timeout(Duration::from_millis(700), child.wait())
                    .await
                    .is_err()
                {
                    let _ = child.start_kill();
                    let _ = timeout(Duration::from_millis(300), child.wait()).await;
                }
            }
        }
        self.child = None;
        self.reader = None;
        self.writer = None;
        let _ = std::fs::remove_file(&self.socket);
    }
}

pub fn harden_command(command: &mut Command) {
    command.kill_on_drop(true);
    #[cfg(target_os = "linux")]
    {
        // SAFETY: `pre_exec` runs in the forked child before exec; the closure
        // only calls `prctl`, which is async-signal-safe.
        unsafe {
            command.pre_exec(|| {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
}

impl Default for Mpv {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        self.reset();
    }
}
