# 300–500 行文件例外清单

约定见 [AGENTS.md](../AGENTS.md)：软限 300 行，硬限 500 行。下面这些文件超过软限、
但仍在硬限内，保留为单文件并记录职责与不拆理由。改动它们时优先拆到相邻文件，
不要继续加长；`app/tests.rs` 已正好 500 行，再加测试前必须先拆。

| 文件 | 行数 | 职责 | 保留原因 |
|---|---:|---|---|
| `crates/bvox-core/src/app/tests.rs` | 500 | `App` 状态机单元测试 | 按计划原样搬家、未改断言；再按主题拆会改变测试模块路径并引入跨文件 helper，留到专门拆测试时处理。已到硬限。 |
| `crates/bvox-core/src/cache/tests.rs` | 367 | 缓存读写、统计与淘汰的单元测试 | 单一测试职责；与 `app/tests.rs` 同理，拆分收益低于改动面。 |
| `crates/bvox-core/src/resume.rs` | 406 | 播放进度模型、磁盘持久化、捕获与应用 | 重构前已存在，计划明确保留；内部三个阶段耦合于同一恢复协议。 |
| `crates/bvox-core/src/ffi/commands.rs` | 327 | 所有 `#[uniffi::export] impl Core` 命令 | 保持导出面集中在一个 `impl Core` 块，避免 UniFFI 跨文件导出带来的绑定/checksum 风险。 |
| `android/app/src/main/java/dev/yoa/bvox/playback/PlaybackService.kt` | 400 | Media3 播放服务、播放器桥接与通知 | 重构前已存在；继续拆需要改服务生命周期和桥接状态，属于独立任务。 |
| `android/app/src/main/java/dev/yoa/bvox/ui/screens/nowplaying/PlayerContent.kt` | 347 | 播放页主体布局与控件 | 已从 `NowPlayingScreen` 拆出；再拆需要先抽出大量状态参数，收益有限。 |
| `android/app/src/main/java/dev/yoa/bvox/lyrics/LyricOverlayController.kt` | 376 | 桌面歌词悬浮窗生命周期与视图 | 重构前已存在；悬浮窗权限、窗口参数与视图更新耦合，拆开风险高。 |
| `android/app/src/main/java/dev/yoa/bvox/ui/MainScreen.kt` | 357 | 导航壳、根 Tab 与页面切换 | 已拆出 `MiniPlayer`、`BvoxBottomBar` 等组件；剩余部分是导航编排。 |

## 已在本轮拆出的文件

- `api/dns.rs`：IPv4 优先与 CDN 可达性探测，从 `api/transport.rs` 拆出。
- `api/playurl.rs`：`/x/player/playurl` DASH 响应模型，从 `api/audio.rs` 拆出。
- `api/playlist_form.rs`：收藏夹创建/改名/删除/加歌的表单体，从 `api/playlists.rs` 拆出。
- `api/quality.rs`：`TrackKind` / `track_kind` / `preferred_kind` 从 `api/audio.rs` 归位到计划目标文件。
- `model.rs` / `format.rs` / `title.rs`：cache 需要的共享模型与格式化函数下沉到中立层，`cache` 不再依赖 `api`。
- `screens/me/AccountSection.kt`、`screens/me/MeEntryCards.kt`：从 `MeScreen.kt` 拆出账号卡片与入口卡片。
- `screens/me/CacheUsage.kt`：缓存占用模型、条形图、图例与 `formatBytes`，从 `CacheCard.kt` 归位，解掉两个文件的双向依赖。
- `screens/lyrics/LyricSettingControls.kt`、`screens/lyrics/LyricColorPicker.kt`：从 `LyricSettingsScreen.kt` 拆出设置控件与颜色选择。

拆分时同步清掉了机械复制产生的未使用 import（29 个文件、约 1170 行）。
