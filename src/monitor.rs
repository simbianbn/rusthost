use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, ProcessRefreshKind, RefreshKind, System};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceResourceUsage {
    pub group_key: String,
    pub group_name: String,
    pub process_count: usize,
    pub cpu_percent: f32,
    pub memory_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorSnapshot {
    pub cpu_brand: String,
    pub logical_cores: usize,
    pub global_cpu_pct: f32,
    pub per_core_cpu_pct: Vec<f32>,
    pub total_ram_mb: u64,
    pub used_ram_mb: u64,
    pub total_ram_gb: f64,
    pub used_ram_gb: f64,
    pub ram_usage_pct: f32,
    pub total_swap_mb: u64,
    pub used_swap_mb: u64,
    pub service_groups: Vec<ServiceResourceUsage>,
    pub cpu_history: Vec<f32>,
    pub ram_history: Vec<f32>,
    pub os_name: String,
    pub system_uptime_secs: u64,
}

pub struct ResourceMonitor {
    sys: System,
    cpu_brand: String,
    logical_cores: usize,
    cpu_history: VecDeque<f32>,
    ram_history: VecDeque<f32>,
    last_snapshot: MonitorSnapshot,
}

impl ResourceMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        sys.refresh_cpu();
        sys.refresh_memory();

        let cpus = sys.cpus();
        let cpu_brand = cpus
            .first()
            .map(|c| c.brand().to_string())
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| "Apple Silicon / x86_64 Processor".to_string());
        let logical_cores = cpus.len().max(1);

        let mut monitor = Self {
            sys,
            cpu_brand,
            logical_cores,
            cpu_history: VecDeque::from(vec![0.0; 60]),
            ram_history: VecDeque::from(vec![0.0; 60]),
            last_snapshot: MonitorSnapshot {
                cpu_brand: String::new(),
                logical_cores: 1,
                global_cpu_pct: 0.0,
                per_core_cpu_pct: vec![],
                total_ram_mb: 0,
                used_ram_mb: 0,
                total_ram_gb: 0.0,
                used_ram_gb: 0.0,
                ram_usage_pct: 0.0,
                total_swap_mb: 0,
                used_swap_mb: 0,
                service_groups: vec![],
                cpu_history: vec![0.0; 60],
                ram_history: vec![0.0; 60],
                os_name: System::long_os_version().unwrap_or_else(|| std::env::consts::OS.to_string()),
                system_uptime_secs: System::uptime(),
            },
        };
        monitor.refresh();
        monitor
    }

    pub fn refresh(&mut self) -> MonitorSnapshot {
        self.sys.refresh_cpu();
        self.sys.refresh_memory();
        self.sys
            .refresh_processes_specifics(ProcessRefreshKind::new().with_cpu().with_memory());

        let global_cpu = self.sys.global_cpu_info().cpu_usage();
        let per_core: Vec<f32> = self.sys.cpus().iter().map(|c| c.cpu_usage()).collect();

        let total_ram_bytes = self.sys.total_memory();
        let used_ram_bytes = self.sys.used_memory();
        let total_ram_mb = total_ram_bytes / (1024 * 1024);
        let used_ram_mb = used_ram_bytes / (1024 * 1024);
        let total_ram_gb = (total_ram_bytes as f64) / (1024.0 * 1024.0 * 1024.0);
        let used_ram_gb = (used_ram_bytes as f64) / (1024.0 * 1024.0 * 1024.0);
        let ram_pct = if total_ram_bytes > 0 {
            ((used_ram_bytes as f64 / total_ram_bytes as f64) * 100.0) as f32
        } else {
            0.0
        };

        let total_swap_mb = self.sys.total_swap() / (1024 * 1024);
        let used_swap_mb = self.sys.used_swap() / (1024 * 1024);

        if self.cpu_history.len() >= 60 {
            self.cpu_history.pop_front();
        }
        self.cpu_history.push_back(global_cpu);

        if self.ram_history.len() >= 60 {
            self.ram_history.pop_front();
        }
        self.ram_history.push_back(ram_pct);

        let mut apache = ServiceResourceUsage {
            group_key: "apache".to_string(),
            group_name: "Apache HTTP Server (httpd)".to_string(),
            ..Default::default()
        };
        let mut mysql = ServiceResourceUsage {
            group_key: "mysql".to_string(),
            group_name: "MySQL Database (mysqld/mariadbd)".to_string(),
            ..Default::default()
        };
        let mut php = ServiceResourceUsage {
            group_key: "php".to_string(),
            group_name: "PHP Workers (php/php-cgi/php-fpm)".to_string(),
            ..Default::default()
        };
        let mut redis = ServiceResourceUsage {
            group_key: "redis".to_string(),
            group_name: "Redis Server (redis-server)".to_string(),
            ..Default::default()
        };
        let mut node_pm2 = ServiceResourceUsage {
            group_key: "node_pm2".to_string(),
            group_name: "Node.js & PM2 Workers (node/pm2)".to_string(),
            ..Default::default()
        };
        let mut rusthost = ServiceResourceUsage {
            group_key: "rusthost".to_string(),
            group_name: "rusthost Manager".to_string(),
            ..Default::default()
        };

        let self_pid = std::process::id();
        let core_div = (self.logical_cores as f32).max(1.0);

        for (pid, proc_info) in self.sys.processes() {
            let name = proc_info.name().to_lowercase();
            let cpu = proc_info.cpu_usage() / core_div;
            let mem_mb = (proc_info.memory() as f64) / (1024.0 * 1024.0);

            if pid.as_u32() == self_pid || name.contains("rusthost") {
                rusthost.process_count += 1;
                rusthost.cpu_percent += cpu;
                rusthost.memory_mb += mem_mb;
            } else if name.contains("httpd") || name.contains("apache") {
                apache.process_count += 1;
                apache.cpu_percent += cpu;
                apache.memory_mb += mem_mb;
            } else if name.contains("mysqld") || name.contains("mariadbd") {
                mysql.process_count += 1;
                mysql.cpu_percent += cpu;
                mysql.memory_mb += mem_mb;
            } else if name.starts_with("php") {
                php.process_count += 1;
                php.cpu_percent += cpu;
                php.memory_mb += mem_mb;
            } else if name.contains("redis-server") {
                redis.process_count += 1;
                redis.cpu_percent += cpu;
                redis.memory_mb += mem_mb;
            } else if name == "node" || name == "node.exe" || name.contains("pm2") {
                node_pm2.process_count += 1;
                node_pm2.cpu_percent += cpu;
                node_pm2.memory_mb += mem_mb;
            }
        }

        let snapshot = MonitorSnapshot {
            cpu_brand: self.cpu_brand.clone(),
            logical_cores: self.logical_cores,
            global_cpu_pct: global_cpu,
            per_core_cpu_pct: per_core,
            total_ram_mb,
            used_ram_mb,
            total_ram_gb,
            used_ram_gb,
            ram_usage_pct: ram_pct,
            total_swap_mb,
            used_swap_mb,
            service_groups: vec![apache, mysql, php, redis, node_pm2, rusthost],
            cpu_history: self.cpu_history.iter().copied().collect(),
            ram_history: self.ram_history.iter().copied().collect(),
            os_name: System::long_os_version().unwrap_or_else(|| std::env::consts::OS.to_string()),
            system_uptime_secs: System::uptime(),
        };

        self.last_snapshot = snapshot.clone();
        snapshot
    }

    pub fn cached_snapshot(&self) -> MonitorSnapshot {
        self.last_snapshot.clone()
    }
}
