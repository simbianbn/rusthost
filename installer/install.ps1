# ==============================================================================
# rusthost — Windows GUI Setup Wizard & Legacy Stack Cleaner (XAMPP/Laragon/MAMP)
# ==============================================================================
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$ErrorActionPreference = "SilentlyContinue"
$InstallDir = "$env:LOCALAPPDATA\Programs\rusthost"
$BackupDir  = "$env:USERPROFILE\Documents\rusthost-migrated-projects"

# 1. Detect Existing XAMPP / Laragon / MAMP Before Installation
$LegacyStacks = @(
    @{ Name = "XAMPP";   Path = "C:\xampp";   Www = "C:\xampp\htdocs";   Procs = @("xampp-control", "httpd", "mysqld") },
    @{ Name = "Laragon"; Path = "C:\laragon"; Www = "C:\laragon\www";    Procs = @("laragon", "httpd", "mysqld", "redis-server") },
    @{ Name = "MAMP";    Path = "C:\MAMP";    Www = "C:\MAMP\htdocs";    Procs = @("MAMP", "httpd", "mysqld") }
)

foreach ($stack in $LegacyStacks) {
    if (Test-Path $stack.Path) {
        $msg = "ตรวจพบโปรแกรม $($stack.Name) ติดตั้งอยู่ที่ $($stack.Path)`n`nคุณต้องการให้ตัวติดตั้ง rusthost สำรองโปรเจกต์ใน $($stack.Www) แล้วลบโปรแกรม $($stack.Name) ออกจากเครื่องก่อนติดตั้ง เพื่อป้องกันพอร์ต 80/3306 ชนกันหรือไม่?`n`n- กด Yes: สำรองโปรเจกต์ไปที่ $BackupDir แล้วลบ $($stack.Name)`n- กด No: เก็บ $($stack.Name) ไว้ก่อนและติดตั้ง rusthost ต่อ"
        $choice = [System.Windows.Forms.MessageBox]::Show(
            $msg,
            "rusthost Installer — ตรวจพบ $($stack.Name) ในเครื่อง",
            [System.Windows.Forms.MessageBoxButtons]::YesNoCancel,
            [System.Windows.Forms.MessageBoxIcon]::Question
        )

        if ($choice -eq [System.Windows.Forms.DialogResult]::Cancel) {
            exit 0
        }

        if ($choice -eq [System.Windows.Forms.DialogResult]::Yes) {
            foreach ($p in $stack.Procs) {
                Stop-Process -Name $p -Force -ErrorAction SilentlyContinue
            }
            if (Test-Path $stack.Www) {
                New-Item -ItemType Directory -Force -Path $BackupDir | Out-Null
                Copy-Item -Path "$($stack.Www)\*" -Destination $BackupDir -Recurse -Force
            }
            Remove-Item -Path $stack.Path -Recurse -Force
            [System.Windows.Forms.MessageBox]::Show(
                "สำรองโปรเจกต์ไว้ที่ $BackupDir และลบ $($stack.Name) เรียบร้อยแล้ว!",
                "rusthost Cleanup Complete",
                [System.Windows.Forms.MessageBoxButtons]::OK,
                [System.Windows.Forms.MessageBoxIcon]::Information
            ) | Out-Null
        }
    }
}

# 2. Install rusthost
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$SourceExe = Join-Path $PSScriptRoot "..\target\release\rusthost.exe"
if (Test-Path $SourceExe) {
    Copy-Item -Path $SourceExe -Destination "$InstallDir\rusthost.exe" -Force
}

# 3. Create Desktop Shortcut
$DesktopPath = [Environment]::GetFolderPath("Desktop")
$WshShell = New-Object -ComObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut("$DesktopPath\rusthost.lnk")
$Shortcut.TargetPath = "$InstallDir\rusthost.exe"
$Shortcut.WorkingDirectory = $InstallDir
$Shortcut.Description = "rusthost — Local Development & Admin Environment Manager"
$Shortcut.Save()

# 4. Register in Windows Apps & Features Registry
$RegPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\rusthost"
New-Item -Path $RegPath -Force | Out-Null
Set-ItemProperty -Path $RegPath -Name "DisplayName" -Value "rusthost — Local Dev Manager"
Set-ItemProperty -Path $RegPath -Name "DisplayVersion" -Value "1.0.0"
Set-ItemProperty -Path $RegPath -Name "Publisher" -Value "rusthost"
Set-ItemProperty -Path $RegPath -Name "InstallLocation" -Value $InstallDir
Set-ItemProperty -Path $RegPath -Name "UninstallString" -Value "powershell.exe -ExecutionPolicy Bypass -File `"$InstallDir\uninstall.ps1`""

[System.Windows.Forms.MessageBox]::Show(
    "ติดตั้งโปรแกรม rusthost เรียบร้อยแล้ว!`nสร้างไอคอนทางลัดไว้บนหน้าจอ Desktop แล้ว",
    "rusthost Setup Complete",
    [System.Windows.Forms.MessageBoxButtons]::OK,
    [System.Windows.Forms.MessageBoxIcon]::Information
) | Out-Null
