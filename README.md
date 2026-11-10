# rusthost

**`rusthost`** is a lightweight, native desktop local development stack manager (Apache, MySQL, PHP, Redis, Node.js/PM2) written in Rust (`tao` + `wry`) with a minimal monochrome dark UI and zero-wakeup sleep optimization.

---

## Key Features

- **Native Desktop Application (`tao` + `wry`)**
  - Uses an in-process custom protocol (`rusthost://localhost/`) with zero local HTTP ports or external browser windows required.
- **Minimal Monochrome Dark Interface**
  - Unified Overview strip for **PHP**, **MySQL**, **Redis**, and **Node.js** with 1-click **Start All / Stop All** and per-service port controls.
- **Multi-PHP Runtime Manager (`7.4` – `8.5`)**
  - Instant global & per-project PHP switcher, `php.ini` tuning (`memory_limit`, `upload_max_filesize`, `post_max_size`, `max_execution_time`), and extension toggler.
- **MySQL & Redis Engine Console**
  - Live MySQL status metrics, `SHOW FULL PROCESSLIST` inspector, 1-click `.sql` backup (`mysqldump`), interactive SQL runner, and native TCP RESP Redis console.
- **VirtualHosts, Auto-SSL & LAN QR Sharing**
  - Automatic `.test` domain routing, self-signed TLS certificate generation, project scaffolder (Laravel, WordPress, Node.js, PHP), and SVG QR code generator for mobile testing.
- **Built-in SMTP Mail Catcher (`127.0.0.1:1025`)**
  - Captures outgoing local emails in memory without external dependencies.
- **Eco Guard & Zero-Polling on Sleep**
  - Uses `ControlFlow::Wait` and pauses background telemetry when minimized or when the host enters sleep mode (`0%` idle CPU overhead).
- **Legacy Stack Detector & Migrator**
  - Detects existing **XAMPP**, **Laragon**, or **MAMP** installations to migrate projects and prevent port `:80` / `:3306` conflicts.

---

## Quick Start

### macOS
```bash
cargo build --release
./installer/install-macos.sh
open /Applications/rusthost.app
```

### Windows
```powershell
.\Build-And-Package.bat
.\Install-RustHost.bat
```

### CLI Diagnostics
```bash
# Run all 8 automated verification suites
cargo run -- --self-test

# Dump current JSON state snapshot
cargo run -- --dump-state
```

---

## Project Structure

```text
├── Cargo.toml
├── project.md                 # Full technical specification
├── src/
│   ├── main.rs                # Native window (tao + wry) & custom protocol IPC
│   ├── config.rs              # Persistent configuration (~/.rusthost)
│   ├── services.rs            # Daemon lifecycle & port conflict scanner
│   ├── monitor.rs             # System & 6-group service telemetry
│   ├── power_saver.rs         # Eco Guard zero-wakeup sleep engine
│   ├── php_manager.rs         # Multi-PHP version & php.ini manager
│   ├── mysql_monitor.rs       # MySQL status, processlist, backup & SQL runner
│   ├── node_redis.rs          # Redis RESP client, Node.js & PM2 manager
│   ├── vhost.rs               # VirtualHosts, Auto-SSL, QR share & scaffolder
│   ├── mail_catcher.rs        # Built-in SMTP trap server (:1025)
│   ├── legacy_cleaner.rs      # XAMPP / Laragon / MAMP detector & migrator
│   └── tray.rs                # Native .app / .lnk desktop integration
├── ui/
│   ├── index.html             # Minimal monochrome UI shell
│   ├── styles.css             # Matte zinc dark theme (#09090b / #111115)
│   ├── app.js                 # Frontend state & IPC controller
│   ├── icon.png               # App icon
│   └── AppIcon.icns           # macOS bundle icon
└── installer/
    ├── install-macos.sh       # macOS .app packager & installer
    ├── uninstall-macos.sh     # macOS uninstaller
    ├── install.ps1            # Windows installer
    └── uninstall.ps1          # Windows uninstaller
```
