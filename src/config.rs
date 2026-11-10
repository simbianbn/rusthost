use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RouteMode {
    SubPathOnly,
    VirtualHostOnly,
    Both,
    ReverseProxy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub id: String,
    pub name: String,
    pub alias_path: String,
    pub domain: String,
    pub document_root: String,
    pub route_mode: RouteMode,
    pub php_version: String,
    pub ssl_enabled: bool,
    pub allow_override: bool,
    pub directory_indexes: bool,
    pub directory_index_files: String,
    pub proxy_target_port: Option<u16>,
    pub env_vars: HashMap<String, String>,
    pub custom_directives: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerSaveConfig {
    pub eco_mode_enabled: bool,
    pub pause_polling_on_sleep: bool,
    pub pause_polling_on_minimize: bool,
    pub throttle_when_unfocused: bool,
    pub auto_suspend_services_on_deep_sleep: bool,
    pub active_refresh_ms: u64,
    pub unfocused_refresh_ms: u64,
}

impl Default for PowerSaveConfig {
    fn default() -> Self {
        Self {
            eco_mode_enabled: true,
            pause_polling_on_sleep: true,
            pause_polling_on_minimize: true,
            throttle_when_unfocused: true,
            auto_suspend_services_on_deep_sleep: false,
            active_refresh_ms: 2000,
            unfocused_refresh_ms: 15000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub stack_root: String,
    pub apache_bin: String,
    pub apache_conf: String,
    pub mysql_bin: String,
    pub mysql_cli_bin: String,
    pub mysqldump_bin: String,
    pub mysql_host: String,
    pub mysql_port: u16,
    pub mysql_user: String,
    pub mysql_password: String,
    pub php_root_dir: String,
    pub active_php_version: String,
    pub php_bin: String,
    pub php_ini_path: String,
    pub redis_server_bin: String,
    pub redis_cli_bin: String,
    pub redis_port: u16,
    pub node_bin: String,
    pub npm_bin: String,
    pub pm2_bin: String,
    pub apache_port: u16,
    pub apache_ssl_port: u16,
    pub mail_catcher_smtp_port: u16,
    pub mail_catcher_enabled: bool,
    pub phpmyadmin_url: String,
    pub www_root: String,
    pub ssl_certs_dir: String,
    pub projects: Vec<ProjectConfig>,
    pub power_save: PowerSaveConfig,
    pub first_run_cleanup_checked: bool,
}

impl Config {
    pub fn app_data_root() -> PathBuf {
        let mut base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        base.push(".rusthost");
        let _ = fs::create_dir_all(&base);
        base
    }
}

impl Default for Config {
    fn default() -> Self {
        let app_root = Self::app_data_root().to_string_lossy().to_string();
        let runtime_www = format!("{}/www", app_root);
        let runtime_ssl = format!("{}/ssl", app_root);

        if cfg!(target_os = "macos") {
            let brew_prefix = if PathBuf::from("/opt/homebrew").exists() {
                "/opt/homebrew"
            } else {
                "/usr/local"
            };

            let default_project = ProjectConfig {
                id: "proj-default".to_string(),
                name: "rusthost-demo".to_string(),
                alias_path: "/demo".to_string(),
                domain: "demo.test".to_string(),
                document_root: format!("{}/rusthost-demo", runtime_www),
                route_mode: RouteMode::Both,
                php_version: "8.4".to_string(),
                ssl_enabled: true,
                allow_override: true,
                directory_indexes: true,
                directory_index_files: "index.php index.html".to_string(),
                proxy_target_port: None,
                env_vars: HashMap::from([
                    ("APP_ENV".to_string(), "local".to_string()),
                    ("DB_HOST".to_string(), "127.0.0.1".to_string()),
                    ("REDIS_HOST".to_string(), "127.0.0.1".to_string()),
                    ("MAIL_PORT".to_string(), "1025".to_string()),
                ]),
                custom_directives: "# Custom directives for demo.test".to_string(),
                enabled: true,
            };

            Self {
                stack_root: brew_prefix.to_string(),
                apache_bin: format!("{}/bin/httpd", brew_prefix),
                apache_conf: format!("{}/etc/httpd/httpd.conf", brew_prefix),
                mysql_bin: format!("{}/bin/mysqld", brew_prefix),
                mysql_cli_bin: format!("{}/bin/mysql", brew_prefix),
                mysqldump_bin: format!("{}/bin/mysqldump", brew_prefix),
                mysql_host: "127.0.0.1".to_string(),
                mysql_port: 3306,
                mysql_user: "root".to_string(),
                mysql_password: "".to_string(),
                php_root_dir: format!("{}/Cellar/php", brew_prefix),
                active_php_version: "8.4".to_string(),
                php_bin: format!("{}/bin/php", brew_prefix),
                php_ini_path: format!("{}/php.ini", app_root),
                redis_server_bin: format!("{}/bin/redis-server", brew_prefix),
                redis_cli_bin: format!("{}/bin/redis-cli", brew_prefix),
                redis_port: 6379,
                node_bin: format!("{}/bin/node", brew_prefix),
                npm_bin: format!("{}/bin/npm", brew_prefix),
                pm2_bin: format!("{}/bin/pm2", brew_prefix),
                apache_port: 8080,
                apache_ssl_port: 8443,
                mail_catcher_smtp_port: 1025,
                mail_catcher_enabled: true,
                phpmyadmin_url: "http://localhost:8080/phpmyadmin".to_string(),
                www_root: runtime_www,
                ssl_certs_dir: runtime_ssl,
                projects: vec![default_project],
                power_save: PowerSaveConfig::default(),
                first_run_cleanup_checked: false,
            }
        } else {
            let root = if PathBuf::from("C:/laragon").exists() {
                "C:/laragon"
            } else if PathBuf::from("C:/xampp").exists() {
                "C:/xampp"
            } else {
                "C:/rusthost"
            };

            let default_project = ProjectConfig {
                id: "proj-default".to_string(),
                name: "rusthost-demo".to_string(),
                alias_path: "/demo".to_string(),
                domain: "demo.test".to_string(),
                document_root: format!("{}/www/rusthost-demo", root),
                route_mode: RouteMode::Both,
                php_version: "8.3".to_string(),
                ssl_enabled: true,
                allow_override: true,
                directory_indexes: true,
                directory_index_files: "index.php index.html".to_string(),
                proxy_target_port: None,
                env_vars: HashMap::new(),
                custom_directives: String::new(),
                enabled: true,
            };

            Self {
                stack_root: root.to_string(),
                apache_bin: format!("{}/bin/apache/bin/httpd.exe", root),
                apache_conf: format!("{}/bin/apache/conf/httpd.conf", root),
                mysql_bin: format!("{}/bin/mysql/bin/mysqld.exe", root),
                mysql_cli_bin: format!("{}/bin/mysql/bin/mysql.exe", root),
                mysqldump_bin: format!("{}/bin/mysql/bin/mysqldump.exe", root),
                mysql_host: "127.0.0.1".to_string(),
                mysql_port: 3306,
                mysql_user: "root".to_string(),
                mysql_password: "".to_string(),
                php_root_dir: format!("{}/bin/php", root),
                active_php_version: "8.3".to_string(),
                php_bin: format!("{}/bin/php/php-8.3/php.exe", root),
                php_ini_path: format!("{}/bin/php/php-8.3/php.ini", root),
                redis_server_bin: format!("{}/bin/redis/redis-server.exe", root),
                redis_cli_bin: format!("{}/bin/redis/redis-cli.exe", root),
                redis_port: 6379,
                node_bin: format!("{}/bin/nodejs/node.exe", root),
                npm_bin: format!("{}/bin/nodejs/npm.cmd", root),
                pm2_bin: format!("{}/bin/nodejs/pm2.cmd", root),
                apache_port: 80,
                apache_ssl_port: 443,
                mail_catcher_smtp_port: 1025,
                mail_catcher_enabled: true,
                phpmyadmin_url: "http://localhost/phpmyadmin".to_string(),
                www_root: format!("{}/www", root),
                ssl_certs_dir: format!("{}/etc/ssl", root),
                projects: vec![default_project],
                power_save: PowerSaveConfig::default(),
                first_run_cleanup_checked: false,
            }
        }
    }
}

impl Config {
    pub fn config_file_path() -> PathBuf {
        let mut dir = Self::app_data_root();
        dir.push("config.json");
        dir
    }

    pub fn load() -> Self {
        let path = Self::config_file_path();
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(mut cfg) = serde_json::from_str::<Config>(&raw) {
                // Ensure macOS TCC-protected ~/Documents paths are migrated to ~/.rusthost
                if cfg!(target_os = "macos") && cfg.www_root.contains("/Documents/") {
                    let app_root = Self::app_data_root().to_string_lossy().to_string();
                    cfg.www_root = format!("{}/www", app_root);
                    cfg.ssl_certs_dir = format!("{}/ssl", app_root);
                    for p in &mut cfg.projects {
                        if p.document_root.contains("/Documents/") {
                            p.document_root = format!("{}/{}", cfg.www_root, p.name);
                        }
                    }
                    let _ = cfg.save();
                }
                return cfg;
            }
        }
        let cfg = Self::default();
        let _ = cfg.save();
        cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_file_path();
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())
    }
}
