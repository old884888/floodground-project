# 血壤 · 快捷启动
$ErrorActionPreference = "Stop"
Set-Location -LiteralPath (Join-Path $PSScriptRoot "..")

Write-Host ""
Write-Host "  血壤 · Bloodsoil" -ForegroundColor Red
Write-Host "  ----------------" -ForegroundColor DarkGray
Write-Host ""

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "[错误] 找不到 cargo。先装 Rust：winget install Rustlang.Rustup" -ForegroundColor Yellow
    Read-Host "按回车退出"
    exit 1
}

cargo run
if ($LASTEXITCODE -ne 0) {
    Write-Host ""
    Write-Host "[错误] 启动失败。看上面的编译信息。" -ForegroundColor Yellow
    Read-Host "按回车退出"
}
