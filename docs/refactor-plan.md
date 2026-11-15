# Bvox 代码组织重构计划

> 执行基线：`main` @ `59c3743`（2026-11-14）
> 目标读者：执行本次重构的模型/开发者
> 性质：**纯结构性重构（move-only）**。不改行为、不改 UI、不改依赖版本、不改 UniFFI 类型与方法名、不改用户可见文案。
> 执行后调整：实际布局以 README 为准。收尾阶段将 `session/parse.rs` 改名为 `input.rs`，`CacheUsageBar.kt` 改名为 `CacheUsage.kt`，`Rows.kt`/`Header.kt` 改名为 `TrackRow.kt`/`ScreenHeader.kt`，`nowplaying/Artwork.kt` 改名为 `NowPlayingArtwork.kt`，并把 `import_bv` 归回 `commands/import.rs`；`cache` 需要的共享模型/格式化函数下沉到 `model.rs`/`format.rs`/`title.rs`，依赖方向改为 `api -> cache`，`cache` 不再依赖 `api`。

---

## 0. 执行前必读

1. **工作区当前有未提交改动**：`README.md`、`android/.../CoreBootstrap.kt`、`crates/tui/src/tui_app.rs`、`crates/tui/src/ui.rs`（TUI 帮助弹窗等 WIP）。
   开始前先 `git status` + `git diff`，与用户确认：先提交为 WIP，或 `git stash`。
   **禁止在脏工作区直接重构**，否则结构移动和功能改动会混在同一个 diff 里，无法审查。
2. **每个 Phase 单独提交，不 push**。提交信息沿用仓库现有 Conventional Commits 风格，例如
   `refactor(core): split api into domain modules`。
3. **验证以静态检查为主**，不要为了验证去启动 TUI 或安装 Android 应用（用户偏好）。允许：`cargo fmt` / `cargo check` / `cargo clippy` / `cargo test` / `cargo build` / UniFFI 绑定生成 / Gradle 编译。
4. **只搬代码，不改逻辑**。如果拆分过程中发现真实 bug，记录到本文件末尾的「发现的问题」，单独开 commit/PR，不要顺手修。
5. 文件移动优先用 `git mv`，保留历史；内容拆分用「新建子文件 + 删除原文件中的对应块」，确保 `git diff -M` 能识别为移动。
6. `.zcode/plans/` 是本地会话计划目录，已被 git 忽略，不属于本次重构范围。

---

## 1. 采用的代码组织约定

以下规则取自本机既有项目 `hestia/AGENTS.md`、`diver/AGENTS.md`，本项目沿用并落到根目录 `AGENTS.md`：

### 1.1 文件长度

- 软限 **300 行**，硬限 **500 行**（含测试、含注释）。
- 超过软限就按职责拆文件；硬限是红线。
- 根模块文件（`api.rs`、`app.rs`、`session.rs`、`cache.rs`、`ffi.rs` 这类）允许放该领域的**主类型 + 构造/生命周期入口**，但业务逻辑必须下沉到子文件。

### 1.2 一文件一职责

同一文件禁止混放多类内容：

| 内容 | 放哪 |
|---|---|
| 领域类型 / 枚举 / 常量 | `model.rs` / 领域根文件 |
| 纯逻辑（无 IO、无 UI） | 独立模块，优先可单测 |
| 网络 / 文件 / 进程 IO | `transport.rs`、`store.rs`、`paths.rs` 等具体名字 |
| 状态变更（`on_*`、命令） | 按领域拆到 `app/`、`session/commands/` |
| 后台任务（`spawn_*`） | `session/tasks/` |
| 终端绘制 | `ui/` 子模块 |
| 键位映射 | `tui_app/keys/` |
| CLI / headless | `cli.rs` |

### 1.3 模块形态

- 优先 `foo.rs` + `foo/` 目录的形式（如 `hestia-core/src/installer.rs` + `installer/`），而不是每个目录都塞 `mod.rs`。
- `mod.rs` 如果出现，只做模块声明与稳定 re-export，不写业务逻辑。
- 子模块默认私有，跨文件 helper 用 `pub(super)` / `pub(crate)`；不要为了拆分把内部函数改成 `pub`。
- 根模块通过 `pub use` 保持既有公开路径不变（见 1.6）。

### 1.4 命名

- 用具体领域名：`installer/plan.rs`、`catalog/tree.rs` 这种；禁止 `utils.rs`、`common.rs`、`helpers.rs`、`misc.rs`、`base.rs` 当杂物间。
- 用具体动词/名词：`resolve_aid`、`parse_bvid`、`spawn_cache_download`；避免 `handle_xxx`、`do_xxx`。
- 不留 `v2`、`new`、`old`、`legacy` 过渡文件；迁移完成即删除被替代文件。

### 1.5 依赖方向

```text
bvox-tui (bin)     ──▶ bvox-core
android (Kotlin)   ──▶ UniFFI (bvox-core)
bvox-core          ──▶ 无 ratatui / mpv / Android / 终端依赖
```

- `bvox-core` 不出现 `println!`、终端判断、退出码；路径由 `CorePaths` 注入，不直接摸 `HOME`（现状已基本满足，重构不得破坏）。
- 前端只做渲染/输入/平台适配；业务状态变更继续走 `Session` / `Core`。

### 1.6 公开面稳定

本次重构**不改变**以下路径/Dart 侧观感：

