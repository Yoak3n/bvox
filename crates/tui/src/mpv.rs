//! Thin mpv control layer over the JSON IPC socket.
//!
//! mpv owns decoding, buffering and seeking; the TUI only sends commands and
//! polls playback properties. This keeps the Rust side independent of codecs.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, watch};
use tokio::time::timeout;

use bvox_core::api::{REFERER, UA};
use bvox_core::player::{Player, PlayerConfig, PlayerState};

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
        let socket =
            std::env::temp_dir().join(format!("bvox-mpv-{}.sock", std::process::id()));
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

    async fn number(&mut self, property: &str) -> Option<f64> {
        self.get(property).await.and_then(|value| value.as_f64())
    }

    async fn get(&mut self, property: &str) -> Option<Value> {
        self.command(json!({"command": ["get_property", property]}))
            .await
            .ok()
            .flatten()
    }

    async fn ensure_started(&mut self) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }

        let _ = std::fs::remove_file(&self.socket);

        let mut command = Command::new("mpv");
        command
            .arg("--idle=yes")
            .arg("--no-video")
            .arg("--force-window=no")
            .arg("--no-terminal")
            .arg("--really-quiet")
            .arg("--keep-open=no")
            .arg(format!("--referrer={}", self.config.referer))
            .arg(format!("--user-agent={}", self.config.user_agent))
            .arg("--cache=yes")
            .arg("--cache-pause=yes")
            .arg(format!("--input-ipc-server={}", self.socket.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        harden_command(&mut command);
        let child = command.spawn().context("无法启动 mpv，请先安装 mpv")?;
        self.child = Some(child);

        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            match UnixStream::connect(&self.socket).await {
                Ok(stream) => {
                    let (read_half, write_half) = stream.into_split();
                    self.reader = Some(BufReader::new(read_half));
                    self.writer = Some(write_half);
                    return Ok(());
                }
                Err(error) => {
                    if tokio::time::Instant::now() >= deadline {
                        bail!("连接 mpv IPC 失败: {error}");
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
    }

    async fn command(&mut self, mut command: Value) -> Result<Option<Value>> {
        self.request_id += 1;
        let request_id = self.request_id;
        command["request_id"] = json!(request_id);
        let payload = format!("{}\n", command);

        let (Some(reader), Some(writer)) = (self.reader.as_mut(), self.writer.as_mut()) else {
            bail!("mpv 未启动");
        };
        writer
            .write_all(payload.as_bytes())
            .await
            .context("写入 mpv IPC 失败")?;
        writer.flush().await?;

        let read_response = async {
            let mut line = String::new();
            loop {
                line.clear();
                let read = reader.read_line(&mut line).await?;
                if read == 0 {
                    bail!("mpv IPC 已关闭");
                }
                let Ok(message) = serde_json::from_str::<Value>(line.trim()) else {
                    continue;
                };
                if message.get("request_id").and_then(Value::as_u64) != Some(request_id) {
                    continue;
                }
                if message.get("error").and_then(Value::as_str) == Some("success") {
                    return Ok(message.get("data").cloned());
                }
                bail!(
                    "mpv: {}",
                    message
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("未知错误")
                );
            }
        };

        match timeout(Duration::from_secs(3), read_response).await {
            Ok(result) => result,
            Err(_) => bail!("mpv IPC 超时"),
        }
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

    fn reset(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
        self.child = None;
        self.reader = None;
        self.writer = None;
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// Make sure an mpv child process cannot outlive this program.
///
/// `kill_on_drop` covers ordinary exits through `Drop`. On Linux,
/// `PR_SET_PDEATHSIG` additionally makes the kernel kill mpv when the parent
/// dies abruptly (SIGKILL, abort, a panic outside the async unwinding path),
/// so audio never keeps playing after the TUI is gone.
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

/// `Player` implementation over the mpv IPC layer.
///
/// Commands are serialized through an inner mutex; state snapshots are pushed
/// to a `watch` channel every 500 ms so `bvox_core::session::Session` sees the
/// same event stream an ExoPlayer-backed player would produce.
pub struct MpvPlayer {
    inner: Mutex<Mpv>,
    state_tx: watch::Sender<PlayerState>,
}

impl MpvPlayer {
    pub fn new() -> Arc<Self> {
        let (state_tx, _) = watch::channel(PlayerState::default());
        let player = Arc::new(Self {
            inner: Mutex::new(Mpv::new()),
            state_tx,
        });

        // Poll mpv for property snapshots. Holds a weak reference so the task
        // ends once the player is dropped.
        let weak = Arc::downgrade(&player);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let Some(player) = weak.upgrade() else {
                    break;
                };
                let state = player.inner.lock().await.poll_state().await;
                if let Ok(Some(state)) = state {
                    player.state_tx.send_replace(state);
                }
            }
        });

        player
    }
}

#[async_trait]
impl Player for MpvPlayer {
    async fn is_started(&self) -> bool {
        self.inner.lock().await.is_started()
    }

    async fn load(&self, target: &str, config: &PlayerConfig) -> Result<()> {
        self.inner.lock().await.load(target, config).await
    }

    async fn toggle_pause(&self) -> Result<bool> {
        self.inner.lock().await.toggle_pause().await
    }

    async fn seek(&self, seconds: f64) -> Result<()> {
        self.inner.lock().await.seek(seconds).await
    }

    async fn seek_to(&self, seconds: f64) -> Result<()> {
        self.inner.lock().await.seek_to(seconds).await
    }

    async fn set_volume(&self, volume: f64) -> Result<()> {
        self.inner.lock().await.set_volume(volume).await
    }

    async fn quit(&self) {
        self.inner.lock().await.quit().await;
    }

    fn watch(&self) -> watch::Receiver<PlayerState> {
        self.state_tx.subscribe()
    }
}
