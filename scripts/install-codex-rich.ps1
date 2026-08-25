[CmdletBinding()]
param(
    [string]$SourceBinary,
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
    if (-not (Test-Path -LiteralPath $currentBinary) -or -not (Test-Path -LiteralPath $previousBinary)) {
        throw '回滚需要 codex-rich.exe 和 codex-rich.previous.exe 同时存在'
    }
    $rollbackTemporary = Join-Path $InstallDir 'codex-rich.rollback.exe'
    if (Test-Path -LiteralPath $rollbackTemporary) {
        Remove-Item -LiteralPath $rollbackTemporary -Force
    }
    Move-Item -LiteralPath $currentBinary -Destination $rollbackTemporary
    try {
        Move-Item -LiteralPath $previousBinary -Destination $currentBinary
        Move-Item -LiteralPath $rollbackTemporary -Destination $previousBinary
    }
    catch {
        if (-not (Test-Path -LiteralPath $currentBinary) -and (Test-Path -LiteralPath $rollbackTemporary)) {
            Move-Item -LiteralPath $rollbackTemporary -Destination $currentBinary
        }
        throw
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

Copy-Item -LiteralPath $SourceBinary -Destination $stagedBinary -Force
try {
    if (Test-Path -LiteralPath $previousBinary) {
        Remove-Item -LiteralPath $previousBinary -Force
    }
    if (Test-Path -LiteralPath $currentBinary) {
        Move-Item -LiteralPath $currentBinary -Destination $previousBinary
    }
    Move-Item -LiteralPath $stagedBinary -Destination $currentBinary
}
catch {
    if (-not (Test-Path -LiteralPath $currentBinary) -and (Test-Path -LiteralPath $previousBinary)) {
        Move-Item -LiteralPath $previousBinary -Destination $currentBinary
    }
    throw
}
finally {
    if (Test-Path -LiteralPath $stagedBinary) {
        Remove-Item -LiteralPath $stagedBinary -Force
    }
}

if (-not $NoPathUpdate) {
    Add-InstallDirToUserPath -Directory $InstallDir
}

Write-Output "已安装：$currentBinary"
Write-Output '启动命令：codex-rich'
Write-Output '官方 Codex 未被覆盖，回退时直接运行：codex'
