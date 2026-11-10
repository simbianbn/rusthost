#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod config;
mod legacy_cleaner;
mod mail_catcher;
mod monitor;
mod mysql_monitor;
mod node_redis;
mod php_manager;
mod power_saver;
mod services;
mod tray;
mod vhost;

use chrono::Local;
use config::{Config, ProjectConfig};
use legacy_cleaner::{LegacyStackCleaner, LegacyStackItem};
use mail_catcher::{CapturedEmail, MailCatcher};
use monitor::{MonitorSnapshot, ResourceMonitor};
use mysql_monitor::{MysqlDashboard, MysqlDashboardSnapshot};
use node_redis::{NodeManager, NodeRedisSnapshot, Pm2Manager, RedisManager};
use php_manager::{PhpExtensionItem, PhpIniQuickSettings, PhpManager, PhpVersionInfo};
use power_saver::{PowerSaverEngine, PowerSaverStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use services::{PortConflictEntry, PortScanner, ServiceProcess, ServiceStatusDto};
use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use tray::TrayController;
use vhost::{
    DevLauncher, EnvEntry, EnvManager, ProjectScaffolder, SslManager, TunnelManager, VHostManager,
};

#[derive(Serialize)]
pub struct FullAppStateResponse {
    pub config: Config,
    pub services: Vec<ServiceStatusDto>,
    pub port_conflicts: Vec<PortConflictEntry>,
    pub monitor: MonitorSnapshot,
    pub power_saver: PowerSaverStatus,
    pub mysql: MysqlDashboardSnapshot,
    pub php_versions: Vec<PhpVersionInfo>,
    pub php_extensions: Vec<PhpExtensionItem>,
    pub php_ini: PhpIniQuickSettings,
    pub node_redis: NodeRedisSnapshot,
    pub emails: Vec<CapturedEmail>,
    pub legacy_stacks: Vec<LegacyStackItem>,
    pub logs: Vec<String>,
}

pub struct RustHostApp {
    pub config: Config,
    pub apache_service: ServiceProcess,
    pub mysql_service: ServiceProcess,
    pub redis_service: ServiceProcess,
    pub monitor: ResourceMonitor,
    pub power_saver: PowerSaverEngine,
    pub mysql_dashboard: MysqlDashboard,
    pub pm2_manager: Pm2Manager,
    pub mail_catcher: MailCatcher,
    pub logs: Vec<String>,
}

impl RustHostApp {
    pub fn new() -> Self {
        let cfg = Config::load();
        for proj in &cfg.projects {
            let _ = VHostManager::ensure_project_directory(proj);
        }
        let _ = VHostManager::write_apache_projects_conf(&cfg);

        let apache_service = ServiceProcess::new(
            "apache",
            "Apache HTTP Server",
            &cfg.apache_bin,
            vec![],
            cfg.apache_port,
        );
        let mysql_service = ServiceProcess::new(
            "mysql",
            "MySQL Database",
            &cfg.mysql_bin,
            vec![],
            cfg.mysql_port,
        );
        let redis_service = ServiceProcess::new(
            "redis",
            "Redis Cache Server",
            &cfg.redis_server_bin,
            vec!["--port".to_string(), cfg.redis_port.to_string()],
            cfg.redis_port,
        );

        let mail_catcher = MailCatcher::new(cfg.mail_catcher_smtp_port);

        let mut logs = Vec::new();
        logs.push(format!(
            "[{}] rusthost Native App เริ่มทำงานเรียบร้อยแล้ว (Eco/Sleep Power-Saving Mode: ON)",
            Local::now().format("%H:%M:%S")
        ));

        Self {
            config: cfg,
            apache_service,
            mysql_service,
            redis_service,
            monitor: ResourceMonitor::new(),
            power_saver: PowerSaverEngine::new(),
            mysql_dashboard: MysqlDashboard::new(),
            pm2_manager: Pm2Manager::new(),
            mail_catcher,
            logs,
        }
    }

    pub fn start_all(&mut self) -> String {
        let _ = VHostManager::write_apache_projects_conf(&self.config);
        let _ = self.apache_service.start(&mut self.logs);
        let _ = self.mysql_service.start(&mut self.logs);
        let _ = self.redis_service.start(&mut self.logs);
        "สั่งเริ่มการทำงานทุกเซอร์วิส (Apache, MySQL, Redis) เรียบร้อยแล้ว".to_string()
    }

    pub fn stop_all(&mut self) -> String {
        let _ = self.apache_service.stop(&mut self.logs);
        let _ = self.mysql_service.stop(&mut self.logs);
        let _ = self.redis_service.stop(&mut self.logs);
        "หยุดการทำงานทุกเซอร์วิสเรียบร้อยแล้ว".to_string()
    }

    pub fn build_state_snapshot(
        &mut self,
        visible: bool,
        focused: bool,
        system_sleeping: bool,
        force: bool,
    ) -> FullAppStateResponse {
        let woke = self.power_saver.update_window_state(
            visible,
            focused,
            system_sleeping,
            &self.config.power_save,
        );
        if woke {
            self.logs.push(self.power_saver.last_sleep_wake_log.clone());
        }

        let should_poll = self
            .power_saver
            .should_run_background_poll(&self.config.power_save, force);

        let monitor_snap = if should_poll {
            self.monitor.refresh()
        } else {
            self.monitor.cached_snapshot()
        };

        let mysql_snap = if should_poll {
            self.mysql_dashboard.refresh_all(&self.config)
        } else {
            self.mysql_dashboard.last_snapshot.clone()
        };

        let services = vec![
            self.apache_service.to_dto(),
            self.mysql_service.to_dto(),
            self.redis_service.to_dto(),
        ];

        let port_conflicts = PortScanner::scan_conflicts(&self.config);
        let php_versions = PhpManager::scan_installed_versions(&self.config);
        let php_extensions = PhpManager::get_extensions(&self.config);
        let php_ini = PhpManager::get_php_ini_settings(&self.config);

        let node_redis = NodeRedisSnapshot {
            redis: RedisManager::get_info_stats(&self.config),
            node: NodeManager::detect_versions(&self.config),
            pm2_processes: self.pm2_manager.list_processes(&self.config),
        };

        let emails = self.mail_catcher.list_messages();
        let legacy_stacks = LegacyStackCleaner::scan_all();
        let power_status = self.power_saver.status(&self.config.power_save);

        FullAppStateResponse {
            config: self.config.clone(),
            services,
            port_conflicts,
            monitor: monitor_snap,
            power_saver: power_status,
            mysql: mysql_snap,
            php_versions,
            php_extensions,
            php_ini,
            node_redis,
            emails,
            legacy_stacks,
            logs: self.logs.clone(),
        }
    }

    pub fn handle_action(&mut self, body: &Value) -> Value {
        let action = body
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match action {
            "service_start" => {
                let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let res = match name {
                    "apache" => self.apache_service.start(&mut self.logs),
                    "mysql" => self.mysql_service.start(&mut self.logs),
                    "redis" => self.redis_service.start(&mut self.logs),
                    _ => Err("Unknown service".to_string()),
                };
                match res {
                    Ok(_) => json!({"ok": true, "message": format!("เริ่มเซอร์วิส {} สำเร็จ", name)}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "service_stop" => {
                let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let res = match name {
                    "apache" => self.apache_service.stop(&mut self.logs),
                    "mysql" => self.mysql_service.stop(&mut self.logs),
                    "redis" => self.redis_service.stop(&mut self.logs),
                    _ => Err("Unknown service".to_string()),
                };
                match res {
                    Ok(_) => json!({"ok": true, "message": format!("หยุดเซอร์วิส {} เรียบร้อยแล้ว", name)}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "kill_port" => {
                let port = body.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
                match PortScanner::kill_port_owner(port, &mut self.logs) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "switch_php" => {
                let ver = body.get("version").and_then(|v| v.as_str()).unwrap_or("8.3");
                match PhpManager::switch_global_php(&mut self.config, ver) {
                    Ok(m) => {
                        self.logs.push(format!("[{}] {}", Local::now().format("%H:%M:%S"), m));
                        json!({"ok": true, "message": m})
                    }
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "toggle_php_ext" => {
                let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let enable = body.get("enable").and_then(|v| v.as_bool()).unwrap_or(true);
                match PhpManager::toggle_extension(&self.config, name, enable) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "save_php_ini" => {
                let settings = PhpIniQuickSettings {
                    memory_limit: body
                        .get("memory_limit")
                        .and_then(|v| v.as_str())
                        .unwrap_or("512M")
                        .to_string(),
                    upload_max_filesize: body
                        .get("upload_max_filesize")
                        .and_then(|v| v.as_str())
                        .unwrap_or("128M")
                        .to_string(),
                    post_max_size: body
                        .get("post_max_size")
                        .and_then(|v| v.as_str())
                        .unwrap_or("128M")
                        .to_string(),
                    max_execution_time: body
                        .get("max_execution_time")
                        .and_then(|v| v.as_str())
                        .unwrap_or("300")
                        .to_string(),
                    display_errors: body
                        .get("display_errors")
                        .and_then(|v| v.as_str())
                        .unwrap_or("On")
                        .to_string(),
                };
                match PhpManager::update_php_ini_settings(&self.config, &settings) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "open_php_ini" => match PhpManager::open_php_ini(&self.config) {
                Ok(m) => json!({"ok": true, "message": m}),
                Err(e) => json!({"ok": false, "message": e}),
            },
            "pm2_control" => {
                let pm_id = body.get("pm_id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let cmd = body.get("cmd").and_then(|v| v.as_str()).unwrap_or("restart");
                match self.pm2_manager.control_app(pm_id, cmd) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "pm2_add" => {
                let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("node-app");
                let script = body
                    .get("script_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("server.js");
                let port = body.get("port").and_then(|v| v.as_u64()).unwrap_or(3002) as u16;
                match self.pm2_manager.add_app(name, script, port) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "redis_cmd" => {
                let cmd = body.get("cmd").and_then(|v| v.as_str()).unwrap_or("PING");
                match RedisManager::execute_command(&self.config, cmd) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "redis_flush" => match RedisManager::flush_all(&self.config) {
                Ok(m) => json!({"ok": true, "message": m}),
                Err(e) => json!({"ok": false, "message": e}),
            },
            "create_project" => {
                let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let template = body
                    .get("template")
                    .and_then(|v| v.as_str())
                    .unwrap_or("blank_php");
                let php_ver = body
                    .get("php_version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("8.3");
                match ProjectScaffolder::create_project(&mut self.config, name, template, php_ver) {
                    Ok(p) => json!({
                        "ok": true,
                        "message": format!("สร้างโปรเจกต์ `{}` (https://{}) สำเร็จ!", p.name, p.domain)
                    }),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "gen_ssl" => {
                let domain = body.get("domain").and_then(|v| v.as_str()).unwrap_or("demo.test");
                match SslManager::generate_cert_for_domain(&self.config, domain) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "sync_hosts" => match VHostManager::sync_windows_hosts(&self.config) {
                Ok(m) => json!({"ok": true, "message": m}),
                Err(e) => json!({"ok": false, "message": e}),
            },
            "write_apache_conf" => match VHostManager::inject_include_into_httpd_conf(&self.config) {
                Ok(m) => json!({"ok": true, "message": m}),
                Err(e) => json!({"ok": false, "message": e}),
            },
            "open_ide" => {
                let path = body.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                match DevLauncher::open_ide(path) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "open_terminal" => {
                let pid = body.get("project_id").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(proj) = self.config.projects.iter().find(|p| p.id == pid).cloned() {
                    match DevLauncher::open_scoped_terminal(&self.config, &proj) {
                        Ok(m) => json!({"ok": true, "message": m}),
                        Err(e) => json!({"ok": false, "message": e}),
                    }
                } else {
                    json!({"ok": false, "message": "ไม่พบโปรเจกต์"})
                }
            }
            "load_env" => {
                let path = body.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let entries = EnvManager::load_env(path);
                json!({"ok": true, "env_entries": entries})
            }
            "save_env" => {
                let path = body.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let entries: Vec<EnvEntry> = body
                    .get("entries")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                match EnvManager::save_env(path, &entries) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "share_tunnel" => {
                let pid = body.get("project_id").and_then(|v| v.as_str()).unwrap_or("");
                let proj = self
                    .config
                    .projects
                    .iter()
                    .find(|p| p.id == pid)
                    .cloned()
                    .unwrap_or_else(|| ProjectConfig {
                        id: "default".to_string(),
                        name: "demo".to_string(),
                        alias_path: "/demo".to_string(),
                        domain: "demo.test".to_string(),
                        document_root: self.config.www_root.clone(),
                        route_mode: config::RouteMode::Both,
                        php_version: "8.3".to_string(),
                        ssl_enabled: true,
                        allow_override: true,
                        directory_indexes: true,
                        directory_index_files: "index.php".to_string(),
                        proxy_target_port: None,
                        env_vars: Default::default(),
                        custom_directives: String::new(),
                        enabled: true,
                    });
                let tunnel = TunnelManager::create_share_preview(&self.config, &proj);
                json!({"ok": true, "tunnel": tunnel})
            }
            "mysql_kill" => {
                let id = body.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                match MysqlDashboard::kill_connection(&self.config, id) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "mysql_create_db" => {
                let db_name = body.get("db_name").and_then(|v| v.as_str()).unwrap_or("");
                let charset = body
                    .get("charset")
                    .and_then(|v| v.as_str())
                    .unwrap_or("utf8mb4");
                let collation = body
                    .get("collation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("utf8mb4_unicode_ci");
                match MysqlDashboard::create_database(&self.config, db_name, charset, collation) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "mysql_backup" => {
                let db_name = body.get("db_name").and_then(|v| v.as_str()).unwrap_or("");
                match MysqlDashboard::backup_database(&self.config, db_name) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "mysql_sql" => {
                let db_name = body.get("db_name").and_then(|v| v.as_str()).unwrap_or("");
                let sql = body.get("sql").and_then(|v| v.as_str()).unwrap_or("");
                let res = MysqlDashboard::execute_dev_sql(&self.config, db_name, sql);
                json!({"ok": res.success, "message": res.message, "sql_result": res})
            }
            "send_test_email" => match self.mail_catcher.send_test_email() {
                Ok(m) => json!({"ok": true, "message": m}),
                Err(e) => json!({"ok": false, "message": e}),
            },
            "clear_emails" => {
                self.mail_catcher.clear_inbox();
                json!({"ok": true, "message": "ล้างกล่องจดหมายจำลองเรียบร้อยแล้ว"})
            }
            "toggle_eco_mode" => {
                self.config.power_save.eco_mode_enabled = !self.config.power_save.eco_mode_enabled;
                let _ = self.config.save();
                json!({
                    "ok": true,
                    "message": if self.config.power_save.eco_mode_enabled {
                        "เปิดโหมดประหยัดพลังงานตอนเครื่อง Sleep แล้ว"
                    } else {
                        "ปิดโหมดประหยัดพลังงานชั่วคราว"
                    }
                })
            }
            "legacy_action" => {
                let stack_id = body.get("stack_id").and_then(|v| v.as_str()).unwrap_or("xampp");
                let mode = body
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("backup_and_remove");
                match LegacyStackCleaner::handle_legacy_action(&mut self.config, stack_id, mode) {
                    Ok(m) => json!({"ok": true, "message": m}),
                    Err(e) => json!({"ok": false, "message": e}),
                }
            }
            "dismiss_legacy_wizard" => {
                self.config.first_run_cleanup_checked = true;
                let _ = self.config.save();
                json!({"ok": true, "message": "บันทึกการตรวจสอบโปรแกรมเดิมเรียบร้อยแล้ว"})
            }
            "create_shortcut" => match TrayController::create_desktop_shortcut() {
                Ok(m) => json!({"ok": true, "message": m}),
                Err(e) => json!({"ok": false, "message": e}),
            },
            "open_localhost" => {
                let url = format!("http://localhost:{}", self.config.apache_port);
                let _ = TrayController::open_browser(&url);
                json!({"ok": true, "message": format!("เปิด {} แล้ว", url)})
            }
            "open_phpmyadmin" => {
                let _ = TrayController::open_phpmyadmin(&self.config);
                json!({"ok": true, "message": "เปิด phpMyAdmin แล้ว"})
            }
            "save_paths" => {
                if let Some(v) = body.get("apache_bin").and_then(|s| s.as_str()) {
                    self.config.apache_bin = v.to_string();
                    self.apache_service.binary_path = v.to_string();
                }
                if let Some(v) = body.get("mysql_bin").and_then(|s| s.as_str()) {
                    self.config.mysql_bin = v.to_string();
                    self.mysql_service.binary_path = v.to_string();
                }
                if let Some(v) = body.get("redis_server_bin").and_then(|s| s.as_str()) {
                    self.config.redis_server_bin = v.to_string();
                    self.redis_service.binary_path = v.to_string();
                }
                if let Some(v) = body.get("node_bin").and_then(|s| s.as_str()) {
                    self.config.node_bin = v.to_string();
                }
                let _ = self.config.save();
                json!({"ok": true, "message": "บันทึกการตั้งค่า Path เซอร์วิสเรียบร้อยแล้ว"})
            }
            _ => json!({"ok": false, "message": "ไม่พบคำสั่งที่ระบุ"}),
        }
    }
}

#[derive(Deserialize, Default)]
struct StateRequestBody {
    visible: Option<bool>,
    focused: Option<bool>,
    system_sleeping: Option<bool>,
    force: Option<bool>,
}

fn url_decode(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.as_bytes().iter().copied();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next().unwrap_or(b'0');
            let h2 = chars.next().unwrap_or(b'0');
            let hex = [h1, h2];
            if let Ok(s) = std::str::from_utf8(&hex) {
                if let Ok(val) = u8::from_str_radix(s, 16) {
                    bytes.push(val);
                    continue;
                }
            }
        } else if b == b'+' {
            bytes.push(b' ');
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

fn extract_request_json_str(request: &wry::http::Request<Vec<u8>>) -> String {
    if !request.body().is_empty() {
        return String::from_utf8_lossy(request.body()).to_string();
    }
    if let Some(q) = request.uri().query() {
        for pair in q.split('&') {
            if let Some(val) = pair.strip_prefix("payload=") {
                return url_decode(val);
            }
        }
    }
    "{}".to_string()
}

fn handle_custom_protocol_request(
    app: &Arc<Mutex<RustHostApp>>,
     bundled_html: &str,
    css_raw: &'static str,
    js_raw: &'static str,
    request: wry::http::Request<Vec<u8>>,
) -> wry::http::Response<Cow<'static, [u8]>> {
    let path = request.uri().path().to_string();

    if path == "/" || path == "/index.html" {
        return wry::http::Response::builder()
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Access-Control-Allow-Origin", "*")
            .body(Cow::Owned(bundled_html.as_bytes().to_vec()))
            .unwrap();
    }
    if path == "/styles.css" {
        return wry::http::Response::builder()
            .header("Content-Type", "text/css; charset=utf-8")
            .body(Cow::Borrowed(css_raw.as_bytes()))
            .unwrap();
    }
    if path == "/app.js" {
        return wry::http::Response::builder()
            .header("Content-Type", "application/javascript; charset=utf-8")
            .body(Cow::Borrowed(js_raw.as_bytes()))
            .unwrap();
    }
    if path == "/icon.png" {
        let icon_bytes: &'static [u8] = include_bytes!("../ui/icon.png");
        return wry::http::Response::builder()
            .header("Content-Type", "image/png")
            .body(Cow::Borrowed(icon_bytes))
            .unwrap();
    }

    let body_str = extract_request_json_str(&request);

    let response_json = if let Ok(mut guard) = app.lock() {
        if path == "/api/state" {
            let parsed: StateRequestBody = serde_json::from_str(&body_str).unwrap_or_default();
            let snap = guard.build_state_snapshot(
                parsed.visible.unwrap_or(true),
                parsed.focused.unwrap_or(true),
                parsed.system_sleeping.unwrap_or(false),
                parsed.force.unwrap_or(false),
            );
            serde_json::to_string(&snap).unwrap_or_else(|_| "{}".to_string())
        } else if path == "/api/power_state" {
            let parsed: StateRequestBody = serde_json::from_str(&body_str).unwrap_or_default();
            let cfg_ps = guard.config.power_save.clone();
            guard.power_saver.update_window_state(
                parsed.visible.unwrap_or(false),
                parsed.focused.unwrap_or(false),
                parsed.system_sleeping.unwrap_or(true),
                &cfg_ps,
            );
            let status = guard.power_saver.status(&cfg_ps);
            serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string())
        } else if path == "/api/services/start_all" {
            let msg = guard.start_all();
            json!({"ok": true, "message": msg}).to_string()
        } else if path == "/api/services/stop_all" {
            let msg = guard.stop_all();
            json!({"ok": true, "message": msg}).to_string()
        } else if path == "/api/action" {
            let val: Value = serde_json::from_str(&body_str).unwrap_or_else(|_| json!({}));
            guard.handle_action(&val).to_string()
        } else {
            json!({"error": "Not found"}).to_string()
        }
    } else {
        json!({"error": "Lock error"}).to_string()
    };

    wry::http::Response::builder()
        .header("Content-Type", "application/json; charset=utf-8")
        .header("Access-Control-Allow-Origin", "*")
        .body(Cow::Owned(response_json.into_bytes()))
        .unwrap()
}

fn run_self_test() {
    println!("====================================================================");
    println!("  rusthost Native App Self-Test & Verification Suite");
    println!("====================================================================");

    let mut app = RustHostApp::new();

    println!("[1/8] Config & VHost Apache Config Generation...");
    let apache_conf = VHostManager::generate_apache_config(&app.config);
    assert!(apache_conf.contains("VirtualHost"));
    assert!(apache_conf.contains("Alias"));
    println!("  ✓ Generated Apache config ({} bytes)", apache_conf.len());

    println!("[2/8] CPU, RAM & 6-Group Service Monitor...");
    let mon = app.monitor.refresh();
    println!(
        "  ✓ CPU: {} ({} cores) @ {:.1}% | RAM: {:.2}/{:.2} GB ({:.1}%)",
        mon.cpu_brand,
        mon.logical_cores,
        mon.global_cpu_pct,
        mon.used_ram_gb,
        mon.total_ram_gb,
        mon.ram_usage_pct
    );
    assert_eq!(mon.service_groups.len(), 6);

    println!("[3/8] Testing Eco & Sleep Power-Saving Engine (Zero-Polling on Sleep)...");
    let cfg_ps = app.config.power_save.clone();
    app.power_saver
        .update_window_state(false, false, true, &cfg_ps);
    let should_poll_when_asleep = app.power_saver.should_run_background_poll(&cfg_ps, false);
    assert!(
        !should_poll_when_asleep,
        "Polling MUST be paused (0 Hz) when system is sleeping!"
    );
    let ps_status = app.power_saver.status(&cfg_ps);
    println!(
        "  ✓ Sleep State: {:?} | Polling Paused: {} | CPU Overhead: {:.1}%",
        ps_status.state, ps_status.polling_paused, ps_status.cpu_overhead_estimate_pct
    );

    println!("[4/8] Testing Multi-PHP Version Scanner & php.ini Manager...");
    let php_versions = PhpManager::scan_installed_versions(&app.config);
    println!("  ✓ Detected {} PHP version slots", php_versions.len());
    let _ = PhpManager::switch_global_php(&mut app.config, "8.4");
    assert_eq!(app.config.active_php_version, "8.4");

    println!("[5/8] Testing Redis, Node.js & PM2 Manager...");
    let node_info = NodeManager::detect_versions(&app.config);
    println!(
        "  ✓ Node.js: {} | npm: {} | PM2: {}",
        node_info.node_version, node_info.npm_version, node_info.pm2_version
    );
    let pm2_list = app.pm2_manager.list_processes(&app.config);
    println!("  ✓ PM2 Managed Processes: {}", pm2_list.len());

    println!("[6/8] Testing Built-in Rust SMTP Mail Catcher (:1025)...");
    let mail_msg = app.mail_catcher.send_test_email().unwrap();
    println!("  ✓ {}", mail_msg);
    assert!(!app.mail_catcher.list_messages().is_empty());

    println!("[7/8] Testing Auto SSL Certificate & QR Code Share Tunnel...");
    let ssl_msg = SslManager::generate_cert_for_domain(&app.config, "demo.test").unwrap();
    println!("  ✓ {}", ssl_msg);
    let qr_svg = TunnelManager::render_qr_svg("https://demo-rusthost.trycloudflare.com");
    assert!(qr_svg.contains("<svg"));
    println!("  ✓ Generated SVG QR Code ({} bytes)", qr_svg.len());

    println!("[8/8] Testing In-Process Custom Protocol IPC (`rusthost://localhost/api/state`)...");
    let shared_app = Arc::new(Mutex::new(app));
    let req = wry::http::Request::builder()
        .method("POST")
        .uri("rusthost://localhost/api/state")
        .body(b"{\"visible\":true,\"focused\":true}".to_vec())
        .unwrap();
    let resp = handle_custom_protocol_request(&shared_app, "", "", "", req);
    assert_eq!(resp.status(), 200);
    println!(
        "  ✓ Custom Protocol Native IPC Response ({} bytes, zero browser/HTTP port!)",
        resp.body().len()
    );

    println!("====================================================================");
    println!("  ALL 8 MODULE SUITES PASSED SUCCESSFULLY!");
    println!("====================================================================");
}

fn main() {
    let _ = std::env::set_current_dir(Config::app_data_root());
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--self-test") {
        run_self_test();
        return;
    }
    if args.iter().any(|a| a == "--test-stop-start") {
        let mut app = RustHostApp::new();
        println!("==> Testing stop_all()...");
        let stop_msg = app.stop_all();
        println!("  {}", stop_msg);
        for mut svc in [
            app.apache_service.to_dto(),
            app.mysql_service.to_dto(),
            app.redis_service.to_dto(),
        ] {
            println!("  After stop_all -> {}: running={} (port {} open={})", svc.display_name, svc.running, svc.port, svc.port_open);
            let _ = &mut svc;
        }
        return;
    }
    if args.iter().any(|a| a == "--dump-state") {
        let mut app = RustHostApp::new();
        let snap = app.build_state_snapshot(true, true, false, true);
        println!("{}", serde_json::to_string(&snap).unwrap_or_default());
        return;
    }

    let app = Arc::new(Mutex::new(RustHostApp::new()));
    let app_for_protocol = Arc::clone(&app);

    let html_raw = include_str!("../ui/index.html");
    let css_raw = include_str!("../ui/styles.css");
    let js_raw = include_str!("../ui/app.js");

    let bundled_html = html_raw
        .replace("/*__INLINE_CSS__*/", css_raw)
        .replace("/*__INLINE_JS__*/", js_raw);

    use tao::{
        dpi::LogicalSize,
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
    };
    use wry::WebViewBuilder;

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("rusthost")
        .with_inner_size(LogicalSize::new(1240.0, 800.0))
        .with_min_inner_size(LogicalSize::new(980.0, 660.0))
        .build(&event_loop)
        .expect("Failed to create rusthost native window");

    let _webview = WebViewBuilder::new()
        .with_custom_protocol("rusthost".into(), move |_webview_id, request| {
            handle_custom_protocol_request(
                &app_for_protocol,
                &bundled_html,
                css_raw,
                js_raw,
                request,
            )
        })
        .with_url("rusthost://localhost/index.html")
        .build(&window)
        .expect("Failed to initialize rusthost native WebView");

    event_loop.run(move |event, _, control_flow| {
        // ControlFlow::Wait ensures 0% CPU wakeups when idle or system sleeping!
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::Focused(focused),
                ..
            } => {
                if let Ok(mut guard) = app.lock() {
                    let cfg_ps = guard.config.power_save.clone();
                    guard
                        .power_saver
                        .update_window_state(true, focused, false, &cfg_ps);
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if let Ok(mut guard) = app.lock() {
                    let _ = guard.stop_all();
                }
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}
