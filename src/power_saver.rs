use crate::config::PowerSaveConfig;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerState {
    Active,
    UnfocusedEco,
    MinimizedZeroPoll,
    SystemSleeping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerSaverStatus {
    pub state: PowerState,
    pub state_label: String,
    pub eco_mode_enabled: bool,
    pub recommended_poll_ms: u64,
    pub polling_paused: bool,
    pub sleep_events_detected: u64,
    pub wakeups_prevented: u64,
    pub last_sleep_wake_log: String,
    pub cpu_overhead_estimate_pct: f32,
}

pub struct PowerSaverEngine {
    pub state: PowerState,
    pub window_visible: bool,
    pub window_focused: bool,
    pub last_tick: Instant,
    pub last_refresh: Instant,
    pub sleep_events_detected: u64,
    pub wakeups_prevented: u64,
    pub last_sleep_wake_log: String,
}

impl PowerSaverEngine {
    pub fn new() -> Self {
        Self {
            state: PowerState::Active,
            window_visible: true,
            window_focused: true,
            last_tick: Instant::now(),
            last_refresh: Instant::now() - Duration::from_secs(10),
            sleep_events_detected: 0,
            wakeups_prevented: 0,
            last_sleep_wake_log: "Eco Engine Ready (Zero-Polling on Sleep enabled)".to_string(),
        }
    }

    /// Detects OS sleep/wake jumps and updates current power state
    pub fn update_window_state(
        &mut self,
        visible: bool,
        focused: bool,
        system_sleeping_hint: bool,
        cfg: &PowerSaveConfig,
    ) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick);
        self.last_tick = now;

        let mut woke_from_sleep = false;
        if elapsed > Duration::from_secs(15) {
            self.sleep_events_detected += 1;
            self.wakeups_prevented += elapsed.as_secs() / 2;
            self.last_sleep_wake_log = format!(
                "[{}] ตรวจพบเครื่องตื่นจากโหมด Sleep (พักการทำงานไป {} วินาที — ประหยัดพลังงานสำเร็จ)",
                Local::now().format("%H:%M:%S"),
                elapsed.as_secs()
            );
            woke_from_sleep = true;
        }

        self.window_visible = visible;
        self.window_focused = focused;

        if !cfg.eco_mode_enabled {
            self.state = PowerState::Active;
            return woke_from_sleep;
        }

        if system_sleeping_hint && cfg.pause_polling_on_sleep {
            self.state = PowerState::SystemSleeping;
            self.wakeups_prevented += 1;
        } else if !visible && cfg.pause_polling_on_minimize {
            self.state = PowerState::MinimizedZeroPoll;
            self.wakeups_prevented += 1;
        } else if !focused && cfg.throttle_when_unfocused {
            self.state = PowerState::UnfocusedEco;
        } else {
            self.state = PowerState::Active;
        }

        woke_from_sleep
    }

    /// Returns whether heavy system/process polling (`sysinfo`, `mysql`, `redis`, `pm2`) should run right now
    pub fn should_run_background_poll(&mut self, cfg: &PowerSaveConfig, force: bool) -> bool {
        if force {
            self.last_refresh = Instant::now();
            return true;
        }

        match self.state {
            PowerState::SystemSleeping | PowerState::MinimizedZeroPoll => {
                self.wakeups_prevented += 1;
                false
            }
            PowerState::UnfocusedEco => {
                let interval = Duration::from_millis(cfg.unfocused_refresh_ms.max(5000));
                if self.last_refresh.elapsed() >= interval {
                    self.last_refresh = Instant::now();
                    true
                } else {
                    self.wakeups_prevented += 1;
                    false
                }
            }
            PowerState::Active => {
                let interval = Duration::from_millis(cfg.active_refresh_ms.max(1000));
                if self.last_refresh.elapsed() >= interval {
                    self.last_refresh = Instant::now();
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn status(&self, cfg: &PowerSaveConfig) -> PowerSaverStatus {
        let (label, poll_ms, paused, cpu_est) = match self.state {
            PowerState::Active => (
                "Active (Normal Real-Time 2s)".to_string(),
                cfg.active_refresh_ms,
                false,
                0.3,
            ),
            PowerState::UnfocusedEco => (
                "Eco Throttled (Background 15s)".to_string(),
                cfg.unfocused_refresh_ms,
                false,
                0.04,
            ),
            PowerState::MinimizedZeroPoll => (
                "Zero-Poll Standby (Window Hidden / Tray)".to_string(),
                0,
                true,
                0.0,
            ),
            PowerState::SystemSleeping => (
                "Deep Sleep Protection (0% CPU Wakeups)".to_string(),
                0,
                true,
                0.0,
            ),
        };

        PowerSaverStatus {
            state: self.state,
            state_label: label,
            eco_mode_enabled: cfg.eco_mode_enabled,
            recommended_poll_ms: poll_ms,
            polling_paused: paused,
            sleep_events_detected: self.sleep_events_detected,
            wakeups_prevented: self.wakeups_prevented,
            last_sleep_wake_log: self.last_sleep_wake_log.clone(),
            cpu_overhead_estimate_pct: cpu_est,
        }
    }
}