- `bvox_core::CorePaths`
- `bvox_core::api::{ApiClient, VideoItem, AudioQuality, PlayMode, ...}`
- `bvox_core::app::{App, PlayRequest, ...}`
- `bvox_core::cache::{Cache, CacheMode, ...}`
- `bvox_core::player::*`、`bvox_core::resume::*`
- `bvox_core::session::{Session, Msg, CoreEvent, parse_bvid}`
- `bvox_core::ffi::{Core, Snapshot, CoreEvent, PlayerBridge, ...}` 及所有 `#[uniffi::export]` 方法名

唯一允许的模块改名：`crates/core/src/playlists.rs` → `playlist_store.rs`（内部持久化，TUI/Android 未直接使用）。根模块用 re-export 兜住其余路径。

### 1.7 测试

- 每个领域的现有 `#[cfg(test)] mod tests` 移到同级 `tests.rs`，用 `#[cfg(test)] mod tests;` 挂载。
- **测试只搬家，不删、不改断言**。移动后 `use super::*` 或显式 import 修到编译通过。
- 新增纯逻辑拆分点可补测试，但不要在本重构里扩大测试范围。
- 集成级 CLI 行为测试（若以后新增）放 `crates/bvox-tui/tests/*.rs`。

### 1.8 前端（Kotlin）同样适用

- 300 / 500 行限制同样适用于 `.kt`。
- 屏幕文件只做编排与组合；区块、卡片、弹窗、列表行拆到同 feature 子包。
- 共享组件按用途分类，不再用一个 `Common.kt` 装所有东西。

---

## 2. 现状盘点

### 2.1 Rust

| 文件 | 行数 | 主要问题 |
|---|---:|---|
| `crates/core/src/api.rs` | 2815 | 类型、常量、DNS、HTTP、WBI 签名、登录、搜索、视频、音频、收藏夹、歌词、图片、测试全部混在一起 |
| `crates/core/src/app.rs` | 1907 | 状态结构 + 初始化 + 登录 + 歌单 + 导入 + 搜索 + 播放队列 + 随机 + 歌词 + 测试 |
| `crates/core/src/session.rs` | 1769 | `Msg` + `Session` + 命令 + 后台任务 + 消息分发 + BV 解析 + 测试 |
| `crates/core/src/cache.rs` | 1125 | 类型 + 缓存读写 + 文件命名解析 + 统计/淘汰 + 测试 |
| `crates/tui/src/ui.rs` | 820 | 主布局 + 列表 + 歌词 + 播放器 + 页脚 + 登录/选择器/确认/帮助全部绘制 |
| `crates/core/src/ffi.rs` | 675 | UniFFI 类型 + 回调桥 + `Core` 命令 + Snapshot + pump |
| `crates/tui/src/tui_app.rs` | 562 | Action + TuiState + 各视图键位处理 |
| `crates/tui/src/main.rs` | 373 | 入口 + CLI 解析 + headless 命令 + 事件循环 + 帮助文本 |
| `crates/tui/src/mpv.rs` | 406 | mpv 进程/IPC + Player trait 适配 |
| `crates/core/src/lib.rs` | 86 | 模块声明 + `CorePaths` + 环境变量解析混在一起 |

### 2.2 Android Kotlin

| 文件 | 行数 |
|---|---:|
| `ui/screens/MeScreen.kt` | 774 |
| `ui/screens/NowPlayingScreen.kt` | 654 |
| `ui/components/Common.kt` | 644 |
| `ui/screens/LibraryScreen.kt` | 531 |
| `ui/MainScreen.kt` | 447 |
| `ui/Dialogs.kt` | 415 |
| `PlaybackService.kt` | 391 |
| `LyricOverlayController.kt` | 374 |
| `ui/screens/LyricSettingsScreen.kt` | 367 |
| `ui/LoginPanel.kt` | 361 |

另有根包下散落的 service / prefs / lyric 控制器文件，需要按职责归包。

### 2.3 其他

- 根目录残留空的 `src/`（初始单 crate 布局遗留），应删除。
- `crates/core/uniffi-bindgen.rs` 在 crate 根，按习惯应放 `src/bin/`。
- crate 目录名 `crates/core`、`crates/tui` 与既有项目 `crates/<project>-core` 的命名习惯不一致。

---

## 3. 目标目录

### 3.1 仓库根

```text
bili-music-tui/
├── AGENTS.md                 # 新增：维护者/agent 约定
├── Cargo.toml
├── crates/
│   ├── bvox-core/
│   └── bvox-tui/
├── android/
├── docs/
│   ├── refactor-plan.md      # 本文件
│   └── icon.png
└── .github/
```

### 3.2 `crates/bvox-core/src/`

