# rusthost
**Local Development & Admin Environment Manager (Rust + Tauri v2)**

- **สถาปัตยกรรม (Architecture)**: **Tauri v2** (Rust Backend ล้วน `src-tauri/src/*.rs` + HTML/CSS/SVG Glassmorphism Frontend)
- **ระบบปฏิบัติการที่รองรับ (Cross-Platform)**: รองรับทั้ง **macOS** (Homebrew / XAMPP / MAMP / Custom binaries, `/etc/hosts`, `kill`/`pkill`) และ **Windows** (Laragon / XAMPP / MAMP / Custom binaries, `httpd.exe`, `mysqld.exe`, `redis-server.exe`, `node.exe`, `pm2`, `C:\Windows\System32\drivers\etc\hosts`, `taskkill`, PowerShell Installer & `.exe` Packaging)
- **ระบบประหยัดพลังงานอัจฉริยะ (Eco & Sleep Power-Saving Engine)**: ลดการใช้ CPU/Battery เหลือเกือบ `0%` อัตโนมัติเมื่อพับจอ (System Sleep), ย่อหน้าต่างลง Tray หรือไม่ได้โฟกัสหน้าต่าง

---

## สรุปฟังก์ชันการทำงานทั้งหมดของโปรแกรม

### 1. ระบบจัดการเซอร์วิสหลัก & Port Scanner (Service & Port Management — `src/services.rs`)
- `ServiceProcess::start()` : สั่งรันเซอร์วิสหลัก (**Apache**, **MySQL**, **Redis Server**) แบบซ่อนหน้าต่าง CMD พร้อมบันทึกเวลาลง Log
- `ServiceProcess::stop()` : หยุดการทำงานของเซอร์วิสและเคลียร์โปรเซสค้างด้วย `taskkill` / `kill`
- `ServiceProcess::check_alive()` : ตรวจสอบสถานะโปรเซสแบบ Real-time ว่ายังทำงานอยู่หรือหยุดกะทันหัน
- `ServiceProcess::is_port_open()` : ทดสอบการเชื่อมต่อพอร์ต TCP (เช่น Apache `80`/`443`, MySQL `3306`, Redis `6379`, SMTP Trap `1025`) ว่าเปิดรับการเชื่อมต่อได้จริงหรือไม่
- `PortScanner::scan_conflicts()` / `kill_port_owner()` : **Port Conflict Scanner & Killer** — ตรวจจับว่ามีโปรเซสใดแย่งพอร์ตสำคัญ (`80`, `443`, `3306`, `6379`, `3000`, `8080`) อยู่ (แสดง PID, ชื่อโปรแกรม, โปรโตคอล) พร้อมปุ่มสั่ง Kill โปรเซสนั้นได้ทันที
- `RustHostApp::start_all()` : เริ่มการทำงานทุกเซอร์วิสที่เปิดใช้งานพร้อมกันในคลิกเดียว พร้อมโหลดคอนฟิกโปรเจกต์อัตโนมัติ
- `RustHostApp::stop_all()` : หยุดการทำงานทุกเซอร์วิสพร้อมกัน (และทำงานอัตโนมัติเมื่อปิดโปรแกรม)

---

### 2. ระบบตรวจวัดทรัพยากรเครื่อง, รายงานระบบ & โหมดประหยัดพลังงาน (Monitor, Report & Eco Mode — `src/monitor.rs`, `src/power_saver.rs`)
- `ResourceMonitor::new()` : ดึงข้อมูลสเปกเครื่อง ชื่อรุ่น CPU, จำนวนแกน (Logical Cores), ขนาด RAM ทั้งหมด, ระบบปฏิบัติการ และ Uptime
- `ResourceMonitor::refresh()` : อัปเดตค่าการใช้งานจริงแบบ Real-time (เฉพาะเฉพาะตอนเปิดดูหน้าจอ หรือตามจังหวะ Adaptive Polling):
  - เปอร์เซ็นต์การใช้ CPU รวมของระบบ (Global CPU %) และแยกตามรายแกน (Per-Core CPU %)
  - ปริมาณการใช้ RAM และ Swap ของระบบ (Used / Total MB & GB)
  - **แยกคำนวณการใช้ CPU (%) และ RAM (MB) เฉพาะของแต่ละกลุ่มเซอร์วิสในหน้ารายงานระบบ**:
    1. **Apache HTTP Server** (`httpd` / `httpd.exe`)
    2. **MySQL Database Server** (`mysqld` / `mariadbd`)
    3. **PHP Workers** (`php` / `php-cgi` / `php-fpm`)
    4. **Redis Server** (`redis-server` / `redis-server.exe`)
    5. **Node.js & PM2 Processes** (`node` / `node.exe` / `PM2 God Daemon`)
    6. **ตัวโปรแกรม `rusthost` เอง**
  - บันทึกประวัติย้อนหลัง 60 วินาที (Rolling History) สำหรับวาดกราฟเส้นแบบสด
