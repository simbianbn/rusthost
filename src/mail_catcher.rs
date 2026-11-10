use chrono::Local;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedEmail {
    pub id: u64,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub received_at: String,
    pub html_body: String,
    pub raw_body: String,
}

#[derive(Clone)]
pub struct MailCatcher {
    pub port: u16,
    pub inbox: Arc<Mutex<Vec<CapturedEmail>>>,
}

impl MailCatcher {
    pub fn new(port: u16) -> Self {
        let initial = vec![CapturedEmail {
            id: 1,
            from: "no-reply@demo.test".to_string(),
            to: "developer@rusthost.local".to_string(),
            subject: "ยินดีต้อนรับสู่ rusthost Local Mail Catcher (SMTP Trap :1025)".to_string(),
            received_at: Local::now().format("%H:%M:%S").to_string(),
            html_body: "<div style='font-family:sans-serif;padding:16px;background:#261c35;color:#fff;border-radius:12px;'><h3 style='color:#ff66a3;margin-top:0;'>rusthost SMTP Trap Ready!</h3><p>ตั้งค่า <code>MAIL_HOST=127.0.0.1</code> และ <code>MAIL_PORT=1025</code> ในไฟล์ <code>.env</code> เพื่อดักจับอีเมลทั้งหมดจาก PHP / Laravel / Node.js ได้ทันที</p></div>".to_string(),
            raw_body: "From: no-reply@demo.test\r\nTo: developer@rusthost.local\r\nSubject: Welcome to rusthost Mail Catcher\r\n\r\nrusthost SMTP Trap Ready on port 1025.".to_string(),
        }];

        let catcher = Self {
            port,
            inbox: Arc::new(Mutex::new(initial)),
        };
        catcher.spawn_listener();
        catcher
    }

    pub fn spawn_listener(&self) {
        let port = self.port;
        let inbox = Arc::clone(&self.inbox);

        thread::spawn(move || {
            if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) {
                for stream in listener.incoming().flatten() {
                    let inbox_ref = Arc::clone(&inbox);
                    thread::spawn(move || {
                        Self::handle_smtp_client(stream, inbox_ref);
                    });
                }
            }
        });
    }

    fn handle_smtp_client(mut stream: TcpStream, inbox: Arc<Mutex<Vec<CapturedEmail>>>) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.write_all(b"220 rusthost.local ESMTP MailCatcher Ready\r\n");

        let cloned = match stream.try_clone() {
            Ok(s) => s,
            Err(_) => return,
        };
        let mut reader = BufReader::new(cloned);
        let mut line = String::new();
        let mut from = "app@localhost".to_string();
        let mut to = "dev@localhost".to_string();
        let mut in_data = false;
        let mut raw_data = String::new();

        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
            if in_data {
                if trimmed == "." {
                    in_data = false;
                    let _ = stream.write_all(b"250 2.0.0 Ok: queued in rusthost inbox\r\n");
                    Self::store_email(&inbox, &from, &to, &raw_data);
                    raw_data.clear();
                } else {
                    raw_data.push_str(trimmed);
                    raw_data.push('\n');
                }
            } else {
                let upper = trimmed.to_uppercase();
                if upper.starts_with("HELO") || upper.starts_with("EHLO") {
                    let _ = stream.write_all(b"250-rusthost.local\r\n250 SIZE 10485760\r\n");
                } else if upper.starts_with("MAIL FROM:") {
                    from = trimmed[10..].trim().trim_matches(&['<', '>'][..]).to_string();
                    let _ = stream.write_all(b"250 2.1.0 Ok\r\n");
                } else if upper.starts_with("RCPT TO:") {
                    to = trimmed[8..].trim().trim_matches(&['<', '>'][..]).to_string();
                    let _ = stream.write_all(b"250 2.1.5 Ok\r\n");
                } else if upper == "DATA" {
                    in_data = true;
                    let _ = stream.write_all(b"354 End data with <CR><LF>.<CR><LF>\r\n");
                } else if upper.starts_with("QUIT") {
                    let _ = stream.write_all(b"221 2.0.0 Bye\r\n");
                    break;
                } else {
                    let _ = stream.write_all(b"250 Ok\r\n");
                }
            }
            line.clear();
        }
    }

    fn store_email(inbox: &Arc<Mutex<Vec<CapturedEmail>>>, from: &str, to: &str, raw: &str) {
        let mut subject = "(No Subject)".to_string();
        let mut body_lines = Vec::new();
        let mut in_headers = true;

        for l in raw.lines() {
            if in_headers {
                if l.trim().is_empty() {
                    in_headers = false;
                } else if l.to_lowercase().starts_with("subject:") {
                    subject = l[8..].trim().to_string();
                }
            } else {
                body_lines.push(l);
            }
        }

        let body = if body_lines.is_empty() {
            raw.to_string()
        } else {
            body_lines.join("\n")
        };

        if let Ok(mut guard) = inbox.lock() {
            let next_id = guard.iter().map(|m| m.id).max().unwrap_or(0) + 1;
            guard.insert(
                0,
                CapturedEmail {
                    id: next_id,
                    from: from.to_string(),
                    to: to.to_string(),
                    subject,
                    received_at: Local::now().format("%H:%M:%S").to_string(),
                    html_body: body,
                    raw_body: raw.to_string(),
                },
            );
        }
    }

    pub fn send_test_email(&self) -> Result<String, String> {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", self.port)) {
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let payload = format!(
                "EHLO localhost\r\nMAIL FROM:<system@rusthost.test>\r\nRCPT TO:<admin@rusthost.test>\r\nDATA\r\nFrom: system@rusthost.test\r\nTo: admin@rusthost.test\r\nSubject: ทดสอบส่งอีเมลผ่าน rusthost SMTP ({})\r\n\r\n<div style='padding:14px;background:#2f2240;color:#fff;border-radius:10px;'><h4>ทดสอบระบบ SMTP Trap สำเร็จ!</h4><p>อีเมลนี้ถูกส่งเข้าพอร์ต <strong>{}</strong> และดักจับโดย rusthost เรียบร้อยแล้ว</p></div>\r\n.\r\nQUIT\r\n",
                Local::now().format("%H:%M:%S"),
                self.port
            );
            let _ = stream.write_all(payload.as_bytes());
            thread::sleep(Duration::from_millis(80));
            Ok("ส่งอีเมลทดสอบเข้าสู่กล่องจดหมายจำลอง (Port 1025) สำเร็จ".to_string())
        } else {
            Self::store_email(
                &self.inbox,
                "system@rusthost.test",
                "admin@rusthost.test",
                "Subject: ทดสอบอีเมลจำลอง rusthost\n\nอีเมลทดสอบถูกบันทึกเข้าสู่ระบบเรียบร้อยแล้ว",
            );
            Ok("สร้างอีเมลทดสอบในกล่องจดหมายจำลองสำเร็จ".to_string())
        }
    }

    pub fn list_messages(&self) -> Vec<CapturedEmail> {
        self.inbox.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn clear_inbox(&self) {
        if let Ok(mut guard) = self.inbox.lock() {
            guard.clear();
        }
    }
}
