use crate::config::Config;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatusDto {
    pub name: String,
    pub display_name: String,
    pub binary_path: String,
    pub port: u16,
    pub running: bool,
    pub port_open: bool,
    pub pid: Option<u32>,
    pub uptime_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConflictEntry {
    pub port: u16,
    pub service_hint: String,
    pub is_open: bool,
    pub pid: Option<u32>,
    pub process_name: String,
    pub protocol: String,
}

pub struct ServiceProcess {
    pub name: String,
    pub display_name: String,
    pub binary_path: String,
    pub args: Vec<String>,
    pub port: u16,
    pub child: Option<Child>,
    pub pid_override: Option<u32>,
    pub started_at: Option<String>,
}

impl ServiceProcess {
    pub fn new(name: &str, display_name: &str, binary_path: &str, args: Vec<String>, port: u16) -> Self {
        Self {
            name: name.to_string(),
            display_name: display_name.to_string(),
            binary_path: binary_path.to_string(),
            args,
            port,
            child: None,
            pid_override: None,
            started_at: None,
        }
    }

    pub fn is_port_open(port: u16) -> bool {
        let addr4 = SocketAddr::from(([127, 0, 0, 1], port));
        if TcpStream::connect_timeout(&addr4, Duration::from_millis(150)).is_ok() {
            return true;
        }
        false
    }

    pub fn check_alive(&mut self) -> bool {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(_)) => {
                    self.child = None;
                }
                Ok(None) => return true,
                Err(_) => {
                    self.child = None;
                }
            }
        }
        Self::is_port_open(self.port)
    }

    pub fn start(&mut self, logs: &mut Vec<String>) -> Result<(), String> {
        if Self::is_port_open(self.port) {
            logs.push(format!(
                "[{}] {} กำลังทำงานอยู่แล้วบนพอร์ต {}",
                Local::now().format("%H:%M:%S"),
                self.display_name,
                self.port
            ));
            return Ok(());
        }

        if cfg!(target_os = "macos") {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/simbianbn".to_string());
            let brew_svc = match self.name.as_str() {
                "apache" => "httpd",
                "mysql" => "mysql",
                "redis" => "redis",
                _ => "",
            };
            let plist_path = format!("{}/Library/LaunchAgents/sh.brew.{}.plist", home, brew_svc);
            if Path::new(&plist_path).exists() {
                let _ = Command::new("/bin/launchctl")
                    .args(["load", "-w", &plist_path])
                    .output();
            }

            if !Self::is_port_open(self.port) {
                let mut cmd = match self.name.as_str() {
                    "apache" => {
                        let mut c = Command::new(&self.binary_path);
                        c.args(["-D", "FOREGROUND"]);
                        c
                    }
                    "mysql" => {
                        let mysqld_safe = "/opt/homebrew/opt/mysql/bin/mysqld_safe";
                        if Path::new(mysqld_safe).exists() {
                            let mut c = Command::new(mysqld_safe);
                            c.arg("--datadir=/opt/homebrew/var/mysql");
                            c
                        } else {
                            Command::new(&self.binary_path)
                        }
                    }
                    _ => {
                        let mut c = Command::new(&self.binary_path);
                        c.args(&self.args);
                        c
                    }
                };

                cmd.stdout(Stdio::null()).stderr(Stdio::null());
                if let Ok(child) = cmd.spawn() {
                    self.pid_override = Some(child.id());
                    self.child = Some(child);
                }
            }

            // Wait briefly for port to come up
            let start_wait = Instant::now();
            while start_wait.elapsed() < Duration::from_millis(1400) {
                if Self::is_port_open(self.port) {
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }

            self.started_at = Some(Local::now().format("%H:%M:%S").to_string());
            logs.push(format!(
                "[{}] เริ่มทำงาน {} สำเร็จ (Port: {})",
                Local::now().format("%H:%M:%S"),
                self.display_name,
                self.port
            ));
            return Ok(());
        }

        if !Path::new(&self.binary_path).exists() {
            let msg = format!(
                "[{}] ไม่พบไฟล์เซอร์วิส {} ที่ '{}' (ตรวจสอบได้ในหน้า Settings)",
                Local::now().format("%H:%M:%S"),
                self.display_name,
                self.binary_path
            );
            logs.push(msg.clone());
            return Err(msg);
        }

        let mut cmd = Command::new(&self.binary_path);
        cmd.args(&self.args)
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                self.pid_override = Some(pid);
                self.child = Some(child);
                self.started_at = Some(Local::now().format("%H:%M:%S").to_string());
                logs.push(format!(
                    "[{}] เริ่มทำงาน {} สำเร็จ (PID: {}, Port: {})",
                    Local::now().format("%H:%M:%S"),
                    self.display_name,
                    pid,
                    self.port
                ));
                Ok(())
            }
            Err(e) => {
                let err_msg = format!(
                    "[{}] ไม่สามารถสั่งรัน {} ได้: {}",
                    Local::now().format("%H:%M:%S"),
                    self.display_name,
                    e
                );
                logs.push(err_msg.clone());
                Err(err_msg)
            }
        }
    }

    pub fn stop(&mut self, logs: &mut Vec<String>) -> Result<(), String> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.pid_override = None;

        if cfg!(target_os = "windows") {
            let exe_name = match self.name.as_str() {
                "apache" => "httpd.exe",
                "mysql" => "mysqld.exe",
                "redis" => "redis-server.exe",
                _ => "",
            };
            if !exe_name.is_empty() {
                let mut kill_cmd = Command::new("taskkill");
                kill_cmd.args(["/F", "/IM", exe_name, "/T"]);
                #[cfg(target_os = "windows")]
                {
                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                    kill_cmd.creation_flags(CREATE_NO_WINDOW);
                }
                let _ = kill_cmd.output();
            }
        } else {
            // macOS / Linux: Unload LaunchAgent plist first so launchd does NOT auto-restart it!
            let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/simbianbn".to_string());
            let brew_svc = match self.name.as_str() {
                "apache" => "httpd",
                "mysql" => "mysql",
                "redis" => "redis",
                _ => "",
            };
            if !brew_svc.is_empty() {
                let plist1 = format!("{}/Library/LaunchAgents/sh.brew.{}.plist", home, brew_svc);
                let plist2 = format!(
                    "{}/Library/LaunchAgents/homebrew.mxcl.{}.plist",
                    home, brew_svc
                );
                for pl in [plist1, plist2] {
                    if Path::new(&pl).exists() {
                        let _ = Command::new("/bin/launchctl")
                            .args(["unload", &pl])
                            .output();
                    }
                }
            }

            // Also kill the actual running processes
            match self.name.as_str() {
                "apache" => {
                    let _ = Command::new("/opt/homebrew/bin/apachectl")
                        .arg("stop")
                        .output();
                    let _ = Command::new("/usr/bin/pkill")
                        .args(["-15", "-f", "httpd"])
                        .output();
                }
                "mysql" => {
                    let _ = Command::new("/opt/homebrew/bin/mysqladmin")
                        .args(["-u", "root", "shutdown"])
                        .output();
                    let _ = Command::new("/usr/bin/pkill")
                        .args(["-15", "-f", "mysqld_safe"])
                        .output();
                    let _ = Command::new("/usr/bin/pkill")
                        .args(["-15", "-f", "mysqld"])
                        .output();
                }
                "redis" => {
                    let _ = Command::new("/opt/homebrew/bin/redis-cli")
                        .args(["-p", &self.port.to_string(), "shutdown"])
                        .output();
                    let _ = Command::new("/usr/bin/pkill")
                        .args(["-15", "-f", "redis-server"])
                        .output();
                }
                _ => {}
            }

            // Wait up to 900ms for graceful stop, then force-kill if port is still open
            let wait_start = Instant::now();
            while wait_start.elapsed() < Duration::from_millis(900) {
                if !Self::is_port_open(self.port) {
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }

            if Self::is_port_open(self.port) {
                if let (Some(pid), _) = PortScanner::find_port_owner(self.port) {
                    let _ = Command::new("/bin/kill")
                        .args(["-9", &pid.to_string()])
                        .output();
                }
                let proc_pattern = match self.name.as_str() {
                    "apache" => "httpd",
                    "mysql" => "mysqld",
                    "redis" => "redis-server",
                    _ => "",
                };
                if !proc_pattern.is_empty() {
                    let _ = Command::new("/usr/bin/pkill")
                        .args(["-9", "-f", proc_pattern])
                        .output();
                }
                thread::sleep(Duration::from_millis(150));
            }
        }

        self.started_at = None;
        logs.push(format!(
            "[{}] หยุดการทำงาน {} เรียบร้อยแล้ว",
            Local::now().format("%H:%M:%S"),
            self.display_name
        ));
        Ok(())
    }

    pub fn to_dto(&mut self) -> ServiceStatusDto {
        let port_open = Self::is_port_open(self.port);
        let alive = self.check_alive();
        let pid = self
            .child
            .as_ref()
            .map(|c| c.id())
            .or(self.pid_override);

        ServiceStatusDto {
            name: self.name.clone(),
            display_name: self.display_name.clone(),
            binary_path: self.binary_path.clone(),
            port: self.port,
            running: alive || port_open,
            port_open,
            pid,
            uptime_note: self
                .started_at
                .clone()
                .map(|t| format!("Started at {}", t))
                .unwrap_or_else(|| {
                    if port_open {
                        "Active / Online".to_string()
                    } else {
                        "Stopped".to_string()
                    }
                }),
        }
    }
}

