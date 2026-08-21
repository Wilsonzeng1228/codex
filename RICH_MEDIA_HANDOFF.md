# Codex TUI 富媒体项目阶段性交接

> 更新时间：2026-08-22
> 当前状态：Phase 1 进行中；已经完成图片语法、来源校验、协议抽象、媒体节点建模和布局请求，尚未把真实位图显示到聊天消息中。

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
- 本轮继续开发前 HEAD：`cb3fa06bdd`
- 远程仓库只有：`upstream https://github.com/openai/codex.git`
- 尚无 `origin`：用户还没有提供 fork 地址，不要自行猜测或推送。
- 父目录的 `D:\hermes\agent-repl\graphify-out` 是未完成的旁路分析产物，没有可查询的 `graph.json`，不属于本仓库，不要加入提交。

最近的实现提交（不含本轮待提交变更）：

```text
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

### 3.3 终端协议与 Kitty 控制序列

- 通用终端图片协议探测已抽到 `media/protocol.rs`。
- 当前协议类型覆盖 `Kitty`、`KittyLocalFile`、`Sixel`，并保留 tmux/Zellij 安全判断。
- pets 功能已经改为复用通用探测结果。
- Kitty PNG 发送与删除控制序列已经抽到 `media/image.rs`。
- 图片 ID 已改为强类型、非零的 `MediaId`。
- iTerm2 行为仍是既有实现，并未作为本阶段的新目标。

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
- 当前两个生产入口都显式传入 `None`，因此用户可见行为仍是安全的文本降级；尚未发送 Kitty/Sixel 字节，也没有图片 I/O 或 placement 注册表。

## 4. 当前架构判断

这是下一阶段最重要的约束：

- 活跃消息由 Ratatui 经 `chatwidget/rendering.rs` 绘制；
- 已提交消息经 `app/history_ui.rs` → `resize_reflow.rs` → `tui.insert_history...` 写入终端 scrollback；
- 因而“每次 frame 绘完后发一个 Kitty overlay”只可能暂时覆盖活跃区，无法天然绑定到已经进入 scrollback 的消息；
- 下一步必须同时考虑活跃消息和已提交历史的布局、重绘、滚动、resize、会话切换和清理生命周期；
- Kitty 控制序列只能作为终端副作用发送，禁止写入原始 Markdown、复制文本或持久化 `Line`。

如果 Kitty 直接 placement 无法可靠锚定 scrollback，应评估 Kitty Unicode placeholders，或调整为由 alternate screen 统一绘制可见 transcript。不要先盲发控制序列再补生命周期。

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

最终通过的测试：

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

验证边界：

- 本轮布局测试先确认缺少布局类型和接口的 RED，再做最小实现并确认 GREEN。
- 完整 `just test -p codex-tui` 共运行 3739 项，3737 项通过、2 项失败、10 项跳过；两项失败单独重跑两次仍失败，分别是未修改的项目权限历史测试（断言得到 `../trusted`）和 pets Kitty 本地文件测试（Base64 输出包含 `cG5n`）。它们不在本轮修改路径内，未为消错扩大修改范围。
- Rust `cargo fmt --all -- --check` 通过，但稳定版会提示 `imports_granularity=Item` 需要 nightly；这不是格式化失败。
- 完整 `just fmt` 在 Bazel/Starlark 步骤失败，因为 Windows 上找不到/无法下载 `tools/buildifier`，错误为 `[WinError 2]`；不能宣称完整格式检查通过。
- `cargo insta pending-snapshots -p codex-tui` 无法执行，因为当前环境没有安装 `cargo-insta` 子命令。
- 一次冗余的 `cargo check -p codex-tui --tests` 因使用另一套缓存、开始重编所有依赖而被手动取消；不要把它记录为代码失败。上述 `just test` 结果才是当前证据。

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
```

环境事实：

