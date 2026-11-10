# ==============================================================================
# Build rusthost.exe & Compile Windows Setup Installer (rusthost-windows-x64-setup.exe)
# ==============================================================================
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..\..")

Write-Host "==> [1/3] Compiling rusthost.exe (Release)..." -ForegroundColor Cyan
cargo build --release

Write-Host "==> [2/3] Packaging Portable Windows Folder..." -ForegroundColor Cyan
$DistDir = "dist\rusthost-windows"
New-Item -ItemType Directory -Force -Path "$DistDir\ui" | Out-Null
New-Item -ItemType Directory -Force -Path "$DistDir\installer\windows" | Out-Null
Copy-Item "target\release\rusthost.exe" "$DistDir\rusthost.exe" -Force
Copy-Item "ui\icon.ico" "$DistDir\ui\icon.ico" -Force
Copy-Item "ui\icon.png" "$DistDir\ui\icon.png" -Force
Copy-Item "installer\install.ps1" "$DistDir\installer\install.ps1" -Force
Copy-Item "installer\uninstall.ps1" "$DistDir\installer\uninstall.ps1" -Force
Copy-Item "installer\windows\RustHost-Windows-Setup.bat" "$DistDir\RustHost-Windows-Setup.bat" -Force
Compress-Archive -Path "$DistDir\*" -DestinationPath "dist\rusthost-windows-portable-x64.zip" -Force

Write-Host "==> [3/3] Checking Inno Setup Compiler (iscc)..." -ForegroundColor Cyan
$Iscc = Get-Command iscc -ErrorAction SilentlyContinue
if ($Iscc) {
    iscc installer\windows\rusthost-setup.iss
    Write-Host "Built: dist\rusthost-windows-x64-setup.exe" -ForegroundColor Green
} else {
    Write-Host "Built: dist\rusthost-windows-portable-x64.zip (Install Inno Setup to also build .exe installer)" -ForegroundColor Green
}
