# bili-music

把 B 站当音乐库的终端播放器。只拉取 DASH 音频流，不下载视频画面；mpv 负责解码和播放，ratatui 负责界面。

实现上参考了两个项目：

- [MareDevi/bilibili-tui](https://github.com/MareDevi/bilibili-tui)：Rust + ratatui + tokio + mpv JSON IPC 的整体架构、WBI 签名和请求头处理。
- [huahuadeliaoliao/RoseSong](https://github.com/huahuadeliaoliao/RoseSong)：纯音频取流的核心思路，`/x/player/playurl` → `data.dash.audio[]`。

## 功能

- 关键词搜索 B 站音乐区（分区 3，WBI 签名 + `buvid3`，无需登录）
- 只播放音频，`mpv --no-video`，不消耗视频带宽
- 搜索结果列表、上下选择、Enter 播放（搜到的歌播完即止，不会顺着结果列表自动连播）
- 暂停/继续、前进/后退 5 秒、音量调节
- 自动连播：多 P 视频按 P 顺序播完；收藏夹/云同步列表播完一条自动切下一条
- `n` / `p` 手动切歌，跨 P 和跨列表都支持
- 音质自动取最高可用 AAC（通常 192K）
- 扫码登录，Cookie 保存到 `~/.config/bili-music/cookie`（0600）
- 导入自己的收藏夹（登录后按 `f` 选择）
- 用 `b` 输入 BV 号或视频链接，直接导入当前列表
- 云同步：导入的 BV 自动加入私有收藏夹「bili-music 云同步」，`y` 拉取，启动/登录后自动加载
- 播放器左侧显示封面（kitty/sixel/iTerm2 原生协议；不支持时回落到 56×32 / 80×46 半块，缩略图 640×360）
- 歌词视图：读取视频字幕，按播放进度高亮当前行
- 本地缓存：音频、封面、歌词、视频信息落盘，重复播放不再下载
- 退出时自动结束 mpv 进程

## 依赖

- Rust（stable，edition 2024）
- `mpv`（播放、缓冲、seek 全部交给它）

Arch：

```bash
sudo pacman -S rust mpv
```

## 构建

```bash
cd ~/Projects/RustProjets/bili-music-tui
cargo build --release
./target/release/bili-music --help
```

安装到用户目录：

```bash
install -Dm755 target/release/bili-music ~/.local/bin/bili-music
```

## 安卓版

同一个 Rust 核心（`crates/core`）通过 UniFFI 暴露给 Kotlin，UI 用 Jetpack
Compose，播放用 Media3/ExoPlayer（含通知栏控制、锁屏、后台播放、音频焦点）。
队列、自动连播、缓存、登录、云同步逻辑全部在 Rust 核心里，桌面与安卓共享。

架构：

```
crates/core    bili-core：api/cache/app/session/player + ffi（UniFFI 导出）
crates/tui     bili-music-tui：ratatui 前端 + mpv IPC（Player trait 实现）
android/       Kotlin 前端：Compose UI + PlaybackService（ExoPlayer 桥）
```

工具链（Linux，全部可装在用户目录，无需 root）：

- JDK 17+（如 Temurin 21）
- Android SDK：cmdline-tools、platforms;android-37.0、build-tools、
  platform-tools、NDK 29.0.14206865（SDK Manager 或 sdkmanager 装）
- `rustup target add x86_64-linux-android aarch64-linux-android`
- `cargo install cargo-ndk`

构建 APK：

```bash
cd android
export JAVA_HOME=~/opt/jdk-21.0.13+9      # 按实际 JDK 路径
export ANDROID_HOME=~/Android/Sdk
./gradlew assembleDebug                    # 自动执行 cargo ndk + uniffi-bindgen
# 产物: app/build/outputs/apk/debug/app-debug.apk
```

模拟器运行：

```bash
sdkmanager "emulator" "system-images;android-35;google_apis;x86_64"
avdmanager create avd -n test -k "system-images;android-35;google_apis;x86_64"
emulator -avd test &
adb install app/build/outputs/apk/debug/app-debug.apk
```



## 使用

```bash
bili-music                    # 启动 TUI
bili-music --search 周杰伦     # 命令行搜索
bili-music --play BV1GJ411x7h7 # 命令行播放（无 TUI，Ctrl-C 停止）
bili-music --fav 4371608344   # 命令行列出公开收藏夹内容
bili-music --probe-cover      # 查看终端支持的图片协议
```

TUI 按键：

| 按键 | 作用 |
| --- | --- |
| `/` 或 `s` | 输入关键词搜索 |
| `b` | 输入 BV 号或视频链接，导入当前列表 |
| `Enter` | 播放选中结果 / 确认导入收藏夹 |
| `space` | 暂停 / 继续 |
| `n` / `p` | 下一首 / 上一首（多 P 视频内先切 P） |
| `←` / `→` | 后退 / 前进 5 秒 |
| `+` / `-` | 音量 +5 / -5 |
| `g` | 扫码登录（弹窗内 `r` 重新生成，`Esc` 关闭） |
| `f` | 打开我的收藏夹（需登录，`j/k` 选择，`Enter` 导入） |
| `y` | 从云同步收藏夹载入列表 |
| `l` | 切换歌词视图 |
| `r` | 重新获取播放地址（流地址过期时用） |
| `q` | 退出 |

登录成功后，Cookie 会自动保存。也可以用环境变量覆盖，程序不会把它写进日志：

```bash
export BILI_COOKIE='SESSDATA=...; bili_jct=...; DedeUserID=...'
```

## 核心原理

### 1. 搜索

`/x/web-interface/search/type` 会被风控返回非 JSON。可用的是：

```text
GET https://api.bilibili.com/x/web-interface/wbi/search/type
    ?search_type=video&keyword=...&page=1&tids=3&wts=...&w_rid=...
```

`tids=3` 把结果限定在音乐分区，避免搜歌时混进其他分区的视频。

签名步骤（`src/api.rs`）：

1. `GET /x/frontend/finger/spi` 取 `data.b_3`，作为 `buvid3` Cookie
2. `GET /x/web-interface/nav` 取 `data.wbi_img.img_url` / `sub_url` 的文件名
3. 用 64 位重排表生成 `mixin_key`
4. 参数按 key 排序，追加 `wts`，URL 编码后拼接
5. `w_rid = md5(query + mixin_key)`

未登录请求 nav 会返回 `code: -101`，但 `wbi_img` 仍然在响应里，可以直接用。

### 2. 视频信息与音频流

```text
GET https://api.bilibili.com/x/web-interface/view?bvid=BV...
```

取 `data.cid`、`data.title`、`data.owner.name`、`data.pic`，以及 `data.pages[]`（多 P 的 cid / 时长 / 分 P 标题）。

```text
GET https://api.bilibili.com/x/player/playurl
    ?bvid=BV...&cid=...&fnval=4048&fnver=0&fourk=1
```

从 `data.dash.audio[]` 里按 `bandwidth` 选最高的一条，取 `baseUrl`（失败时用 `backupUrl[0]`）。返回的是 `.m4s`（fMP4 + AAC），mpv 可以直接解码。

这个接口**不需要 WBI 签名，也不需要登录**。实测返回：

| id | 含义 | 约码率 |
| --- | --- | --- |
| 30216 | 64K AAC | 44 kbps |
| 30232 | 132K AAC | 103 kbps |
| 30280 | 192K AAC | 204 kbps |
| 30250 | Dolby | 需账号权益 |
| 30251 | Hi-Res FLAC | 需账号权益 |

注意事项：

- 流地址有时效（`deadline` 参数，约 2 小时），不要持久化；过期后按 `r` 重新取。
- 请求时带桌面 UA 和 `Referer: https://www.bilibili.com/`。
- 音质只使用普通 AAC 流；Dolby / Hi-Res 只在没有普通流时回退，避免无权益账号拿到不可播地址。

### 3. 扫码登录

```text
GET https://passport.bilibili.com/x/passport-login/web/qrcode/generate
→ data.url, data.qrcode_key

GET https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key=...
→ data.code:
     0      登录成功（响应 Set-Cookie 里带 SESSDATA / bili_jct / DedeUserID）
     86101  未扫码
     86090  已扫码未确认
     86038  二维码过期
```

`src/api.rs` 把 QR URL 生成模块矩阵，`src/ui.rs` 用 `▀/▄/█` 半块字符渲染成黑白二维码（白底、4 模块静区、EC level L）。登录成功后只保留需要的 Cookie 字段并写入 `~/.config/bili-music/cookie`。

### 4. 收藏夹

```text
GET /x/v3/fav/folder/created/list-all?up_mid=<DedeUserID>
→ data.list[] { id, title, media_count }

GET /x/v3/fav/resource/list?media_id=<id>&pn=1&ps=20&platform=web
→ data.medias[] { bvid, title, cover, duration, upper.name, type }
   data.has_more
```

`type == 2` 才是普通视频；收藏夹里的音频、番剧条目会跳过。程序按 20 条一页循环拉取，最多 100 页。`up_mid` 从 Cookie 的 `DedeUserID` 解析，不需要额外请求。

公开收藏夹即使没有登录也能通过 `--fav <media_id>` 列出。

### 5. BV 导入与云同步

TUI 里按 `b`，输入 BV 号或完整视频链接：

1. `view` 拿到 `aid`、标题、UP 主、分 P、封面
2. 插入当前列表顶部（已存在则只选中，不重复）
3. 已登录时自动加入云同步收藏夹

云同步收藏夹：

- 默认名称 `bili-music 云同步`，`privacy=1`（私有）；可用环境变量 `BILI_SYNC_FOLDER` 改名
- 首次导入时先 `GET /x/v3/fav/folder/created/list-all?up_mid=<mid>` 找同名收藏夹；没有则
  `POST /x/v3/fav/folder/add`，正文 `title=...&privacy=1&csrf=<bili_jct>`
- 加入视频：`POST /x/v3/fav/resource/deal`，正文
  `rid=<aid>&type=2&add_media_ids=<folder_id>&del_media_ids=&csrf=<bili_jct>`
  `code=11201` 表示已经收藏过，按成功处理
- 拉取复用收藏夹分页接口；`y` 手动拉取，启动/登录后自动拉取。自动加载只在列表非空时替换界面，空收藏夹不会覆盖默认搜索页
- 收藏夹 id 缓存在 `~/.config/bili-music/sync_folder`。自动加载**不信任缓存 id**，会按当前账号的收藏夹标题重新匹配，避免换账号或旧缓存串到别人的公开收藏夹
- 写操作依赖 Cookie 里的 `bili_jct`，二维码登录会一并保存

实测：创建后无 Cookie 访问该收藏夹返回 `-403`，确认私有；BV 导入后收藏夹 `media_count` 增加，`y` 能拉回。

### 6. 字幕 / 歌词

```text
GET /x/player/v2?bvid=BV...&cid=...
→ data.subtitle.subtitles[] { lan, lan_doc, subtitle_url }

GET <subtitle_url>
→ [{"from": 0.0, "to": 2.5, "content": "..."}, ...]
```

优先选 `zh-CN` 或 `ai-zh`，没有就取第一条带 URL 的中文。播放时按 `position` 找到最后一条 `from <= 当前时间` 的字幕作为当前行。

字幕 JSON 有两种格式，解析器都支持：

```json
[{"from": 0.0, "to": 2.5, "content": "..."}]
```

```json
{"type": "AIsubtitle", "body": [{"from": 0.0, "to": 2.5, "content": "..."}]}
```

注意：

- B 站的字幕列表经常需要登录才返回（尤其是 AI 字幕），很多纯音乐 MV 本身也没有字幕。
- 字幕 URL 偶尔会返回空字符串（风控/节点问题）。这时会重试一次，第二次改用 WBI 签名的 `/x/player/wbi/v2`。
- AI 字幕是对音频的自动转写，音乐视频里可能不准确或跑题；没有字幕时界面会显示“该视频没有字幕/歌词”，不会伪造歌词。

### 7. 封面

视频的 `pic` 字段通常是 `http://i0.hdslb.com/...jpg`。程序把 `http` 升级为 `https`，并追加 B 站图片服务的缩略参数：

```text
原图 3456×2160 / 284 KB  →  @640w_360h.jpg / 约 25 KB
```

下载后用 `image` crate 解码，然后分两档渲染：

- **原生图片协议**：启动时用 `ratatui-image` 查询终端能力。kitty / sixel / iTerm2 终端会用 `StatefulProtocol` + `StatefulImage` 直接画原生图片，按播放器区域缩放，画质是终端像素级，不再受字符网格限制。
- **半块回落**：不支持的终端、或 tmux 默认情况下，用 Lanczos3 缩放后按两个像素一行用 `▀`（上半个字符用前景色、下半个用背景色）渲染：
  - 宽 ≥ 112、高 ≥ 32：80×46 像素 → 80 列 × 23 行
  - 宽 ≥ 96、高 ≥ 28：56×32 像素 → 56 列 × 16 行

两种尺寸都接近 16:9。想确认当前终端走哪条路径：

```bash
bili-music --probe-cover
# Kitty / Sixel / Iterm2 / Halfblocks
```

终端查询在进入 raw mode 前执行，超时 400ms；失败自动回落，不影响启动。控制变量：

```bash
BILI_COVER=blocks   # 强制半块
BILI_COVER=native   # 即使在 tmux 里也尝试原生协议（需要 tmux allow-passthrough）
```

### 8. 本地缓存

缓存放在 `~/.cache/bili-music`（遵循 `XDG_CACHE_HOME`）：

```text
audio/<bvid>_<cid>_<音质id>_<码率>.m4s
covers/<md5(缩略图 URL)>.img
lyrics/<bvid>_<cid>.json
view/<bvid>.json
```

- 音频：播放前先查本地文件；命中时不再请求 `playurl`，mpv 直接读本地 `.m4s`。未命中时立即流播，同时在后台下载一份写入缓存，所以第一次播放会消耗双倍流量，第二次起零下载。
- 下载先写 `.m4s.part`，完成后原子 rename，避免半截文件被当成缓存；同一视频重复触发下载会按 `bvid:cid` 去重。
- 封面按缩略图 URL 的 md5 缓存（URL 指向的内容不变）；歌词按 `bvid + cid` 缓存，只缓存非空结果（登录后字幕可能才出现）；`view` 元数据缓存 24 小时，请求失败时回退到任意年龄的旧数据，保证离线也能播已缓存的歌。
- 总容量由 `BILI_CACHE_MB` 限制（默认 1024），超出后按最近使用时间淘汰到 90%；`BILI_CACHE_MB=0` 完全关闭缓存。
- `BILI_CACHE_DIR` 可以改缓存目录。清理缓存：`rm -rf ~/.cache/bili-music`。
- 按 `r` 重新取播放地址时，如果本地已有缓存会直接重播缓存文件；想强制走网络先删除对应缓存。
- `--play` 无 TUI 模式只读取已有缓存，不在后台写缓存。

### 9. 播放与控制

`src/mpv.rs` 启动一个常驻 mpv：

```text
mpv --idle=yes --no-video --force-window=no \
    --input-ipc-server=/tmp/bili-music-mpv-<pid>.sock \
    --referrer=https://www.bilibili.com/ ...
```

通过 Unix socket 发 JSON：

```json
{"command": ["loadfile", "<audio-url>"], "request_id": 1}
{"command": ["set_property", "pause", true], "request_id": 2}
{"command": ["seek", 5, "relative"], "request_id": 3}
{"command": ["get_property", "time-pos"], "request_id": 4}
```

每 500ms 轮询 `time-pos`、`duration`、`pause`、`volume`、`idle-active`。曲目结束用「播放过（position > 0.5s）且 mpv 进入 idle」判定；mpv 到 EOF 后会卸载文件，单看 `eof-reached` 不可靠。

### 10. DNS：跳过黑洞 CDN 地址

`passport.bilibili.com` 会解析出多个 IPv4，其中偶尔有 TCP 能建连但 TLS 永远不回包的节点。reqwest 不会像 curl 那样做 Happy Eyeballs，会卡在第一个地址直到 20 秒超时。

`src/api.rs` 的自定义 resolver 做了三件事：

1. 优先 IPv4
2. 对多个候选地址并行发 HTTPS HEAD 探测（使用同一个 reqwest/rustls 栈，能过滤“TCP 能连但 TLS/HTTP 不回包”的节点），把第一个可用地址排到最前
3. 结果缓存 5 分钟

这是扫码登录从“偶尔卡 20 秒”变成稳定的关键。

## 代码结构

```text
src/
├── api.rs   # WBI、搜索、二维码登录、收藏夹/云同步、字幕、封面、DASH 音频流、DNS
├── cache.rs # 磁盘缓存：音频、封面、歌词、view，TTL 与 LRU 淘汰
├── mpv.rs   # mpv 进程与 JSON IPC（load / pause / seek / volume / poll）
├── app.rs   # 状态机：列表、选中项、分 P、登录/收藏夹弹窗、歌词游标
├── cover.rs # 半块回落的封面解码与 56×32 RGB 网格
├── ui.rs    # ratatui 布局：结果表、播放器、封面、歌词、二维码/收藏夹弹窗
└── main.rs  # 入口、事件循环、后台任务（搜索/播放/登录/收藏夹/封面/字幕/缓存下载）
```

数据流：

```text
键盘线程 ──Msg::Key──▶ 主循环 ──Action──▶ 搜索/播放/登录/收藏夹任务
                          ▲                    │
                          │                 API + mpv
                          └──Msg::*（结果、播放状态、封面、歌词、扫码状态）
```

## 已知限制

- 没有歌单管理、历史记录、每日推荐；收藏夹只读导入。
- 云同步只支持“导入时加入”和“拉取列表”；从云收藏夹移除、改名请到 B 站网页端操作。
- 只有 `b` 导入的 BV 会写入云同步；搜索后直接播放不会自动收藏。
- 歌词来自 B 站字幕，不接第三方歌词库；纯音乐 MV 经常没有字幕。
- 不显示视频画面、不接弹幕。
- 只支持普通视频（`/x/player/playurl`），番剧/课程/直播未接入。
- 收藏夹最多导入 100 页（2000 条）。
- 首次播放未命中缓存时会同时流播和后台下载，消耗双倍流量；缓存默认上限 1 GiB，按 LRU 淘汰。
- 搜索结果和云同步列表不缓存，仍以网络结果为准。
- 搜索受 B 站风控影响，偶尔首次请求较慢或返回 412；重试即可。
- `--search` 输出管道给 `head` 时可能因 broken pipe 报 panic，属 Rust 默认行为，不影响 TUI。

## 声明

仅供个人学习和本地播放使用。请遵守 B 站用户协议，不要用于批量下载、二次分发或绕过付费内容。程序不包含任何 DRM 绕过逻辑。