```text
src/
├── lib.rs                    # 模块声明 + uniffi::setup_scaffolding! + re-export
├── paths.rs                  # CorePaths + desktop_default / 环境变量
├── duration.rs               # format_secs / parse_duration
├── player.rs                 # Player trait（保持）
├── resume.rs                 # 播放进度恢复（保持，测试移 tests.rs）
├── playlist_store.rs         # 原 playlists.rs：收藏夹离线缓存（改名）
│
├── api.rs                    # ApiClient 结构、new、只读访问器、mod 声明 + re-export
├── api/
│   ├── model.rs              # VideoItem / SearchPage / VideoPage / VideoDetail /
│   │                         # AudioStream / AudioSource / FavoriteFolder / LyricLine /
│   │                         # UserInfo / QrLogin / QrPhase
│   ├── quality.rs            # AudioQuality / PlayMode / quality_label /
│   │                         # is_premium_audio / TrackKind / preferred_kind
│   ├── sign.rs               # signed_query / mixin_key / extract_key / encode_component
│   ├── transport.rs          # Ipv4FirstResolver / first_reachable / get_* / post_form /
│   │                         # combine_cookie / cookie_path / mid / csrf_token /
│   │                         # buvid_extra / wbi_keys / check_response
│   ├── login.rs              # has_cookie / set_cookie / clear_cookie / qr_generate / qr_poll
│   ├── account.rs            # user_info / 音质与播放模式偏好 / 隐藏歌单
│   ├── search.rs             # search / search_page
│   ├── video.rs              # resolve_share_link / view / fetch_view / normalize_duration
│   ├── audio.rs              # audio_stream / audio_source / download_audio /
│   │                         # push_unique / order_audio_candidates / progress_step /
│   │                         # premium_stream_playable
│   ├── playlists.rs          # 收藏夹列表与内容 / 增删改 / 请求体构造 / playlist_folders
│   ├── lyrics.rs             # lyrics / refresh_lyrics / fetch_lyrics /
│   │                         # parse_lyrics / subtitle 选择
│   ├── text.rs               # clean_title / snippet
│   ├── image.rs              # normalize_url / cover_thumbnail_url / image_bytes
│   └── tests.rs              # 原 api.rs 测试
│
├── app.rs                    # App 结构、new、Default、set_status/set_error、mod + re-export
├── app/
│   ├── model.rs              # PlayRequest / PlayingInfo / PageSummary / InputMode /
│   │                         # MainView / PlaylistPicker / BvImport / PendingImport / LoginModal
│   ├── login.rs              # open_login / on_login_* / qr_matrix
│   ├── library.rs            # 歌单库加载、选择、隐藏、打开结果
│   ├── playlist_ops.rs       # 创建/改名/删除/加歌/移歌结果
│   ├── picker.rs             # 加入歌单选择器
│   ├── import.rs             # BV 导入、pending import
│   ├── search.rs             # 搜索分页与结果
│   ├── playback.rs           # on_playing / on_player_state / auto_advance / skip_failed
│   ├── queue.rs              # 队列步进、选中、切换条目
│   ├── shuffle.rs            # ShuffleRng + 随机播放历史
│   ├── lyrics.rs             # on_lyrics / lyric_cursor / current_lyric
│   ├── cache.rs              # on_cache_progress/done、is_cached、cached tracks
│   └── tests.rs              # 原 app.rs 测试
│
├── session.rs                # Session 结构、new、restore、生命周期、mod + re-export
├── session/
│   ├── msg.rs                # Msg / CoreEvent
│   ├── process.rs            # process 分发（约 400 行，允许的硬限内例外）
│   ├── lifecycle.rs          # stop / save_playback / persist_playlists
│   ├── parse.rs              # parse_bvid / share_link
│   ├── commands.rs           # 子模块声明 + re-export
│   ├── commands/
│   │   ├── search.rs
│   │   ├── playback.rs
│   │   ├── library.rs
│   │   ├── account.rs
│   │   ├── cache.rs
│   │   └── lyrics.rs
│   ├── tasks.rs              # 子模块声明 + re-export
│   ├── tasks/
│   │   ├── search.rs
│   │   ├── playback.rs       # spawn_play / resolve_aid / quality_status / track_display_title
│   │   ├── login.rs          # spawn_login / spawn_user_info
│   │   ├── library.rs        # 收藏夹、打开、增删改
│   │   ├── tracks.rs         # 加歌/移歌/导入
│   │   ├── cover.rs
│   │   ├── lyrics.rs
│   │   └── cache.rs          # spawn_cache_download
│   └── tests.rs              # 原 session.rs 测试
│
├── cache.rs                  # Cache 结构、new、模式/上限、下载守卫、mod + re-export
├── cache/
│   ├── model.rs              # CacheMode / CachedAudio / CachedTrack / CachedView /
│   │                         # CachedLyrics / CacheStats / CacheFile / format_bytes / 常量
│   ├── audio.rs              # 音频查找、提交、文件名解析、BV 集合
│   ├── media.rs              # 封面 / 歌词 / view 的读写与 TTL
│   ├── eviction.rs           # stats / clear / evict / stale part / 文件收集
│   ├── paths.rs              # cover/lyrics/view 路径、write_atomic
│   └── tests.rs              # 原 cache.rs 测试
│
├── ffi.rs                    # Core 结构、new、run、snapshot、mod + re-export
└── ffi/
    ├── types.rs              # CoreConfig / Snapshot / CacheProgress / CacheUsage /
    │                         # CoreEvent / CoreError
    ├── bridge.rs             # PlayerBridge / CoreObserver / RemotePlayer / UA/referer
    ├── commands.rs           # 所有 #[uniffi::export] impl Core 命令
    ├── snapshot.rs           # build_snapshot
    └── pump.rs               # pump_loop
```

### 3.3 `crates/bvox-tui/src/`

