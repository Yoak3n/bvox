## Context

当前每次播放都会重新请求 `view` 元数据、`playurl` 流地址、封面图片和字幕，重复播放同一首歌时这些请求全部重来，封面/歌词尤其浪费。目标是加一层磁盘缓存：音频按 `bvid + cid` 落盘，封面按 URL 哈希，歌词按 `bvid + cid`，`view` 元数据按 `bvid` 缓存 24 小时。

已确认的选择：
- 缓存范围：音频 + 封面 + 歌词 + 元数据
- 首次播放（未命中）：立即用远程 URL 流播，同时后台再下载一份写入缓存（首次会消耗双倍流量）

## 实现方案

### 1. 新模块 `src/cache.rs`

缓存根目录，默认 `$XDG_CACHE_HOME/bili-music`（回退 `~/.cache/bili-music`），可用 `BILI_CACHE_DIR` 覆盖：

```text
<root>/audio/{bvid}_{cid}_{quality_id}_{bandwidth}.m4s
<root>/covers/{md5(url)}.img
<root>/lyrics/{bvid}_{cid}.json
<root>/view/{bvid}.json
```

`Cache` 提供：
- `find_audio(bvid, cid) -> Option<CachedAudio>`：扫描文件名前缀，取最新一份并 touch mtime（LRU）
- `audio_target(...)` / `commit_audio(...)`：先写 `.m4s.part`，成功后原子 rename；失败删除 part
- `get/put_cover`、`get/put_lyrics`、`get_view`（带 24h TTL）+ `get_stale_view`（网络失败时兜底）
- `begin_audio_download/end_audio_download`：`HashSet` 记录 in-flight，避免同一首歌重复后台下载
- 容量管理：`BILI_CACHE_MB`（默认 1024，`0` 表示完全禁用缓存）；写完后按 mtime 从旧到新淘汰到上限的 90%，跳过正在播放/刚写入的文件和 `.part`
- 启动时清理超过 1 天的残留 `.part`
- JSON 写入同样走临时文件 + rename，读取失败/损坏时返回 `None` 并删除该文件

### 2. `src/api.rs`

- `ApiClient` 持有 `Cache`。
- 新增 `AudioSource` 枚举：
  - `Cached { path, quality_id, bandwidth }` —— 不再请求 `playurl`
  - `Remote(AudioStream)` —— 保留现有 URL 行为
- 新增 `audio_source(bvid, cid)`：先查本地音频，未命中再走现有 `audio_stream`。
- 新增 `download_audio(bvid, cid, &AudioStream, on_progress)`：用 reqwest 的 `Response::chunk()` 流式写入 part 文件；`Content-Length` 作为进度总量；单请求超时放宽到 600s（客户端默认 20s 是全请求超时，不适合下载）；进度回调节流到约 5 次/秒。
- `view`：先读缓存（24h），未命中请求并写入；请求失败时回退到任意年龄的旧缓存，保证离线/风控时已缓存歌曲仍可播放。`VideoDetail`/`VideoPage` 增加 `Serialize, Deserialize`。
- `lyrics`：先读缓存（只缓存非空结果，避免登录后字幕可用却读到空结果）；新增 `LyricLine: Serialize, Deserialize`。
- `image_bytes`：先按 `cover_thumbnail_url` 后的 URL 哈希查封面缓存，命中直接返回；下载成功后写入。
- `playurl` 的 URL 不缓存（约 2 小时过期）。

### 3. `src/main.rs`

- `Msg` 增加 `CacheProgress { key, downloaded, total }`、`CacheDone { key, ok }`。
- `spawn_play` 改用 `api.audio_source()`：
  - 命中缓存：直接 `mpv loadfile <本地路径>`
  - 未命中：立即加载远程 URL 并发送 `Playing`，随后 `spawn_cache_download(...)` 在后台下载
  - `PlayingInfo` 增加 `cached: bool`
- 新增 `spawn_cache_download`：调用 `download_audio`，发送进度/完成消息；失败静默忽略，不影响播放。
- `--play` 无 TUI 模式也复用 `audio_source`，缓存命中时播放本地文件。

### 4. `src/app.rs` / `src/ui.rs`

- `App` 增加 `playback_key`、`cache_progress: Option<(u64, Option<u64>)>`、`cached_now: bool`。
- `on_playing` 记录 key 与是否命中缓存；`on_cache_progress/on_cache_done` 只响应当前播放 key。
- `ui.rs` 的 `title_line` 在音质标签后追加：缓存命中/完成显示 `· 缓存`，下载中显示 `· 缓存 42%`。

### 5. 其他

- `Cargo.toml`：tokio 增加 `fs` feature（`io-util` 已有）。
- `main.rs` 的 `print_help` 和 `README.md` 增加缓存目录、`BILI_CACHE_DIR`、`BILI_CACHE_MB` 说明，代码结构补 `cache.rs`。
- 不支持直接清空缓存的 UI；文档说明 `rm -rf ~/.cache/bili-music`。

## 验证

1. `cargo test`：新增 `cache.rs` 单测（音频文件名生成/解析往返、损坏文件名拒绝、新鲜度判断）。
2. `cargo clippy --all-targets` + `cargo build`。
3. `BILI_CACHE_DIR=/tmp/bili-cache-test cargo run`，播放同一首歌两次：
   - 第一次状态栏显示 `缓存 xx%`，结束后 `<tmp>/audio/*.m4s`、`covers/*.img`、`lyrics/*.json`、`view/*.json` 均出现
   - 第二次播放音质旁显示 `缓存`，且不再请求 playurl
4. `BILI_CACHE_MB=0` 启动确认不写任何缓存；用较小上限（如 `BILI_CACHE_MB=20`）播放多首确认旧文件被淘汰。
5. 缓存后用 `--play` 验证本地文件可被 mpv 正常 seek；断网重播已缓存歌曲，验证 `view` 回退与本地音频仍可播放。