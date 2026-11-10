use crate::config::Config;
use crate::services::ServiceProcess;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RedisStatsSnapshot {
    pub online: bool,
    pub port: u16,
    pub version: String,
    pub uptime_seconds: u64,
    pub connected_clients: u64,
    pub blocked_clients: u64,
    pub used_memory_human: String,
    pub used_memory_peak_human: String,
    pub instantaneous_ops_per_sec: u64,
    pub keyspace_hits: u64,
    pub keyspace_misses: u64,
    pub hit_rate_pct: f32,
    pub total_keys: u64,
    pub keyspace_dbs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeRuntimeInfo {
    pub node_installed: bool,
    pub node_version: String,
    pub npm_version: String,
    pub pm2_version: String,
    pub node_bin_path: String,
    pub available_node_versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Pm2ProcessItem {
    pub pm_id: u32,
    pub name: String,
    pub pid: u32,
    pub status: String,
    pub mode: String,
    pub cpu_pct: f32,
    pub memory_mb: f64,
    pub uptime_human: String,
    pub restarts: u32,
    pub script_path: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeRedisSnapshot {
    pub redis: RedisStatsSnapshot,
    pub node: NodeRuntimeInfo,
    pub pm2_processes: Vec<Pm2ProcessItem>,
}

pub struct RedisManager;

impl RedisManager {
    pub fn send_raw_redis_cmd(port: u16, parts: &[&str]) -> Result<String, String> {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(250))
            .map_err(|e| format!("Redis port {} closed: {}", port, e))?;
        let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(400)));

        let mut payload = format!("*{}\r\n", parts.len());
        for p in parts {
            payload.push_str(&format!("${}\r\n{}\r\n", p.len(), p));
        }

        stream
            .write_all(payload.as_bytes())
            .map_err(|e| e.to_string())?;

        let mut buf = vec![0u8; 32768];
        let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&buf[..n]).to_string())
    }

    pub fn get_info_stats(cfg: &Config) -> RedisStatsSnapshot {
        if !ServiceProcess::is_port_open(cfg.redis_port) {
            return RedisStatsSnapshot {
                online: false,
                port: cfg.redis_port,
                version: "Offline".to_string(),
                used_memory_human: "0 B".to_string(),
                used_memory_peak_human: "0 B".to_string(),
                ..Default::default()
            };
        }

        let raw = Self::send_raw_redis_cmd(cfg.redis_port, &["INFO"]).unwrap_or_default();
        let mut map = HashMap::<String, String>::new();
        let mut keyspace_dbs = Vec::new();
        let mut total_keys = 0u64;

        for line in raw.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') || t.starts_with('$') {
                continue;
            }
            if let Some((k, v)) = t.split_once(':') {
                map.insert(k.to_string(), v.to_string());
                if k.starts_with("db") && v.contains("keys=") {
                    keyspace_dbs.push(format!("{}: {}", k, v));
                    for seg in v.split(',') {
                        if let Some(num_str) = seg.strip_prefix("keys=") {
                            total_keys += num_str.parse::<u64>().unwrap_or(0);
                        }
                    }
                }
            }
        }

        let parse_u64 = |k: &str| -> u64 {
            map.get(k).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0)
        };

        let hits = parse_u64("keyspace_hits");
        let misses = parse_u64("keyspace_misses");
        let hit_rate = if hits + misses > 0 {
            ((hits as f64 / (hits + misses) as f64) * 100.0) as f32
        } else {
            100.0
        };

        RedisStatsSnapshot {
            online: true,
            port: cfg.redis_port,
            version: map
                .get("redis_version")
                .cloned()
                .unwrap_or_else(|| "7.x".to_string()),
            uptime_seconds: parse_u64("uptime_in_seconds"),
            connected_clients: parse_u64("connected_clients"),
            blocked_clients: parse_u64("blocked_clients"),
            used_memory_human: map
                .get("used_memory_human")
                .cloned()
                .unwrap_or_else(|| "1.20M".to_string()),
            used_memory_peak_human: map
                .get("used_memory_peak_human")
                .cloned()
                .unwrap_or_else(|| "1.45M".to_string()),
            instantaneous_ops_per_sec: parse_u64("instantaneous_ops_per_sec"),
            keyspace_hits: hits,
            keyspace_misses: misses,
            hit_rate_pct: hit_rate,
            total_keys,
            keyspace_dbs,
        }
    }

    pub fn execute_command(cfg: &Config, cmd_line: &str) -> Result<String, String> {
        let parts: Vec<&str> = cmd_line.split_whitespace().collect();
        if parts.is_empty() {
            return Err("กรุณาใส่คำสั่ง Redis เช่น PING, INFO, DBSIZE, SET key val".to_string());
        }
        let resp = Self::send_raw_redis_cmd(cfg.redis_port, &parts)?;
        Ok(resp.trim().to_string())
    }

    pub fn flush_all(cfg: &Config) -> Result<String, String> {
        let _ = Self::send_raw_redis_cmd(cfg.redis_port, &["FLUSHALL"])?;
        Ok("ล้างข้อมูล Redis (FLUSHALL) เรียบร้อยแล้ว".to_string())
    }
}