```text
src/
├── main.rs                   # 仅 #[tokio::main] + 调 cli::run
├── cli.rs                    # 参数解析、help、headless search/play/fav、probe-cover
├── run.rs                    # run_tui / tui_loop / key reader / signal handler
├── cover.rs                  # 保持
├── mpv.rs                    # Mpv 结构 + 生命周期 + harden_command + mod 声明
├── mpv/
│   ├── ipc.rs                # ensure_started / command / get / number / reset
│   └── player.rs             # MpvPlayer + impl Player
├── tui_app.rs                # TuiState + handle_key 顶层分发 + start_input /
│   │                         # request_confirm + re-export
├── tui_app/
│   ├── action.rs             # Action / ConfirmAction
│   └── keys/
│       ├── library.rs
│       ├── queue.rs
│       ├── confirm.rs
│       ├── picker.rs
│       ├── login.rs
│       └── input.rs
├── ui.rs                     # draw() 顶层分发 + centered 等共享小工具 + mod 声明
└── ui/
    ├── header.rs
    ├── results.rs
    ├── library.rs
    ├── lyrics.rs
    ├── player.rs             # draw_player / draw_player_text / gauge / cover_lines
    ├── footer.rs
    ├── modals.rs             # draw_login / draw_picker / draw_confirm
    ├── qr.rs                 # qr_lines
    └── help.rs               # draw_help / help_group_lines
```

### 3.4 Android Kotlin

```text
dev/yoa/bvox/
├── MainActivity.kt
├── BvoxApplication.kt
├── core/CoreBootstrap.kt
├── playback/PlaybackService.kt
├── prefs/CachePrefs.kt
├── lyrics/
│   ├── LyricOverlayController.kt
│   ├── LyricNotificationProvider.kt
│   ├── LyricDisplayPrefs.kt
│   └── LyricUiLine.kt
└── ui/
    ├── MainScreen.kt                 # 导航壳，MiniPlayer 拆出
    ├── theme/Theme.kt
    ├── components/
    │   ├── Surfaces.kt               # GradientBackground / riseIn / GlassCard
    │   ├── Buttons.kt                # CircleIconButton / PillButton / PlayButton / InfoChip
    │   ├── Artwork.kt                # ArtworkTile / TrackArtwork / rememberRemoteImage
    │   ├── Rows.kt                   # TrackRow
    │   ├── Feedback.kt               # StatusLine / EmptyState
    │   ├── Header.kt                 # ScreenHeader
    │   ├── Navigation.kt             # BvoxNavItem / BvoxBottomBar
    │   └── MiniPlayer.kt
    ├── dialogs/
    │   ├── PlaylistDialogs.kt        # PlaylistNameDialog / AddToPlaylistDialog
    │   ├── PlayerDialogs.kt          # PagePickerDialog / AudioQualityDialog + label/hint
    │   ├── CacheDialogs.kt           # CacheModeDialog / CacheLimitDialog
    │   └── ImportDialog.kt           # BvImportDialog
    ├── login/
    │   ├── LoginPanel.kt
    │   ├── WebViewLogin.kt
    │   ├── QrLoginTab.kt
    │   ├── CookiePasteTab.kt
    │   └── QrCode.kt
    └── screens/
        ├── me/
        │   ├── MeScreen.kt           # 编排 + 账号/入口卡片
        │   ├── CacheCard.kt          # CacheCard / CacheChip / CacheLegend
        │   ├── CacheUsageBar.kt      # 使用量条 + formatBytes
        │   └── AccountSection.kt     # VipBadge 等
        ├── nowplaying/
        │   ├── NowPlayingScreen.kt   # 编排 + PlayerContent
        │   ├── Artwork.kt            # Artwork / CoverBackdrop
        │   ├── TrackChips.kt
        │   ├── LyricsView.kt
        │   ├── QueueSheet.kt
        │   └── PlayMode.kt           # next / label / icon
        ├── library/
        │   ├── LibraryScreen.kt
        │   ├── LibraryHome.kt
        │   ├── NowPlayingHero.kt
        │   ├── PlaylistCard.kt
        │   └── PlaylistDetail.kt
        ├── discover/DiscoverScreen.kt
        ├── lyrics/LyricSettingsScreen.kt
        └── cached/CachedTracksScreen.kt
```

---

## 4. 分阶段任务

### Phase 0 — 基线与准备

1. `git status` / `git diff`，处理 4 个 WIP 文件（提交或 stash）。与用户确认后再动。
2. 跑基线并记录：
   ```bash
   cargo fmt --all -- --check
   cargo check --workspace --all-targets
   cargo test --workspace --locked
   ```
   若基线 fmt 不通过，先单独 `cargo fmt --all` 提交一次，避免后续格式化噪音混入结构提交。
3. 生成并保存 UniFFI 基线绑定，供 Phase 6 对比：
   ```bash
   cargo build -p bvox-core
   cargo run --features uniffi-cli -p bvox-core --bin uniffi-bindgen -- \
     generate --library target/debug/libbvox_core.so \
     --language kotlin --out-dir /tmp/bvox-bindings-before --config uniffi.toml
   ```
4. 记录当前文件行数基线：
   ```bash
   find crates -name '*.rs' -not -path '*/target/*' | xargs wc -l | sort -n
   ```

### Phase 1 — 仓库布局与约定落地

1. 目录改名（保留 git 历史）：
   ```bash
   git mv crates/core crates/bvox-core
   git mv crates/tui  crates/bvox-tui
   ```
2. 更新 `Cargo.toml`：
   ```toml
   members = ["crates/bvox-core", "crates/bvox-tui"]
   bvox-core = { path = "crates/bvox-core" }
   ```
