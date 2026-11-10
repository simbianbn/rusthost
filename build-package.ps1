# ==============================================================================
# rusthost — Release Build & ZIP Packager
# ==============================================================================
Write-Host "==> Building rusthost in Release mode..." -ForegroundColor Magenta
cargo build --release

$DistDir = "dist\rusthost-package"
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null

if (Test-Path "target\release\rusthost.exe") {
    Copy-Item "target\release\rusthost.exe" "$DistDir\rusthost.exe" -Force
}
Copy-Item "Install-RustHost.bat" "$DistDir\" -Force
Copy-Item "installer" "$DistDir\installer" -Recurse -Force

Compress-Archive -Path "$DistDir\*" -DestinationPath "dist\rusthost-v1.0.0-win64.zip" -Force
Write-Host "==> Packaged successfully: dist\rusthost-v1.0.0-win64.zip" -ForegroundColor Green
