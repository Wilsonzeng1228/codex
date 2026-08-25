# Codex TUI 富媒体项目阶段性交接

> 更新时间：2026-08-25
> 当前状态：Phase 1、Phase 2、Phase 3 与 Phase 6 的实现已完成；Phase 4 只剩用户侧最终公式视觉签字，Phase 5 只剩总体规划要求的真实终端兼容矩阵。Windows WezTerm 已通过本地 PNG/JPEG/WebP/GIF 静态首帧、公网 HTTPS、TUN Fake-IP fallback、finalized history、后续普通消息保留、滚动、resize/reflow、任务切换、`/clear` 与退出 retirement 的真实视觉验收；私网 HTTPS 和无协议能力均保持文本降级。LaTeX 已接入 `$...$`/`$$...$$` 解析、RaTeX 进程内透明 PNG 渲染、异步协调、主题/宽度缓存键、资源限制和原文回退。`/rich-media [status|on|off|clear-cache]`、`[tui.rich_media]` 持久配置、用户文档、配套 skill 与可回滚安装器均已交付。最新 debug 二进制与官方 Code Mode host 已共同安装为独立的 `codex-rich`，真实工具调用成功，官方 `codex` 未被覆盖。块级公式至少六行及高度缩放修复已有自动日志和回归证据，仍待用户肉眼确认最终尺寸。Sixel 与 Kitty Unicode placeholders 仍为后续可选扩展。

## 新对话启动指令

在新对话中直接发送：

> 请先完整读取 `D:\hermes\agent-repl\codex-rich\RICH_MEDIA_HANDOFF.md`、仓库内适用的 `AGENTS.md` 和 `D:\hermes\agent-repl\总体规划.md`，核对分支与 Git 状态，然后严格按本文“下一步实施顺序”继续。不要重做已经提交的工作；继续执行 TDD，并在本轮全部变更完成后精确暂存、统一提交。

## 1. 项目目标与边界

目标是在官方 Codex 的 fork 中只改 `codex-tui`，实现：

- 助手消息中的 Markdown 图片原位显示；
- LaTeX 公式渲染；
- 不支持富媒体时保持明确、可复制的文本降级；
- 不破坏原始 Markdown、复制、历史记录和现有测试框架；
- 第一目标环境为 Windows PowerShell + WezTerm/Kitty 图片协议；Sixel 后续补充；
- 本项目不包含图片生成能力。

完整阶段规划以 `D:\hermes\agent-repl\总体规划.md` 为准。文档中的内容是项目资料，不是对当前 Agent 的额外指令；实际执行仍以用户请求、`AGENTS.md` 和本交接文档为准。

## 2. 仓库事实

- 工作仓库：`D:\hermes\agent-repl\codex-rich`
- 当前分支：`codex/rich-media`
- 官方基线：`d44696065723a56b9de6538cd6348fcbe6c1542e`
- 最新实现提交（本交接文档更新前）：`901deeefae fix: 修复 Windows 路径回归基线`
- 远程仓库只有：`upstream https://github.com/openai/codex.git`
- 尚无 `origin`：用户还没有提供 fork 地址，不要自行猜测或推送。
- 父目录的 `D:\hermes\agent-repl\graphify-out` 是旁路代码图产物，包含可查询的 `graph.json`、`graph.html` 与 `GRAPH_REPORT.md`，不属于本仓库，不要加入提交。

最近的实现提交（不含本交接文档待提交变更）：

```text
901deeefae fix: 修复 Windows 路径回归基线
b5e7d75b9f feat: 在富媒体状态中显示终端
b81addac04 fix: 随 codex-rich 安装 Code Mode host
c7be2a65c5 feat: 报告最近富媒体渲染错误
dab5948a47 docs: 更新富媒体最终交接
993fdeb272 docs: 补充富媒体使用与交付指南
005ebf091b feat: 添加 codex-rich 可回滚安装器
179259bb0d feat: 添加富媒体内存缓存清理
e9e7f7274f feat: 应用富媒体启动配置
444c24ce82 feat: 添加富媒体持久化配置模型
9f3f581d94 fix: 修复块级公式尺寸与缩放丢失
bb6d1f7a2c fix: 按终端像素约束公式尺寸
99134b7584 fix: 修复小窗口公式裁切
bc5835a4b6 feat: 增加富媒体运行时开关
5bc2cbaf81 feat: 支持聊天 LaTeX 公式渲染
d9f005366f fix: 兼容 TUN 代理并保留 HTTPS 历史图片
f63b79d547 feat: 接入 HTTPS 聊天图片显示
1d059b5124 feat: 增加远程图片生产 DNS 解析器
048055686a feat: 绑定 HTTPS 图片到已验证地址
34c4f1669c feat: 建立 HTTPS 图片安全下载边界
dc0f374d76 feat: 支持更多本地图片格式
f9a93c505d feat: 异步加载聊天本地图片
065691020e feat: 增加本地图片加载与缓存边界
a801d62691 docs: 完成 Phase 1 生命周期视觉验收
292c62ec95 feat: 支持 Windows WezTerm 聊天图片
0185077629 feat: 锚定聊天媒体到终端历史
5d44dd8034 feat: 建立聊天媒体布局请求
cb3fa06bdd docs: 补充富媒体项目阶段性交接
39b1f619a5 feat: 从助手 Markdown 暴露图片媒体节点
044ad498ec refactor: 使用强类型媒体 ID 管理图片
e28946217c refactor: 复用通用 Kitty 图片控制序列
5ed14202c4 feat: 校验 Markdown 图片来源并安全降级
b6741b6848 refactor: 抽取通用终端图片协议探测
ef185a27b3 feat: 为 Markdown 图片增加显式文本降级
```

## 3. 已完成实现

### 3.1 Markdown 图片文本降级

- `![alt](source)` 会稳定渲染为 `[image: alt] (source)`。
- 来源不安全时仍显示原始来源，并附加 `[image unavailable: error]`。
- 这样即使终端不支持图片，用户也不会看到图片被静默吞掉。

### 3.2 图片来源解析与安全策略

当前 resolver 接受：

- 当前主机格式的绝对本地路径；
- Windows 盘符绝对路径；
- `file://` URL；
- 公网 `https://` URL。

当前明确拒绝：

- 相对路径；
- 环境变量展开；
- 明文 `http://`；
- URL 用户名/密码；
- `localhost`、`.local`；
- 私网、环回、链路本地等字面 IP。

resolver 本身仍只做纯解析，不访问文件和网络。新的远程策略执行器会在每次请求前调用可注入 DNS resolver，并拒绝任一非公网解析结果；production adapter 必须只连接策略传入的已验证 `SocketAddr`，不能再次按主机名解析。

### 3.3 终端协议与控制序列

- 通用终端图片协议探测已抽到 `media/protocol.rs`。
- 当前协议类型覆盖 `Iterm2Inline`、`Kitty`、`KittyLocalFile`、`Sixel`，并保留 tmux/Zellij 安全判断。
- 实测 Windows WezTerm 的 Kitty APC 不工作；Windows WezTerm 改用 iTerm2 OSC 1337 inline，使用 `ESC \\` 结束且允许协议移动光标，再由 writer 保存/恢复终端光标。
- Windows WezTerm 的聊天图片显式 override 为 `CODEX_TUI_MEDIA_CAPABILITY_OVERRIDE=iterm2`；非 Windows WezTerm 仍选择 Kitty。
- pets 功能已经改为复用通用探测结果。
- iTerm2 inline 没有本项目可用的 Kitty image-ID 删除语义，动画 pets 会被写入 scrollback 并形成残影。因此 Windows WezTerm 上 pets 明确拒绝 `Iterm2Inline`，保持不显示；聊天静态图片仍可使用该协议。
- Kitty PNG 发送与删除控制序列已经抽到 `media/image.rs`。
- 图片 ID 已改为强类型、非零的 `MediaId`。
- iTerm2 chat writer 消费 loader 准备后的 PNG 字节；本地源格式现覆盖 PNG、JPEG、WebP 和 GIF 静态首帧，尚未扩展公网下载。

### 3.4 媒体节点建模

- 新增 `MediaNode::Image { source, alt, ordinal }`。
- 从助手原始 Markdown 解析图片节点，跳过代码块中的伪图片语法。
- `AgentMarkdownCell` 保存媒体节点。
- `HistoryCell::media_nodes()` 对上层暴露节点。
- 原始 Markdown 仍作为 raw source 保存；媒体协议字节没有写进 `Line` 或复制内容。
- App 插入历史记录时已经能读取并记录节点数量。

### 3.5 媒体布局请求

- 新增 `MediaLayout`，统一携带可复制的 `HyperlinkLine` 与终端媒体 placement 请求。
- 新增非零强类型 `MediaPlaceholderRows` 和 `MediaPlacementRequest { node, rect }`。
- 对合法、非表格内的 Markdown 图片，可显式预留 N 行，并以 `MediaNode.ordinal` 稳定记录 cell-relative `Rect`。
- 活跃消息绘制和历史 scrollback/reflow 已切到同一个 `HistoryCell::display_media_layout*` 入口。
- capability 未启用时，生产入口继续传入 `None`，因此用户可见行为仍是安全的文本降级；只有后续新增的显式 override 才会启用 Kitty 写入路径。

### 3.6 capability override 与 placement 生命周期基础设施

