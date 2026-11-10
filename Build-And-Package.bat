@echo off
title Build and Package rusthost
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-package.ps1"
pause
