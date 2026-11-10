; Inno Setup Script for rusthost (Windows x64 Native Installer)
#define MyAppName "rusthost"
#define MyAppVersion "1.0.0"
#define MyAppPublisher "simbianbn"
#define MyAppURL "https://github.com/simbianbn/rusthost"
#define MyAppExeName "rusthost.exe"

[Setup]
AppId={{8F9D4A12-5C3E-4B8F-9A11-7E2D3C4B5A6F}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName=C:\RustHost
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\..\dist
OutputBaseFilename=rusthost-windows-x64-setup
SetupIconFile=..\..\ui\icon.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "..\..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\ui\icon.ico"; DestDir: "{app}\ui"; Flags: ignoreversion
Source: "..\..\ui\icon.png"; DestDir: "{app}\ui"; Flags: ignoreversion
Source: "..\install.ps1"; DestDir: "{app}\installer"; Flags: ignoreversion
Source: "..\uninstall.ps1"; DestDir: "{app}\installer"; Flags: ignoreversion

[Dirs]
Name: "{app}\www"
Name: "{app}\ssl"
Name: "{app}\logs"
Name: "{app}\backups"

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\ui\icon.ico"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\ui\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
