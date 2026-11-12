# bili-music 安卓版实施计划（Kotlin UI + Rust 核心）

## 背景

bili-music-tui 是桌面 TUI B 站音乐播放器（~4900 行 Rust：api 1898 / app 859 / main 813 / cache 479 / ui 504 / mpv 313）。目标：新增安卓版——UI 用 Kotlin + Jetpack Compose，播放用 Media3/ExoPlayer，Rust 核心经 UniFFI 复用。首个可用版本**全功能对齐 TUI**：二维码登录、搜索、播放（含缓存/音质）、歌词、收藏夹导入、BV 导入、云同步、封面。调试设备为模拟器（x86_64，需 KVM）。

**验证策略（按你的要求）**：过程中只做静态验证（cargo check/clippy/test、Gradle 编译、Kotlin 类型对齐），不反复实际运行；每个 Phase 收尾最多一次冒烟且先征得同意；最终验收由你在模拟器上操作。

## 目标架构

```
仓库根
├── Cargo.toml                 # [workspace]
├── crates/
│   ├── core/                  # bili-core（lib + cdylib + uniffi 导出）
│   │   └── src/{lib.rs, api.rs, cache.rs, cover.rs, app.rs, session.rs, player.rs, ffi.rs}
│   └── tui/                   # bili-music-tui（桌面端，行为不变）
│       └── src/{main.rs, tui_app.rs, ui.rs, mpv.rs}
└── android/                   # Kotlin Gradle 工程（Compose + Media3）
```

数据流：Kotlin UI ⇄ `Core`（UniFFI：命令方法 + CoreObserver 回调）⇄ `Session`（原 tui_loop 调度逻辑）→ `ApiClient` + `Player`（桌面=mpv IPC / 安卓=ExoPlayer 桥）。**队列、自动切歌、缓存逻辑全在 Rust 核心，两端共享。**

## Phase 0 — Git 基线 + workspace 拆分（桌面行为不变）

1. `git commit` 当前全部文件（仓库尚无任何提交，先留基线）
2. 根 Cargo.toml 改 `[workspace]`，members = `crates/core`、`crates/tui`，公共依赖版本上移
3. 文件搬移（`git mv`）：`api.rs`、`cache.rs`、`cover.rs` → crates/core（新建 lib.rs）；`main.rs`、`ui.rs`、`mpv.rs` → crates/tui（path 依赖 core，修 `crate::` 路径）
4. 验证：`cargo build` + `cargo test`（api.rs 现有 WBI 签名/cookie 解析单测）通过

## Phase 1 — 核心改造：UI 无关化 + Player 接缝

1. `crates/core/src/player.rs`：`#[async_trait] trait Player: Send+Sync`（load/toggle_pause/seek/set_volume/quit/is_started，load 带 PlayerConfig{user_agent, referer}——解除 mpv.rs:18-19 对 api 常量的硬编码）+ `PlayerState`（自 mpv.rs:21 迁入）+ `watch` 事件通道
2. MpvPlayer（留 tui crate）实现 trait；main.rs 的 `spawn_player_poller`(440) 变为其内部任务
3. `crates/core/src/session.rs`：新 `Session` —— 把 main.rs `tui_loop`(246-393) 的调度与全部 `spawn_*` 任务(471-763)移入：持 `Arc<ApiClient>` + `Arc<dyn Player>` + App + `mpsc<Msg>`；**EOF 自动接歌（take_eof+step）进 Session**；对外输出 `CoreEvent` 流
4. app.rs 去 UI 化：删 4 个 ratatui 依赖（app.rs:6-9）——`TableState`→已有 `selected` 镜像；`cover_protocol/picker/DynamicImage` 字段删除，CoverReady 事件改带 `Vec<u8>`；半块渲染逻辑移回 tui/ui.rs；`handle_key` 留 tui（改为调 Session 命令），Session 提供与 `Action` 对应的命令方法
5. 配置注入：`CorePaths{data_dir, cache_dir, cache_mb}` 参数化路径（现硬读 `$XDG_CONFIG_HOME/HOME`：api.rs:1404-1444、cache.rs:418-434）；桌面传原路径行为不变，安卓传 context 目录
6. reqwest features → `["json", "rustls-tls-webpki-roots"]`（安卓无系统证书目录，webpki 内置根；桌面无影响）
7. 验证：cargo build/test 通过；补 Session/EOF 接歌单测

## Phase 2 — UniFFI 导出层

