# Agent / 维护者约定

改代码前先读。模块地图见 README「代码结构」，架构原理见 README「核心原理」。

## 文件长度

- 软限 **300 行**，硬限 **500 行**（含测试、含注释）。
- 超过软限就按职责拆文件；硬限是红线。
- 根模块文件（`api.rs`、`app.rs`、`session.rs`、`cache.rs`、`ffi.rs`）只放该领域的主类型、构造/生命周期入口、模块声明和 re-export，业务逻辑必须下沉到子文件。

## 一文件一职责

同一文件禁止混放多类内容：

| 内容 | 放哪 |
|---|---|
| 领域类型 / 枚举 / 常量 | `model.rs` / 领域根文件 |
| 纯逻辑（无 IO、无 UI） | 独立模块，优先可单测 |
| 网络 / 文件 / 进程 IO | `transport.rs` / `store.rs` / `paths.rs` 等具体名字 |
| 状态变更（`on_*`、命令） | `app/`、`session/commands/` |
| 后台任务（`spawn_*`） | `session/tasks/` |
| 终端绘制 | `ui/` 子模块 |
| 键位映射 | `tui_app/keys/` |
| CLI / headless | `cli.rs` |

## 模块形态

- 优先 `foo.rs` + `foo/` 子目录的形式，而不是每个目录都塞 `mod.rs`。
- `mod.rs` 如果出现，只做模块声明与稳定 re-export，不写业务逻辑。
- 子模块默认私有；跨文件 helper 用 `pub(super)` / `pub(crate)`，不要为拆分把内部项改成 `pub`。
- 公开路径由根模块 `pub use` 兜住，子模块不对外 re-export。

## 依赖方向

```text
bvox-tui (bin)   → bvox-core
android          → UniFFI (bvox-core)
bvox-core        → 不依赖 ratatui / mpv / Android / 终端
```

- core 不写 `println!`、不判断终端、不碰 `HOME`；路径通过 `CorePaths` 注入。
- 前端只做渲染、输入与平台适配；业务状态变更走 `Session`（桌面）或 `Core`（Android）。

## 命名

- 用具体领域名（`installer/plan.rs`、`catalog/tree.rs` 这种），禁止 `utils.rs` / `common.rs` / `helpers.rs` / `misc.rs` / `base.rs` 当杂物间。
- 用具体动词/名词（`resolve_aid`、`parse_bvid`、`spawn_cache_download`），避免 `handle_xxx` / `do_xxx`。
- 不留 `v2` / `old` / `legacy` 过渡文件；迁移完成即删除被替代文件。

## 测试

- 单元测试放同级 `tests.rs`（`#[cfg(test)] mod tests;`）。
- CLI 集成测试放 `crates/bvox-tui/tests/*.rs`。
- 测试不读写真实用户目录；用临时目录。

## 提交前

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
```

Android 改动额外跑（有 SDK 时）：

```bash
cd android && ./gradlew :app:compileDebugKotlin
```

不要为了验证启动 TUI 或安装 APK；以静态检查为主。

## Kotlin 端

- 300 / 500 行限制同样适用于 `.kt`。
- 屏幕文件只做编排与组合；区块、卡片、弹窗、列表行拆到同 feature 子包。
- 共享组件按用途分类，不放一个 `Common.kt` 装所有东西。
- `private` composable 只在同文件使用；跨文件提升为 `internal`，不要直接 `public`。
