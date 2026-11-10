use crate::config::Config;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct TrayController;

impl TrayController {
    pub fn create_desktop_shortcut() -> Result<String, String> {
        let desktop = dirs::desktop_dir().unwrap_or_else(|| PathBuf::from("."));
        let exe_path = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("rusthost"))
            .to_string_lossy()
            .to_string();

        if cfg!(target_os = "windows") {
            let lnk_path = desktop.join("rusthost.lnk");
            let ps_script = format!(
                "$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut('{}'); $s.TargetPath = '{}'; $s.Description = 'rusthost Local Dev Manager'; $s.Save()",
                lnk_path.to_string_lossy(),
                exe_path
            );
            let _ = Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps_script])
                .output();
            Ok(format!(
                "สร้างไอคอนแอปบนหน้าจอ Desktop ({}) เรียบร้อยแล้ว",
                lnk_path.to_string_lossy()
            ))
        } else {
            let app_bundle = desktop.join("rusthost.app");
            let macos_dir = app_bundle.join("Contents/MacOS");
            let res_dir = app_bundle.join("Contents/Resources");
            fs::create_dir_all(&macos_dir).map_err(|e| e.to_string())?;
            fs::create_dir_all(&res_dir).map_err(|e| e.to_string())?;

            let target_bin = macos_dir.join("rusthost");
            let _ = fs::copy(&exe_path, &target_bin);
            let _ = Command::new("chmod")
                .args(["+x", &target_bin.to_string_lossy()])
                .output();

            let icns_bytes = include_bytes!("../ui/AppIcon.icns");
            let _ = fs::write(res_dir.join("AppIcon.icns"), icns_bytes);

            let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>rusthost</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIdentifier</key>
    <string>dev.rusthost.app</string>
    <key>CFBundleName</key>
    <string>rusthost</string>
    <key>CFBundleDisplayName</key>
    <string>rusthost</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>"#;
            fs::write(app_bundle.join("Contents/Info.plist"), plist).map_err(|e| e.to_string())?;
            let _ = Command::new("codesign")
                .args(["--force", "--deep", "--sign", "-", &app_bundle.to_string_lossy()])
                .output();

            Ok(format!(
                "สร้างแอป rusthost.app บนหน้าจอ Desktop ({}) เรียบร้อยแล้ว",
                app_bundle.to_string_lossy()
            ))
        }
    }

    pub fn open_browser(url: &str) -> Result<String, String> {
        let _ = open::that(url);
        Ok(format!("เปิด {}", url))
    }

    pub fn open_phpmyadmin(cfg: &Config) -> Result<String, String> {
        let _ = open::that(&cfg.phpmyadmin_url);
        Ok(format!("เปิด phpMyAdmin ({})", cfg.phpmyadmin_url))
    }
}