- `ResourceMonitor::generate_system_report()` : สรุปรายงานสถานะระบบทั้งหมด (Health Status, Active Ports, SSL Status, Active Versions ของ Apache / MySQL / PHP / Redis / Node.js / PM2, ปริมาณการใช้ทรัพยากรรวม) พร้อมส่งออกหรือคัดลอกรายงานได้
- `PowerSaverEngine` (`src/power_saver.rs`) : **ระบบประหยัดพลังงานเครื่องตอน Sleep / Idle**:
  - **Sleep & Wake Detection**: ตรวจจับสถานะเครื่องเข้าสู่โหมด Sleep / พับหน้าจอ (`visibilitychange`, Window Blur/Minimize และ System Sleep Time-Jump Detection)
  - **Zero-Polling on Sleep / Hidden**: หยุดการดึงข้อมูล `sysinfo`, `SHOW GLOBAL STATUS`, `redis-cli INFO` และ `pm2 jlist` ทันทีเมื่อหน้าต่างถูกซ่อนหรือเครื่องหลับ เพื่อไม่ให้ปลุก CPU (Prevent Wakeups & Battery Drain)
  - **Adaptive Eco Throttling**: ปรับความถี่การอัปเดตอัตโนมัติ (`Active Mode: 2s`, `Background/Unfocused: 15s`, `Minimized/Sleep: 0Hz Paused`) และหยุด CSS/Canvas Animation ทั้งหมดเมื่อไม่ได้มองหน้าจอ
  - **Auto-Suspend Services on Sleep (ตัวเลือกเปิด/ปิดได้)**: สามารถตั้งให้พักการทำงานของเซอร์วิสชั่วคราว (`SIGSTOP` / Stop on Sleep) เมื่อเครื่องเข้าโหมด Sleep นานเกินกำหนด และปลุกกลับมาทำงานอัตโนมัติเมื่อเปิดฝาเครื่อง

---

### 3. ระบบแดชบอร์ด MySQL & 1-Click Backup/Restore (MySQL Dashboard — `src/mysql_monitor.rs`)
#### ส่วนของ Admin — ตรวจสอบการเชื่อมต่อและประสิทธิภาพเซิร์ฟเวอร์
- `MysqlDashboard::refresh_all()` : ดึงค่าสถานะเชิงลึกจาก MySQL Server ได้แก่:
  - **สถานะการเชื่อมต่อ**: `Threads_connected` (จำนวนที่เชื่อมต่ออยู่), `Threads_running` (ที่กำลังรันคำสั่ง), `Max_used_connections` (สูงสุดที่เคยใช้), `max_connections` (ลิมิตสูงสุดของเซิร์ฟเวอร์)
  - **สุขภาพการเชื่อมต่อ**: `Connections` (จำนวนครั้งที่เชื่อมต่อสะสม), `Aborted_connects` (เชื่อมต่อล้มเหลว), `Aborted_clients` (หลุดการเชื่อมต่อ), `Threads_cached`
  - **ประสิทธิภาพคำสั่ง SQL**: Total Queries, QPS (Queries Per Second), `Slow_queries` (คำสั่งที่ทำงานช้า), สถิติแยกตามคำสั่ง `SELECT` / `INSERT` / `UPDATE` / `DELETE`
  - **ทราฟฟิกและหน่วยความจำ**: `Bytes_received`, `Bytes_sent` และขนาดการใช้ InnoDB Buffer Pool (MB)
  - **รายการโปรเซสสด (`SHOW FULL PROCESSLIST`)**: แสดง ID, User, Host, Database, Command, Time, State, SQL Query
- `MysqlDashboard::kill_connection()` : สั่งตัดการเชื่อมต่อ (`KILL <process_id>`) สำหรับ Admin เพื่อหยุด Query ที่ค้างหรือกินทรัพยากร

#### ส่วนของ Developer — จัดการฐานข้อมูล, รันคำสั่ง SQL และ Backup/Import
- `MysqlDashboard::create_database()` : สร้างฐานข้อมูลใหม่ (`CREATE DATABASE`) พร้อมกำหนด Character Set และ Collation (เช่น `utf8mb4_unicode_ci`)
- `MysqlDashboard::load_tables_for_db()` : ดูโครงสร้างตารางทั้งหมดในฐานข้อมูลที่เลือก (ชื่อตาราง, Engine, จำนวนแถว, ขนาดไฟล์ KB, Collation)
- `MysqlDashboard::execute_dev_sql()` : หน้าต่าง SQL Console สำหรับรันคำสั่ง SQL สด พร้อมจับเวลาการทำงาน (ms) และแสดงผลลัพธ์เป็นตาราง
- `MysqlDashboard::backup_database()` / `import_sql_file()` : **MySQL 1-Click Backup & Import (.sql)** — สำรองฐานข้อมูลผ่าน `mysqldump` และนำเข้าไฟล์ `.sql` ขนาดใหญ่ผ่าน `mysql` CLI โดยตรง

