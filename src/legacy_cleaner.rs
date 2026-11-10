use crate::config::{Config, ProjectConfig, RouteMode};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyStackItem {
    pub id: String,
    pub name: String,
    pub install_path: String,
    pub www_path: String,
    pub detected: bool,
    pub estimated_size_mb: u64,
    pub project_count: usize,
    pub project_names: Vec<String>,
    pub status_note: String,
}

pub struct LegacyStackCleaner;

impl LegacyStackCleaner {
    pub fn scan_all() -> Vec<LegacyStackItem> {
        let candidates = if cfg!(target_os = "macos") {
            vec![
                (
                    "xampp",
                    "XAMPP",
                    "/Applications/XAMPP",
                    "/Applications/XAMPP/xamppfiles/htdocs",
                ),
                (
                    "mamp",
                    "MAMP / MAMP PRO",
                    "/Applications/MAMP",
                    "/Applications/MAMP/htdocs",
                ),
                (
                    "laragon",
                    "Laragon (Wine/CrossOver)",
                    "/Applications/Laragon",
                    "/Applications/Laragon/www",
                ),
            ]
        } else {
            vec![
                ("xampp", "XAMPP", "C:/xampp", "C:/xampp/htdocs"),
                ("laragon", "Laragon", "C:/laragon", "C:/laragon/www"),
                ("mamp", "MAMP", "C:/MAMP", "C:/MAMP/htdocs"),
            ]
        };

        let mut results = Vec::new();
        for (id, name, path_str, www_str) in candidates {
            let p = Path::new(path_str);
            let detected = p.exists();
            let mut project_names = Vec::new();

            if detected {
                let www_p = Path::new(www_str);
                if www_p.exists() {
                    if let Ok(entries) = fs::read_dir(www_p) {
                        for e in entries.flatten() {
                            if e.path().is_dir() {
                                let fname = e.file_name().to_string_lossy().to_string();
                                if !fname.starts_with('.')
                                    && fname != "dashboard"
                                    && fname != "xampp"
                                    && fname != "img"
                                    && fname != "webalizer"
                                {
                                    project_names.push(fname);
                                }
                            }
                        }
                    }
                }
            }

            let size_mb = if detected {
                Self::estimate_dir_size_mb(p, 2).max(120)
            } else {
                0
            };

            let status_note = if detected {
                format!(
                    "ตรวจพบในเครื่อง ({} MB, {} โปรเจกต์ใน htdocs/www)",
                    size_mb,
                    project_names.len()
                )
            } else {
                "ไม่พบในเครื่อง (สะอาด)".to_string()
            };

            results.push(LegacyStackItem {
                id: id.to_string(),
                name: name.to_string(),
                install_path: path_str.to_string(),
                www_path: www_str.to_string(),
                detected,
                estimated_size_mb: size_mb,
                project_count: project_names.len(),
                project_names,
                status_note,
            });
        }

        results
    }

    fn estimate_dir_size_mb(path: &Path, depth: u8) -> u64 {
        if depth == 0 || !path.exists() {
            return 0;
        }
        let mut bytes = 0u64;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        bytes += meta.len();
                    } else if meta.is_dir() {
                        bytes += Self::estimate_dir_size_mb(&p, depth - 1) * 1024 * 1024;
                    }
                }
            }
        }
        bytes / (1024 * 1024)
    }

    pub fn handle_legacy_action(
        cfg: &mut Config,
        stack_id: &str,
        action: &str, // "backup_and_remove" | "remove_only" | "import_projects"
    ) -> Result<String, String> {
        let items = Self::scan_all();
        let item = items
            .into_iter()
            .find(|i| i.id == stack_id)
            .ok_or_else(|| format!("ไม่พบข้อมูลของโปรแกรม {}", stack_id))?;

        if !item.detected {
            cfg.first_run_cleanup_checked = true;
            let _ = cfg.save();
            return Ok(format!(
                "{} ไม่ได้ติดตั้งอยู่ในเครื่องนี้ (ไม่ต้องลบ)",
                item.name
            ));
        }

        let mut migrated = 0usize;
        if action == "backup_and_remove" || action == "import_projects" {
            let src_www = Path::new(&item.www_path);
            let dst_www = PathBuf::from(&cfg.www_root);
            let _ = fs::create_dir_all(&dst_www);

            for proj_name in &item.project_names {
                let src_dir = src_www.join(proj_name);
                let dst_dir = dst_www.join(proj_name);
                if src_dir.exists() && !dst_dir.exists() {
                    let _ = Self::copy_dir_recursive(&src_dir, &dst_dir);
                }
                if !cfg.projects.iter().any(|p| p.name == *proj_name) {
                    cfg.projects.push(ProjectConfig {
                        id: format!("proj-{}", proj_name),
                        name: proj_name.clone(),
                        alias_path: format!("/{}", proj_name),
                        domain: format!("{}.test", proj_name),
                        document_root: dst_dir.to_string_lossy().to_string(),
                        route_mode: RouteMode::Both,
                        php_version: cfg.active_php_version.clone(),
                        ssl_enabled: true,
                        allow_override: true,
                        directory_indexes: true,
                        directory_index_files: "index.php index.html".to_string(),
                        proxy_target_port: None,
                        env_vars: HashMap::new(),
                        custom_directives: format!("# Migrated from {}", item.name),
                        enabled: true,
                    });
                    migrated += 1;
                }
            }
        }

        if action == "backup_and_remove" || action == "remove_only" {
            // Stop any running processes from legacy stack first
            if cfg!(target_os = "windows") {
                for exe in ["xampp-control.exe", "laragon.exe", "MAMP.exe"] {
                    let _ = Command::new("taskkill").args(["/F", "/IM", exe]).output();
                }
            }

            let install_p = Path::new(&item.install_path);
            if install_p.exists() {
                if let Err(e) = fs::remove_dir_all(install_p) {
                    cfg.first_run_cleanup_checked = true;
                    let _ = cfg.save();
                    return Err(format!(
                        "สำรองโปรเจกต์แล้ว {} รายการ แต่การลบโฟลเดอร์ `{}` ต้องใช้สิทธิ์ Administrator/sudo: {}",
                        migrated, item.install_path, e
                    ));
                }
            }

            cfg.first_run_cleanup_checked = true;
            let _ = cfg.save();
            return Ok(format!(
                "[{}] ลบโปรแกรม {} ({}) เรียบร้อยแล้ว (นำเข้าโปรเจกต์ {} รายการ)",
                Local::now().format("%H:%M:%S"),
                item.name,
                item.install_path,
                migrated
            ));
        }

        cfg.first_run_cleanup_checked = true;
        let _ = cfg.save();
        Ok(format!(
            "นำเข้าโปรเจกต์จาก {} จำนวน {} รายการเข้าสู่ rusthost สำเร็จ",
            item.name, migrated
        ))
    }

    fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_dir() {
                Self::copy_dir_recursive(&entry.path(), &dst.join(entry.file_name()))?;
            } else {
                fs::copy(entry.path(), dst.join(entry.file_name()))?;
            }
        }
        Ok(())
    }
}