- bili-core 加 uniffi 0.32：`crate-type = ["cdylib", "lib"]`、`uniffi::setup_scaffolding!()`、`[[bin]] uniffi-bindgen`（library mode）
- `crates/core/src/ffi.rs` 导出面：
  - `CoreConfig{data_dir?, cache_dir?, cache_mb?}`；`Core::new(config, player_bridge, observer) -> Arc<Core>`（OnceCell 建 tokio runtime）
  - 命令方法（同步、内部 spawn，结果走事件——沿用现有 Msg 模式）：start_login / search / play / toggle_pause / seek / set_volume / import_bv / pull_sync / load_folders / import_folder / stop
  - 廉价同步查询：queue / now_playing / position / duration / paused / volume / user / cover_bytes(key) / lyrics / current_lyric
  - `on_player_state(PlayerState)`：Kotlin 回推 ExoPlayer 状态
  - 回调接口：`CoreObserver{on_event(CoreEvent)}`（CoreEvent 映射 Msg 去掉 Key/Quit/Wake，加 Status/Loading）；`PlayerBridge{load(url,ua,referer)/play/pause/seek_to/set_volume/stop}`
- `uniffi.toml`：kotlin package `dev.yoa.bilimusic.core`；QR 矩阵导 `Vec<Vec<bool>>`（嵌套若有兼容问题降级为行主序 `Vec<u8>`+行宽）
- 验证：rustup target add x86_64-linux-android aarch64-linux-android + cargo-ndk 后，交叉编译通过 + uniffi-bindgen 生成 .kt 成功（纯静态）

## Phase 3 — 工具链 + 安卓工程骨架

- 工具链（pacman/sudo 需你在场确认）：`pacman -S android-tools android-udev`；Android Studio（官方 tarball 或 AUR）；SDK Manager 装 platform-tools / platform 35 / build-tools / NDK 29.0.14206865 / Emulator + API 35 x86_64 镜像；确认 KVM（/dev/kvm、kvm 组）
- `android/` 新 Gradle 工程（Compose 模板 + version catalog）：minSdk 23、target/compileSdk 35；依赖 media3 1.11.1、JNA `net.java.dev.jna:jna:5.x@aar`（@aar 必需）、compose BOM、navigation-compose
- Manifest：INTERNET / FOREGROUND_SERVICE / FOREGROUND_SERVICE_MEDIA_PLAYBACK / POST_NOTIFICATIONS；PlaybackService 声明 `foregroundServiceType="mediaPlayback"`
- Gradle 接线：`buildRust` Exec 任务 → `cargo ndk -t x86_64 -t arm64-v8a --platform 23 -o rustJniLibs build --release`；`generateUniffi` 任务 → .kt 进 generated sources；android_logger 初始化
- PlaybackService：ExoPlayer + `DefaultHttpDataSource.Factory`（UA + Referer 请求头，B 站 CDN 需要）+ MediaSession；CoreBootstrap：Core 单例 + PlayerBridge 实现（转发 MediaController 命令）+ CoreObserver→StateFlow
- 验证：`./gradlew assembleDebug` 通过；收尾一次模拟器安装启动冒烟（征得同意）

## Phase 4 — 全功能 UI（TUI 对齐）

Compose Navigation 屏幕：
1. **登录**：Canvas 画 QR 矩阵 + 阶段状态
2. **搜索/结果列表**：搜索栏、标题/作者/时长、选择与当前高亮（selected/current 语义）、BV 导入（对齐 b 键）、云同步收藏（对齐 y 键）
3. **收藏夹导入**：文件夹列表 + loading（对齐 g/f 流程）
4. **播放页**：封面（cover_bytes→BitmapFactory）、音质/分P 信息、播放暂停/上下首、进度条 seek、音量、歌词滚动（cursor 逻辑已在核心，暴露 current_lyric）、缓存下载进度（CacheProgress 事件）
5. **状态条**：Status/Error 事件 → Snackbar
- 验证：assembleDebug 通过；端到端验收由你在模拟器操作（我可代跑一轮冒烟，先征求同意）

## 风险与备注

- UniFFI 0.32.2 proc-macro 工作流（勿混 UDL）；JNA @aar 必须；无需 JNI_OnLoad；NDK r29 默认 16KB 页对齐
- 模拟器音频/网络与真机略有差异；后续真机只需 arm64-v8a（已在编译目标内）
- 包名 `dev.yoa.bilimusic`（可改）；每个 Phase 结束 git commit 一次

## 各阶段验证命令

- Rust：`cargo check`、`cargo clippy`、`cargo test`、`cargo ndk ... build`（交叉编译）
- 安卓：`./gradlew assembleDebug`、`uniffi-bindgen generate`
- 运行测试：仅在 Phase 收尾最少化进行（模拟器验收以你操作为主）