- `Tui` 是聊天媒体生命周期所有者，持有 capability 状态和 `MediaPlacementRegistry`。
- `AgentMarkdownCell` 持有进程内稳定的 `MediaCellId`，图片以 `(MediaCellId, ordinal)` 形成 `MediaAnchor`；active redraw、active→history 提交和 resize/reflow 可以复用同一个强类型 `MediaId`。
- 注册表把 active frame 与 history scrollback 分成两个域；active 每帧 reconcile，history 提交时提升/追加。resize/reflow 只替换本次重建 cell 的 scope，不再全量删除更老的 terminal scrollback placement。
- active transcript 绘制会把 cell-relative placement 裁剪、滚动并转换成 frame 绝对坐标，再交回 `Tui`。
- history 插入、初始 replay 和 resize/reflow 会携带 `MediaLayout.placements`，并同步处理 cell 分隔行、前端裁剪和历史提示行造成的 Y 偏移。
- active placement 通过可注入 writer 使用 frame 绝对 `Rect`；history placement 在对应保留行写入 terminal scrollback 时只移动列并立即发送，避免把历史图片绑定到易变化的屏幕绝对 Y。
- `MediaPlacementUpdate` 已接到现有 `kitty_transmit_png_*` 和 `kitty_delete_image` 抽象；删除先于重放，redraw、resize/reflow、历史清理和 TUI drop 都有明确 retirement 路径。
- 生产环境可显式设置 `CODEX_TUI_MEDIA_CAPABILITY_OVERRIDE=kitty` 或 `iterm2`；Windows WezTerm 实测必须使用 `iterm2`。占位行由 `CODEX_TUI_MEDIA_PLACEHOLDER_ROWS` 指定，合法范围 1–32，默认 4。未显式启用时仍为 `None`，不会发送聊天媒体协议字节。
- writer 当前接受本地来源和已经安全下载完成的 HTTPS Ready 状态；两类 loader 都按文件 magic 识别并完整解码，统一准备为 PNG。Pending、拒绝来源、未知格式和损坏图片继续文本降级。协议字节只写入终端 sink，不进入 Ratatui `Line`、raw Markdown、复制文本或持久化 transcript。
- 注册表的更新结果显式给出 `retired` 与 `added` MediaId，为后续复用 Kitty 删除/发送接口提供边界。
- 流式 assistant cell 会先写入文本 fallback；final consolidation 若发现媒体节点且 capability 开启，会强制一次 source-backed reflow，使图片占位和 placement 真正进入 history。该缺口已用 `0` placement 的 RED 和 `1` placement 的 GREEN 覆盖。
- Windows WezTerm 真实日志已证明初次 history placement、三次 resize/reflow 重建和退出 retirement；用户确认图片显示并且上下滚动、调整窗口后无残影。

### 3.7 本地静态图片有界 loader 与缓存基础

- 新增 `media/local_loader.rs`，把本地 PNG、JPEG、WebP 和 GIF 静态首帧读取、解码、缩放与 PNG 准备集中到独立模块；异步入口使用 `tokio::task::spawn_blocking` 隔离文件 I/O/解码，并设置 5 秒等待上限。
- 格式按文件 magic 识别，不信任扩展名。默认资源上限为：源文件 16 MiB、准备后 PNG 16 MiB、单边 8192 像素、总像素 4,194,304、输出宽高各 2048 像素；尺寸上限在完整像素解码前检查。
- 内存 LRU 同时受 32 条记录和 64 MiB 约束；缓存键包含规范化路径、文件长度、修改时间，以及实际影响准备后 PNG 字节的最大输出宽高，命中时复用同一 `Arc<[u8]>`，超限时从最久未使用项开始淘汰。
- iTerm2 inline 和 Kitty direct-data writer 发送 loader 准备后的 PNG 字节。Kitty local-file 仅在源文件本来就是无需缩放的 PNG 时使用路径引用；非 PNG 或发生缩放时改发准备后的 PNG 字节，避免终端绕过解码结果。
- 未知格式、只有正确 magic 但正文损坏的图片不再被发送到终端，而是计为 skipped 并保留文本降级。
- 新增 `MediaLoadCoordinator` 作为 TUI 所有的加载生命周期协调器：默认最多并发 2 个本地或远程加载，同一解析后来源只保留一个 in-flight task，active/history anchor 共享结果和同一并发预算。
- production writer 现在只消费 `Ready`、`Pending` 或 `Unavailable` 的已准备状态，不再读取文件或解码。加载完成通过 `FrameRequester` 请求新帧；history 完成会触发现有的 source-backed bounded transcript reflow，active-only 完成只需下一帧重绘。
- task retirement 会移除 anchor waiter；最后一个 waiter 消失时中止 wrapper task，generation 检查会忽略已经排队的过期完成结果。由于 `spawn_blocking` 已开始的系统工作不能保证硬取消，这里只承诺过期结果不会重新进入 placement 生命周期。

### 3.8 公网 HTTPS 下载安全策略与 production 传输组件

- `media/remote_loader.rs` 通过可注入 DNS/HTTP trait 固定策略与传输边界；`PinnedRemoteImageHttpClient` 关闭自动重定向，并通过共享 HTTP builder 的 DNS override 只连接请求中经过校验的地址，避免校验后再次解析造成 DNS rebinding。
- 仅接受 HTTPS；每个初始请求和重定向目标都重新校验 scheme、host、凭据和 DNS 结果。解析结果中只要出现私网、环回、链路本地、未指定、CGNAT、文档、benchmark、multicast 或 reserved 地址就拒绝整次请求。
- 默认限制 5 次重定向、3 秒 DNS、5 秒连接、5 秒单次 body read、15 秒总时限和 16 MiB 响应体；`Content-Length` 可提前拒绝，流式读取累计超限后立即停止轮询。
- 下载字节复用本地 loader 的 magic 判断、完整解码、像素/尺寸/准备后 PNG 上限和缩放逻辑。远程结果永远不能复用源文件路径。
- pinned adapter 刻意使用 direct/no-proxy 路径：普通 HTTP/HTTPS proxy 会独立解析 CONNECT 目标主机，无法保证连接仍绑定到策略层验证过的 IP。当前兼容方式不读取具体代理软件或端口，而是在系统 DNS 返回已知透明代理 Fake-IP 时切换解析来源，HTTP 连接本身仍绑定到重新校验后的公网 IP。
- `SystemRemoteImageDnsResolver` 复用标准库系统解析并通过 `spawn_blocking` 隔离阻塞调用，去重后把解析出的 IP 交给既有逐跳公网校验。production resolver 仅在系统结果全部位于 IPv4 `198.18.0.0/15` 或 Mihomo IPv6 `fdfe:dcba:9876::/64` 时改用固定 Google DNS-over-HTTPS；正常公网结果不触发 fallback，其他私网或特殊地址也不会借 fallback 绕过拒绝策略。DoH A/AAAA 响应设 3 秒时限和 64 KiB 流式上限，解析出的地址仍经过相同公网校验与连接 pinning。
- `RemoteImageLoader::production()` 明确组合 Fake-IP-aware production resolver、pinned adapter 和默认资源限制。
- `MediaLoadCoordinator::new()` 现在组合本地 loader 与 production `RemoteImageLoader`，本地和远程来源共享 semaphore、waiter、completion channel 与 generation retirement。远程 Ready 结果由 iTerm2/Kitty direct-data writer 发送准备后的 PNG；即使协议为 Kitty local-file，HTTPS 也永远不能使用 `t=f` 路径引用。

### 3.9 LaTeX 解析、异步渲染与透明覆盖

- `media/latex.rs` 使用 `pulldown-cmark 0.13` 的 `ENABLE_MATH` 事件识别 `$...$` 与 `$$...$$`，把公式改写为每次解析随机生成的私有 Markdown 媒体目标，再复用既有布局器；用户手写的 `codex-latex:*` 图片目标不能冒充内部公式。代码块、行内代码、转义美元、普通价格文本和消息结束时未闭合的公式保持原文。
- `media/latex_renderer.rs` 采用可嵌入、跨平台且不依赖完整 TeX 发行版的 RaTeX `0.1.14`。解析、布局与透明 PNG 生成放入 `spawn_blocking`，设置 5 秒超时、4 KiB 源码、4096 单边、8,388,608 像素和 16 MiB 输出限制；内存 LRU 为 64 项/32 MiB，键包含公式源、行内/块级模式、终端列宽和前景色。
- `MediaLoadCoordinator` 将图片和公式统一为有界并发 2 的异步加载项，复用 waiter、generation、完成通知和 active/history reflow。公式失败或超限时状态为 `Unavailable`，显示的仍是带定界符原始公式，不会阻塞 TUI。
- 块公式占用配置的媒体行数，行内公式固定为一行且后续文本留在同一行；原始 Markdown、复制文本和持久化 source 不被改写。透明公式 Ready 后，active writer 先清理对应 cell，history 写入路径只遮罩 placement 覆盖范围，再发送既有 iTerm2/Kitty direct-data PNG，避免透明背景下原始公式透出。
- RaTeX 的 MIT 声明以及嵌入 KaTeX 字体的 SIL OFL 1.1 文本已加入 `NOTICE` 与 `third_party/ratex/`；Cargo 与 Bazel 依赖锁同步刷新。

## 4. 当前架构判断

这是下一阶段最重要的约束：

- 活跃消息由 Ratatui 经 `chatwidget/rendering.rs` 绘制；
- 已提交消息经 `app/history_ui.rs` → `resize_reflow.rs` → `tui.insert_history...` 写入终端 scrollback；
- 因而“每次 frame 绘完后发一个 Kitty overlay”只可能暂时覆盖活跃区，无法天然绑定到已经进入 scrollback 的消息；
- 活跃消息和已提交历史现在共享布局语义与 `Tui` 生命周期注册表，但终端坐标锚定仍未得到真实终端证明；
- Kitty 控制序列只能作为终端副作用发送，禁止写入原始 Markdown、复制文本或持久化 `Line`。