---

### 4. ระบบจัดการหลายโปรเจกต์, VirtualHost, Auto SSL & Dev Workflow (Multi-Location, SSL & Scaffolder — `src/vhost.rs`)
- `VHostManager::generate_apache_config()` : สร้างไฟล์คอนฟิก Apache อัตโนมัติ แยกตามรายโปรเจกต์ รองรับ 3 รูปแบบ:
  1. **Sub-path Alias** (เช่น `http://localhost/project1` -> ชี้ไปโฟลเดอร์ที่ 1, `http://localhost/project2` -> ชี้ไปโฟลเดอร์ที่ 2)
  2. **VirtualHost แยกโดเมน** (เช่น `http://project1.test` และ `https://project1.test` -> ชี้ไปโฟลเดอร์ที่ 1)
  3. **เปิดใช้งานทั้ง Sub-path และ VirtualHost พร้อมกัน**
  - รองรับ **Reverse Proxy สำหรับโปรเจกต์ Node.js / PM2** (เช่นผูกโดเมน `https://api.test` เข้ากับพอร์ต `localhost:3000` ของแอป Node.js/PM2 ได้ในคลิกเดียว)
  - ตั้งค่าเฉพาะรายโปรเจกต์ได้อิสระ: โฟลเดอร์ปลายทาง (คนละไดรฟ์ได้), `AllowOverride` (`.htaccess`), `Options +Indexes`, `DirectoryIndex`, ตัวแปร `SetEnv` เฉพาะโปรเจกต์, เลือกเวอร์ชัน PHP เฉพาะโปรเจกต์ และ Custom Apache Directives
- `SslManager::generate_cert_for_domain()` : **Auto SSL / HTTPS (`.test` Domains)** — สร้าง Local Root CA และออกใบรับรอง SSL (`mkcert` / OpenSSL) ให้แต่ละโดเมนพร้อมตั้งค่า VirtualHost `:443` อัตโนมัติ
- `ProjectScaffolder::create_project()` : **1-Click Project Scaffolder** — สร้างโครงสร้างโปรเจกต์ใหม่ได้ทันที (Laravel, WordPress, Express/Node.js, Next.js หรือ Blank PHP พร้อมหน้าทดสอบ)
- `EnvManager::load_env()` / `save_env()` : **`.env` Visual Editor** — อ่านและแก้ไขตัวแปรในไฟล์ `.env` ของแต่ละโปรเจกต์แบบตาราง
- `DevLauncher::open_ide()` / `open_scoped_terminal()` : **Quick IDE & Scoped Terminal** — เปิดโปรเจกต์ด้วย VS Code / Cursor ทันที หรือเปิดหน้าต่าง Terminal ที่โหลด `PATH` ของ PHP, Composer, Node.js, npm ตามเวอร์ชันของโปรเจกต์นั้นให้อัตโนมัติ
- `TunnelManager::start_tunnel()` / `stop_tunnel()` : **Public Share Tunnel + QR Code** — แชร์โปรเจกต์ในเครื่องออกสู่อินเทอร์เน็ตชั่วคราวพร้อมสร้าง **QR Code** บนหน้าจอ

---

### 5. ระบบจัดการหลายเวอร์ชัน PHP (Multi-PHP Version Manager — `src/php_manager.rs`)
- `PhpManager::scan_installed_versions()` : สแกนหาเวอร์ชัน PHP ทั้งหมดที่มีในเครื่องอัตโนมัติ (เช่น `7.4`, `8.1`, `8.2`, `8.3`, `8.4`)
- `PhpManager::switch_global_php()` : สลับเวอร์ชัน PHP หลัก (Global PHP Version) ของ Apache และ CLI ในคลิกเดียว พร้อมรีโหลด Apache อัตโนมัติ
- `PhpManager::configure_per_project_php()` : รองรับการกำหนดเวอร์ชัน PHP แยกตามแต่ละโปรเจกต์/VirtualHost
- `PhpManager::get_extensions()` / `toggle_extension()` : อ่านและเปิด/ปิด PHP Extensions (เช่น `mysqli`, `pdo_mysql`, `redis`, `curl`, `gd`, `mbstring`, `openssl`, `zip`) ในไฟล์ `php.ini`
- `PhpManager::update_php_ini_settings()` : ปรับค่าสำคัญใน `php.ini` แบบรวดเร็วผ่าน UI (`memory_limit`, `upload_max_filesize`, `post_max_size`, `max_execution_time`, `display_errors`)

