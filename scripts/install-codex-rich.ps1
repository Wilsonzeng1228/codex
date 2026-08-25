[CmdletBinding()]
param(
    [string]$SourceBinary,
    [string]$CodeModeHostBinary,
    [ValidateSet('Debug', 'Release')]
    [string]$Profile = 'Release',
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\codex-rich\bin'),
    [switch]$Rollback,
    [switch]$NoPathUpdate
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    throw 'InstallDir 不能为空'
}
$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)
$currentBinary = Join-Path $InstallDir 'codex-rich.exe'
$previousBinary = Join-Path $InstallDir 'codex-rich.previous.exe'
$stagedBinary = Join-Path $InstallDir 'codex-rich.new.exe'
$currentHostBinary = Join-Path $InstallDir 'codex-code-mode-host.exe'
$previousHostBinary = Join-Path $InstallDir 'codex-code-mode-host.previous.exe'
$stagedHostBinary = Join-Path $InstallDir 'codex-code-mode-host.new.exe'
$downloadedHostBinary = $null

$codeModeHostRelease = 'rust-v0.149.1'
$codeModeHostAssets = @{
    X64 = @{
        Name = 'codex-code-mode-host-x86_64-pc-windows-msvc.exe'
        Sha256 = '8f98cc7aa079b51dbfbb16a8e655a468a9c37c1cd23e22422c10cdfd6cace543'
    }
    Arm64 = @{
        Name = 'codex-code-mode-host-aarch64-pc-windows-msvc.exe'
        Sha256 = '8704eaeb4be03e4b921fcc0cb8be78620d831fc125de85ccb7d6a0fdaf3355da'
    }
}