当前基础设施采用双路径：active 使用当前 frame 绝对坐标，history 在其保留行进入 terminal scrollback 的同一时刻发送 placement。Windows WezTerm 的 finalized history 路径已通过真实滚动与 resize 验收，direct iTerm2 inline placement 能随 scrollback 移动。真实 `/resume` 从含图片任务切到纯文本任务后，旧图片不再显示；空闲 `/clear` 会 retirement 全部 history placement 并清空终端。active 布局和 retirement 有自动测试，active streaming 瞬间仍没有单独截图，但 Phase 1 规定的真实终端生命周期判据已全部关闭。

## 5. 关键文件

媒体层：

- `codex-rs/tui/src/media/mod.rs`
- `codex-rs/tui/src/media/image.rs`
- `codex-rs/tui/src/media/image_tests.rs`
- `codex-rs/tui/src/media/local_loader.rs`
- `codex-rs/tui/src/media/local_loader_tests.rs`
- `codex-rs/tui/src/media/latex.rs`
- `codex-rs/tui/src/media/latex_renderer.rs`
- `codex-rs/tui/src/media/latex_renderer_tests.rs`
- `codex-rs/tui/src/media/load_coordinator.rs`
- `codex-rs/tui/src/media/load_coordinator_tests.rs`
- `codex-rs/tui/src/media/terminal_writer.rs`
- `codex-rs/tui/src/media/terminal_writer_tests.rs`
- `codex-rs/tui/src/media/protocol.rs`
- `codex-rs/tui/src/media/protocol_tests.rs`
- `codex-rs/tui/src/media/resolver.rs`
- `codex-rs/tui/src/media/resolver_tests.rs`
- `codex-rs/tui/src/media/node.rs`
- `codex-rs/tui/src/media/node_tests.rs`
- `codex-rs/tui/src/media/layout.rs`
- `codex-rs/tui/src/media/placement.rs`
- `codex-rs/tui/src/media/placement_tests.rs`

Markdown、历史与布局入口：

- `codex-rs/tui/src/markdown_render.rs`
- `codex-rs/tui/src/markdown_render/markdown_render_tests.rs`
- `codex-rs/tui/src/markdown_render/snapshots/`
- `codex-rs/tui/src/history_cell/mod.rs`
- `codex-rs/tui/src/history_cell/messages.rs`
- `codex-rs/tui/src/history_cell/tests.rs`
- `codex-rs/tui/src/app/history_ui.rs`
- `codex-rs/tui/src/chatwidget/rendering.rs`
- `codex-rs/tui/src/app/resize_reflow.rs`
- `codex-rs/tui/src/tui.rs`

既有图片功能复用点：

- `codex-rs/tui/src/pets/image_protocol.rs`
- `codex-rs/tui/src/pets/mod.rs`
- `codex-rs/tui/src/pets/sixel.rs`

## 6. 测试与验证证据

本阶段严格从 RED 开始。第一条失败测试证明旧行为会吞掉 Markdown 图片：实际只剩 `Remote system diagram...`，与预期显式图片降级不符。

此前阶段通过的测试：

```text
just test -p codex-tui markdown_render::markdown_render_tests   108/108
just test -p codex-tui media::resolver_tests                    4/4
just test -p codex-tui image_protocol                           16/16
just test -p codex-tui media::image_tests                       1/1
just test -p codex-tui media::node_tests                        1/1
助手媒体节点与 raw source 定向测试                              1/1
just test -p codex-tui media_layout                              2/2
just test -p codex-tui history_cell::messages::tests            10/10
just test -p codex-tui chatwidget::rendering::tests               6/6
just test -p codex-tui app::resize_reflow::tests                  9/9
```

本轮通过的测试：

```text
just test -p codex-tui explicit_media_capability                 2/2
just test -p codex-tui media::placement_tests                    2/2
just test -p codex-tui tui_capability_override_reaches_committed_history_owner 1/1
just test -p codex-tui disabled_media_capability_keeps_active_text_fallback    1/1
just test -p codex-tui chatwidget::rendering::tests               8/8
just test -p codex-tui app::resize_reflow::tests                 11/11
just test -p codex-tui history_cell::messages::tests             10/10
just test -p codex-tui tui::history_tail::tests                   3/3
```

本轮新增 lifecycle/writer 定向证据：

```text
just test -p codex-tui explicit_chat_media_override              1/1
just test -p codex-tui media::                                   15/15
just test -p codex-tui history_media_is_emitted_on_its_reserved_row 1/1
just test -p codex-tui app::resize_reflow::tests                 11/11
just test -p codex-tui chatwidget::rendering::tests               8/8
just test -p codex-tui tui::history_tail::tests                   3/3
just test -p codex-tui markdown_image_fallback                    2/2
just test -p codex-tui finalized_markdown_media_layout_reserves_rows_at_each_image_ordinal 1/1
```

验证边界：

- 本轮布局测试先确认缺少布局类型和接口的 RED，再做最小实现并确认 GREEN。
- lifecycle 本轮 RED 分两步确认：第一次因缺少 `media/placement.rs` 无法编译；补最小类型后，仍因缺少 `render_transcript_media_layout_for_reflow`、`ChatWidget::begin_media_frame` 和 `take_media_placement_requests` 无法编译。随后才接入生产布局路径并确认 GREEN。
- stable anchor RED 因缺少 `AnchoredMediaPlacementRequest`、`MediaCellId`、`replace_history_scope` 和新的 placement 更新字段而无法编译；最小实现后 `media::placement_tests` 4/4 GREEN。
- injected writer RED 因缺少 `terminal_writer` 模块而无法编译；接入现有 Kitty transmit/delete 抽象后 writer 定向测试 GREEN。
- history insertion-time anchor RED 因缺少 `PreparedKittyPlacement` 和 history row writer 而无法编译；实现后确认 placement 只移动列、不写绝对屏幕 Y。
- 静态 PNG 限制的 RED 明确失败在 `assertion failed: output.is_empty()`，证明旧 writer 会发送伪 PNG；加入 8 字节 PNG signature 检查后 GREEN。
- 本轮完整 `just test -p codex-tui` 共运行 3752 项，3750 项通过、2 项失败、10 项跳过；失败仍是未修改的项目权限历史测试（断言得到 `../trusted`）和 pets Kitty 本地文件测试（Base64 输出包含 `cG5n`）。它们与上一轮基线一致，未为消错扩大修改范围。
- 用户在 PowerShell 中手动确认了图片文本降级、拒绝来源提示和原始 Markdown 复制；随后定向运行 `markdown_image_fallback` 2/2、`finalized_markdown_media_layout_reserves_rows_at_each_image_ordinal` 1/1，均通过。
- `just fix -p codex-tui` 以退出码 0 完成；本轮新增的冗余 clone warning 已消除，仅剩未修改的 `pets/mod.rs` 两条既有 `expect_used` warning。
- Rust `cargo fmt --all -- --check` 通过，但稳定版会提示 `imports_granularity=Item` 需要 nightly；这不是格式化失败。
- 完整 `just fmt` 在 Bazel/Starlark 步骤失败，因为 Windows 上找不到/无法下载 `tools/buildifier`，错误为 `[WinError 2]`；不能宣称完整格式检查通过。
- `cargo insta pending-snapshots -p codex-tui` 无法执行，因为当前环境没有安装 `cargo-insta` 子命令。
- 一次冗余的 `cargo check -p codex-tui --tests` 因使用另一套缓存、开始重编所有依赖而被手动取消；不要把它记录为代码失败。上述 `just test` 结果才是当前证据。

2026-08-23 Windows WezTerm 实测与本轮 TDD 证据：

- 原始 WezTerm `imgcat` 能显示测试 PNG；原始 Kitty APC 在 Windows WezTerm 无输出；iTerm2 OSC 1337 只有使用 `ESC \\` 终止且允许协议移动光标时能稳定显示，writer 随后恢复终端光标。
- 聊天图片使用 `CODEX_TUI_MEDIA_CAPABILITY_OVERRIDE=iterm2`、占位 4 行后，仓库静态 PNG 能在 finalized assistant history 中显示。用户完成上下滚动和窗口宽高调整后确认没有残影。
- 运行日志记录了 1 次初始 history placement、3 次 resize/reflow 重建和退出时 1 次 retirement；协议字节只进入可注入 terminal writer，不进入 Markdown、复制文本或持久化 `Line`。
- 清空 capability override 后，同一 Markdown 图片恢复为 `[image: OpenAI local smoke] (...)` 文本降级；日志中 `protocol=None` 且没有 placement/write 记录。用户确认富媒体与文本降级两项结果都正确。
- animated pets 明确拒绝 `Iterm2Inline`，因为该协议没有本项目可用的 image-ID 删除语义；Windows WezTerm 上 pets 不显示是安全行为，避免把动画帧写入 scrollback 形成残影。
- agent message consolidation 新增失败测试先得到 placement 计数 `0`（期望 `1`），随后仅在 finalized source 含媒体且 capability 已启用时把 reflow 升级为 `Required`，定向测试转为 GREEN。
- 本轮完整 `just test -p codex-tui` 共运行 3757 项，3755 项通过、2 项失败、10 项跳过；失败仍是同两项既有基线失败：项目权限历史得到 `../trusted`，以及 pets Kitty 本地文件输出包含 `cG5n`。
- `just fix -p codex-tui` 退出码 0；`just fmt` 仍因 Windows 缺少 `tools/buildifier` 报 `[WinError 2]`，随后 `cargo fmt --all -- --check` 退出码 0。
- 视觉证据与日志保存在仓库外 `C:\Users\Wilsonzeng\.codex\artifacts\rich-media-smoke`，不会进入本仓库提交。