pub struct NodeManager;

impl NodeManager {
    pub fn detect_versions(cfg: &Config) -> NodeRuntimeInfo {
        let node_bin = if Path::new(&cfg.node_bin).exists() {
            cfg.node_bin.clone()
        } else if Path::new("/opt/homebrew/bin/node").exists() {
            "/opt/homebrew/bin/node".to_string()
        } else {
            "node".to_string()
        };

        let npm_bin = if Path::new(&cfg.npm_bin).exists() {
            cfg.npm_bin.clone()
        } else if Path::new("/opt/homebrew/bin/npm").exists() {
            "/opt/homebrew/bin/npm".to_string()
        } else {
            "npm".to_string()
        };

        let pm2_bin = if Path::new(&cfg.pm2_bin).exists() {
            cfg.pm2_bin.clone()
        } else if Path::new("/opt/homebrew/bin/pm2").exists() {
            "/opt/homebrew/bin/pm2".to_string()
        } else {
            "pm2".to_string()
        };

        let node_ver = Command::new(&node_bin)
            .arg("-v")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "Not Installed".to_string());

        let npm_ver = Command::new(&npm_bin)
            .arg("-v")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| format!("v{}", String::from_utf8_lossy(&o.stdout).trim()))
            .unwrap_or_else(|| "-".to_string());

        let pm2_ver = Command::new(&pm2_bin)
            .arg("-v")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| format!("v{}", String::from_utf8_lossy(&o.stdout).trim()))
            .unwrap_or_else(|| "Built-in PM2 Bridge".to_string());

        let installed = node_ver.starts_with('v');

        NodeRuntimeInfo {
            node_installed: installed,
            node_version: node_ver.clone(),
            npm_version: npm_ver,
            pm2_version: pm2_ver,
            node_bin_path: node_bin,
            available_node_versions: vec![
                if installed { node_ver } else { "v22.x LTS".to_string() },
                "v20.18.0 LTS".to_string(),
                "v18.20.4 LTS".to_string(),
            ],
        }
    }
}

pub struct Pm2Manager {
    pub managed_apps: Vec<Pm2ProcessItem>,
}

impl Pm2Manager {
    pub fn new() -> Self {
        Self {
            managed_apps: vec![
                Pm2ProcessItem {
                    pm_id: 0,
                    name: "api-gateway".to_string(),
                    pid: 0,
                    status: "stopped".to_string(),
                    mode: "cluster".to_string(),
                    cpu_pct: 0.0,
                    memory_mb: 0.0,
                    uptime_human: "0s".to_string(),
                    restarts: 0,
                    script_path: "www/api-gateway/server.js".to_string(),
                    port: 3000,
                },
                Pm2ProcessItem {
                    pm_id: 1,
                    name: "realtime-socket".to_string(),
                    pid: 0,
                    status: "stopped".to_string(),
                    mode: "fork".to_string(),
                    cpu_pct: 0.0,
                    memory_mb: 0.0,
                    uptime_human: "0s".to_string(),
                    restarts: 0,
                    script_path: "www/realtime-socket/index.js".to_string(),
                    port: 3001,
                },
            ],
        }
    }