3. 更新 `android/app/build.gradle.kts` 的两处路径：
   - `inputs.dir(repoRoot.resolve("crates/core/src"))` → `crates/bvox-core/src`
   - `inputs.file(repoRoot.resolve("crates/core/Cargo.toml"))` → `crates/bvox-core/Cargo.toml`
4. 移动 `crates/bvox-core/uniffi-bindgen.rs` → `crates/bvox-core/src/bin/uniffi-bindgen.rs`，同步 `[[bin]] path`。
5. 删除空的根目录 `src/`（`rmdir src`）。
6. 新增根 `AGENTS.md`，内容至少包含：
   - 300/500 行限制；
   - 一文件一职责表；
   - `foo.rs` + `foo/` 子目录约定、`mod.rs` 只做声明；
   - 依赖方向（core 不依赖 UI、不写 stdout）；
   - 命名规范（禁止 utils/common/base）；
   - 测试放置（同级 `tests.rs`，CLI 集成测试 `crates/bvox-tui/tests/`）；
   - 验证命令（fmt/check/clippy/test）；
   - 3.2/3.3 的模块地图（可精简）。
7. 更新 README 中所有 `crates/core` / `crates/tui` 路径引用（约在 71、86-87、191、248、406、438、449-456 行）。
8. 验证：`cargo check --workspace --all-targets`、`cargo test --workspace --locked`。
9. 提交：`refactor(repo): align crate layout and document conventions`

### Phase 2 — `api.rs` 拆分

把 2815 行拆成 3.2 中 `api/` 下的文件。

要点：

- `api.rs` 只保留 `ApiClient` 结构体、`new`、只读访问器（`cache` / `data_dir` / `audio_cache_enabled`）、`mod` 声明和 `pub use`。
- 公开类型全部从 `api.rs` re-export，保证 `bvox_core::api::VideoItem` 等路径不变。
- 常量 `UA`、`REFERER`、`CONFIG_DIR_NAME` 留在 `api.rs` 根部（或从子模块 `pub use`）；TUI 的 mpv/headless 与 `paths.rs` 依赖这些路径。
- 公开函数 `quality_label` 与 `format_secs` 必须能从 `bvox_core::api` 取到（`format_secs` 从 `duration.rs` re-export）。
- `clean_title`、`cover_thumbnail_url` 目前是 `pub(crate)` 且被 `session` 使用：在 `api.rs` 用
  `pub(crate) use text::clean_title;` / `pub(crate) use image::cover_thumbnail_url;` 兜住。
- `format_secs` 移到 `duration.rs` 后，`api.rs` 保留 `pub use crate::duration::format_secs;`，避免改 TUI/`session` 的 import。
- `parse_duration` 当前在 `app.rs` 私有，移到 `duration.rs` 后改为 `pub(crate)`，`app` 引用它。
- 测试块移到 `api/tests.rs`，`api.rs` 写 `#[cfg(test)] mod tests;`。
- 子模块中的共享私有 helper 用 `pub(super)`，例如 `check_response` 只在 `transport.rs` 内使用就不提升可见性。

验证：

```bash
cargo fmt --all
cargo check --workspace --all-targets
cargo test -p bvox-core
wc -l crates/bvox-core/src/api.rs crates/bvox-core/src/api/*.rs
```

提交：`refactor(core): split api into domain modules`

### Phase 3 — `app.rs` 拆分

按 3.2 的 `app/` 结构搬：

- `App` 结构体、`new`、`Default`、`set_status`/`set_error` 留在 `app.rs`。
- `ShuffleRng` 与随机游标逻辑进 `app/shuffle.rs`。
- 登录相关进 `app/login.rs`（含 `qr_matrix`）。
- 歌单库选择/隐藏留 `library.rs`，创建/改名/删除/加歌/移歌的结果处理进 `playlist_ops.rs`。
- 选择器 `picker.rs`、导入 `import.rs` 分开。
- 播放事件、自动切歌进 `playback.rs`；队列步进/选中进 `queue.rs`。
- 测试移到 `app/tests.rs`。

注意：`App` 的字段仍为 `pub`（前端直接读），子模块方法通过 `impl App` 访问私有字段即可；跨子模块调用的私有方法用 `pub(super)`。

验证同 Phase 2，另加 `cargo test --workspace --locked`。

提交：`refactor(core): split app state into domain modules`

### Phase 4 — `session.rs` 拆分

按 3.2 的 `session/` 结构搬：

- 根 `session.rs`：`Session` 结构、`new`、`restore_last_playback`、`pending_resume_request`、`progress_save_due`、`mod` 声明和 re-export。
- `process.rs`：原 `process` 的 `match` 分发。预计约 400 行，**允许作为硬限内例外**（<500）；不要为了压到 300 行而改分发逻辑。
- `commands/`：按领域拆方法；每个文件写 `impl Session { ... }`。
- `tasks/`：`spawn_*` 后台任务 + 其辅助函数；`session/tasks.rs` 只做声明与 `pub(super) use`。
- `parse.rs`：`parse_bvid`、`share_link`；根 `session.rs` re-export `parse_bvid`，保证 `bvox_core::session::parse_bvid` 不变。
- `msg.rs`：`Msg` / `CoreEvent`；根 re-export。
- 测试移到 `session/tests.rs`，注意它 import 了 `parse_bvid`、`share_link`、`track_display_title`，后者在 `tasks/playback.rs`，需在 `session.rs` re-export 或改 import 路径。
- Rust 隐私：子模块是 `session` 的后代，可以直接访问 `Session` 私有字段；兄弟模块之间共享的 helper 用 `pub(super)`。