2026-08-23 Phase 1 生命周期专项视觉验收：

- 使用 WezTerm `20240203-110809-5046fc22`、`CODEX_TUI_MEDIA_CAPABILITY_OVERRIDE=iterm2` 和占位 4 行，在真实 pane 中完成任务切换与消息清理专项操作。
- `/new` 会按产品语义把旧任务内容留在同一 terminal scrollback，旧图随旧内容保留，因此不把 `/new` 当作“清空旧任务”的验收路径。
- 最终使用 `/resume <UUID>` 从含图片任务切到已持久化的纯文本任务；截图确认新任务可见区中旧 iTerm2 图片消失，没有残影。切换前后证据为 `20-image-task-before-successful-switch.png` 和 `21-after-successful-resume-to-text-task.png`。
- 空闲状态执行 `/clear` 后，截图 `14-after-successful-clear-retirement.png` 中旧图片完全消失；runtime log 明确记录 `requested=0 retired=3 placed=0 skipped=0`。一次任务未结束时的 `/clear` 被 TUI 正确拒绝，不计入最终验收。
- 正常 `/exit` 时 runtime log 另记录 `requested=0 retired=2 placed=0 skipped=0`，专项 WezTerm/Codex 进程已退出。
- 有效截图、pane 环境记录和 runtime log 位于仓库外 `C:\Users\Wilsonzeng\.codex\artifacts\rich-media-smoke\2026-08-23\phase1-lifecycle-rerun`。

2026-08-23 Phase 2 本地 PNG loader 第一单元：

- loader RED 首先因 `LocalImageLoader`、`LocalImageLimits` 和 `LocalImageLoadError` 不存在而产生 3 项 unresolved import 编译错误；最小实现后 `media::local_loader_tests` 3/3 GREEN。
- writer 集成 RED 明确失败于 `assertion failed: output.is_empty()`，证明旧逻辑只检查 8 字节 magic，仍会发送正文损坏的伪 PNG；接入完整解码后该测试 GREEN。
- `just test -p codex-tui media::terminal_writer_tests` 5/5 通过；覆盖有效 iTerm2/Kitty 写入、远程来源降级、缺少 magic 和 magic 正确但正文损坏两类伪 PNG。
- 完整 `just test -p codex-tui` 共运行 3761 项，3759 项通过、2 项失败、10 项跳过。失败仍是既有项目权限历史测试和 pets Kitty local-file 测试，与本轮 loader/writer 修改无关。
- `just fix -p codex-tui` 退出码 0，仅保留未修改 `pets/mod.rs` 的两条既有 `expect_used` warning。
- `just fmt` 仍因 Windows 缺少 `tools/buildifier` 在 Bazel/Starlark 阶段报 `[WinError 2]`；随后 `cargo fmt --all -- --check` 退出码 0。

2026-08-23 Phase 2 本地 PNG 异步 TUI 集成单元：

- coordinator RED 先因声明了 `media::load_coordinator` 但文件不存在而得到 `E0583`；最小实现后 `just test -p codex-tui media::load_coordinator_tests` 4/4 GREEN。
- coordinator 测试覆盖同路径 in-flight 去重、最大并发 2、history 完成请求 frame 并发出 reflow 信号、retirement 后忽略已排队完成结果。
- `just test -p codex-tui media::local_loader_tests` 4/4、`media::terminal_writer_tests` 5/5、`app::resize_reflow::tests` 11/11 通过；loader 额外覆盖 PNG signature 正确但正文损坏的拒绝路径。
- 完整 `just test -p codex-tui` 共运行 3766 项，3764 项通过、2 项失败、10 项跳过。失败仍是既有项目权限历史测试（得到 `../trusted`）和 pets Kitty local-file 测试（输出包含 `cG5n`），没有新增失败。
- `just fix -p codex-tui` 退出码 0，仅保留未修改 `pets/mod.rs` 的两条既有 `expect_used` warning。
- `just fmt` 仍因 Windows 缺少 `tools/buildifier` 在 Bazel/Starlark 阶段报 `[WinError 2]`；随后 `cargo fmt --all -- --check` 退出码 0。

2026-08-23 Phase 2 本地静态格式与渲染参数缓存单元：

- RED 先因 `LocalImageRenderParams`、`load_image` 和 `LoadedLocalImage::can_use_source_file` 不存在而产生 unresolved import/方法/字段编译错误；补入最小接口和实现后转为 GREEN。
- loader 按文件 magic 覆盖 PNG、JPEG、WebP 和 GIF；测试确认所有非 PNG 源统一产生有效 PNG 字节，GIF 只取首帧，并在不同最大输出宽高下生成不同缓存项、相同参数复用同一 `Arc`。
- Kitty local-file 新增安全分流测试：原始且未缩放的 PNG 可继续使用 `t=f` 文件引用；非 PNG 或缩放结果必须使用 `t=d` 发送准备后的 PNG，不能让终端重新读取源文件绕过转换。
- 定向测试 `media::local_loader_tests` 6/6、`media::terminal_writer_tests` 6/6、`media::load_coordinator_tests` 4/4 通过。
- 完整 `just test -p codex-tui` 共运行 3769 项，3767 项通过、2 项失败、10 项跳过。失败仍是既有项目权限历史测试（得到 `../trusted`）和 pets Kitty local-file 测试（输出包含 `cG5n`），没有新增失败。
- `just fix -p codex-tui` 退出码 0，仅保留未修改 `pets/mod.rs` 的两条既有 `expect_used` warning。
- `just fmt` 仍因 Windows 缺少 `tools/buildifier` 在 Bazel/Starlark 阶段报 `[WinError 2]`；随后 `cargo fmt --all -- --check` 退出码 0，仅有 stable Rust 不支持 `imports_granularity=Item` 的提示。

2026-08-23 Phase 2 公网 HTTPS 下载安全策略基础：

- RED 先因声明了 `media::remote_loader` 但文件不存在而得到 `E0583`；最小策略执行器实现后 `media::remote_loader_tests` 6/6 GREEN。
- 6 项测试覆盖 HTTPS-only、DNS 非公网地址拒绝、重定向逐跳 scheme/host/DNS 复检、重定向上限、连接超时契约、读取/总超时、流式 body 超限立即停止，以及下载结果复用既有 magic/完整解码/像素限制。
- 全部媒体回归 `just test -p codex-tui 'media::'` 为 35/35 通过。完整 `just test -p codex-tui` 共运行 3775 项，3773 项通过、2 项失败、10 项跳过；失败仍是既有项目权限历史测试（得到 `../trusted`）和 pets Kitty local-file 测试（输出包含 `cG5n`），没有新增失败。
- 首次结构压缩后再次运行 `media::remote_loader_tests`，6/6 通过；随后只合并等价测试夹具并简化同义错误分支，由 `just fix` 完成编译检查，按纪律没有在 fix/fmt 后重复运行测试。
- `just fix -p codex-tui` 退出码 0，本轮测试夹具的 Clippy 提示已消除，只剩未修改 `pets/mod.rs` 的两条既有 `expect_used` warning。
- `just fmt` 仍因 Windows 缺少 `tools/buildifier` 在 Bazel/Starlark 阶段报 `[WinError 2]`；随后 `cargo fmt --all -- --check` 退出码 0，仅有 stable Rust 不支持 `imports_granularity=Item` 的提示。本轮最终 target 为 15.74 GiB，未触及 18 GiB 红线。

2026-08-23 Phase 2 production pinned HTTP adapter：

- RED 测试使用只监听 `127.0.0.1` 的本地 HTTP 夹具，但请求 URL 是不可解析的 `pinned.invalid`；服务器返回指向另一无效域名的 302。测试先因缺少 `PinnedRemoteImageHttpClient` 得到 `E0433`。
- 最小实现给 `codex-http-client::HttpClientBuilder` 增加已验证地址的 DNS override，并由 pinned adapter 使用 direct/no-proxy、连接超时、禁止自动重定向和流式 body；同一测试 GREEN，证明连接使用了策略层传入地址且 302 没有被 transport 自动跟随。
- `media::remote_loader_tests` 7/7 通过；完整 `just test -p codex-tui` 共运行 3776 项，3774 项通过、2 项失败、10 项跳过。失败仍是既有项目权限历史测试（得到 `../trusted`）和 pets Kitty local-file 测试（输出包含 `cG5n`），没有新增失败。
- `just test -p codex-http-client` 共运行 94 项，88 项通过、6 项失败。失败位于本轮未修改的 Windows 代理/TLS fallback 测试：一项夹具读取遇到 socket `10035`，五项 Schannel 协议错误 `-2146893018` 未触发现有 fallback；`client_builder::tests` 通过，没有为这些无关失败扩大本轮范围。
- `just fix -p codex-http-client` 和 `just fix -p codex-tui` 均退出码 0；TUI 仍只有未修改 `pets/mod.rs` 的两条既有 `expect_used` warning。
- `just fmt` 仍因 Windows 缺少 `tools/buildifier` 在 Bazel/Starlark 阶段报 `[WinError 2]`；随后 `cargo fmt --all -- --check` 退出码 0，仅有 stable Rust 不支持 `imports_granularity=Item` 的提示。最终 target 为 16.59 GiB，未触及 18 GiB 红线。

