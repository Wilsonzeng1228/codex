# codex-rich 富媒体指南

`codex-rich` 是 Codex CLI 的富媒体 fork。它保留官方 Codex 的登录、会话、审批和工具工作流，只在 TUI 中为助手 Markdown 增加已有图片与 LaTeX 渲染。官方 `codex` 可以与它并存。

## 安装与回退

在仓库根目录运行：

```powershell
pwsh -File .\scripts\install-codex-rich.ps1
```

脚本默认构建 release 版并安装为 `%LOCALAPPDATA%\Programs\codex-rich\bin\codex-rich.exe`，同时安装工具调用所需的 `codex-code-mode-host.exe`，并把该目录加入用户 `PATH`。host 使用 OpenAI Codex 官方 Windows release 资产，按固定 SHA-256 校验后才会安装。打开新的 PowerShell 后单命令启动：

```powershell
codex-rich
```

安装脚本不会覆盖官方 `codex`。需要立即回退到官方版本时直接运行：

```powershell
codex
```

更新时在新代码上重新执行同一安装命令；安装器会同时保留 `codex-rich.previous.exe` 与 `codex-code-mode-host.previous.exe`。回滚上一版：

```powershell
pwsh -File .\scripts\install-codex-rich.ps1 -Rollback
```

开发验收可用 `-Profile Debug`。已有构建产物也可通过 `-SourceBinary <codex.exe>` 安装；此模式不会重新编译。离线安装或使用自编译 host 时，再传入 `-CodeModeHostBinary <codex-code-mode-host.exe>`。

## 输出格式

- 行内公式：`$H(s)=\frac{Y(s)}{X(s)}$`
- 块公式：独立的 `$$...$$`
- 本地图片：`![示波器波形](D:\measurements\scope.png)`
- 远程图片：`![系统框图](https://example.org/block-diagram.png)`

Windows 原生路径可以直接包含 `.codex` 等隐藏目录；解析器会保护目录前的反斜杠，避免 CommonMark 把 `\.` 当成转义。

图片只引用已有文件；本项目不包含图片生成能力。WezTerm/iTerm2 路径会按图片真实宽高完整缩放到占位矩形内，不需要放大终端字体来避免裁切。即使终端不能显示图像，原始公式、图片 alt 与地址仍保留在历史、复制和持久化文本中。

## 持久配置

官方 Codex 从用户级 `~/.codex/config.toml` 和受信任项目的 `.codex/config.toml` 读取配置；配置层级见 [OpenAI configuration reference](https://developers.openai.com/codex/config-reference)。以下字段是本 fork 的扩展：

```toml
[tui.rich_media]
enabled = true
placeholder_rows = 12
```

- `enabled`：`true` 时在探测到受支持协议后启动富媒体；`false` 时以纯文本启动；省略时保持兼容默认行为。
- `placeholder_rows`：图片占位高度，范围 `1..=32`，默认 12 行，避免宽图及图内文字被压入过矮区域。块公式至少占 6 行，并按每个数学行 3 个终端行自动扩展，同时覆盖完整源码回退；普通 iTerm2 图片会保持宽高比 contain 到这些占位行内。

诊断环境变量的优先级高于持久配置：

```powershell
$env:CODEX_TUI_MEDIA_CAPABILITY_OVERRIDE='iterm2' # 或 kitty
$env:CODEX_TUI_MEDIA_PLACEHOLDER_ROWS='6'
codex-rich
```

这些变量用于 smoke test 和终端兼容诊断，不建议写入长期配置。

## 运行时命令

```text
/rich-media
/rich-media status
/rich-media on
/rich-media off
/rich-media clear-cache
```

`on`/`off` 只影响当前会话，不写配置。`clear-cache` 清空本地图片与 LaTeX 的内存 LRU，使当前 placement 重新加载；HTTPS 图片没有独立磁盘缓存，因此会重新下载。

`status` 会显示启动时探测到的终端标识，以及最近一次本地图片、远程图片或 LaTeX 渲染错误；没有错误记录时显示 `none`。执行 `clear-cache` 会清除旧错误，若重新加载仍失败则记录新的错误。

## 支持与安全边界

- Windows 第一目标终端为 WezTerm；也支持已实现的 Kitty 路径。Sixel 尚未纳入聊天媒体 MVP。
- 本地图片接受 PNG、JPEG、WebP 和 GIF 静态首帧，并在后台解码、缩放为受限 PNG。
- 远程图片只接受公开 HTTPS 地址。每次重定向都重新做地址策略检查，拒绝私网、环回、链路本地和特殊用途地址。
- 默认远程响应上限 16 MiB、总超时 15 秒；解码、像素、尺寸和准备后 PNG 另有上限。
- 无协议能力、坏链接、坏图片、超限或公式解析失败时保留明确文本回退，不向历史文本写入 ESC/OSC/APC 控制字节。

## 故障排查

1. 运行 `/rich-media status`，确认终端标识、状态、协议、占位行数和最近一次渲染错误。
2. 状态为 `unavailable` 时，先确认使用 WezTerm/Kitty 且没有 tmux/Zellij 中间层；必要时用诊断 override 做一次对照。
3. 图像仍旧时运行 `/rich-media clear-cache`。
4. 远程图失败时确认地址为公开 HTTPS，且响应不是重定向到私网或超出限制。
5. 公式或图片不显示时复制消息；若原始 Markdown 正常，问题位于终端能力/协议层而不是会话数据。
6. Windows 构建若报可执行文件 `os error 5`，先退出占用该目标二进制的旧 `codex-rich` 进程，再重试构建或安装。

真实终端发布验收统一使用 [`terminal-acceptance.md`](terminal-acceptance.md)；自动测试不能替代其中的 WezTerm、Kitty、Windows Terminal 和 SSH/远程观察。

## 上游同步与 PR 拆分

同步前先保存工作区并运行：

```powershell
git fetch upstream
git merge-base HEAD upstream/main
git merge-tree (git merge-base HEAD upstream/main) HEAD upstream/main
```

不要用删除上游新行为的方式消冲突。当前改动适合按以下边界评估上游 PR：

1. Markdown 媒体节点、纯文本回退与 source-backed history 语义；
2. 终端能力/placement/writer 生命周期；
3. 受限本地与 HTTPS loader、异步 coordinator；
4. LaTeX renderer 与富媒体配置/命令。

终端协议和 LaTeX 依赖会扩大评审面，不建议把全部 fork 压成单个上游 PR。
