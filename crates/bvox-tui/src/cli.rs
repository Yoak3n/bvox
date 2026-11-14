//! CLI argument handling and headless (no-TUI) modes.

use anyhow::{Context, Result, bail};
use ratatui_image::picker::Picker;
use ratatui_image::picker::cap_parser::QueryStdioOptions;

use crate::mpv;
use crate::run::run_tui;
use bvox_core::CorePaths;
use bvox_core::api::{ApiClient, REFERER, UA, VideoItem, quality_label};
use bvox_core::session::parse_bvid;

/// Parse CLI arguments and run the requested mode.
pub async fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    match args.first().map(String::as_str) {
        Some("--search") => {
            let keyword = args[1..].join(" ");
            if keyword.trim().is_empty() {
                print_help();
                bail!("--search 需要关键词");
            }
            headless_search(&keyword).await
        }
        Some("--play") => {
            let Some(input) = args.get(1) else {
                print_help();
                bail!("--play 需要 BV 号或视频链接");
            };
            headless_play(input).await
        }
        Some("--fav") => {
            let Some(media_id) = args.get(1) else {
                print_help();
                bail!("--fav 需要收藏夹 media_id");
            };
            let media_id = media_id
                .parse::<i64>()
                .context("--fav 的 media_id 必须是数字")?;
            headless_favorites(media_id).await
        }
        Some("--probe-cover") => {
            let options = QueryStdioOptions {
                timeout_ms: 400,
                ..Default::default()
            };
            match Picker::from_query_stdio_with_options(options) {
                Ok(picker) => println!("{:?}", picker.protocol_type()),
                Err(error) => println!("halfblocks (query failed: {error})"),
            }
            Ok(())
        }
        Some("-h" | "--help") => {
            print_help();
            Ok(())
        }
        Some(other) => {
            print_help();
            bail!("未知参数: {other}");
        }
        None => run_tui().await,
    }
}

async fn headless_search(keyword: &str) -> Result<()> {
    let api = ApiClient::new(CorePaths::desktop_default())?;
    println!("搜索: {keyword}");
    let items = api.search(keyword).await?;
    print_items(&items);
    Ok(())
}

async fn headless_favorites(media_id: i64) -> Result<()> {
    let api = ApiClient::new(CorePaths::desktop_default())?;
    println!("收藏夹: {media_id}");
    let items = api.favorite_items(media_id).await?;
    print_items(&items);
    Ok(())
}

fn print_items(items: &[VideoItem]) {
    if items.is_empty() {
        println!("没有可播放的视频");
        return;
    }
    for (index, item) in items.iter().enumerate() {
        println!("{:>2}. {}  [{}]", index + 1, item.title, item.duration);
        println!("    {} · {}", item.bvid, item.author);
    }
}

async fn headless_play(input: &str) -> Result<()> {
    let bvid = parse_bvid(input).context("无法识别 BV 号")?;
    let api = ApiClient::new(CorePaths::desktop_default())?;
    let detail = api.view(&bvid).await?;
    let preference = api.audio_quality();
    let vip = api
        .user_info()
        .await
        .ok()
        .flatten()
        .is_some_and(|user| user.vip);
    let source = api.audio_source(&bvid, detail.cid, preference, vip).await?;

    println!("{} — {}", detail.title, detail.author);
    if detail.pages.len() > 1 {
        println!("共 {} P，播放 P1", detail.pages.len());
    }
    println!(
        "音质 {} · {} kbps{}（Ctrl-C 停止）",
        quality_label(source.quality_id()),
        source.bandwidth() / 1000,
        if source.is_cached() {
            " · 本地缓存"
        } else {
            ""
        }
    );

    let mut command = tokio::process::Command::new("mpv");
    command
        .arg("--no-video")
        .arg("--force-window=no")
        .arg(format!("--referrer={REFERER}"))
        .arg(format!("--user-agent={UA}"))
        .arg(source.target());
    mpv::harden_command(&mut command);
    let status = command
        .status()
        .await
        .context("启动 mpv 失败，请先安装 mpv")?;

    if !status.success() {
        bail!("mpv 退出码: {status}");
    }
    Ok(())
}

fn print_help() {
    println!(
        "\
bvox — B 站音频播放器（终端界面）

用法:
  bvox                     启动 TUI
  bvox --search 关键词      命令行搜索
  bvox --play BV...        命令行播放（mpv，无 TUI）
  bvox --fav MEDIA_ID      命令行列出公开收藏夹内容
  bvox --probe-cover       查看终端图片协议（kitty/sixel/halfblocks）
  bvox --help              显示帮助

TUI 按键:
  / 或 s   搜索              b      导入 BV 号/链接
  Enter    播放 / 打开歌单   space  暂停/继续
  n / p    下一首/上一首      ← / →  后退/前进 5 秒
  + / -    音量               g      扫码登录
  f        歌单列表/刷新
  a        加入歌单           d      移除歌曲/删除歌单
  n / r    新建/重命名歌单（歌单列表内）
  h / H    隐藏歌单 / 显示隐藏项（歌单列表内）
  m        切换播放模式（顺序/列表循环/单曲循环/随机）
  Esc      歌单列表 ↔ 歌曲列表
  l        歌词视图           r      重新获取播放地址
  c        切换缓存策略（自动/仅小文件/关闭）
  C        清理本地缓存（正在播放的缓存会保留）
  q        退出

播放模式:
  顺序播放：播完列表停止；列表循环：播完回到第一首；
  单曲循环：反复播放当前曲目；随机播放：随机顺序，可回退上一首。
  模式会保存在本地，重启后沿用。

歌单:
  登录后主界面是「我的歌单」（B 站收藏夹）。Enter 打开歌单，
  n 新建、r 改名、d 删除；歌曲列表里 a 加入歌单、d 从歌单移除。
  h 隐藏/恢复歌单，H 临时显示隐藏项；
  隐藏列表按账号保存在本地，只影响本机显示。
  导入 BV 时若已打开歌单，会直接加入该歌单；否则弹出「加入歌单」列表，
  可选择现有收藏夹，或选末尾的「＋ 新建歌单」创建并加入；Esc 则只留在本地列表。

缓存:
  音频、封面、歌词和视频信息缓存在 ~/.cache/bvox
  （遵循 XDG_CACHE_HOME）。BVOX_CACHE_DIR 自定义目录，
  BVOX_CACHE_MB 限制总大小（默认 1024，0 关闭缓存），
  BVOX_CACHE_MODE 设默认策略（auto/small_files/off，可被 c 键临时覆盖）。
  占用空间显示在「正在播放」边框上；按 C 清理（保留
  正在播放的缓存文件）。

依赖:
  mpv（播放与跳转由 mpv 负责）
  BVOX_COOKIE 环境变量可选；登录成功后 Cookie 会保存到
  ~/.config/bvox/cookie
"
    );
}