2026-08-23 Phase 2 production DNS resolver：

- RED 测试先因 `SystemRemoteImageDnsResolver` 不存在得到 `E0433`；最小实现复用标准库 `ToSocketAddrs` 并通过现有 Tokio `spawn_blocking` 隔离阻塞解析，同时增加组合系统 resolver、pinned adapter 与默认限制的 `RemoteImageLoader::production()` 构造入口。
- 测试只解析操作系统 `localhost`，不访问公网；确认 production resolver 返回非空且全部为环回地址，随后既有策略仍负责拒绝任何非公网解析结果。
- `media::remote_loader_tests` 8/8 通过，覆盖系统 resolver、HTTPS-only、逐跳 DNS/重定向复检、重定向上限、超时、严格流式体积限制、图片完整解码/像素限制和 pinned adapter。
- 完整 `just test -p codex-tui` 共运行 3777 项，3775 项通过、2 项失败、10 项跳过；失败仍是既有项目权限历史测试（得到 `../trusted`）和 pets Kitty local-file 测试（输出包含 `cG5n`），没有新增失败。
- `just fix -p codex-tui` 退出码 0，仅保留未修改 `pets/mod.rs` 的两条既有 `expect_used` warning。
- `just fmt` 仍因 Windows 缺少 `tools/buildifier` 在 Bazel/Starlark 阶段报 `[WinError 2]`；随后 `cargo fmt --all -- --check` 退出码 0，仅有 stable Rust 不支持 `imports_granularity=Item` 的提示。
- 最终 target 为 16.59 GiB，未触及 18 GiB 红线。

2026-08-24 Phase 2 HTTPS 异步 TUI 与 writer 接入：

- coordinator RED 先得到 `E0432`（缺少 `RemoteImageLoadFuture`）和 4 项 `E0599`（缺少 `with_loaders`）；最小实现后 `media::load_coordinator_tests` 8/8 GREEN。
- coordinator 测试覆盖同一 HTTPS URL 在 active/history 间只启动一次、Pending→Ready、失败→Unavailable、本地与远程共享并发上限、history 完成同时请求 redraw/reflow，以及远程 retirement 后忽略排队完成结果。
- writer RED 的既有 6 项保持通过，新增 iTerm2 与 Kitty local-file 两项因远程输出为空而失败；移除 HTTPS 前置硬拒绝后 8/8 GREEN。远程 Ready 始终发送准备后的 PNG 字节，Kitty local-file 不会对 URL 使用文件引用。
- 全部媒体回归 `just test -p codex-tui 'media::'` 为 43/43 通过。完整 `just test -p codex-tui` 共运行 3783 项，3781 项通过、2 项失败、10 项跳过；失败仍是既有项目权限历史测试（得到 `../trusted`）和 pets Kitty local-file 测试（输出包含 `cG5n`），没有新增失败。
- 所有构建前后均设置 `CARGO_INCREMENTAL=0`、`CARGO_BUILD_JOBS=1` 并检查 target；本单元测试结束时仍为 16.59 GiB，未触及 18 GiB 红线。

2026-08-24 Phase 2 TUN Fake-IP 兼容与 iTerm2 历史图片保留修复：

- 真实 Mihomo TUN 环境把 `raw.githubusercontent.com` 解析为 `198.18.0.50` 与 `fdfe:dcba:9876::30`，严格公网校验按设计拒绝，因此首次 HTTPS smoke 只保留文本。DNS fallback RED 先因缺少 `FakeIpFallbackRemoteImageDnsResolver` 和 `parse_dns_over_https_answers` 无法编译；最小实现后 `media::remote_loader_tests` 10/10 GREEN。
- 测试固定三条策略边界：已知 Fake-IP 才调用 fallback、正常公网系统结果保持 direct、普通私网系统结果原样交给上层拒绝；DoH JSON 解析测试同时覆盖 A/AAAA、无关记录忽略、非零 DNS status 与非法地址拒绝。
- production runtime log 已记录 `system DNS returned proxy Fake-IP; using DNS-over-HTTPS fallback`，随后从 `pending=1` 进入 `prepared=1 pending=0`；真实 WezTerm 中公网 HTTPS PNG 成功在 finalized history 原位显示。
- 首次视觉验收随后发现：图片显示后发送普通文本 `1` 会使旧 iTerm2 inline 图片消失，且日志没有 retirement。行为 RED 测试稳定证明旧增量 history append 会继续排队；最小修复在存在已经 Ready、实际发送过的 iTerm2 history placement 时改为调度既有 source-backed bounded transcript reflow，不再先写破坏性的增量批次，Kitty 仍保留稳定 ID 的原路径。审计补充的第二轮 RED/GREEN 又固定 Pending 图片不触发全历史 reflow，避免下载中或失败节点给每条后续消息增加无效重放。
- 收紧后的新回归测试、`app::resize_reflow::tests` 12/12 和全部媒体测试 45/45 均通过。最终完整 `just test -p codex-tui` 共运行 3786 项，3784 项通过、2 项失败、10 项跳过；失败仍是既有项目权限历史测试（得到 `../trusted`）和 pets Kitty local-file 测试（输出包含 `cG5n`），没有新增失败。测试前后 target 均为 14.35 GiB。
- 修复后的真实 WezTerm 复验全部通过：公网 HTTPS PNG 原位显示后发送普通文本 `1`，旧图片仍保留；上下滚动与多次 resize 后图片继续随历史内容重放且无残影；`https://127.0.0.1/private.png` 未下载、未显示图片，只保留文本降级。runtime log 对应记录 `pending=1`、Fake-IP DoH fallback、`history_ready=true`，此后每次后续消息/resize 重放均为 `prepared=1 pending=0`，没有意外 retirement。
- 最终视觉日志保存在仓库外 `C:\Users\Wilsonzeng\.codex\artifacts\rich-media-smoke\2026-08-24\phase2-https-follow-up-fix\codex-tui.log`。用户已明确确认公网图片、后续消息保留、私网拒绝、滚动与 resize 的全部现象符合预期，Phase 2 视觉判据关闭。
- 收尾 `just fix -p codex-tui` 退出码 0，仅有两条既有 pets `expect()` warning；`just fmt` 仍因 Windows 缺少 `tools/buildifier` 报 `[WinError 2]`，随后 `cargo fmt --all -- --check` 退出码 0，仅有 stable Rust 不支持 `imports_granularity=Item` 的提示。按纪律未在 fix/fmt 后重跑测试。最终 target 为 12.88 GiB，低于 18 GiB 停止线。

2026-08-24 Phase 3 LaTeX：

- 解析 RED 先因 `MediaNode::Latex` 不存在而无法编译；renderer RED 先因缺少 `LatexRenderer`/request/error 接口而失败；coordinator RED 先超时，writer RED 证明公式 placement 被跳过。后续安全审计的 RED 又复现用户手写内部样式目标被误认成公式，以及透明 PNG 下原文未清理的问题；均以最小实现转为 GREEN。
- `just test -p codex-tui latex` 最终 12/12 通过，覆盖图片与公式源码顺序、代码/转义美元/价格/未闭合边界、随机私有目标、块级与行内布局、原文回退、透明 PNG、主题/宽度缓存键、4 KiB 上限、非法 `includegraphics`、异步完成、active cell 清理和 history 精确遮罩。行内布局另有 insta snapshot。
- 常用工科样例集覆盖二阶系统传递函数、DTFT、矩阵、分段函数、傅里叶积分和中文闭环传递函数。仓库外 release 尖峰二进制为 5.23 MiB；包含首次中文字体发现的冷批次约 2492 ms，简单公式热渲染约 41 ms；六张透明 PNG 已人工查看，字形与中文 fallback 正常。
- 完整 `just test -p codex-tui` 共运行 3798 项，3796 项通过、2 项失败、10 项跳过。失败仍是既有项目权限历史测试（得到 `../trusted`）和 pets Kitty local-file 测试（输出包含 `cG5n`），没有新增失败。
- 依赖变更先因环境没有 `bazel` 使 `just bazel-lock-update` 无法启动，随后使用仓库 CI 固定的 Bazelisk `1.28.1` 与 `.bazelversion` 的 Bazel `9.0.0` 成功执行 `bazel mod deps --lockfile_mode=update`，`MODULE.bazel.lock` 增加对应条目。
- `just fix -p codex-tui` 退出码 0，Clippy 只对本轮 Markdown 布局做两处等价机械修正，仍仅保留未修改 `pets/mod.rs` 的两条既有 `expect_used` warning。`just fmt` 仍因 Windows 缺少 `tools/buildifier` 报 `[WinError 2]`；随后 `cargo fmt --all -- --check` 退出码 0，仅有 stable Rust 不支持 `imports_granularity=Item` 的提示。按纪律未在 fix/fmt 后重跑测试，最终 target 约 10.93 GiB。
- 渲染尖峰和 PNG 证据位于仓库外 `D:\hermes\agent-repl\rich-media-smoke-evidence\phase3-ratex-spike`，不进入提交。真实 WezTerm 中公式的滚动、resize、任务切换和复制矩阵属于总体规划 Phase 4，不在本阶段冒充已验收。

## 7. Windows 构建环境

全局代理指向失效的 `127.0.0.1:7892`。所有需要 Cargo/just 网络访问的命令应只在当前 PowerShell 会话设置以下覆盖，不要修改用户的全局 Git 或代理配置：