pub struct PortScanner;

impl PortScanner {
    pub fn scan_conflicts(cfg: &Config) -> Vec<PortConflictEntry> {
        let watched = vec![
            (cfg.apache_port, "Apache HTTP"),
            (cfg.apache_ssl_port, "Apache HTTPS / SSL"),
            (cfg.mysql_port, "MySQL Server"),
            (cfg.redis_port, "Redis Server"),
            (cfg.mail_catcher_smtp_port, "Mail Catcher SMTP"),
            (3000, "Node.js / PM2 App"),
            (80, "Standard HTTP (80)"),
        ];

        let mut results = Vec::new();
        for (port, hint) in watched {
            if results.iter().any(|r: &PortConflictEntry| r.port == port) {
                continue;
            }
            let is_open = ServiceProcess::is_port_open(port);
            let (pid, proc_name) = if is_open {
                Self::find_port_owner(port)
            } else {
                (None, "-".to_string())
            };
            results.push(PortConflictEntry {
                port,
                service_hint: hint.to_string(),
                is_open,
                pid,
                process_name: proc_name,
                protocol: "TCP".to_string(),
            });
        }
        results
    }

    pub fn find_port_owner(port: u16) -> (Option<u32>, String) {
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            let lsof_bin = if Path::new("/usr/sbin/lsof").exists() {
                "/usr/sbin/lsof"
            } else {
                "lsof"
            };
            if let Ok(out) = Command::new(lsof_bin)
                .args(["-i", &format!("TCP:{}", port), "-sTCP:LISTEN", "-n", "-P"])
                .output()
            {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines().skip(1) {
                    let cols: Vec<&str> = line.split_whitespace().collect();
                    if cols.len() >= 2 {
                        let proc_name = cols[0].to_string();
                        let pid = cols[1].parse::<u32>().ok();
                        return (pid, proc_name);
                    }
                }
            }
        } else if cfg!(target_os = "windows") {
            if let Ok(out) = Command::new("netstat").args(["-ano"]).output() {
                let text = String::from_utf8_lossy(&out.stdout);
                let needle = format!(":{}", port);
                for line in text.lines() {
                    if line.contains(&needle) && line.contains("LISTENING") {
                        let cols: Vec<&str> = line.split_whitespace().collect();
                        if let Some(last) = cols.last() {
                            if let Ok(pid) = last.parse::<u32>() {
                                return (Some(pid), format!("PID {}", pid));
                            }
                        }
                    }
                }
            }
        }
        (None, "Active Socket".to_string())
    }

    pub fn kill_port_owner(port: u16, logs: &mut Vec<String>) -> Result<String, String> {
        let (pid_opt, proc_name) = Self::find_port_owner(port);
        if let Some(pid) = pid_opt {
            if pid == std::process::id() {
                return Err("พอร์ตนี้เป็นของโปรแกรม rusthost เอง (เช่น Mail Catcher)".to_string());
            }
            let status = if cfg!(target_os = "windows") {
                Command::new("taskkill")
                    .args(["/F", "/PID", &pid.to_string()])
                    .status()
            } else {
                Command::new("/bin/kill")
                    .args(["-9", &pid.to_string()])
                    .status()
            };

            match status {
                Ok(s) if s.success() => {
                    let msg = format!(
                        "[{}] สั่งปิดโปรเซส {} (PID {}) บนพอร์ต {} สำเร็จ",
                        Local::now().format("%H:%M:%S"),
                        proc_name,
                        pid,
                        port
                    );
                    logs.push(msg.clone());
                    Ok(msg)
                }
                _ => Err(format!("ไม่สามารถปิดโปรเซส PID {} บนพอร์ต {} ได้", pid, port)),
            }
        } else {
            Err(format!("ไม่พบ PID ที่ถือครองพอร์ต {}", port))
        }
    }
}