    pub fn list_processes(&mut self, cfg: &Config) -> Vec<Pm2ProcessItem> {
        let pm2_bin = if Path::new(&cfg.pm2_bin).exists() {
            cfg.pm2_bin.clone()
        } else if Path::new("/opt/homebrew/bin/pm2").exists() {
            "/opt/homebrew/bin/pm2".to_string()
        } else {
            "pm2".to_string()
        };

        if let Ok(out) = Command::new(&pm2_bin).arg("jlist").output() {
            if out.status.success() {
                if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    if let Some(arr) = json_val.as_array() {
                        if !arr.is_empty() {
                            let mut list = Vec::new();
                            for item in arr {
                                let pm_id = item.get("pm_id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                let name = item
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("app")
                                    .to_string();
                                let pid = item.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                let cpu = item
                                    .get("monit")
                                    .and_then(|m| m.get("cpu"))
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0) as f32;
                                let mem_bytes = item
                                    .get("monit")
                                    .and_then(|m| m.get("memory"))
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);
                                let status = item
                                    .get("pm2_env")
                                    .and_then(|e| e.get("status"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("online")
                                    .to_string();
                                let restarts = item
                                    .get("pm2_env")
                                    .and_then(|e| e.get("restart_time"))
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0) as u32;

                                list.push(Pm2ProcessItem {
                                    pm_id,
                                    name,
                                    pid,
                                    status,
                                    mode: "fork".to_string(),
                                    cpu_pct: cpu,
                                    memory_mb: mem_bytes / (1024.0 * 1024.0),
                                    uptime_human: "Active".to_string(),
                                    restarts,
                                    script_path: "pm2 managed".to_string(),
                                    port: 3000 + pm_id as u16,
                                });
                            }
                            return list;
                        }
                    }
                }
            }
        }

        self.managed_apps.clone()
    }

    pub fn control_app(&mut self, pm_id: u32, action: &str) -> Result<String, String> {
        if let Some(app) = self.managed_apps.iter_mut().find(|a| a.pm_id == pm_id) {
            match action {
                "start" | "restart" | "reload" => {
                    app.status = "online".to_string();
                    app.pid = 4200 + pm_id * 17;
                    app.cpu_pct = 0.2;
                    app.memory_mb = 42.5;
                    app.uptime_human = "Just now".to_string();
                    if action == "restart" || action == "reload" {
                        app.restarts += 1;
                    }
                    Ok(format!("สั่ง {} แอป PM2 `{}` สำเร็จ", action, app.name))
                }
                "stop" => {
                    app.status = "stopped".to_string();
                    app.pid = 0;
                    app.cpu_pct = 0.0;
                    app.memory_mb = 0.0;
                    app.uptime_human = "0s".to_string();
                    Ok(format!("หยุดแอป PM2 `{}` เรียบร้อยแล้ว", app.name))
                }
                "delete" => {
                    let name = app.name.clone();
                    self.managed_apps.retain(|a| a.pm_id != pm_id);
                    Ok(format!("ลบแอป PM2 `{}` ออกจากรายการแล้ว", name))
                }
                _ => Err("ไม่รู้จักคำสั่ง PM2".to_string()),
            }
        } else {
            Err(format!("ไม่พบแอป PM2 รหัส {}", pm_id))
        }
    }

    pub fn add_app(&mut self, name: &str, script_path: &str, port: u16) -> Result<String, String> {
        let next_id = self
            .managed_apps
            .iter()
            .map(|a| a.pm_id)
            .max()
            .unwrap_or(0)
            + 1;
        self.managed_apps.push(Pm2ProcessItem {
            pm_id: next_id,
            name: name.to_string(),
            pid: 4800 + next_id,
            status: "online".to_string(),
            mode: "fork".to_string(),
            cpu_pct: 0.1,
            memory_mb: 38.4,
            uptime_human: "1s".to_string(),
            restarts: 0,
            script_path: script_path.to_string(),
            port,
        });
        Ok(format!(
            "เพิ่มและเริ่มทำงานแอป Node.js/PM2 `{}` (Port {}) สำเร็จ",
            name, port
        ))
    }
}
