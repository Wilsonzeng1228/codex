# Codex TUI 富媒体项目阶段性交接

> 更新时间：2026-08-23
> 当前状态：Phase 1 核心路径已通过 Windows WezTerm 真实 smoke。Windows 版 WezTerm 使用显式 `iterm2` override 后，静态本地 PNG 能在 finalized assistant history 中显示；滚动、窗口宽高调整、重复 reflow 和退出后重启均未观察到幽灵图片。空 override 已真实确认回到纯文本降级且不生成 placement。任务切换/消息移除仍缺一轮专门的人工视觉操作，因此继续保持 Phase 1“进行中”，不扩大到网络下载、缓存、Sixel 或 LaTeX。

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
- 本轮继续开发前 HEAD：`0185077629697c1743d14690d434fc6be8e1495e feat: 锚定聊天媒体到终端历史`
- 远程仓库只有：`upstream https://github.com/openai/codex.git`
- 尚无 `origin`：用户还没有提供 fork 地址，不要自行猜测或推送。
- 父目录的 `D:\hermes\agent-repl\graphify-out` 是未完成的旁路分析产物，没有可查询的 `graph.json`，不属于本仓库，不要加入提交。

最近的实现提交（不含本轮待提交变更）：

```text
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

注意：resolver 目前只做纯解析，不访问文件和网络。它还没有 DNS 解析后的私网复检，因此远程下载器不能直接把“语法校验通过”等同于“SSRF 防护完成”。

### 3.3 终端协议与控制序列

- 通用终端图片协议探测已抽到 `media/protocol.rs`。
- 当前协议类型覆盖 `Iterm2Inline`、`Kitty`、`KittyLocalFile`、`Sixel`，并保留 tmux/Zellij 安全判断。
- 实测 Windows WezTerm 的 Kitty APC 不工作；Windows WezTerm 改用 iTerm2 OSC 1337 inline，使用 `ESC \\` 结束且允许协议移动光标，再由 writer 保存/恢复终端光标。
- Windows WezTerm 的聊天图片显式 override 为 `CODEX_TUI_MEDIA_CAPABILITY_OVERRIDE=iterm2`；非 Windows WezTerm 仍选择 Kitty。
- pets 功能已经改为复用通用探测结果。
- iTerm2 inline 没有本项目可用的 Kitty image-ID 删除语义，动画 pets 会被写入 scrollback 并形成残影。因此 Windows WezTerm 上 pets 明确拒绝 `Iterm2Inline`，保持不显示；聊天静态图片仍可使用该协议。
- Kitty PNG 发送与删除控制序列已经抽到 `media/image.rs`。
- 图片 ID 已改为强类型、非零的 `MediaId`。
- iTerm2 chat writer 只处理通过 8 字节 PNG signature 校验的静态本地文件；未扩展公网下载或其他格式。

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
- writer 当前只接受本地来源且要求 8 字节 PNG magic；HTTPS、拒绝来源和伪 PNG 均继续文本降级。协议字节只写入终端 sink，不进入 Ratatui `Line`、raw Markdown、复制文本或持久化 transcript。
- 注册表的更新结果显式给出 `retired` 与 `added` MediaId，为后续复用 Kitty 删除/发送接口提供边界。
- 流式 assistant cell 会先写入文本 fallback；final consolidation 若发现媒体节点且 capability 开启，会强制一次 source-backed reflow，使图片占位和 placement 真正进入 history。该缺口已用 `0` placement 的 RED 和 `1` placement 的 GREEN 覆盖。
- Windows WezTerm 真实日志已证明初次 history placement、三次 resize/reflow 重建和退出 retirement；用户确认图片显示并且上下滚动、调整窗口后无残影。

## 4. 当前架构判断

这是下一阶段最重要的约束：

- 活跃消息由 Ratatui 经 `chatwidget/rendering.rs` 绘制；
- 已提交消息经 `app/history_ui.rs` → `resize_reflow.rs` → `tui.insert_history...` 写入终端 scrollback；
- 因而“每次 frame 绘完后发一个 Kitty overlay”只可能暂时覆盖活跃区，无法天然绑定到已经进入 scrollback 的消息；
- 活跃消息和已提交历史现在共享布局语义与 `Tui` 生命周期注册表，但终端坐标锚定仍未得到真实终端证明；
- Kitty 控制序列只能作为终端副作用发送，禁止写入原始 Markdown、复制文本或持久化 `Line`。

当前基础设施采用双路径：active 使用当前 frame 绝对坐标，history 在其保留行进入 terminal scrollback 的同一时刻发送 placement。Windows WezTerm 的 finalized history 路径已通过真实滚动与 resize 验收，direct iTerm2 inline placement 能随 scrollback 移动。active 布局和 retirement 有自动测试，但 active streaming 瞬间、任务切换和消息移除尚未分别做专门人工截图；在这些场景完成前，不应把 Phase 1 的全部视觉判据写成无保留完成。

## 5. 关键文件

媒体层：

- `codex-rs/tui/src/media/mod.rs`
- `codex-rs/tui/src/media/image.rs`
- `codex-rs/tui/src/media/image_tests.rs`
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
- 可注入 writer 已验证删除顺序、active 绝对 `Rect`、远程来源跳过、伪 PNG 跳过和 request 不被协议字节污染；
- history writer 在保留行进入 scrollback 时发送，只使用列定位；
- scoped reflow 保留未参与本次重建的旧 scrollback placement。

Windows WezTerm 核心 smoke 已完成：finalized history 图片、滚动、resize/reflow、退出 retirement 和无 override 文本降级均通过。下一步若继续收口 Phase 1，只需专门补做任务切换/消息移除的人工视觉操作，并保存截图或日志；不要重复已经通过的核心 smoke，也不要在这一缺口关闭前扩大到网络下载、缓存、Sixel 或 LaTeX。

### 8.3 再进入异步 I/O

完成布局与生命周期后，再实现：

- 本地文件异步读取、解码、缩放与缓存；
- 像素、字节、格式 magic、解码时限等资源上限；
- 公网 HTTPS 下载器；
- DNS 解析后再次拒绝私网/环回/链路本地地址；
- 每次重定向都复检目标，并设置重定向次数、超时与最大响应体。

不要在当前纯解析 resolver 上直接叠一个无边界的 HTTP 请求。

## 9. 尚未完成

- 聊天区的 Windows WezTerm iTerm2 inline 静态本地 PNG 核心视觉验收已通过；任务切换/消息移除仍缺专门人工操作记录；
- active streaming 瞬间没有单独截图，当前可靠证据集中在 finalized history、滚动、resize/reflow、退出 retirement 与文本降级；
- 尚无本地图片解码、缩放和缓存；
- 尚无远程 HTTPS 下载器和 DNS 级 SSRF 防护；
- Sixel 编码仍留在 pets 专用实现，尚未完全移入通用媒体层；
- 媒体节点尚未记录源字节范围；生命周期更新已能发送/删除 Kitty placement，但本地文件仍在同步路径读取，尚无解码、缩放、缓存和资源上限；
- LaTeX 渲染尚未开始；
- 富媒体交互、配置开关、文档、最终打包尚未开始。

阶段状态：

| 阶段 | 状态 |
|---|---|
| Phase 0：基线与架构勘察 | 已完成 |
| Phase 1：图片语法、协议、节点与布局 | 进行中（Windows WezTerm 核心 smoke 已通过） |
| Phase 2：本地/远程图片 I/O 与缓存 | 待开始（仅来源解析已提前完成） |
| Phase 3：LaTeX | 待开始 |
| Phase 4：交互与配置 | 待开始 |
| Phase 5：文档、技能与验收 | 待开始 |
| Phase 6：打包/交付 | 待开始 |

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
- 本轮建议提交信息：`feat: 支持 Windows WezTerm 聊天图片`。

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

不要重做已经通过的 Windows WezTerm 核心 smoke、capability override、稳定 anchor、可注入 writer、history insertion-time 锚定或 finalized consolidation reflow。若继续 Phase 1，优先专门验证任务切换/消息移除；若该场景失败，先记录 iTerm2 inline placement 的具体生命周期缺口，再决定最小修复。

## 12. Phase 1 完成判据

不能仅凭单元测试或控制序列生成正确就宣布 Phase 1 完成。至少需要同时满足：

- [x] 静态本地 PNG 在真实 WezTerm 的 finalized 助手消息位置显示；
- [x] 文本宽度变化与 resize/reflow 后未观察到位置残影；
- [x] redraw/reflow、滚动和退出后重启未遗留幽灵图片；
- [ ] 任务切换或消息移除仍需一轮专门视觉记录；
- [x] 不支持协议时文本降级正常且日志中没有 placement/write；
- [x] raw Markdown、复制内容和持久化 `Line` 不含终端协议字节；
- [x] 相关定向测试通过，完整 `codex-tui` 测试集只保留两项既有基线失败；
- [x] 本轮相关文档已同步并纳入精确 Git 提交。
