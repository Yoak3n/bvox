# Bvox

<img align="right" width="96" src="docs/icon.png" alt="Bvox 图标">

把 B 站当音乐库的桌面与安卓播放器。名字取自 BV（B 站视频号）+ vox（声音）。只拉取 DASH 音频流，不下载视频画面：桌面端用 ratatui + mpv，安卓端用 Compose + Media3/ExoPlayer，两端共享同一个 Rust 核心。

实现上参考了两个项目：

- [MareDevi/bilibili-tui](https://github.com/MareDevi/bilibili-tui)：Rust + ratatui + tokio + mpv JSON IPC 的整体架构、WBI 签名和请求头处理。
- [huahuadeliaoliao/RoseSong](https://github.com/huahuadeliaoliao/RoseSong)：纯音频取流的核心思路，`/x/player/playurl` → `data.dash.audio[]`。

## 功能

- 关键词搜索 B 站音乐区（分区 3，WBI 签名 + `buvid3`，无需登录）
- 只播放音频，`mpv --no-video`，不消耗视频带宽
- 搜索结果列表、上下选择、Enter 播放；翻到列表末尾自动加载下一页（安卓端滚动到底部自动加载）
- 暂停/继续、前进/后退 5 秒、音量调节
- 播放模式（`m`）：顺序播放 / 列表循环 / 单曲循环 / 随机播放；模式本地持久化，重启沿用
- 自动连播：多 P 视频按 P 顺序播完；列表播完后的行为由播放模式决定
- 启动恢复：自动恢复上次打开的队列/歌单、当前歌曲和播放进度；启动后保持暂停，按播放或重试时从原位置继续
- `n` / `p` 手动切歌，跨 P 和跨列表都支持；随机模式下上一首会回退到随机历史
- 音质偏好可切换（`o`）：`自动` 模式下大会员优先 Hi-Res FLAC / Dolby，普通账号最高 192K AAC；高音质不可用时自动回退 AAC
- 扫码登录，Cookie 保存到 `~/.config/bvox/cookie`（0600）
- 歌单：登录后主界面是「我的歌单」（B 站收藏夹），`Enter` 打开，`n` 新建、`r` 改名、`d` 删除
- 歌单管理：搜索/列表里的歌按 `a` 加入任意歌单；歌单内按 `d` 移除；新建歌单默认私有
- 隐藏歌单：可逐个隐藏/恢复收藏夹；隐藏列表按账号保存在本地，`H` 临时查看隐藏项
- 用 `b` 输入 BV 号或视频链接，直接导入当前列表；若没有打开歌单，会弹出「加入歌单」列表，可选择现有收藏夹或新建，取消则只留在本地列表
- 播放器左侧显示封面（kitty/sixel/iTerm2 原生协议；不支持时回落到 56×32 / 80×46 半块，缩略图 640×360）
- 歌词视图：读取视频字幕，按播放进度高亮当前行
- 本地缓存：音频、封面、歌词、视频信息落盘，重复播放不再下载；可在自动缓存 / 仅小文件 / 关闭之间切换
- 离线歌单：收藏夹列表和打开过的歌单曲目会缓存到本机，断网时仍可浏览并播放已缓存音频
- 已缓存歌曲（安卓）：「我的」页独立入口，不依赖收藏夹列表是否加载成功
- 退出时自动结束 mpv 进程

## 依赖

- Rust（stable，edition 2024）
- `mpv`（播放、缓冲、seek 全部交给它）

Arch：

```bash
sudo pacman -S rust mpv
```

## 构建

也可以直接下载 [Releases](https://github.com/Yoak3n/bvox/releases/latest) 里的
`bvox-v<版本>-x86_64-linux.tar.gz`（静态链接，任意 x86_64 Linux 发行版），
解压后运行 `./bvox`（需要系统安装 `mpv`）。

从源码构建：

```bash
git clone https://github.com/Yoak3n/bvox
cd bvox
cargo build --release
./target/release/bvox --help
```

安装到用户目录：

```bash
install -Dm755 target/release/bvox ~/.local/bin/bvox
```

## 安卓版

安装包见 [Releases](https://github.com/Yoak3n/bvox/releases/latest)（正式签名的 `bvox-v<版本>-release.apk`）。

同一个 Rust 核心（`crates/core`）通过 UniFFI 暴露给 Kotlin，UI 用 Jetpack
Compose，播放用 Media3/ExoPlayer（含通知栏控制、锁屏、后台播放、音频焦点）。
歌单库、歌曲队列、自动连播、缓存、登录逻辑全部在 Rust 核心里，
桌面与安卓共享。安卓端登录后首页是「我的歌单」，点开进入歌曲列表；
行尾的 `+` 加入歌单，歌单视图下的 `×` 从歌单移除，新增/改名/删除歌单
都在列表页操作。歌单卡片右侧 `⋮` 菜单可隐藏/恢复歌单，标题栏眼睛按钮
临时显示隐藏项。右上角的齿轮可切换音质偏好，播放页右上角可切换播放模式
（顺序 / 列表循环 / 单曲循环 / 随机）；大会员账号旁会显示「大会员」标识。
收藏夹列表和打开过的歌单会缓存在应用私有目录，断网启动时直接显示缓存列表，
已缓存的音频仍可播放。「我的」页还有独立的「已缓存歌曲」入口，完全绕开收藏夹
列表，适合断网或收藏夹加载失败时直接听本地缓存。

架构：

```
crates/core    bvox-core：api/cache/app/session/player + ffi（UniFFI 导出）
crates/tui     bvox-tui：ratatui 前端 + mpv IPC（Player trait 实现）
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
# 产物: app/build/outputs/apk/debug/bvox-v0.1.1-debug.apk
```

模拟器运行：

```bash
sdkmanager "emulator" "system-images;android-35;google_apis;x86_64"
avdmanager create avd -n test -k "system-images;android-35;google_apis;x86_64"
emulator -avd test &
adb install app/build/outputs/apk/debug/bvox-v0.1.1-debug.apk
```

## CI

GitHub Actions（`.github/workflows/ci.yml`）在 push / PR 时跑 `cargo test`，
构建 TUI 和安卓 APK 并上传为 Artifact：

- TUI：`bvox-v<版本>-x86_64-linux.tar.gz`（静态 musl 构建）
- 安卓：`bvox-v<版本>-debug.apk` / `bvox-v<版本>-release.apk`，不再是 `app-release.apk`
- PR 只构建 debug APK；push 到 `main` 或打 `v*` tag 时同时构建 release
- 打 `v*` tag 会自动创建 GitHub Release，附上签名 APK、TUI 压缩包和 `SHA256SUMS.txt`
- 在仓库 Secrets 配置 `ANDROID_KEYSTORE_BASE64`（keystore 文件的 base64）、
  `ANDROID_KEYSTORE_PASSWORD`、`ANDROID_KEY_ALIAS`、`ANDROID_KEY_PASSWORD`
  后 release 使用正式签名；打 tag 时若缺少这些 secret 会直接失败，避免发布未签名包



## 使用

```bash
bvox                    # 启动 TUI
bvox --search 周杰伦     # 命令行搜索
bvox --play BV1GJ411x7h7 # 命令行播放（无 TUI，Ctrl-C 停止）
bvox --fav 4371608344   # 命令行列出公开收藏夹内容
bvox --probe-cover      # 查看终端支持的图片协议
```

TUI 按键：

| 按键 | 作用 |
| --- | --- |
| `/` 或 `s` | 输入关键词搜索 |
| `b` | 输入 BV 号或视频链接，导入当前列表（打开歌单时加入，否则弹出加入歌单列表） |
| `Enter` | 播放选中结果 / 打开选中的歌单 |
| `space` | 暂停 / 继续 |
| `n` / `p` | 下一首 / 上一首（多 P 视频内先切 P） |
| `←` / `→` | 后退 / 前进 5 秒 |
| `+` / `-` | 音量 +5 / -5 |
| `g` | 扫码登录（弹窗内 `r` 重新生成，`Esc` 关闭） |
| `o` | 切换音质偏好：自动 / AAC / Dolby / Hi-Res |
| `m` | 切换播放模式：顺序 / 列表循环 / 单曲循环 / 随机 |
| `f` | 打开/刷新歌单列表（需登录） |
| `n` / `r` / `d` | 歌单列表内：新建 / 重命名 / 删除歌单 |
| `h` | 歌单列表内：隐藏 / 恢复选中的歌单 |
| `H` | 显示 / 收起隐藏歌单 |
| `a` | 把选中的歌加入歌单（弹出歌单选择） |
| `d` | 歌单内：把选中的歌从歌单移除（需确认） |
| `Esc` | 歌单列表 ↔ 歌曲列表切换 |
| `l` | 切换歌词视图 |
| `L` | 重新拉取当前歌曲的歌词（绕过并清除歌词缓存） |
| `c` | 切换缓存策略（自动缓存 / 仅小文件 / 关闭） |
| `C` | 清理本地缓存（正在播放的缓存文件会保留） |
| `r` | 歌曲列表内：重新获取播放地址（流地址过期时用） |
| `q` | 退出 |

登录成功后，Cookie 会自动保存。也可以用环境变量覆盖，程序不会把它写进日志：

```bash
export BVOX_COOKIE='SESSDATA=...; bili_jct=...; DedeUserID=...'
```

## 核心原理

### 1. 搜索

`/x/web-interface/search/type` 会被风控返回非 JSON。可用的是：

```text
GET https://api.bilibili.com/x/web-interface/wbi/search/type
    ?search_type=video&keyword=...&page=1&tids=3&wts=...&w_rid=...
```

`tids=3` 把结果限定在音乐分区，避免搜歌时混进其他分区的视频。

签名步骤（`crates/core/src/api.rs`）：

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
- 音质选择：`自动`（默认）在大会员账号下优先 Hi-Res FLAC（30251）→ Dolby（30250）→ 最高码率 AAC，普通账号只用 AAC；也可显式选 `AAC` / `Dolby` / `Hi-Res`。
- 高音质流在交给播放器前会做一次 2 字节 Range 预检；CDN 返回 403（无权益/链接失效）时自动回退到 AAC，而不是把死链接交给 mpv/ExoPlayer。
- 偏好保存在 `~/.config/bvox/audio_quality`（安卓在应用 data 目录），下次启动沿用。

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

`crates/core/src/api.rs` 把 QR URL 生成模块矩阵，`crates/tui/src/ui.rs` 用 `▀/▄/█` 半块字符渲染成黑白二维码（白底、4 模块静区、EC level L）。登录成功后只保留需要的 Cookie 字段并写入 `~/.config/bvox/cookie`。

### 4. 收藏夹 / 歌单

```text
GET /x/v3/fav/folder/created/list-all?up_mid=<DedeUserID>
→ data.list[] { id, title, media_count }

GET /x/v3/fav/resource/list?media_id=<id>&pn=1&ps=20&platform=web
→ data.medias[] { id, bvid, title, cover, duration, upper.name, type, attr }
   data.has_more
```

`type == 2` 才是普通视频，`attr != 0` 是失效条目，两者都会跳过。`data.medias[].id` 就是视频的 `aid`，程序把它存进 `VideoItem.aid`，增删歌单都靠它。程序按 20 条一页循环拉取，最多 100 页。`up_mid` 从 Cookie 的 `DedeUserID` 解析，不需要额外请求。

公开收藏夹即使没有登录也能通过 `--fav <media_id>` 列出。

登录后 B 站收藏夹就是 App 的歌单：

```text
新建：POST /x/v3/fav/folder/add
      title=<标题>&intro=<简介>&privacy=1&csrf=<bili_jct>   → data.id
改名：POST /x/v3/fav/folder/edit
      media_id=<id>&title=<新标题>&csrf=<bili_jct>
删除：POST /x/v3/fav/folder/del
      media_ids=<id>&csrf=<bili_jct>
加歌：POST /x/v3/fav/resource/deal
      rid=<aid>&type=2&add_media_ids=<folder_id>&del_media_ids=&csrf=<bili_jct>
移除：POST /x/v3/fav/resource/deal
      rid=<aid>&type=2&add_media_ids=&del_media_ids=<folder_id>&csrf=<bili_jct>
```

- 新建歌单默认私有（`privacy=1`），不会出现在公开主页；简介、封面请到 B 站网页端调整，改名只发送标题，不会覆盖这些字段。
- `code=11201`（已收藏）和移除时的 `11202`（未收藏）按「状态未改变」处理，不弹错误；只有状态真的改变时才更新界面里的歌曲数。
- 删除歌单只删除收藏夹，不会删除视频；如果删除的正是当前播放队列的来源歌单，本地队列继续可播，但不再关联歌单。
- 歌单内容需要联网读取，不做离线缓存；歌单内排序由 B 站收藏时间决定，程序不支持手动排序。

**隐藏歌单（本地偏好）**

主要用于把只看视频、不想混进音乐库的收藏夹收起来：

- 隐藏/恢复：TUI 歌单列表按 `h`；安卓在歌单卡片右侧 `⋮` 菜单里选择「隐藏 / 取消隐藏」。
- `H`（TUI）或列表右上角的眼睛按钮（安卓）可临时显示隐藏项，再按 `h` 恢复。
- 隐藏列表按账号 `DedeUserID` 保存在 `~/.config/bvox/hidden_playlists.json`（安卓在应用私有目录），换账号不会串；只影响本机显示，不会修改 B 站收藏夹。
- 所有收藏夹一视同仁，包括 B 站内置的「默认收藏夹」，都可以隐藏。
- 隐藏列表不会跨设备同步：桌面和安卓各自保存一份。

**离线歌单缓存**

- 成功拉取收藏夹列表后写入 `~/.config/bvox/playlist_cache/<DedeUserID>.json`；打开过的歌单曲目写入同目录的 `<DedeUserID>-<media_id>.json`（安卓在应用私有目录）。
- 断网启动时先显示缓存列表，再后台刷新；刷新失败保留缓存并提示「离线：显示缓存的歌单」。
- 打开缓存的歌单会提示「（离线缓存）」。能否播放取决于音频是否已在本地媒体缓存：默认「自动缓存」模式下播过的歌可以离线播放，未缓存的歌仍需要联网。
- 队列里遇到无法加载的歌（常见于断网 + 未缓存）会自动跳到下一首继续尝试；整张列表都不可播时停止并提示「列表中没有可播放的歌曲（可能未缓存）」。
- 安卓「我的 → 已缓存歌曲」是独立入口：按最近缓存时间列出一个视频一首歌，点击后整张缓存列表成为播放队列；它不依赖收藏夹列表是否加载成功。列表只包含 `view` 元数据仍在缓存的条目，确保每一行都能离线播放。

### 5. BV 导入

TUI 里按 `b`，输入 BV 号或完整视频链接：

1. `view` 拿到 `aid`、标题、UP 主、分 P、封面
2. 插入当前列表顶部（已存在则只选中，不重复）
3. 如果当前打开的是一个歌单，就把该视频加入这个歌单；否则弹出「加入歌单」列表：
   可以选择现有收藏夹，或选末尾的「＋ 新建歌单」创建并加入；取消则只留在本地列表

写操作依赖 Cookie 里的 `bili_jct`，二维码登录会一并保存。未登录时无法创建歌单，
导入只留在本地列表；也可以先在搜索结果里按 `a` 加入已有歌单。

### 6. 字幕 / 歌词

```text
GET /x/player/wbi/v2?bvid=BV...&cid=...&wts=...&w_rid=...   (优先)
GET /x/player/v2?bvid=BV...&cid=...                         (签名失败时兜底)
→ data.subtitle.subtitles[] { lan, lan_doc, subtitle_url }

GET <subtitle_url>
→ [{"from": 0.0, "to": 2.5, "content": "..."}, ...]
```

上传字幕优先于 AI 字幕；同类里优先中文（`zh-*`，其次 `ai-zh`）。播放时按 `position` 找到最后一条 `from <= 当前时间` 的字幕作为当前行。

字幕 JSON 有两种格式，解析器都支持：

```json
[{"from": 0.0, "to": 2.5, "content": "..."}]
```

```json
{"type": "AIsubtitle", "body": [{"from": 0.0, "to": 2.5, "content": "..."}]}
```

注意：

- B 站的字幕列表经常需要登录才返回（尤其是 AI 字幕），很多纯音乐 MV 本身也没有字幕。
- **未签名的 `/x/player/v2` 会“串台”**：同一个 `bvid + cid` 有时返回另一个视频的字幕（甚至整份 `subtitles` 列表都是别人的）。曾出现歌曲拿到 LOL 解说、健身视频的 AI 转写并被缓存。因此现在优先走 WBI 签名端点，签名失败才回退；回退时用 AI 字幕文件名前缀 `<aid><cid>` 校验归属，不匹配的条目直接丢弃。
- 字幕 URL 偶尔会返回空字符串（风控/节点问题），遇到空列表/空 URL 会重试，最多 3 轮。
- 歌词缓存带版本号（当前 `v2`）。旧版缓存是裸数组，升级后会直接忽略并重新拉取，避免继续显示已经串台的歌词。
- 如果显示的歌词仍然不对，可以强制重拉：TUI 在歌词面板按 `L`，Android 点歌词卡片右上角的刷新按钮。它会删除该视频的歌词缓存并重新请求（仍然优先 WBI 签名端点）。
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
bvox --probe-cover
# Kitty / Sixel / Iterm2 / Halfblocks
```

终端查询在进入 raw mode 前执行，超时 400ms；失败自动回落，不影响启动。控制变量：

```bash
BVOX_COVER=blocks   # 强制半块
BVOX_COVER=native   # 即使在 tmux 里也尝试原生协议（需要 tmux allow-passthrough）
```

### 8. 本地缓存

缓存放在 `~/.cache/bvox`（遵循 `XDG_CACHE_HOME`）：

```text
audio/<bvid>_<cid>_<音质id>_<码率>.m4s
covers/<md5(缩略图 URL)>.img
lyrics/<bvid>_<cid>.json
view/<bvid>.json
```

安卓对应应用专属缓存目录（`cacheDir/bvox`）。该目录在系统存储紧张时可能被系统清理，所以调大上限适合自愿承担重新下载的场景。

- 缓存策略（TUI 按 `c` 循环，安卓点缓存卡片底部的「策略 …」胶囊挑选）：
  - `自动缓存`：音频、封面、歌词和视频信息都落盘，未命中时边流播边后台下载；
  - `仅小文件`：音频始终在线播放，只缓存封面、歌词和 `view` 信息，适合不想让音频文件一直堆积的用户；
  - `关闭`：不读也不写缓存，已有文件保留到手动清理。
  安卓的选择保存在本机，重启后沿用；TUI 可用 `BVOX_CACHE_MODE=auto|small_files|off` 设默认值，运行时用 `c` 临时切换。
- 音频（仅「自动缓存」策略）：播放前先查本地文件；命中时不再请求 `playurl`，mpv 直接读本地 `.m4s`。未命中时立即流播，同时在后台下载一份写入缓存，所以第一次播放会消耗双倍流量，第二次起零下载。
- 下载先写 `.m4s.part`，完成后原子 rename，避免半截文件被当成缓存；同一视频重复触发下载会按 `bvid:cid` 去重。
- 列表里已缓存的歌曲带标识：TUI 表格的「缓存」列显示绿色 ✓，安卓在行尾显示「离线可用」下载图标；后台下载完成后标识会自动出现。切到「仅小文件」或「关闭」后不再读音频缓存，标识也随之隐藏。
- 封面按缩略图 URL 的 md5 缓存（URL 指向的内容不变）；歌词按 `bvid + cid` 缓存（带 `v2` 版本号，旧版裸数组会被忽略并重拉），只缓存非空结果（登录后字幕可能才出现）；`view` 元数据缓存 24 小时，请求失败时回退到任意年龄的旧数据，保证离线也能播已缓存的歌。
- 自动缓存的总容量由 `BVOX_CACHE_MB` 限制（默认 1024），超出后按最近使用时间淘汰到 90%；`BVOX_CACHE_MB=0` 等同于「关闭」。安卓点缓存卡片底部的「上限 …」胶囊，可选 `512 MB / 1 GB / 2 GB / 4 GB / 8 GB`，改动立即生效（降低上限会马上按 LRU 清理多余文件）并持久化；「仅小文件」和「关闭」不显示上限胶囊。
- 当前占用显示在 TUI「正在播放」边框上（自动缓存显示 `已用 / 上限`，其他策略显示占用），安卓在「我的 → 缓存」卡片里展示策略、占用和分类图例。自动缓存时进度条长度表示容量占用；其他策略下进度条只表示各类文件的构成。填充部分按类型分色：粉=音频、紫=封面、青=歌词、黄=信息；图例给出每种类型的精确大小。清理入口：TUI 按 `C` 并确认；安卓点「我的 → 缓存 → 清理」。清理会删除音频、封面、歌词和 `view` 元数据，但保留正在播放的缓存文件和未完成的 `.part` 下载。
- `BVOX_CACHE_DIR` 可以改缓存目录。也可以直接删除缓存目录：`rm -rf ~/.cache/bvox`。
- 按 `r` 重新取播放地址时，如果本地已有缓存会直接重播缓存文件；想强制走网络先删除对应缓存。
- `--play` 无 TUI 模式只读取已有缓存，不在后台写缓存。

### 9. 播放与控制

`crates/tui/src/mpv.rs` 启动一个常驻 mpv：

```text
mpv --idle=yes --no-video --force-window=no \
    --input-ipc-server=/tmp/bvox-mpv-<pid>.sock \
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

**播放模式**（TUI 按 `m`，安卓播放页右上角切换）：

- `顺序播放`：播完当前列表停止；
- `列表循环`：播完最后一首回到第一首；
- `单曲循环`：曲目结束后重播当前曲目（多 P 视频重播当前 P）；
- `随机播放`：一轮内不重复地遍历整个列表，上一首按随机历史回退，一轮结束后重新洗牌。
- 多 P 视频在顺序/列表循环/随机模式下仍按 P 顺序播放；只有单曲循环会停在当前 P。
- 模式保存在 `~/.config/bvox/play_mode`（安卓在应用私有目录），重启后沿用；桌面与安卓各自保存。

### 10. DNS：跳过黑洞 CDN 地址

`passport.bilibili.com` 会解析出多个 IPv4，其中偶尔有 TCP 能建连但 TLS 永远不回包的节点。reqwest 不会像 curl 那样做 Happy Eyeballs，会卡在第一个地址直到 20 秒超时。

`crates/core/src/api.rs` 的自定义 resolver 做了三件事：

1. 优先 IPv4
2. 对多个候选地址并行发 HTTPS HEAD 探测（使用同一个 reqwest/rustls 栈，能过滤“TCP 能连但 TLS/HTTP 不回包”的节点），把第一个可用地址排到最前
3. 结果缓存 5 分钟

这是扫码登录从“偶尔卡 20 秒”变成稳定的关键。

## 代码结构

```text
crates/core/src/
├── api.rs     # WBI、搜索、二维码登录、收藏夹/歌单读写、字幕、封面、DASH 音频流、DNS
├── cache.rs   # 磁盘缓存：音频、封面、歌词、view，TTL 与 LRU 淘汰
├── app.rs     # 状态机：歌单库/队列视图、选中项、分 P、登录、加歌弹窗、歌词游标
├── session.rs # 会话调度：命令、后台任务、自动切歌、歌单增删改
├── player.rs  # Player trait（桌面 mpv / 安卓 ExoPlayer 的接缝）
└── ffi.rs     # UniFFI 导出：Snapshot、Core 命令、事件回调
crates/tui/src/
├── mpv.rs      # mpv 进程与 JSON IPC（load / pause / seek / volume / poll）
├── cover.rs    # 半块回落的封面解码与 56×32 RGB 网格
├── ui.rs       # ratatui 布局：歌单表、结果表、播放器、封面、歌词、弹窗
├── tui_app.rs  # 键位映射与确认弹窗
└── main.rs     # 入口、事件循环、后台任务
```

数据流：

```text
键盘线程 ──Msg::Key──▶ 主循环 ──Action──▶ 搜索/播放/登录/歌单增删改任务
                          ▲                    │
                          │                 API + mpv
                          └──Msg::*（结果、播放状态、封面、歌词、扫码状态）
```

## 已知限制

- 没有本地歌单、历史记录、每日推荐；歌单直接使用 B 站收藏夹。收藏夹列表和打开过的歌单会缓存到本机，断网时仍可浏览，但只能播放已缓存音频。
- 离线播放依赖「自动缓存」模式；「仅小文件」和「关闭」不读音频缓存，未缓存的歌断网时无法获取 `playurl` 流地址。加载失败的歌会自动跳过，整单不可播时停止并提示。
- 隐藏歌单列表只保存在本机，不跨设备同步。
- 歌单内排序由 B 站收藏时间决定，不支持手动拖拽排序；批量选择/批量删除也未实现。
- 新建歌单默认私有；简介、封面需要到 B 站网页端调整。
- 没有打开歌单时导入的 BV 会弹出加入歌单列表；取消后只留在本地列表，不会写进账号。
- 只有 `b` 导入或 `a` 手动加入的歌会写进歌单；直接点播放不会自动收藏。
- 歌词来自 B 站字幕，不接第三方歌词库；纯音乐 MV 经常没有字幕。
- 不显示视频画面、不接弹幕。
- 只支持普通视频（`/x/player/playurl`），番剧/课程/直播未接入。
- 收藏夹最多导入 100 页（2000 条）。
- 首次播放未命中缓存时会同时流播和后台下载，消耗双倍流量（仅「自动缓存」策略）；缓存默认上限 1 GiB，按 LRU 淘汰，可以切到「仅小文件」或「关闭」，也可以把上限改成 512 MB–8 GB。
- 搜索结果不缓存，仍以网络结果为准。
- 搜索受 B 站风控影响，偶尔首次请求较慢或返回 412；重试即可。
- `--search` 输出管道给 `head` 时可能因 broken pipe 报 panic，属 Rust 默认行为，不影响 TUI。

## 声明

仅供个人学习和本地播放使用。请遵守 B 站用户协议，不要用于批量下载、二次分发或绕过付费内容。程序不包含任何 DRM 绕过逻辑。
