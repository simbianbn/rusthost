use crate::config::Config;
use crate::services::ServiceProcess;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MysqlProcessRow {
    pub id: u64,
    pub user: String,
    pub host: String,
    pub db: String,
    pub command: String,
    pub time: u64,
    pub state: String,
    pub info: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MysqlTableInfo {
    pub table_name: String,
    pub engine: String,
    pub table_rows: u64,
    pub size_kb: f64,
    pub collation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SqlConsoleResult {
    pub success: bool,
    pub execution_ms: f64,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MysqlDashboardSnapshot {
    pub online: bool,
    pub version: String,
    pub uptime_seconds: u64,
    pub threads_connected: u64,
    pub threads_running: u64,
    pub max_used_connections: u64,
    pub max_connections: u64,
    pub total_connections: u64,
    pub aborted_connects: u64,
    pub aborted_clients: u64,
    pub threads_cached: u64,
    pub total_queries: u64,
    pub qps: f64,
    pub slow_queries: u64,
    pub com_select: u64,
    pub com_insert: u64,
    pub com_update: u64,
    pub com_delete: u64,
    pub bytes_received_mb: f64,
    pub bytes_sent_mb: f64,
    pub innodb_buffer_pool_mb: f64,
    pub process_list: Vec<MysqlProcessRow>,
    pub databases: Vec<String>,
    pub last_updated: String,
}

pub struct MysqlDashboard {
    pub last_snapshot: MysqlDashboardSnapshot,
}

impl MysqlDashboard {
    pub fn new() -> Self {
        Self {
            last_snapshot: MysqlDashboardSnapshot::default(),
        }
    }

    fn run_mysql_batch(cfg: &Config, db: Option<&str>, sql: &str) -> Result<String, String> {
        let bin = if Path::new(&cfg.mysql_cli_bin).exists() {
            cfg.mysql_cli_bin.clone()
        } else {
            "mysql".to_string()
        };

        let mut cmd = Command::new(bin);
        cmd.args([
            "-h",
            &cfg.mysql_host,
            "-P",
            &cfg.mysql_port.to_string(),
            "-u",
            &cfg.mysql_user,
            "--batch",
            "--raw",
        ]);
        if !cfg.mysql_password.is_empty() {
            cmd.arg(format!("-p{}", cfg.mysql_password));
        }
        if let Some(d) = db {
            if !d.is_empty() {
                cmd.arg(d);
            }
        }
        cmd.args(["-e", sql]);

        let out = cmd.output().map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }

    pub fn refresh_all(&mut self, cfg: &Config) -> MysqlDashboardSnapshot {
        let port_open = ServiceProcess::is_port_open(cfg.mysql_port);
        if !port_open {
            let snap = MysqlDashboardSnapshot {
                online: false,
                version: "Offline (Port Closed)".to_string(),
                max_connections: 151,
                databases: vec!["information_schema".to_string(), "mysql".to_string(), "rusthost_db".to_string()],
                last_updated: Local::now().format("%H:%M:%S").to_string(),
                ..Default::default()
            };
            self.last_snapshot = snap.clone();
            return snap;
        }

        let mut vars = HashMap::<String, String>::new();
        if let Ok(raw) = Self::run_mysql_batch(
            cfg,
            None,
            "SHOW GLOBAL STATUS; SHOW GLOBAL VARIABLES LIKE 'max_connections'; SHOW GLOBAL VARIABLES LIKE 'innodb_buffer_pool_size'; SHOW GLOBAL VARIABLES LIKE 'version';",
        ) {
            for line in raw.lines() {
                let mut parts = line.splitn(2, '\t');
                if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                    vars.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }

        let parse_u64 = |k: &str| -> u64 {
            vars.get(k).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0)
        };

        let uptime = parse_u64("Uptime").max(1);
        let queries = parse_u64("Queries");
        let qps = (queries as f64) / (uptime as f64);

        let bytes_rx = (parse_u64("Bytes_received") as f64) / (1024.0 * 1024.0);
        let bytes_tx = (parse_u64("Bytes_sent") as f64) / (1024.0 * 1024.0);
        let buffer_pool_mb =
            (parse_u64("innodb_buffer_pool_size") as f64) / (1024.0 * 1024.0);

        let mut process_list = Vec::new();
        if let Ok(raw_procs) = Self::run_mysql_batch(cfg, None, "SHOW FULL PROCESSLIST;") {
            for line in raw_procs.lines().skip(1) {
                let cols: Vec<&str> = line.split('\t').collect();
                if cols.len() >= 8 {
                    process_list.push(MysqlProcessRow {
                        id: cols[0].parse().unwrap_or(0),
                        user: cols[1].to_string(),
                        host: cols[2].to_string(),
                        db: cols[3].to_string(),
                        command: cols[4].to_string(),
                        time: cols[5].parse().unwrap_or(0),
                        state: cols[6].to_string(),
                        info: cols[7].to_string(),
                    });
                }
            }
        }

        let mut databases = Vec::new();
        if let Ok(raw_dbs) = Self::run_mysql_batch(cfg, None, "SHOW DATABASES;") {
            for line in raw_dbs.lines().skip(1) {
                let db_name = line.trim();
                if !db_name.is_empty() {
                    databases.push(db_name.to_string());
                }
            }
        }
        if databases.is_empty() {
            databases = vec!["information_schema".to_string(), "mysql".to_string(), "performance_schema".to_string()];
        }

        let snap = MysqlDashboardSnapshot {
            online: true,
            version: vars
                .get("version")
                .cloned()
                .unwrap_or_else(|| "MySQL 8.x / MariaDB".to_string()),
            uptime_seconds: uptime,
            threads_connected: parse_u64("Threads_connected"),
            threads_running: parse_u64("Threads_running"),
            max_used_connections: parse_u64("Max_used_connections"),
            max_connections: parse_u64("max_connections").max(151),
            total_connections: parse_u64("Connections"),
            aborted_connects: parse_u64("Aborted_connects"),
            aborted_clients: parse_u64("Aborted_clients"),
            threads_cached: parse_u64("Threads_cached"),
            total_queries: queries,
            qps,
            slow_queries: parse_u64("Slow_queries"),
            com_select: parse_u64("Com_select"),
            com_insert: parse_u64("Com_insert"),
            com_update: parse_u64("Com_update"),
            com_delete: parse_u64("Com_delete"),
            bytes_received_mb: bytes_rx,
            bytes_sent_mb: bytes_tx,
            innodb_buffer_pool_mb: if buffer_pool_mb > 0.0 { buffer_pool_mb } else { 128.0 },
            process_list,
            databases,
            last_updated: Local::now().format("%H:%M:%S").to_string(),
        };

        self.last_snapshot = snap.clone();
        snap
    }

    pub fn kill_connection(cfg: &Config, process_id: u64) -> Result<String, String> {
        let sql = format!("KILL {};", process_id);
        Self::run_mysql_batch(cfg, None, &sql)?;
        Ok(format!("สั่งตัดการเชื่อมต่อ Process ID {} เรียบร้อยแล้ว", process_id))
    }

    pub fn create_database(
        cfg: &Config,
        db_name: &str,
        charset: &str,
        collation: &str,
    ) -> Result<String, String> {
        let clean_name: String = db_name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if clean_name.is_empty() {
            return Err("ชื่อฐานข้อมูลไม่ถูกต้อง".to_string());
        }
        let ch = if charset.is_empty() { "utf8mb4" } else { charset };
        let col = if collation.is_empty() {
            "utf8mb4_unicode_ci"
        } else {
            collation
        };
        let sql = format!(
            "CREATE DATABASE IF NOT EXISTS `{}` CHARACTER SET {} COLLATE {};",
            clean_name, ch, col
        );
        Self::run_mysql_batch(cfg, None, &sql)?;
        Ok(format!(
            "สร้างฐานข้อมูล `{}` ({} / {}) สำเร็จ",
            clean_name, ch, col
        ))
    }

    pub fn load_tables_for_db(cfg: &Config, db_name: &str) -> Result<Vec<MysqlTableInfo>, String> {
        let clean_db: String = db_name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let sql = format!(
            "SELECT TABLE_NAME, IFNULL(ENGINE,'InnoDB'), IFNULL(TABLE_ROWS,0), ROUND((IFNULL(DATA_LENGTH,0)+IFNULL(INDEX_LENGTH,0))/1024, 2), IFNULL(TABLE_COLLATION,'utf8mb4_unicode_ci') FROM information_schema.TABLES WHERE TABLE_SCHEMA='{}';",
            clean_db
        );
        let raw = Self::run_mysql_batch(cfg, None, &sql)?;
        let mut tables = Vec::new();
        for line in raw.lines().skip(1) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() >= 5 {
                tables.push(MysqlTableInfo {
                    table_name: cols[0].to_string(),
                    engine: cols[1].to_string(),
                    table_rows: cols[2].parse().unwrap_or(0),
                    size_kb: cols[3].parse().unwrap_or(0.0),
                    collation: cols[4].to_string(),
                });
            }
        }
        Ok(tables)
    }

    pub fn execute_dev_sql(cfg: &Config, db_name: &str, sql: &str) -> SqlConsoleResult {
        let start = Instant::now();
        let db_opt = if db_name.is_empty() { None } else { Some(db_name) };
        match Self::run_mysql_batch(cfg, db_opt, sql) {
            Ok(out) => {
                let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                let mut lines = out.lines();
                let columns: Vec<String> = lines
                    .next()
                    .map(|h| h.split('\t').map(|s| s.to_string()).collect())
                    .unwrap_or_default();
                let rows: Vec<Vec<String>> = lines
                    .map(|l| l.split('\t').map(|s| s.to_string()).collect())
                    .collect();
                SqlConsoleResult {
                    success: true,
                    execution_ms: elapsed_ms,
                    columns,
                    rows: rows.clone(),
                    message: format!("รันคำสั่ง SQL สำเร็จ ({:.2} ms, {} แถว)", elapsed_ms, rows.len()),
                }
            }
            Err(e) => SqlConsoleResult {
                success: false,
                execution_ms: start.elapsed().as_secs_f64() * 1000.0,
                columns: vec![],
                rows: vec![],
                message: format!("SQL Error: {}", e),
            },
        }
    }

    pub fn backup_database(cfg: &Config, db_name: &str) -> Result<String, String> {
        let clean_db: String = db_name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if clean_db.is_empty() {
            return Err("กรุณาระบุชื่อฐานข้อมูลที่ต้องการสำรอง".to_string());
        }

        let backup_dir = Path::new(&cfg.www_root)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("backups");
        let _ = fs::create_dir_all(&backup_dir);
        let filename = format!(
            "{}_{}.sql",
            clean_db,
            Local::now().format("%Y%m%d_%H%M%S")
        );
        let target_path = backup_dir.join(&filename);

        let bin = if Path::new(&cfg.mysqldump_bin).exists() {
            cfg.mysqldump_bin.clone()
        } else {
            "mysqldump".to_string()
        };

        let mut cmd = Command::new(bin);
        cmd.args([
            "-h",
            &cfg.mysql_host,
            "-P",
            &cfg.mysql_port.to_string(),
            "-u",
            &cfg.mysql_user,
            &clean_db,
        ]);
        if !cfg.mysql_password.is_empty() {
            cmd.arg(format!("-p{}", cfg.mysql_password));
        }

        match cmd.output() {
            Ok(out) if out.status.success() => {
                fs::write(&target_path, &out.stdout).map_err(|e| e.to_string())?;
                Ok(format!(
                    "สำรองฐานข้อมูล `{}` สำเร็จ -> {}",
                    clean_db,
                    target_path.to_string_lossy()
                ))
            }
            Ok(out) => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
            Err(e) => Err(format!("ไม่สามารถเรียกใช้ mysqldump ได้: {}", e)),
        }
    }

    pub fn import_sql_file(cfg: &Config, db_name: &str, sql_file_path: &str) -> Result<String, String> {
        let sql_content = fs::read_to_string(sql_file_path)
            .map_err(|e| format!("ไม่สามารถอ่านไฟล์ SQL '{}': {}", sql_file_path, e))?;
        Self::run_mysql_batch(cfg, Some(db_name), &sql_content)?;
        Ok(format!(
            "นำเข้าไฟล์ SQL '{}' เข้าสู่ฐานข้อมูล `{}` สำเร็จ",
            sql_file_path, db_name
        ))
    }
}
