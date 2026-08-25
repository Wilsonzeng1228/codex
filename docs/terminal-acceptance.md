# codex-rich 真实终端验收矩阵

自动测试不能替代终端内图片与公式的肉眼观察。所有环境使用同一份输入和判据，避免把不同 prompt 的输出误当成协议差异。

## 统一输入

启动 `codex-rich` 后先执行：

```text
/rich-media status
/rich-media on
```

然后发送：

```text
请只输出下面的 Markdown，不要解释，也不要放进代码块。

行内公式：$H(s)=\frac{Y(s)}{X(s)}$

$$
\begin{aligned}
u_1(t) &= U_m\cos(\omega t) \\
u_2(t) &= U_m\cos(\omega t-\frac{2\pi}{3}) \\
u_3(t) &= U_m\cos(\omega t+\frac{2\pi}{3}) \\
i_1(t) &= I_m\cos(\omega t-\varphi) \\
i_2(t) &= I_m\cos(\omega t-\frac{2\pi}{3}-\varphi) \\
i_3(t) &= I_m\cos(\omega t+\frac{2\pi}{3}-\varphi)
\end{aligned}
$$

![acceptance](C:\Users\Wilsonzeng\.codex\artifacts\rich-media-smoke\2026-08-22\02-wezterm-imgcat-works.png)
```

回答进入 finalized history 后，依次缩窄窗口宽度、缩短窗口高度、恢复窗口、上下滚动，再执行 `/rich-media off`。

在 WezTerm 中粘贴上述纯文本请用 `Ctrl+Shift+V`；`Ctrl+V` 是 Codex 的“从剪贴板附加图片”，剪贴板里只有文本时会显示 `Failed to paste image`，这不属于富媒体回答渲染失败。

通过条件：

- `status` 能报告 Terminal、State、Protocol、Renderer、Remote images 和 Last render error；
- 行内公式可读，六行块公式完整且字号接近正文，源码尾部不得越过占位区重新出现；
- 图片和公式随滚动、宽高 resize 留在对应回答位置，无残影；
- `off` 后立即恢复可复制的 Markdown/LaTeX 文本，不出现 ESC/OSC/APC 控制字符；
- 失败时保留原始来源和简短错误，TUI 仍可继续输入、执行工具、退出。

## 环境矩阵

### WezTerm + PowerShell

```powershell
Remove-Item Env:CODEX_TUI_MEDIA_CAPABILITY_OVERRIDE -ErrorAction SilentlyContinue
codex-rich
```

预期自动探测 WezTerm，并在 `/rich-media on` 后使用 iTerm2 inline 路径。该项重点关闭六行块公式与窗口高度 resize 的最终视觉签字。

### Kitty + WSLg

本机已在 `Ubuntu-24.04` 安装 Kitty 0.32.2。从 PowerShell 启动：

```powershell
wsl.exe -d Ubuntu-24.04 -- kitty
```

在新 Kitty 窗口中运行：

```bash
unset CODEX_TUI_MEDIA_CAPABILITY_OVERRIDE
/mnt/c/Users/Wilsonzeng/AppData/Local/Programs/codex-rich/bin/codex-rich.exe
```

预期 Terminal/Protocol 报告 Kitty，图片使用 Kitty graphics protocol；resize、滚动和 `off` 不应遗留旧 image ID。

### Windows Terminal

在 Windows Terminal 的 PowerShell 标签页执行：

```powershell
Remove-Item Env:CODEX_TUI_MEDIA_CAPABILITY_OVERRIDE -ErrorAction SilentlyContinue
codex-rich
```

聊天媒体 MVP 不启用 Sixel。预期状态为不可用或安全文本模式，回答保留 alt、路径和 LaTeX 原文；不得出现乱码、控制字符或布局破坏。

### 显式无能力对照

在一个新的 PowerShell 会话中清除终端标识，再启动：

```powershell
Remove-Item Env:CODEX_TUI_MEDIA_CAPABILITY_OVERRIDE,Env:WEZTERM_PANE,Env:WT_SESSION -ErrorAction SilentlyContinue
$env:TERM = 'dumb'
codex-rich
```

预期 `/rich-media on` 明确报告没有可用 capability，并保持文本降级。

### SSH/远程环境

需要一台用户授权且已安装 `codex-rich` 的 SSH 目标。在真实 SSH 会话中清除诊断 override，重复统一输入。预期不得把本地终端能力误传到远端；即使误判或写入失败，也应保留文本并可用 `/rich-media off` 恢复。

## 结果记录

```text
WezTerm：通过/失败；公式高度 resize：；残影：；Last render error：
Kitty：通过/失败；Protocol：；图片：；resize/滚动：
Windows Terminal：通过/失败；State/Protocol：；乱码或布局破坏：
无能力对照：通过/失败；on 提示：；文本降级：
SSH：通过/失败/无可用目标；State/Protocol：；恢复路径：
```