验证：`cargo test --workspace --locked` + `cargo check --workspace --all-targets`。

提交：`refactor(core): split session commands and tasks`

### Phase 5 — `cache.rs` 拆分

按 3.2 的 `cache/` 结构搬：

- 根 `cache.rs`：`Cache` 结构、`new`、模式/上限/开关访问器、`begin_download`/`end_download`、`mod` + re-export。
- `model.rs`：所有缓存数据类型与常量、`format_bytes`。
- `audio.rs`：音频查找/提交/命名解析/BV 集合。
- `media.rs`：封面、歌词、view 的读写与 TTL。
- `eviction.rs`：统计、清理、LRU 淘汰、stale part。
- `paths.rs`：路径构造与 `write_atomic`。
- 常量 `CACHE_DIR_NAME`、`DEFAULT_LIMIT_MB` 从 `cache.rs` re-export（或留在根文件），`CorePaths::desktop_default` 依赖它们；不要在 `paths.rs` 复制字符串。
- 测试移到 `cache/tests.rs`。

验证同 Phase 4。

提交：`refactor(core): split cache storage responsibilities`

### Phase 6 — `ffi.rs`、`lib.rs` 收敛

1. 新建 `paths.rs`，把 `CorePaths` 与 `desktop_default` 从 `lib.rs` 移出；`lib.rs` 只留：
   ```rust
   pub mod api;
   pub mod app;
   pub mod cache;
   pub mod duration;
   pub mod ffi;
   pub mod player;
   mod playlist_store;
   pub mod resume;
   pub mod session;
   mod paths;

   pub use paths::CorePaths;
   pub use duration::format_secs;

   uniffi::setup_scaffolding!("bvox_core");
   ```
   （`duration` 是否公开按实际 import 需要决定；若 TUI 走 `api::format_secs`，可保持 `mod duration` 私有。）
2. `playlists.rs` → `playlist_store.rs`，更新 `api`/`session` 内部引用（`pub(crate)` 或私有模块）。
3. `ffi.rs` 按 3.2 拆分：
   - `types.rs` 放所有 `#[derive(uniffi::Record)]` / `#[derive(uniffi::Enum)]` / `#[derive(uniffi::Error)]`。
   - `bridge.rs` 放 `PlayerBridge`、`CoreObserver`、`RemotePlayer`、UA/referer helper。
   - `commands.rs` 放所有 `#[uniffi::export] impl Core` 方法。
   - `snapshot.rs` 放 `build_snapshot`，`pump.rs` 放 `pump_loop`。
   - 根 `ffi.rs` 保留 `Core` 结构、`run`、`new`、`snapshot`、`mod` + `pub use`。
4. **FFI 风险控制**：`#[uniffi::export]` 移到子模块后，必须重新生成 Kotlin 绑定并与基线 diff：
   ```bash
   cargo build -p bvox-core
   cargo run --features uniffi-cli -p bvox-core --bin uniffi-bindgen -- \
     generate --library target/debug/libbvox_core.so \
     --language kotlin --out-dir /tmp/bvox-bindings-after --config uniffi.toml
   diff -ru /tmp/bvox-bindings-before /tmp/bvox-bindings-after
   ```
   如果 diff 出现方法/类型变化，说明拆分破坏了导出面：把 `impl Core` 放回根 `ffi.rs`，只保留 `types`/`bridge`/`snapshot`/`pump` 的拆分。
5. 验证：`cargo test --workspace --locked`、`cargo clippy --workspace --all-targets -- -D warnings`。

提交：`refactor(core): separate paths, ffi types and exported commands`

### Phase 7 — TUI 拆分

1. `main.rs` 只保留 `#[tokio::main] fn main()` 与 `mod` 声明。
2. 新增 `cli.rs`：参数解析、`print_help`、`headless_search` / `headless_play` / `headless_favorites` / `print_items`、`--probe-cover`。
3. 新增 `run.rs`：`run_tui` / `tui_loop` / `spawn_key_reader` / `spawn_signal_handler` / `TuiEvent`。
4. `mpv.rs` 拆为根 + `mpv/ipc.rs` + `mpv/player.rs`：
   - 根：`Mpv` 结构、`new`、`load`/`pause`/`seek`/`volume`/`poll_state`/`quit` 等对外方法、`harden_command`、`Default`/`Drop`。
   - `ipc.rs`：`ensure_started`、`command`、`get`、`number`、`reset`。
   - `player.rs`：`MpvPlayer` + `impl Player`。
5. `tui_app.rs` 拆为根 + `tui_app/action.rs` + `tui_app/keys/*.rs`：
   - 根保留 `TuiState`、`handle_key` 顶层分发、`start_input`、`request_confirm`、re-export `Action`/`ConfirmAction`。
   - 每个视图的 `handle_*_key` 进对应 `keys/` 文件。
6. `ui.rs` 拆为根 + `ui/*.rs`：根保留 `draw`、`centered`、`CoverLayout` 等跨视图共享小工具；各视图绘制进对应文件。
7. 验证：
   ```bash
   cargo fmt --all
   cargo check -p bvox-tui --all-targets
   cargo clippy -p bvox-tui --all-targets -- -D warnings
   cargo build -p bvox-tui
   ```
   不启动 TUI。

