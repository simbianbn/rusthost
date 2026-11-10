@echo off
chcp 65001 >nul
title rusthost - Windows Setup Wizard (v1.0.0)
echo ====================================================================
echo   rusthost - Native Local Development Stack Manager Setup (Windows)
echo ====================================================================
echo.

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0..\install.ps1"
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [!] Setup encountered an error. Please run as Administrator.
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo [OK] Setup complete! Launching rusthost...
pause
