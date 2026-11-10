use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpVersionInfo {
    pub version: String,
    pub full_version: String,
    pub path: String,
    pub php_bin: String,
    pub php_cgi_bin: String,
    pub php_ini: String,
    pub installed: bool,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpExtensionItem {
    pub name: String,
    pub enabled: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpIniQuickSettings {
    pub memory_limit: String,
    pub upload_max_filesize: String,
    pub post_max_size: String,
    pub max_execution_time: String,
    pub display_errors: String,
}

impl Default for PhpIniQuickSettings {
    fn default() -> Self {
        Self {
            memory_limit: "512M".to_string(),
            upload_max_filesize: "128M".to_string(),
            post_max_size: "128M".to_string(),
            max_execution_time: "300".to_string(),
            display_errors: "On".to_string(),
        }
    }
}

pub struct PhpManager;

impl PhpManager {
    pub fn scan_installed_versions(cfg: &Config) -> Vec<PhpVersionInfo> {
        let mut versions = Vec::new();
        let mut detected_sys_ver = String::new();

        if Path::new(&cfg.php_bin).exists() {
            if let Ok(out) = Command::new(&cfg.php_bin).arg("-v").output() {
                let text = String::from_utf8_lossy(&out.stdout);
                if let Some(first_line) = text.lines().next() {
                    // e.g. "PHP 8.5.0 (cli) ..."
                    let parts: Vec<&str> = first_line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        detected_sys_ver = parts[1].to_string();
                    }
                }
            }
        }

        // Scan php_root_dir if it exists
        let root_path = Path::new(&cfg.php_root_dir);
        if root_path.exists() {
            if let Ok(entries) = fs::read_dir(root_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let folder_name = entry.file_name().to_string_lossy().to_string();
                        let short_ver = folder_name
                            .trim_start_matches("php-")
                            .split('.')
                            .take(2)
                            .collect::<Vec<&str>>()
                            .join(".");
                        let php_bin = if cfg!(target_os = "windows") {
                            path.join("php.exe")
                        } else {
                            path.join("bin/php")
                        };
                        let php_cgi = if cfg!(target_os = "windows") {
                            path.join("php-cgi.exe")
                        } else {
                            path.join("bin/php-cgi")
                        };
                        let php_ini = if cfg!(target_os = "windows") {
                            path.join("php.ini")
                        } else {
                            PathBuf::from(&cfg.php_ini_path)
                        };

                        if php_bin.exists() {
                            versions.push(PhpVersionInfo {
                                version: short_ver.clone(),
                                full_version: folder_name,
                                path: path.to_string_lossy().to_string(),
                                php_bin: php_bin.to_string_lossy().to_string(),
                                php_cgi_bin: php_cgi.to_string_lossy().to_string(),
                                php_ini: php_ini.to_string_lossy().to_string(),
                                installed: true,
                                is_active: cfg.active_php_version.starts_with(&short_ver),
                            });
                        }
                    }
                }
            }
        }

        // Standard multi-PHP catalog so user can switch or see supported slots
        let catalog = vec![
            ("7.4", "7.4.33"),
            ("8.1", "8.1.29"),
            ("8.2", "8.2.24"),
            ("8.3", "8.3.12"),
            ("8.4", "8.4.1"),
        ];

        for (short_v, full_v) in catalog {
            if !versions.iter().any(|v| v.version == short_v) {
                let is_sys_match = detected_sys_ver.starts_with(short_v);
                versions.push(PhpVersionInfo {
                    version: short_v.to_string(),
                    full_version: if is_sys_match {
                        detected_sys_ver.clone()
                    } else {
                        full_v.to_string()
                    },
                    path: format!("{}/php-{}", cfg.php_root_dir, short_v),
                    php_bin: if is_sys_match {
                        cfg.php_bin.clone()
                    } else if cfg!(target_os = "windows") {
                        format!("{}/php-{}/php.exe", cfg.php_root_dir, short_v)
                    } else {
                        format!("/opt/homebrew/opt/php@{}/bin/php", short_v)
                    },
                    php_cgi_bin: if cfg!(target_os = "windows") {
                        format!("{}/php-{}/php-cgi.exe", cfg.php_root_dir, short_v)
                    } else {
                        format!("/opt/homebrew/opt/php@{}/bin/php-cgi", short_v)
                    },
                    php_ini: cfg.php_ini_path.clone(),
                    installed: is_sys_match
                        || Path::new(&format!("/opt/homebrew/opt/php@{}/bin/php", short_v)).exists(),
                    is_active: cfg.active_php_version == short_v,
                });
            }
        }

        versions.sort_by(|a, b| b.version.cmp(&a.version));
        versions
    }

    pub fn switch_global_php(cfg: &mut Config, target_version: &str) -> Result<String, String> {
        let versions = Self::scan_installed_versions(cfg);
        if let Some(found) = versions.iter().find(|v| v.version == target_version) {
            cfg.active_php_version = found.version.clone();
            if Path::new(&found.php_bin).exists() {
                cfg.php_bin = found.php_bin.clone();
            }
            if Path::new(&found.php_ini).exists() {
                cfg.php_ini_path = found.php_ini.clone();
            }
            cfg.save()?;
            Ok(format!(
                "สลับเวอร์ชัน Global PHP เป็น PHP {} ({}) เรียบร้อยแล้ว",
                found.version, found.full_version
            ))
        } else {
            cfg.active_php_version = target_version.to_string();
            cfg.save()?;
            Ok(format!("ตั้งค่า Global PHP เป็นเวอร์ชัน {} แล้ว", target_version))
        }
    }

    pub fn ensure_local_php_ini(cfg: &Config) -> PathBuf {
        let path = PathBuf::from(&cfg.php_ini_path);
        if path.exists() {
            return path;
        }
        let fallback = Path::new(&cfg.www_root)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("php.ini");
        if !fallback.exists() {
            let default_ini = r#"; rusthost Managed php.ini
memory_limit = 512M
upload_max_filesize = 128M
post_max_size = 128M
max_execution_time = 300
display_errors = On
extension=mysqli
extension=pdo_mysql
extension=redis
extension=curl
extension=gd
extension=mbstring
extension=openssl
extension=zip
;extension=intl
;extension=soap
"#;
            let _ = fs::write(&fallback, default_ini);
        }
        fallback
    }

    pub fn get_extensions(cfg: &Config) -> Vec<PhpExtensionItem> {
        let ini_path = Self::ensure_local_php_ini(cfg);
        let content = fs::read_to_string(&ini_path).unwrap_or_default();

        let known = vec![
            ("mysqli", "MySQLi Driver สำหรับเชื่อมต่อฐานข้อมูล MySQL"),
            ("pdo_mysql", "PDO MySQL Driver สำหรับ Laravel / ORM"),
            ("redis", "PhpRedis Extension สำหรับเชื่อมต่อ Redis Cache"),
            ("curl", "cURL สำหรับเรียก HTTP API ภายนอก"),
            ("gd", "GD Graphics สำหรับประมวลผลรูปภาพ"),
            ("mbstring", "Multibyte String รองรับภาษาไทยและ UTF-8"),
            ("openssl", "OpenSSL สำหรับการเข้ารหัสและ HTTPS"),
            ("zip", "Zip Archive สำหรับบีบอัดและแตกไฟล์"),
            ("intl", "Internationalization สำหรับแปลภาษาและรูปแบบตัวเลข"),
            ("soap", "SOAP Client/Server สำหรับระบบองค์กร"),
        ];

        known
            .into_iter()
            .map(|(name, desc)| {
                let active_pattern = format!("extension={}", name);
                let active_pattern_spaced = format!("extension = {}", name);
                let mut enabled = true;
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with(';')
                        && (trimmed.contains(&active_pattern)
                            || trimmed.contains(&active_pattern_spaced))
                    {
                        enabled = false;
                        break;
                    } else if !trimmed.starts_with(';')
                        && (trimmed.contains(&active_pattern)
                            || trimmed.contains(&active_pattern_spaced))
                    {
                        enabled = true;
                        break;
                    }
                }
                PhpExtensionItem {
                    name: name.to_string(),
                    enabled,
                    description: desc.to_string(),
                }
            })
            .collect()
    }

    pub fn toggle_extension(cfg: &Config, ext_name: &str, enable: bool) -> Result<String, String> {
        let ini_path = Self::ensure_local_php_ini(cfg);
        let content = fs::read_to_string(&ini_path).unwrap_or_default();
        let mut found = false;
        let target = format!("extension={}", ext_name);

        let mut new_lines: Vec<String> = content
            .lines()
            .map(|line| {
                let t = line.trim();
                if t.ends_with(&target) || t == format!(";{}", target) || t == target {
                    found = true;
                    if enable {
                        target.clone()
                    } else {
                        format!(";{}", target)
                    }
                } else {
                    line.to_string()
                }
            })
            .collect();

        if !found {
            if enable {
                new_lines.push(target);
            } else {
                new_lines.push(format!(";{}", target));
            }
        }

        fs::write(&ini_path, new_lines.join("\n")).map_err(|e| e.to_string())?;
        Ok(format!(
            "{} PHP Extension `{}` ใน {} เรียบร้อยแล้ว",
            if enable { "เปิดใช้งาน" } else { "ปิดใช้งาน" },
            ext_name,
            ini_path.to_string_lossy()
        ))
    }

    pub fn get_php_ini_settings(cfg: &Config) -> PhpIniQuickSettings {
        let ini_path = Self::ensure_local_php_ini(cfg);
        let content = fs::read_to_string(&ini_path).unwrap_or_default();
        let mut settings = PhpIniQuickSettings::default();

        for line in content.lines() {
            let t = line.trim();
            if t.starts_with(';') || !t.contains('=') {
                continue;
            }
            let mut parts = t.splitn(2, '=');
            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                let key = k.trim();
                let val = v.trim().to_string();
                match key {
                    "memory_limit" => settings.memory_limit = val,
                    "upload_max_filesize" => settings.upload_max_filesize = val,
                    "post_max_size" => settings.post_max_size = val,
                    "max_execution_time" => settings.max_execution_time = val,
                    "display_errors" => settings.display_errors = val,
                    _ => {}
                }
            }
        }
        settings
    }

    pub fn update_php_ini_settings(
        cfg: &Config,
        settings: &PhpIniQuickSettings,
    ) -> Result<String, String> {
        let ini_path = Self::ensure_local_php_ini(cfg);
        let content = fs::read_to_string(&ini_path).unwrap_or_default();

        let keys = [
            ("memory_limit", &settings.memory_limit),
            ("upload_max_filesize", &settings.upload_max_filesize),
            ("post_max_size", &settings.post_max_size),
            ("max_execution_time", &settings.max_execution_time),
            ("display_errors", &settings.display_errors),
        ];

        let mut updated_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        for (k, v) in keys {
            let mut replaced = false;
            for line in &mut updated_lines {
                let t = line.trim();
                if !t.starts_with(';') && t.starts_with(k) && t.contains('=') {
                    *line = format!("{} = {}", k, v);
                    replaced = true;
                    break;
                }
            }
            if !replaced {
                updated_lines.push(format!("{} = {}", k, v));
            }
        }

        fs::write(&ini_path, updated_lines.join("\n")).map_err(|e| e.to_string())?;
        Ok(format!(
            "บันทึกค่า php.ini ({}) เรียบร้อยแล้ว",
            ini_path.to_string_lossy()
        ))
    }

    pub fn open_php_ini(cfg: &Config) -> Result<String, String> {
        let ini_path = Self::ensure_local_php_ini(cfg);
        let _ = open::that(&ini_path);
        Ok(format!("เปิดไฟล์ {}", ini_path.to_string_lossy()))
    }
}