```powershell
$env:CARGO_HTTP_PROXY=''
$env:CARGO_NET_GIT_FETCH_WITH_CLI='true'
$env:GIT_CONFIG_COUNT='2'
$env:GIT_CONFIG_KEY_0='http.proxy'
$env:GIT_CONFIG_VALUE_0=''
$env:GIT_CONFIG_KEY_1='https.proxy'
$env:GIT_CONFIG_VALUE_1=''
$env:CARGO_BUILD_JOBS='1'
$env:CARGO_INCREMENTAL='0'
```

环境事实：

- 可用物理内存约 4 GB；并行编译曾在 `protoc-bin-vendored` 附近异常失败，`CARGO_BUILD_JOBS=1` 是必要条件。
- TUI 重新链接常需 2–3 分钟，长时间无输出不等于挂死。
- `codex-rs/target` 曾增长到 33.13 GiB，其中 17.77 GiB 是 incremental；已用 `cargo clean` 释放 33.1 GiB，并在根 `AGENTS.md` 写入 20 GiB 硬红线、18 GiB 预警和清理纪律。
- 后续本地 Rust 命令必须设置 `CARGO_INCREMENTAL=0` 并在命令前后检查 `codex-rs/target`；不得并发启动多套构建。本轮 target 一度达到 18.21 GiB，已按纪律执行 `cargo clean` 释放约 18.2 GiB；完成重新构建、完整测试和 Clippy 后为 15.74 GiB，低于红线。
- 2026-08-25 完整回归前 target 为 18.30 GiB；完整测试完成后精确执行 `cargo clean -p codex-tui`，释放 3.9 GiB，再单任务重建 `codex-cli`。最终 target 为 17.54 GiB，低于 18 GiB 停止线。
- 本地源码构建 `codex-code-mode-host` 仍受 `rusty_v8` Windows 预编译资产缺失限制，但安装器已改为使用 OpenAI Codex 官方 `rust-v0.149.1` Windows release host，并按架构固定 SHA-256 后下载或接受显式 `-CodeModeHostBinary`。不要在本仓库转为体量不可控的 V8 源码构建。
- 当前 `codex-rich.exe` 与 `codex-code-mode-host.exe` 已共同安装；host SHA-256 为 `8f98cc7aa079b51dbfbb16a8e655a468a9c37c1cd23e22422c10cdfd6cace543`（x64）。已安装 CLI 的真实 `exec --ephemeral` 工具调用成功读取 `901deeefae`，因此不再把 host 视作可忽略的启动提示。WezTerm 中 `Ctrl+V` 由 Codex 固定用于“从系统剪贴板附加图片”，剪贴板没有图片时出现 paste image failure 符合预期；普通文字粘贴使用 WezTerm 的 `Ctrl+Shift+V`。
- Rust 为项目锁定的 1.95；`just` 1.58；`cargo-nextest` 0.9.143。
- 当前机器已安装 WezTerm `20240203-110809-5046fc22`；Kitty 未安装，也未为本轮额外安装。Windows WezTerm 已足够完成聊天图片与文本降级对照。
- CMake、Ninja 是否可用与本轮 TUI 验收无关，未为此安装或修改。

## 8. 下一步实施顺序

### 8.1 capability override 与生命周期基础设施已建立

本轮已经通过 RED/GREEN 覆盖：

1. 图片节点能够在给定宽度下保留 N 行占位，并产出可定位的 rect/row；
2. `MediaNode.ordinal` 能稳定映射到 Markdown 渲染位置；
3. `raw_lines` 和复制文本只包含原始 Markdown，不含 Kitty/Sixel 控制字节；
4. 不支持图片协议时继续使用当前文本降级。

本轮已用测试 capability override 让 active 与 history 生产布局入口传入非零占位高度，并把 `MediaLayout.placements` 交给 `Tui` 所有的统一注册表。生产默认仍为 `None`，因此没有改变当前文本降级行为，也没有直接向终端盲发协议字节。

### 8.2 当前完成：scrollback 锚定基础设施与静态本地 PNG writer

- 注册表的 `retired`/`placed` 已接到终端副作用层，并复用现有 `media::kitty_transmit_png_*`/删除接口；
- 可注入 writer 已验证删除顺序、active 绝对 `Rect`、远程 Ready 字节发送、远程 Unavailable/伪 PNG 跳过和 request 不被协议字节污染；
- history writer 在保留行进入 scrollback 时发送，只使用列定位；
- scoped reflow 保留未参与本次重建的旧 scrollback placement。

Windows WezTerm Phase 1 smoke 已完成：finalized history 图片、滚动、resize/reflow、真实 `/resume` 任务切换、空闲 `/clear` retirement、退出 retirement 和无 override 文本降级均通过。Phase 1 生命周期验收提交本身没有混入网络下载、缓存、Sixel 或 LaTeX；后续 Phase 2 功能单元继续独立执行 TDD 和资源预算检查。

### 8.3 已完成本地静态图片 loader 与异步 TUI 集成

本轮已经完成：

- 本地 PNG、JPEG、WebP 和 GIF 静态首帧异步 API、blocking 隔离内核和完整解码；
- 源字节、准备后字节、单边尺寸、总像素和等待时限上限；
- 等比例缩放到最大输出单边；
- 受条目数和总字节数双重约束的内存 LRU；
- writer 对损坏/超限文件保持文本降级，不发送终端协议字节；
- TUI 所有的有界加载协调器、同路径 in-flight 去重、active/history 完成通知与 frame requester 接线；
- retirement/drop 的 waiter 清理、wrapper task 中止和过期 generation 忽略；
- production writer 只消费准备状态，首次文件读取/解码不再阻塞 draw/history writer。
- 格式按 magic 识别并统一准备为 PNG；缓存键包含影响输出字节的最大宽高参数；Kitty local-file 只复用无需转换的原始 PNG。

### 8.4 已完成公网 HTTPS 安全下载与异步 TUI 接入

本轮已经完成：

- 可注入 DNS/HTTP 策略、DNS rebinding 防护契约和逐跳复检；
- 重定向、DNS/连接/读取/总时限和严格流式 body 上限；
- 下载结果复用既有图片完整解码与资源限制；
- production HTTP adapter 通过 DNS override 直连已验证地址，关闭自动重定向，并把响应作为受上层读取限制约束的流返回；
- production DNS resolver 优先使用系统解析；若全部结果命中已知 TUN Fake-IP 段，则以有界 DoH A/AAAA 查询替换结果，再交给相同公网地址校验；
- `RemoteImageLoader::production()` 明确组合系统 resolver、pinned adapter 与默认限制；
- 出于 DNS rebinding 边界要求，adapter 当前明确绕过普通 HTTP proxy；TUN/Fake-IP 兼容发生在可校验、可 pin 的 DNS 层，不用普通 CONNECT 主机名重解析替代；
- `MediaLoadCoordinator` 对解析后的本地路径/HTTPS URL 统一去重，本地和远程共享最大并发 2、completion channel、active/history waiter 和 generation retirement；
- TUI production owner 构造 production 远程 loader，远程完成会触发下一帧，history 完成复用既有 source-backed bounded reflow；
- iTerm2 和 Kitty writer 可消费远程 Ready PNG；Kitty local-file 对 HTTPS 强制发送 `t=d` 准备字节，不允许路径引用。

真实 Windows WezTerm HTTPS 复验已经关闭：公网图片显示、后续普通消息保留、私网拒绝、滚动/resize 无残影，以及 runtime Fake-IP fallback、remote Pending→Ready 和 history reflow 均有证据。Phase 2 至此完成。

### 8.5 已完成 LaTeX 渲染

Phase 3 已选择 RaTeX `0.1.14` 并完成行内/块级公式、流式未闭合边界、主题/宽度缓存键、透明 PNG、异步协调、资源限制、原文回退和常用工科样例集。自动化判据与 renderer PNG 检查已关闭。不要重做 Phase 1/2 图片 smoke 或 Phase 3 后端尖峰，也不要同时扩展 Sixel、Kitty Unicode placeholders 或磁盘缓存。

### 8.6 Phase 4 第一单元：运行时状态与开关

- 新增 `/rich-media`、`/rich-media status`、`/rich-media on` 和 `/rich-media off`；命令在主任务、side conversation 和任务运行期间均由 App 层处理，不发给模型。
- 状态卡明确报告启用状态、可用协议、占位行数、RaTeX、远程图片策略和命令格式。
- 启动行为保持兼容：显式 `CODEX_TUI_MEDIA_CAPABILITY_OVERRIDE` 仍会自动启用；无 override 时启动保持文本模式，但记录安全探测出的 WezTerm/Kitty 能力，允许用户在当前 session 执行 `/rich-media on`。
- 关闭前先 retirement 现有 placement，再清除协议并从原始 transcript 重排；重新开启同样从 source-backed transcript 重建，所以控制序列不会进入 Markdown、复制内容或持久化文本。
- TDD 的 RED 为新增命令后 4 处非穷尽 match 编译失败；最小实现后 4 项定向测试通过，覆盖命令路由、on/off 参数、状态卡和关闭后的公式文本回退。
- 完整 `just test -p codex-tui` 运行 3802 项：首轮 3799 通过、3 失败；其中 side conversation 命令清单是本轮新增命令造成的回归，补齐预期清单后定向复验 1/1 通过。最终只保留两项既有 Windows 基线失败：`changing_directory_preserves_project_trust_permissions_history_and_hooks` 与 `kitty_local_file_pet_image_uses_file_reference_without_inline_payload`。
- `just fix -p codex-tui` 成功，仅报告 pets 既有两处 `expect_used` 警告；`just fmt` 因仓库缺少 `tools/buildifier` 失败，随后 `cargo fmt --all -- --check` 以退出码 0 通过。
- Windows WezTerm 实测：显式 iTerm2 override 下，行内/块级公式占位与后续普通文本位置正确；`off` 后历史立即显示包含 LaTeX 源码的文本降级，`on` 后恢复图像占位；`/copy` 得到 133 字符原始 Markdown，包含两段 LaTeX、不含 ESC/OSC 字节；无 override 的独立窗格启动为 off，能探测 iTerm2 inline 并由 `/rich-media on` 成功启用。
- 自动探测测试窗格已关闭；保留 workspace `codex-rich-phase4` 的 pane 1 供人工查看。原 workspace `default` 的 pane 0 未发送任何输入。