- 可用物理内存约 4 GB；并行编译曾在 `protoc-bin-vendored` 附近异常失败，`CARGO_BUILD_JOBS=1` 是必要条件。
- TUI 重新链接常需 2–3 分钟，长时间无输出不等于挂死。
- 最近检查 D 盘剩余约 36.1 GB。
- Rust 为项目锁定的 1.95；`just` 1.58；`cargo-nextest` 0.9.143。
- 当前机器未安装 WezTerm、Kitty、CMake、Ninja。
- 在安装或由用户提供 WezTerm 前，无法完成真实终端图片验收。

## 8. 下一步实施顺序

### 8.1 布局请求已建立，下一步接入 capability override

本轮已经通过 RED/GREEN 覆盖：

1. 图片节点能够在给定宽度下保留 N 行占位，并产出可定位的 rect/row；
2. `MediaNode.ordinal` 能稳定映射到 Markdown 渲染位置；
3. `raw_lines` 和复制文本只包含原始 Markdown，不含 Kitty/Sixel 控制字节；
4. 不支持图片协议时继续使用当前文本降级。

下一功能单元应以显式测试 capability override 让生产绘制入口传入非零占位高度，并把 `MediaLayout.placements` 交给统一生命周期所有者；不要在生命周期就绪前直接向终端盲发协议字节。

### 8.2 建立统一生命周期

- 在 `Tui` 或相邻所有者中维护 `MediaId -> placement` 注册表；
- redraw、resize、scroll、会话切换、退出时必须删除或重建 placement；
- 活跃 Ratatui cell 与提交到 scrollback 的 cell 使用同一个布局语义；
- 先通过显式测试 capability override 实现静态本地 PNG 在活跃聊天区显示；
- 复用现有 `media::kitty_transmit_png_*`，不要复制控制序列；
- 随后解决消息提交到 scrollback 后的锚定问题。

这一部分是 Phase 1 的架构风险点。若不能证明 placement 随 scrollback 正确移动，就不要把仅在当前帧看似正确的 overlay 当作完成。

### 8.3 再进入异步 I/O

完成布局与生命周期后，再实现：

- 本地文件异步读取、解码、缩放与缓存；
- 像素、字节、格式 magic、解码时限等资源上限；
- 公网 HTTPS 下载器；
- DNS 解析后再次拒绝私网/环回/链路本地地址；
- 每次重定向都复检目标，并设置重定向次数、超时与最大响应体。

不要在当前纯解析 resolver 上直接叠一个无边界的 HTTP 请求。

## 9. 尚未完成

- 聊天区尚未显示真实位图；
- 尚未在 WezTerm 中做真实协议验收；
- 尚无本地图片解码、缩放和缓存；
- 尚无远程 HTTPS 下载器和 DNS 级 SSRF 防护；
- Sixel 编码仍留在 pets 专用实现，尚未完全移入通用媒体层；
- 媒体节点尚未记录源字节范围；布局请求已有相对行和矩形，但尚未接入真实协议与生命周期注册表；
- LaTeX 渲染尚未开始；
- 富媒体交互、配置开关、文档、最终打包尚未开始。

阶段状态：

| 阶段 | 状态 |
|---|---|
| Phase 0：基线与架构勘察 | 已完成 |
| Phase 1：图片语法、协议、节点与布局 | 进行中 |
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
- 本轮提交信息：`feat: 建立聊天媒体布局请求`。

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

从 capability override 与 placement 生命周期所有者开始写下一条失败测试，不要重做已经完成的布局请求。

## 12. Phase 1 完成判据

不能仅凭单元测试或控制序列生成正确就宣布 Phase 1 完成。至少需要同时满足：

- 静态本地 PNG 在真实 WezTerm 的助手消息正确位置显示；
- 文本宽度变化与 resize 后位置正确；
- redraw、滚动、切换会话、退出不会遗留幽灵图片；
- 不支持协议时文本降级正常；
- raw Markdown 与复制内容不含终端协议字节；
- 相关定向测试和完整 `codex-tui` 测试集通过；
- 相关文档已同步并完成 Git 提交。