---

### 6. ระบบจัดการ Redis, Node.js และ PM2 (Redis, Node.js & PM2 Manager — `src/node_redis.rs`)
- **Redis Server & Monitor**: เปิด-ปิด `redis-server`, ดึงข้อมูลสดจาก `redis-cli INFO` (Memory, Connected Clients, Hit Ratio, Keys `db0`-`db15`, Ops/sec) และรันคำสั่ง CLI / `FLUSHDB`
- **Node.js & Multi-Node Runtime**: ตรวจจับและสลับเวอร์ชัน `Node.js` / `npm` / `pnpm` / `yarn`
- **PM2 Process Manager & Status Dashboard**: ดึงรายการโปรเซสจาก `pm2 jlist` แสดงสถานะ, โหมด, CPU %, RAM (MB), Uptime, Restart Count พร้อมปุ่ม `Start`, `Restart`, `Reload`, `Stop`, `Delete` และดู Log สด

---

### 7. ระบบดักจับอีเมลในเครื่อง (Local Mail Catcher / SMTP Trap — `src/mail_catcher.rs`)
- รันเซิร์ฟเวอร์จำลอง SMTP ด้วย Rust บนพอร์ต `1025` เพื่อรับอีเมลทั้งหมดที่ส่งออกจาก PHP/Node.js ในเครื่อง มาแสดงในกล่องจดหมายจำลองพร้อม HTML Preview, Plain Text, Raw Headers และปุ่มส่งเมลทดสอบ

---

### 8. ตัวติดตั้งอัจฉริยะ & ระบบตรวจจับ/ถอนการติดตั้ง XAMPP, Laragon, MAMP (Installer & Legacy Stack Cleaner — `src/legacy_cleaner.rs`, `installer/`)
- **การตรวจจับโปรแกรมเดิมก่อนติดตั้ง (Pre-Install Legacy Stack Scanner)**:
  - สแกนหาการติดตั้งเดิมของ **XAMPP**, **Laragon**, และ **MAMP** ทั้งบน Windows (`C:\xampp`, `C:\laragon`, `C:\MAMP`, Registry Uninstall Keys) และ macOS (`/Applications/XAMPP`, `/Applications/MAMP`, `/Library/Application Support/appsolute/MAMP PRO`)
  - คำนวณขนาดพื้นที่ดิสก์ที่โปรแกรมเดิมใช้ (MB/GB), ตรวจสอบว่ามีเซอร์วิสของ XAMPP/Laragon/MAMP รันค้างแย่งพอร์ต `80`/`3306` อยู่หรือไม่ และตรวจหาโฟลเดอร์โปรเจกต์เดิม (`htdocs` / `www`)
- **ตัวเลือกถามผู้ใช้ก่อนติดตั้ง (Interactive Prompt in Installer & In-App Wizard)**:
  1. **สำรองข้อมูลโปรเจกต์ (`htdocs`/`www`) แล้วลบ XAMPP / Laragon / MAMP ออกจากเครื่อง** เพื่อคืนพื้นที่ดิสก์และป้องกันพอร์ต/เซอร์วิสชนกัน
  2. **ลบเฉพาะโปรแกรม XAMPP / Laragon / MAMP ทันที** (สั่งหยุดโปรเซสค้าง, ถอน Service เดิม, และลบโฟลเดอร์/Uninstaller)
  3. **เก็บไว้ก่อน (Keep Existing)** และนำเข้าโปรเจกต์/ไบนารีมาใช้งานร่วมกับ `rusthost`
- **สคริปต์ติดตั้งครบทั้ง 2 ระบบปฏิบัติการ**:
  - `installer/install.ps1` & `Install-RustHost.bat` (Windows GUI Setup Wizard พร้อมตรวจจับและถามลบ XAMPP/Laragon/MAMP ก่อนลงโปรแกรม)
  - `installer/install-macos.sh` (macOS Interactive Setup & Legacy XAMPP/MAMP Uninstaller Wizard)
  - `installer/uninstall.ps1` & `installer/uninstall-macos.sh` (ระบบถอนการติดตั้งสะอาดหมดจด)
- **หน้าจัดการภายในโปรแกรม (`Legacy Cleaner & Migration` ใน `rusthost`)**: สามารถกดสแกน, นำเข้าโปรเจกต์จาก `htdocs`/`www` เดิม หรือสั่งถอนการติดตั้ง XAMPP / Laragon / MAMP จากภายในหน้า UI ของ `rusthost` ได้ตลอดเวลา