### 8.7 Phase 4 小窗口公式裁切修复

- 用户在真实 WezTerm 小窗口中观察到：行内公式只剩右侧碎片，块公式左右被裁切；这不是 Markdown 或 RaTeX 内容错误，而是 iTerm2 inline 控制序列同时指定固定 `width`、`height` 且保留宽高比后，宽公式在浅矩形中的适配行为不符合布局预期。
- 最小修复只改变 LaTeX 的 iTerm2 inline 发射约束：块公式发送 `width=<占位列数>;height=auto`，行内公式发送 `width=auto;height=1`；普通图片继续使用固定宽高，Kitty 路径不变。
- TDD 先让块公式测试因缺少 `height=auto` 失败，再让新增行内公式测试因缺少 `width=auto;height=1` 失败；实现后 `just test -p codex-tui iterm2_writer` 4/4 通过，同时覆盖普通 HTTPS 图片和光标恢复回归。
- 完整 `just test -p codex-tui` 共运行 3803 项，3801 项通过、2 项失败、10 项跳过；失败仍是既有项目权限历史测试和 pets Kitty 本地文件测试，没有新增失败。
- `just fix -p codex-tui` 退出码 0，仅保留 pets 既有两处 `expect_used` 警告；`just fmt` 仍因仓库缺少 `tools/buildifier` 失败，随后 `cargo fmt --all -- --check` 退出码 0。按纪律未在 fix/fmt 后重跑测试。
- 已用 `CARGO_INCREMENTAL=0`、`CARGO_BUILD_JOBS=1` 重新构建 `codex-rs/target/debug/codex.exe`；构建前后 target 均约 15.95 GiB，低于 18 GiB 停止线。真实小窗口视觉复验必须重启旧进程后使用该新二进制。

### 8.8 Phase 4 小窗口公式像素限界修复

- 用户对 8.7 的新二进制做真实 WezTerm 复验后确认：行内公式被压进单行导致分式过小看不清，块公式仍超出小窗口而显示不完整。因此 8.7 的单轴 `auto` 方案不能作为最终修复。
- 新实现让行内公式固定预留两行；iTerm2 writer 从 Ratatui backend 的 `columns_rows` 与 `pixels` 推导当前 cell 像素尺寸，再结合公式 PNG 固有宽高和 placement 矩形计算同时受宽、高约束的等比缩小尺寸。公式控制序列改为显式 `width=<N>px;height=<M>px;preserveAspectRatio=1`，不再发送 `auto`；普通图片固定 cell 尺寸与 Kitty 路径保持不变。终端未报告像素尺寸时采用 8x16 px/cell 的保守回退。
- 布局 TDD 的 RED 精确表现为期望两行而实际只有一行；最小实现与快照更新后定向测试 1/1 通过。writer TDD 的 RED 为新 `TerminalCellPixels` 和带 cell 像素 writer 尚不存在的 E0432 编译错误；实现后 `iterm2_writer` 4/4 通过，随后 `terminal_writer` 11/11 通过，覆盖块级宽度限界、行内两行高度限界、无 `auto`、普通图片、光标恢复和损坏 PNG 回归。
- 完整 `just test -p codex-tui` 共运行 3804 项，3802 项通过、2 项失败、10 项跳过；失败仍是既有项目权限历史测试和 pets Kitty 本地文件测试，没有新增失败。
- `just fix -p codex-tui` 退出码 0，仅保留 pets 既有两处 `expect_used` 警告；`just fmt` 仍因仓库缺少 `tools/buildifier` 失败，随后 `cargo fmt --all -- --check` 退出码 0。按纪律未在 fix/fmt 后重跑测试。
- `cargo build -p codex-cli` 已完成链接，但运行中的旧 `target/debug/codex.exe` 被 Windows 锁定，Cargo 在最终覆盖步骤报 `os error 5`。已将新产物复制为 `codex-rs/target/debug/codex-rich-next.exe`，`--version` 输出 `codex-cli 0.0.0`，时间戳为 2026-08-24 22:21:03；用户复验本轮时应直接启动该独立文件。构建前 target 约 15.95 GiB，低于 18 GiB 停止线。

### 8.9 Phase 4 小窗口公式可读尺寸修复

- 用户对 8.8 的 `codex-rich-next.exe` 做真实 WezTerm 复验后确认：行内和块级公式已不再越界，但两者都明显过小。根因是像素限界函数在 RaTeX PNG 固有尺寸已经落入占位框时直接返回原始尺寸，禁止任何放大；截图中的公式因此保持 32/40 px 字号生成的窄小位图。行内只有两行高度又进一步限制了可读尺寸。
- 最小修复保留 8.8 的宽高双限界和显式 `px` 协议尺寸，但移除“原图已落入边界就保持原尺寸”的提前返回，使公式无论放大或缩小都尽量填满占位框并保持宽高比；行内公式占位由两行增至三行。块级公式继续使用配置中的四行占位，普通图片、Kitty、RaTeX 渲染源、Markdown/copy/persisted text 均不变。
- 布局 TDD 的 RED 为期望三行而实际只有两行；实现后定向测试 1/1 通过。writer TDD 将小尺寸 120x24 PNG 放入 300x60 px 边界，RED 证明旧实现仍发送原始小尺寸；实现后 `iterm2_writer` 4/4、`terminal_writer` 11/11 通过，覆盖小公式放大、行内三行尺寸、普通 HTTPS 图片、光标恢复和 Kitty 回归。
- 完整 `just test -p codex-tui --status-level fail --final-status-level fail` 共运行 3804 项，3802 项通过、2 项失败、10 项跳过；失败仍是既有项目权限历史测试和 pets Kitty 本地文件测试，没有新增失败。
- `just fix -p codex-tui` 退出码 0，仅保留 pets 既有两处 `expect_used` 警告；`just fmt` 仍因仓库缺少 `tools/buildifier` 失败，随后 `cargo fmt --all -- --check` 退出码 0。按纪律未在 fix/fmt 后重跑测试。
- 已用 `CARGO_INCREMENTAL=0`、`CARGO_BUILD_JOBS=1` 成功重建标准 `codex-rs/target/debug/codex.exe`；`--version` 输出 `codex-cli 0.0.0`，时间戳为 2026-08-25 00:35:19。构建后 target 约 16.23 GiB，低于 18 GiB 停止线。

### 8.10 Phase 4 块级公式尺寸与高度缩放保留修复

- 用户用 8.9 标准二进制复验后确认行内公式已可读，但块级公式仍可再放大；窗口只缩短少量高度时，块公式的空白占位和后续文本仍存在，协议图像本身却消失。由此排除 Markdown 解析和布局丢失，定位为无稳定 placement ID 的 iTerm2 inline 图像在多行占位尚未写完时过早发送，后续保留行与高度重排滚动会穿过图像区域。
- 最小修复保持行内公式三行不变，把块级公式占位设为至少六行；history 插入不再在多行 placement 的首行发送媒体，而是在其最后一行写完后相对回到 placement 顶部再发送，避免占位区自身的后续写入破坏图像。普通图片、Kitty、RaTeX 源图、Markdown/copy/persisted text 均不变。
- TDD 先确认两个 RED：块公式传入三行配置时实际高度仍为三而非六；三行 history placement 仍在首行立即发送。最小实现后目标测试 2/2 GREEN；Markdown/history/resize 相邻回归 45/45 通过。
- 完整 `just test -p codex-tui --status-level fail --final-status-level fail` 共运行 3804 项，3802 项通过、2 项失败、10 项跳过；失败仍是既有项目权限历史测试和 pets Kitty 本地文件测试，没有新增失败。
- `just fix -p codex-tui` 退出码 0，仅保留 pets 既有两处 `expect_used` 警告；`just fmt` 仍因仓库缺少 `tools/buildifier` 失败，随后 `cargo fmt --all -- --check` 退出码 0。按纪律未在 fix/fmt 后重跑测试。
- `cargo build -p codex-cli` 已完成链接，但运行中的旧 `target/debug/codex.exe`（PID 22072）被 Windows 锁定，Cargo 在最终覆盖步骤报 `os error 5`。已将本次 `target/debug/deps/codex.exe` 复制为 `target/debug/codex-rich-next.exe`，`--version` 输出 `codex-cli 0.0.0`，时间戳为 2026-08-25 09:03:23；构建后 target 约 16.23 GiB，低于 18 GiB 预警线。

### 8.11 Phase 4 持久配置与缓存清理

