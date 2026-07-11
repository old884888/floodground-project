@echo off
chcp 65001 >nul
cd /d "%~dp0.."

echo.
echo   血壤 · Bloodsoil
echo   ----------------
echo.

where cargo >nul 2>&1
if errorlevel 1 (
    echo [错误] 找不到 cargo。先装 Rust：winget install Rustlang.Rustup
    pause
    exit /b 1
)

cargo run
if errorlevel 1 (
    echo.
    echo [错误] 启动失败。看上面的编译信息。
    pause
)
