$ErrorActionPreference = 'Stop'

$installer = Join-Path $PSScriptRoot '..\install-codex-rich.ps1'
$installDir = Join-Path ([System.IO.Path]::GetTempPath()) ("codex-rich-install-test-" + [guid]::NewGuid())
$firstSource = Join-Path $PSHOME 'pwsh.exe'
$secondSource = Join-Path ([System.Environment]::SystemDirectory) 'WindowsPowerShell\v1.0\powershell.exe'

try {
    & $installer -SourceBinary $firstSource -InstallDir $installDir -NoPathUpdate
    $installed = Join-Path $installDir 'codex-rich.exe'
    if ((Get-FileHash -LiteralPath $installed).Hash -ne (Get-FileHash -LiteralPath $firstSource).Hash) {
        throw '首次安装内容与源二进制不一致'
    }

    & $installer -SourceBinary $secondSource -InstallDir $installDir -NoPathUpdate
    $previous = Join-Path $installDir 'codex-rich.previous.exe'
    if ((Get-FileHash -LiteralPath $previous).Hash -ne (Get-FileHash -LiteralPath $firstSource).Hash) {
        throw '更新未保留上一版二进制'
    }

    & $installer -InstallDir $installDir -Rollback -NoPathUpdate
    if ((Get-FileHash -LiteralPath $installed).Hash -ne (Get-FileHash -LiteralPath $firstSource).Hash) {
        throw '回滚未恢复上一版二进制'
    }

    'install-codex-rich smoke: PASS'
}
finally {
    if (Test-Path -LiteralPath $installDir) {
        Remove-Item -LiteralPath $installDir -Recurse -Force
    }
}