提交：`refactor(tui): split cli, runner, keymaps and views`

### Phase 8 — Android Kotlin 重组（可在有 Android SDK 的环境单独执行）

1. 先只做**包移动**，不改 UI 逻辑：
   - `CoreBootstrap.kt` → `core/`
   - `PlaybackService.kt` → `playback/`
   - 歌词相关 4 个文件 → `lyrics/`
   - `CachePrefs.kt` → `prefs/`
   - `QrCode.kt` → `ui/login/`
2. 更新 `AndroidManifest.xml`：`android:name=".PlaybackService"` → `.playback.PlaybackService`；如果 `BvoxApplication`/`MainActivity` 不动则保持。
3. 更新所有 Kotlin import 与 `PlaybackService::class.java` 之类引用。
4. 再按 3.4 拆大文件：
   - `MeScreen.kt`：编排 + 账号/入口；`CacheCard.kt`、`CacheUsageBar.kt`、`AccountSection.kt`。
   - `NowPlayingScreen.kt`：编排 + `PlayerContent`；`Artwork.kt`、`TrackChips.kt`、`LyricsView.kt`、`QueueSheet.kt`、`PlayMode.kt`。
   - `Common.kt` 按 3.4 的 7 个文件拆。
   - `LibraryScreen.kt` 按 Home/Hero/Card/Detail 拆。
   - `Dialogs.kt` 按 Playlist/Player/Cache/Import 拆。
   - `LoginPanel.kt` 按 WebView/QR/Cookie 拆。
   - `MainScreen.kt` 把 `MiniPlayer` 拆到 `components/MiniPlayer.kt`。
   - `LyricSettingsScreen.kt` 的 `SectionIcon`/`SettingToggle` 拆到同包 components。
5. 每个大文件拆分时保持 `private` → `internal` 的最小可见性变化，只把跨文件使用的 composable 提升为 `internal`。
6. 验证（有 SDK 时）：
   ```bash
   cd android
   ./gradlew :app:compileDebugKotlin
   ```
   若环境缺少 SDK/NDK，记录为「未编译验证」，并至少用 `grep`/IDE 检查 import 与 manifest 引用。
7. 提交：`refactor(android): group services and split oversized screens`

### Phase 9 — 文档与收尾

1. 更新 README「代码结构」一节，换成新的模块地图与数据流（只改结构描述，不改功能说明）。
2. 检查所有文件长度：
   ```bash
   find crates -name '*.rs' -not -path '*/target/*' | xargs wc -l | sort -n | tail -20
   find android/app/src/main/java -name '*.kt' | xargs wc -l | sort -n | tail -20
   ```
   超过 500 的文件必须拆；300-500 的文件在提交说明中注明理由。
3. 全量静态验证：
   ```bash
   cargo fmt --all -- --check
   cargo check --workspace --all-targets
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace --locked
   cargo build --workspace --locked
   ```
4. （可选）给 CI 增加 fmt + clippy job，与既有项目的提交前检查一致；不改现有 release 逻辑。
5. 提交：`docs: document the new module layout`

---

## 5. 拆分操作规范

### 5.1 Rust

- 在 `foo.rs` 顶部写 `mod bar;`，子文件为 `foo/bar.rs`；Rust 2024 不需要 `mod.rs`。
- 子文件开头用 `use super::*;` 仅在不会造成歧义时；更推荐显式 `use super::{ApiClient, ...};`。
- 把 `impl Type` 拆到子文件时，子文件写完整的 `impl Type { ... }`，不要用 trait 包装。
- 私有字段在子模块中可直接访问（子模块是父模块后代）；跨兄弟模块使用的方法/函数用 `pub(super)`。
- `pub use` 只放在根模块，子模块不对外 re-export。
- 移动 `#[cfg(test)]` 时保持测试名与断言不变；`use super::*` 在 `tests.rs` 中应指向根模块，必要时改成 `use crate::api::...`。
- 不要为了消除 `dead_code` 警告而删代码；如果拆分后出现未使用项，说明该项原本只被同文件使用，检查是否漏搬。

### 5.2 Kotlin

- 包移动后先让编译器找错，再逐个修 import；不要手工全局替换类名。
- 拆分 composable 时保持参数列表与 modifier 默认值不变，避免行为差异。
- `private` composable 只在同文件使用；跨文件提升为 `internal`，不要直接 `public`。
- 拆出的文件保持原包（或新 feature 子包），Kotlin 文件名与主 composable 一致。

---

## 6. 验证矩阵

| 阶段 | 必跑 |
|---|---|
| Phase 1 | `cargo check --workspace --all-targets`、`cargo test --workspace --locked` |
| Phase 2-5 | `cargo fmt --all`、`cargo check --workspace --all-targets`、`cargo test --workspace --locked` |
| Phase 6 | 上述 + UniFFI 绑定 diff + `cargo clippy --workspace --all-targets -- -D warnings` |
| Phase 7 | `cargo fmt --all`、`cargo clippy -p bvox-tui --all-targets -- -D warnings`、`cargo build -p bvox-tui` |
| Phase 8 | `./gradlew :app:compileDebugKotlin`（有 SDK 时） |
| Phase 9 | 全量 fmt/check/clippy/test/build + 行数检查 |

