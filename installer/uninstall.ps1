# ==============================================================================
# rusthost — Windows Uninstaller
# ==============================================================================
Add-Type -AssemblyName System.Windows.Forms

$InstallDir = "$env:LOCALAPPDATA\Programs\rusthost"
$DesktopLnk = "$([Environment]::GetFolderPath('Desktop'))\rusthost.lnk"
$RegPath    = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\rusthost"

Stop-Process -Name "rusthost" -Force -ErrorAction SilentlyContinue

if (Test-Path $DesktopLnk) { Remove-Item $DesktopLnk -Force }
if (Test-Path $RegPath)    { Remove-Item $RegPath -Recurse -Force }
if (Test-Path $InstallDir) { Remove-Item $InstallDir -Recurse -Force }

[System.Windows.Forms.MessageBox]::Show(
    "ถอนการติดตั้งโปรแกรม rusthost ออกจากเครื่องเรียบร้อยแล้ว",
    "rusthost Uninstaller",
    [System.Windows.Forms.MessageBoxButtons]::OK,
    [System.Windows.Forms.MessageBoxIcon]::Information
) | Out-Null
