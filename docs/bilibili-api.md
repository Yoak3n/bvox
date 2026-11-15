# B 站接口说明

Bvox 只用到 B 站 Web 端（`api.bilibili.com` / `passport.bilibili.com`）的少数接口：
搜索、视频信息、DASH 音频流、扫码登录、收藏夹（歌单）读写和字幕。本文记录这些接口
实际使用的参数、签名方式与踩过的坑，对应实现在 `crates/bvox-core/src/api/`。

> 结论来自实测而非官方文档，接口随时可能变化；行为不一致时以代码和实际请求为准。

## 通用约定

- 请求带桌面 UA 和 `Referer: https://www.bilibili.com/`。
- 需要身份的接口用 Cookie（`SESSDATA` / `bili_jct` / `DedeUserID`）；写操作的
  `csrf` 字段取 `bili_jct`。
- 搜索和歌词接口需要 WBI 签名；`view` / `playurl` / 收藏夹接口不需要。
- 传输层的 DNS 问题单独见文末「请求传输」。

## 1. 搜索

`/x/web-interface/search/type` 会被风控返回非 JSON。可用的是：

```text
GET https://api.bilibili.com/x/web-interface/wbi/search/type
    ?search_type=video&keyword=...&page=1&tids=3&wts=...&w_rid=...
```

`tids=3` 把结果限定在音乐分区，避免搜歌时混进其他分区的视频。

签名步骤（`crates/bvox-core/src/api/sign.rs`）：

1. `GET /x/frontend/finger/spi` 取 `data.b_3`，作为 `buvid3` Cookie
2. `GET /x/web-interface/nav` 取 `data.wbi_img.img_url` / `sub_url` 的文件名
3. 用 64 位重排表生成 `mixin_key`
4. 参数按 key 排序，追加 `wts`，URL 编码后拼接
5. `w_rid = md5(query + mixin_key)`

未登录请求 nav 会返回 `code: -101`，但 `wbi_img` 仍然在响应里，可以直接用。

## 2. 视频信息与音频流

```text
GET https://api.bilibili.com/x/web-interface/view?bvid=BV...
```

取 `data.cid`、`data.title`、`data.owner.name`、`data.pic`，以及 `data.pages[]`（多 P 的 cid / 时长 / 分 P 标题）。

```text
GET https://api.bilibili.com/x/player/playurl
    ?bvid=BV...&cid=...&fnval=4048&fnver=0&fourk=1
```

从 `data.dash.audio[]` 里按 `bandwidth` 选最高的一条，取 `baseUrl`（失败时用 `backupUrl[0]`）。返回的是 `.m4s`（fMP4 + AAC），mpv / ExoPlayer 可以直接解码。

这个接口**不需要 WBI 签名，也不需要登录**。实测返回：

| id | 含义 | 约码率 |
| --- | --- | --- |
| 30216 | 64K AAC | 44 kbps |
| 30232 | 132K AAC | 103 kbps |
| 30280 | 192K AAC | 204 kbps |
| 30250 | Dolby | 需账号权益 |
| 30251 | Hi-Res FLAC | 需账号权益 |

注意事项：

- 流地址有时效（`deadline` 参数，约 2 小时），不要持久化；过期后重新请求（TUI 按 `r`）。
- 音质选择：`自动`（默认）在大会员账号下优先 Hi-Res FLAC（30251）→ Dolby（30250）→ 最高码率 AAC，普通账号只用 AAC；也可显式选 `AAC` / `Dolby` / `Hi-Res`。
- 高音质流在交给播放器前会做一次 2 字节 Range 预检；CDN 返回 403（无权益/链接失效）时自动回退到 AAC，而不是把死链接交给 mpv/ExoPlayer。
- 偏好保存在 `~/.config/bvox/audio_quality`（安卓在应用 data 目录），下次启动沿用。

## 3. 扫码登录

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

`crates/bvox-core/src/api/login.rs` 把 QR URL 生成模块矩阵，`crates/bvox-tui/src/ui/modals.rs` 用 `▀/▄/█` 半块字符渲染成黑白二维码（白底、4 模块静区、EC level L）。登录成功后只保留需要的 Cookie 字段并写入 `~/.config/bvox/cookie`。

## 4. 收藏夹 / 歌单

B 站收藏夹就是 App 的歌单。

```text
GET /x/v3/fav/folder/created/list-all?up_mid=<DedeUserID>
→ data.list[] { id, title, media_count }

GET /x/v3/fav/resource/list?media_id=<id>&pn=1&ps=20&platform=web
→ data.medias[] { id, bvid, title, cover, duration, upper.name, type, attr }
   data.has_more
```

`type == 2` 才是普通视频，`attr != 0` 是失效条目，两者都会跳过。`data.medias[].id` 就是视频的 `aid`，程序把它存进 `VideoItem.aid`，增删歌单都靠它。程序按 20 条一页循环拉取，最多 100 页。`up_mid` 从 Cookie 的 `DedeUserID` 解析，不需要额外请求。

公开收藏夹不需要登录也能读取（`--fav <media_id>` 就是走这条接口列出内容）。

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
- 歌单内排序由 B 站收藏时间决定，程序不支持手动排序。

写操作依赖 Cookie 里的 `bili_jct`，二维码登录会一并保存。未登录时无法创建歌单。

## 5. BV 导入

TUI 里按 `b`，输入 BV 号或完整视频链接：

1. `view` 拿到 `aid`、标题、UP 主、分 P、封面
2. 插入当前列表顶部（已存在则只选中，不重复）
3. 如果当前打开的是一个歌单，就把该视频加入这个歌单；否则弹出「加入歌单」列表：
   可以选择现有收藏夹，或选末尾的「＋ 新建歌单」创建并加入；取消则只留在本地列表

## 6. 字幕 / 歌词

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

## 7. 请求传输：跳过黑洞 CDN 地址

`passport.bilibili.com` 会解析出多个 IPv4，其中偶尔有 TCP 能建连但 TLS 永远不回包的节点。reqwest 不会像 curl 那样做 Happy Eyeballs，会卡在第一个地址直到 20 秒超时。

`crates/bvox-core/src/api/transport.rs` 的自定义 resolver 做了三件事：

1. 优先 IPv4
2. 对多个候选地址并行发 HTTPS HEAD 探测（使用同一个 reqwest/rustls 栈，能过滤“TCP 能连但 TLS/HTTP 不回包”的节点），把第一个可用地址排到最前
3. 结果缓存 5 分钟

这是扫码登录从“偶尔卡 20 秒”变成稳定的关键。