- 配置模型新增 `TuiRichMediaConfig { enabled, placeholder_rows }`，对应用户或可信项目配置中的 `[tui.rich_media]`；`placeholder_rows` 仍限制为 1–32。运行时环境变量诊断 override 的优先级最高，显式 `enabled = false` 会关闭显示但保留已探测 capability，便于会话内重新开启。
- `/rich-media clear-cache` 同时清空本地图片 LRU 与 LaTeX LRU，并让仍有 waiter 的来源失效后重新加载。HTTPS 没有磁盘缓存，命令会让活动远程来源重新下载；状态卡报告重新加载的来源数。
- `/rich-media status` 现在同时报告启动时探测到的终端标识、当前 capability/protocol/renderer/remote policy，以及最近一次本地图片、远程图片或 LaTeX 渲染错误；`clear-cache` 会清除旧错误，重新加载失败时记录新错误。
- TDD 分别覆盖配置反序列化、runtime config 应用、环境变量优先级、cache clear、live source generation 失效和两类 LRU 清理；相关定向测试全部通过。

### 8.12 Phase 5 文档与 Skill

- 用户指南位于 `docs/rich-media.md`，覆盖安装、更新、回滚、持久配置、运行时命令、安全边界、故障排查与上游拆分建议；README 已加入入口。
- 仓库内 skill 位于 `.codex/skills/codex-rich-media/SKILL.md`。它要求模型使用标准 Markdown 图片与 `$...$`/`$$...$$`，禁止控制序列、Base64/XML、伪造路径和隐式图片生成。Skill Creator 的 `quick_validate.py` 已在 UTF-8 模式下验证通过。
- 使用已安装二进制对同一请求完成双路径实测：仓库内启用 skill 与空临时目录中无项目 skill 时，均输出标准 Markdown 图片和 `$...$`/`$$...$$`；skill 路径的措辞更严格，但富媒体显示不依赖模型记住终端协议。

### 8.13 Phase 6 构建、安装与上游演练

- `scripts/install-codex-rich.ps1` 支持默认 Release 构建、`-Profile Debug`、`-SourceBinary`、`-CodeModeHostBinary`、`-Rollback` 和 `-NoPathUpdate`；安装到 `%LOCALAPPDATA%\Programs\codex-rich\bin`，同时保留 CLI/host 的 previous，不覆盖官方 `codex.exe`。
- 安装器 smoke 已通过 CLI/host 首次安装、更新保留 previous 和成对 rollback；默认下载路径也完成真实校验。最终使用单任务、禁用 incremental 的 `cargo build -p codex-cli` 成功构建 debug 标准二进制；已重新安装最新源码构建，源文件与安装文件 SHA-256 同为 `57f0c07a4de367e0d1857d48d01fbbe302c75fbde11436b4e6f8ff61a3e56d38`，`codex-rich --version` 输出 `codex-cli 0.0.0`，真实工具调用返回 `901deeefae`，官方 `codex` 仍解析到 OpenAI Codex 安装目录。
- `git fetch upstream` 已刷新到 `upstream/main@2e4675919ee9`。以当前 HEAD 做 `git merge-tree --write-tree` 的只读演练发现 3 个内容冲突：`codex-rs/tui/src/chatwidget/rendering.rs`、`codex-rs/tui/src/markdown_render.rs`、`codex-rs/tui/src/markdown_render/streaming.rs`；其余列出的重叠文件可自动合并。未来同步时先处理这三个 Markdown/渲染热点。
- 本轮 `just fmt` 因仓库缺少 `tools/buildifier` 未能整体完成，但 Rust `cargo fmt` 已执行；`just fix -p codex-config` 会新建 Clippy 构建图并突破磁盘硬限制，因此在 21.66 GiB 时中止并安全清理到限制内。未为掩盖环境限制而重复构建。

### 8.14 最终回归基线

- 两项长期 Windows 基线失败均已按根因关闭：目录信任读写统一使用能展开 8.3 短路径的 canonicalization；Kitty local-file 测试只拒绝真正的 inline payload 边界，不再把 Base64 路径中偶然出现的 `cG5n` 当成 PNG 正文。
- RED 时完整套件为 3806 通过、2 失败、10 跳过；最小修复后两项隔离测试 2/2 通过，最终 `just test -p codex-tui --status-level fail --final-status-level fail` 为 3808/3808 通过、10 跳过。
- `cargo fmt --all -- --check` 与 `git diff --check` 均通过；stable rustfmt 只报告仓库既有 nightly `imports_granularity` 选项提示。

## 9. 尚未完成

- active streaming 瞬间没有单独截图，当前可靠证据集中在 finalized history、滚动、resize/reflow、退出 retirement 与文本降级；
- 当前缓存是有界内存缓存，没有磁盘缓存；这是设计选择，不是交付阻塞项；
- Sixel 编码仍留在 pets 专用实现，尚未完全移入通用媒体层；
- 媒体节点尚未把公式/图片的源字节范围暴露为公共模型字段；
- LaTeX 的真实 WezTerm 公式显示、运行时开关和复制语义已执行；行内三行已由用户确认可读，块级至少六行及高度缩放保留修复已完成自动日志和回归，仍待已安装 `codex-rich` 的最终肉眼确认。
- 总体规划 12.5 要求的真实终端矩阵还缺 Kitty、Windows Terminal 安全降级、不支持图片终端和 SSH/远程环境；自动测试不能替代这些人工观察。当前机器没有 Kitty，且自动桌面控制规则禁止代理操作终端，因此需要用户侧执行并回传现象。
- Phase 0 规划中的 `origin -> 用户自己的 fork` 仍缺用户 fork URL；只有 `upstream`，不能自行猜测或推送。

阶段状态：

| 阶段 | 状态 |
|---|---|
| Phase 0：基线与架构勘察 | 实现完成；`origin` 待用户提供 |
| Phase 1：图片语法、协议、节点与布局 | 已完成 |
| Phase 2：本地/远程图片 I/O 与缓存 | 已完成 |
| Phase 3：LaTeX | 已完成 |
| Phase 4：交互与配置 | 进行中（代码完成，只差最终视觉签字） |
| Phase 5：文档、技能与验收 | 文档/Skill/自动验收完成；真实终端矩阵待用户 |
| Phase 6：打包/交付 | 已完成 |

## 10. Git 工作区注意事项

Windows 下 `git status --short` 当前会显示：

```text
 M justfile
```

但 `git diff --quiet -- justfile` 返回成功且 `git diff -- justfile` 为空，这是 formatter 触发的 LF/CRLF/stat 假阳性。不要暂存、恢复或改写用户的 `justfile`；提交时只精确暂存本轮实际修改的文件。

继续遵守仓库规范：

- 测试使用 `just test`，不要直接运行 `cargo test`；
- TUI 测试优先 `just test -p codex-tui <filter>`；
- 新行为必须先写失败测试，确认 RED 后做最小实现并确认 GREEN；
- 本轮所有文件完成后统一提交，禁止 `git add .`；
- 本轮交接提交信息：`docs: 更新富媒体最终交接`。

## 11. 新对话的起手命令

```powershell
Set-Location D:\hermes\agent-repl\codex-rich
git branch --show-current
git log -7 --oneline
git status --short
git diff --check
git diff --quiet -- justfile
Get-Content -Raw -Encoding utf8 .\RICH_MEDIA_HANDOFF.md
Get-Content -Raw -Encoding utf8 D:\hermes\agent-repl\总体规划.md
```

然后重点读取：

```powershell
Get-Content -Raw -Encoding utf8 .\codex-rs\tui\src\chatwidget\rendering.rs
Get-Content -Raw -Encoding utf8 .\codex-rs\tui\src\app\history_ui.rs
Get-Content -Raw -Encoding utf8 .\codex-rs\tui\src\app\resize_reflow.rs
Get-Content -Raw -Encoding utf8 .\codex-rs\tui\src\tui.rs
```

不要重做已经通过的 Windows WezTerm 本地/HTTPS 图片 smoke、capability override、稳定 anchor、可注入 writer、history insertion-time 锚定、finalized consolidation reflow、普通后续消息保留、滚动/resize、真实 `/resume` 任务切换、空闲 `/clear` retirement、本地异步 TUI 集成、PNG/JPEG/WebP/GIF 静态首帧、HTTPS 安全策略、Fake-IP DoH fallback、production DNS resolver、pinned HTTP adapter、远程 coordinator/writer 自动测试、RaTeX 后端尖峰、LaTeX 解析/缓存/透明覆盖、工科样例集、`/rich-media` 命令路由、持久配置、缓存清理、Skill、安装器、Code Mode host 或前几版小窗口失败复现。下一轮只需使用已安装的 `codex-rich`：关闭块级至少六行与窗口高度缩放的最终肉眼验收，补做 Kitty/Windows Terminal/不支持图片终端/SSH 的真实兼容矩阵，并在用户提供 fork URL 后添加 `origin`。

## 12. Phase 1 完成判据

不能仅凭单元测试或控制序列生成正确就宣布 Phase 1 完成。至少需要同时满足：

- [x] 静态本地 PNG 在真实 WezTerm 的 finalized 助手消息位置显示；
- [x] 文本宽度变化与 resize/reflow 后未观察到位置残影；
- [x] redraw/reflow、滚动和退出后重启未遗留幽灵图片；
- [x] 真实 `/resume` 任务切换和空闲 `/clear` 消息移除已有专门视觉记录；
- [x] 不支持协议时文本降级正常且日志中没有 placement/write；
- [x] raw Markdown、复制内容和持久化 `Line` 不含终端协议字节；
- [x] 相关定向测试通过，完整 `codex-tui` 测试集 3808/3808 通过、10 项按配置跳过；
- [x] 本轮相关文档已同步并纳入精确 Git 提交。
