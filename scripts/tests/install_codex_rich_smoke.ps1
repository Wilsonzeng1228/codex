$ErrorActionPreference = 'Stop'

$installer = Join-Path $PSScriptRoot '..\install-codex-rich.ps1'
$installDir = Join-Path ([System.IO.Path]::GetTempPath()) ("codex-rich-install-test-" + [guid]::NewGuid())
$firstSource = Join-Path $PSHOME 'pwsh.exe'
$secondSource = Join-Path ([System.Environment]::SystemDirectory) 'WindowsPowerShell\v1.0\powershell.exe'
$firstHostSource = Join-Path ([System.Environment]::SystemDirectory) 'where.exe'
$secondHostSource = Join-Path ([System.Environment]::SystemDirectory) 'whoami.exe'

try {
    & $installer -SourceBinary $firstSource -CodeModeHostBinary $firstHostSource -InstallDir $installDir -NoPathUpdate
    $installed = Join-Path $installDir 'codex-rich.exe'
    $installedHost = Join-Path $installDir 'codex-code-mode-host.exe'
    if ((Get-FileHash -LiteralPath $installed).Hash -ne (Get-FileHash -LiteralPath $firstSource).Hash) {
        throw '首次安装内容与源二进制不一致'
    }
    if ((Get-FileHash -LiteralPath $installedHost).Hash -ne (Get-FileHash -LiteralPath $firstHostSource).Hash) {
        throw '首次安装未部署 Code Mode host'
    }

    & $installer -SourceBinary $secondSource -CodeModeHostBinary $secondHostSource -InstallDir $installDir -NoPathUpdate
    $previous = Join-Path $installDir 'codex-rich.previous.exe'
    $previousHost = Join-Path $installDir 'codex-code-mode-host.previous.exe'
    if ((Get-FileHash -LiteralPath $previous).Hash -ne (Get-FileHash -LiteralPath $firstSource).Hash) {
        throw '更新未保留上一版二进制'
    }
    if ((Get-FileHash -LiteralPath $previousHost).Hash -ne (Get-FileHash -LiteralPath $firstHostSource).Hash) {
        throw '更新未保留上一版 Code Mode host'
    }

    & $installer -InstallDir $installDir -Rollback -NoPathUpdate
    if ((Get-FileHash -LiteralPath $installed).Hash -ne (Get-FileHash -LiteralPath $firstSource).Hash) {
        throw '回滚未恢复上一版二进制'
    }
    if ((Get-FileHash -LiteralPath $installedHost).Hash -ne (Get-FileHash -LiteralPath $firstHostSource).Hash) {
        throw '回滚未恢复上一版 Code Mode host'
    }

    'install-codex-rich smoke: PASS'
}
finally {
    if (Test-Path -LiteralPath $installDir) {
        Remove-Item -LiteralPath $installDir -Recurse -Force
    }
}