function Add-InstallDirToUserPath {
    param([Parameter(Mandatory)][string]$Directory)

    $userPath = [System.Environment]::GetEnvironmentVariable('Path', 'User')
    $segments = @($userPath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    if ($segments | Where-Object { $_.TrimEnd('\') -ieq $Directory.TrimEnd('\') }) {
        return
    }

    $updated = (@($Directory) + $segments) -join ';'
    [System.Environment]::SetEnvironmentVariable('Path', $updated, 'User')
    $env:Path = if ([string]::IsNullOrWhiteSpace($env:Path)) {
        $Directory
    }
    else {
        "$Directory;$env:Path"
    }
}

New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

if ($Rollback) {
    $rollbackFiles = @($currentBinary, $previousBinary, $currentHostBinary, $previousHostBinary)
    if ($rollbackFiles | Where-Object { -not (Test-Path -LiteralPath $_ -PathType Leaf) }) {
        throw '回滚需要当前版和上一版的 codex-rich 与 Code Mode host 同时存在'
    }

    $rollbackCurrentBinary = Join-Path $InstallDir 'codex-rich.rollback-current.exe'
    $rollbackPreviousBinary = Join-Path $InstallDir 'codex-rich.rollback-previous.exe'
    $rollbackCurrentHost = Join-Path $InstallDir 'codex-code-mode-host.rollback-current.exe'
    $rollbackPreviousHost = Join-Path $InstallDir 'codex-code-mode-host.rollback-previous.exe'
    $rollbackStaging = @(
        $rollbackCurrentBinary,
        $rollbackPreviousBinary,
        $rollbackCurrentHost,
        $rollbackPreviousHost
    )
    try {
        Copy-Item -LiteralPath $currentBinary -Destination $rollbackCurrentBinary -Force
        Copy-Item -LiteralPath $previousBinary -Destination $rollbackPreviousBinary -Force
        Copy-Item -LiteralPath $currentHostBinary -Destination $rollbackCurrentHost -Force
        Copy-Item -LiteralPath $previousHostBinary -Destination $rollbackPreviousHost -Force

        Copy-Item -LiteralPath $rollbackPreviousBinary -Destination $currentBinary -Force
        Copy-Item -LiteralPath $rollbackPreviousHost -Destination $currentHostBinary -Force
        Copy-Item -LiteralPath $rollbackCurrentBinary -Destination $previousBinary -Force
        Copy-Item -LiteralPath $rollbackCurrentHost -Destination $previousHostBinary -Force
    }
    catch {
        if (Test-Path -LiteralPath $rollbackCurrentBinary) {
            Copy-Item -LiteralPath $rollbackCurrentBinary -Destination $currentBinary -Force
        }
        if (Test-Path -LiteralPath $rollbackPreviousBinary) {
            Copy-Item -LiteralPath $rollbackPreviousBinary -Destination $previousBinary -Force
        }
        if (Test-Path -LiteralPath $rollbackCurrentHost) {
            Copy-Item -LiteralPath $rollbackCurrentHost -Destination $currentHostBinary -Force
        }
        if (Test-Path -LiteralPath $rollbackPreviousHost) {
            Copy-Item -LiteralPath $rollbackPreviousHost -Destination $previousHostBinary -Force
        }
        throw
    }
    finally {
        foreach ($path in $rollbackStaging) {
            if (Test-Path -LiteralPath $path) {
                Remove-Item -LiteralPath $path -Force
            }
        }
    }
    Write-Output "已回滚：$currentBinary"
    exit 0
}

if ([string]::IsNullOrWhiteSpace($SourceBinary)) {
    $repoRoot = Split-Path -Parent $PSScriptRoot
    $rustRoot = Join-Path $repoRoot 'codex-rs'
    $profileArgs = if ($Profile -eq 'Release') { @('--release') } else { @() }
    $env:CARGO_BUILD_JOBS = '1'
    $env:CARGO_INCREMENTAL = '0'

    Push-Location $rustRoot
    try {
        & cargo build -p codex-cli @profileArgs
        if ($LASTEXITCODE -ne 0) {
            throw "codex-cli 构建失败，退出码 $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }

    $profileDirectory = if ($Profile -eq 'Release') { 'release' } else { 'debug' }
    $targetRoot = if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
        Join-Path $rustRoot 'target'
    }
    elseif ([System.IO.Path]::IsPathRooted($env:CARGO_TARGET_DIR)) {
        $env:CARGO_TARGET_DIR
    }
    else {
        Join-Path $rustRoot $env:CARGO_TARGET_DIR
    }
    $SourceBinary = Join-Path $targetRoot "$profileDirectory\codex.exe"
}

$SourceBinary = [System.IO.Path]::GetFullPath($SourceBinary)
if (-not (Test-Path -LiteralPath $SourceBinary -PathType Leaf)) {
    throw "找不到待安装二进制：$SourceBinary"
}

if ([string]::IsNullOrWhiteSpace($CodeModeHostBinary)) {
    $architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    $asset = $codeModeHostAssets[$architecture]
    if ($null -eq $asset) {
        throw "不支持的 Windows 架构：$architecture；请用 -CodeModeHostBinary 显式指定 host"
    }

    $downloadedHostBinary = Join-Path ([System.IO.Path]::GetTempPath()) (
        'codex-code-mode-host-' + [guid]::NewGuid().ToString('N') + '.exe'
    )
    $downloadUrl = "https://github.com/openai/codex/releases/download/$codeModeHostRelease/$($asset.Name)"
    Write-Output "正在下载官方 Code Mode host：$codeModeHostRelease / $($asset.Name)"
    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $downloadedHostBinary -UseBasicParsing
        $actualDigest = (Get-FileHash -Algorithm SHA256 -LiteralPath $downloadedHostBinary).Hash
        if ($actualDigest -ine $asset.Sha256) {
            throw "Code Mode host SHA-256 校验失败：期望 $($asset.Sha256)，实际 $actualDigest"
        }
    }
    catch {
        if (Test-Path -LiteralPath $downloadedHostBinary) {
            Remove-Item -LiteralPath $downloadedHostBinary -Force
        }
        throw
    }
    $CodeModeHostBinary = $downloadedHostBinary
}

$CodeModeHostBinary = [System.IO.Path]::GetFullPath($CodeModeHostBinary)
if (-not (Test-Path -LiteralPath $CodeModeHostBinary -PathType Leaf)) {
    throw "找不到 Code Mode host：$CodeModeHostBinary"
}

$hadCurrentBinary = Test-Path -LiteralPath $currentBinary -PathType Leaf
$hadCurrentHost = Test-Path -LiteralPath $currentHostBinary -PathType Leaf
try {
    Copy-Item -LiteralPath $SourceBinary -Destination $stagedBinary -Force
    Copy-Item -LiteralPath $CodeModeHostBinary -Destination $stagedHostBinary -Force

    if ($hadCurrentBinary) {
        Copy-Item -LiteralPath $currentBinary -Destination $previousBinary -Force
    }
    if ($hadCurrentHost) {
        Copy-Item -LiteralPath $currentHostBinary -Destination $previousHostBinary -Force
    }
    elseif ($hadCurrentBinary) {
        Copy-Item -LiteralPath $stagedHostBinary -Destination $previousHostBinary -Force
    }

    Copy-Item -LiteralPath $stagedBinary -Destination $currentBinary -Force
    Copy-Item -LiteralPath $stagedHostBinary -Destination $currentHostBinary -Force
}
catch {
    if ($hadCurrentBinary -and (Test-Path -LiteralPath $previousBinary)) {
        Copy-Item -LiteralPath $previousBinary -Destination $currentBinary -Force
    }
    elseif (-not $hadCurrentBinary -and (Test-Path -LiteralPath $currentBinary)) {
        Remove-Item -LiteralPath $currentBinary -Force
    }
    if ($hadCurrentHost -and (Test-Path -LiteralPath $previousHostBinary)) {
        Copy-Item -LiteralPath $previousHostBinary -Destination $currentHostBinary -Force
    }
    elseif (-not $hadCurrentHost -and (Test-Path -LiteralPath $currentHostBinary)) {
        Remove-Item -LiteralPath $currentHostBinary -Force
    }
    throw
}
finally {
    if (Test-Path -LiteralPath $stagedBinary) {
        Remove-Item -LiteralPath $stagedBinary -Force
    }
    if (Test-Path -LiteralPath $stagedHostBinary) {
        Remove-Item -LiteralPath $stagedHostBinary -Force
    }
    if ($null -ne $downloadedHostBinary -and (Test-Path -LiteralPath $downloadedHostBinary)) {
        Remove-Item -LiteralPath $downloadedHostBinary -Force
    }
}

if (-not $NoPathUpdate) {
    Add-InstallDirToUserPath -Directory $InstallDir
}

Write-Output "已安装：$currentBinary"
Write-Output "Code Mode host：$currentHostBinary"
Write-Output '启动命令：codex-rich'
Write-Output '官方 Codex 未被覆盖，回退时直接运行：codex'