说明：

- `cargo test` 是 headless 单元测试，不会启动 TUI/Android，符合「静态验证为主」的偏好。
- 不要用 `cargo run -p bvox-tui` 做验证。
- 不要为了通过 clippy 改行为；若基线已有 warning，先在 Phase 0 记录，重构只保证不新增。

---

## 7. 验收标准

- [ ] 所有 `.rs` / `.kt` 文件 ≤500 行；>300 行的文件有明确职责说明。
- [ ] `api.rs` / `app.rs` / `session.rs` / `cache.rs` / `ffi.rs` 根文件只剩主类型、构造/生命周期入口、模块声明与 re-export。
- [ ] 没有 `utils.rs` / `common.rs` / `helpers.rs` / `misc.rs` / `base.rs`。
- [ ] `bvox_core` 的公开路径与重构前一致（`playlist_store` 除外）；UniFFI 生成的 Kotlin 绑定与基线无差异。
- [ ] `cargo test --workspace --locked` 通过，测试数量不减少。
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- [ ] `cargo fmt --all -- --check` 通过。
- [ ] TUI 与 Android 的行为、按键、文案、布局均未改变。
- [ ] 根目录存在 `AGENTS.md`，README 代码结构章节已更新。
- [ ] 每个 Phase 是独立 commit，`git log` 可逐段 review；未 push。

---

## 8. 风险与回退

| 风险 | 处理 |
|---|---|
| 工作区 WIP 与重构混在一起 | Phase 0 先提交/stash；拒绝脏树开工 |
| UniFFI 宏跨模块导出异常 | 绑定 diff 必须为空；异常时把 `impl Core` 放回根 `ffi.rs`，仅保留类型/桥/pump 拆分 |
| 拆分后私有 helper 不可见 | 用 `pub(super)`；不要扩大为 `pub` |
| 循环 import / 模块声明遗漏 | 每搬一个文件立即 `cargo check`，不要攒到最后 |
| Android 包移动漏改 manifest | 先改 manifest + import，再跑 `compileDebugKotlin` |
| 行为被顺手改动 | 每个 Phase 的 diff 只允许移动、import、可见性、模块声明；出现逻辑改动立即回退 |
| 文件历史断裂 | 用 `git mv`；内容拆分保持原代码块完整搬移，便于 `git diff -M` |
| 测试被误删 | 对比重构前后 `cargo test -- --list` 的数量与名称 |

---

## 9. 执行顺序速查

```text
Phase 0  处理 WIP + 基线验证 + 保存 UniFFI 基线绑定
Phase 1  crates 目录改名 + AGENTS.md + 删除空 src/ + uniffi-bindgen 归位
Phase 2  api.rs        → api/
Phase 3  app.rs        → app/
Phase 4  session.rs    → session/
Phase 5  cache.rs      → cache/
Phase 6  ffi.rs        → ffi/ + lib.rs 收敛 + playlist_store 改名
Phase 7  tui main/ui/tui_app/mpv 拆分
Phase 8  Android 包归位 + 大文件拆分（有 SDK 时）
Phase 9  README/行数/全量检查
```

建议总提交数：9 个（Phase 0 若只处理 WIP 则单独提交，不计入重构提交）。

---

## 附：AGENTS.md 建议内容（可直接落地）

````markdown
# Agent / 维护者约定

改代码前先读。模块地图见 README「代码结构」。

## 文件长度

- 软限 300 行，硬限 500 行（含测试）。
- 超过软限按职责拆文件；根模块文件只放主类型、构造/生命周期入口、模块声明和 re-export。

## 一文件一职责

| 内容 | 放哪 |
|---|---|
| 领域类型 / 枚举 / 常量 | `model.rs` / 领域根文件 |
| 纯逻辑 | 独立模块，优先可单测 |
| 网络 / 文件 / 进程 IO | `transport.rs` / `store.rs` / `paths.rs` |
| 状态变更 | `app/`、`session/commands/` |
| 后台任务 | `session/tasks/` |
| 终端绘制 | `ui/` 子模块 |
| 键位映射 | `tui_app/keys/` |
| CLI / headless | `cli.rs` |

## 模块形态

- 优先 `foo.rs` + `foo/` 子目录；`mod.rs` 只做声明与 re-export。
- 子模块默认私有；跨文件 helper 用 `pub(super)` / `pub(crate)`。
- 公开路径由根模块 `pub use` 兜住，不在子模块对外 re-export。

## 依赖方向

```text
bvox-tui (bin)   → bvox-core
android          → UniFFI (bvox-core)
bvox-core        → 不依赖 ratatui / mpv / Android / 终端
```

- core 不写 stdout、不判断终端、不碰 `HOME`；路径通过 `CorePaths` 注入。
- 前端只做渲染、输入与平台适配。

## 命名

- 用具体领域名，禁止 `utils` / `common` / `helpers` / `misc` / `base`。
- 不留 `v2` / `old` / `legacy` 过渡文件，迁移完成即删除被替代文件。

## 测试

- 单元测试放同级 `tests.rs`。
- CLI 集成测试放 `crates/bvox-tui/tests/*.rs`。
- 测试不读写真实用户目录；用临时目录。

## 提交前

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
```

Android 改动额外跑 `cd android && ./gradlew :app:compileDebugKotlin`（有 SDK 时）。
不要为了验证启动 TUI 或安装 APK。
````
