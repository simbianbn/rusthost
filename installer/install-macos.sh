#!/bin/zsh
# ==============================================================================
# rusthost — macOS Native .app Bundle Installer & Legacy XAMPP/MAMP Cleaner
# ==============================================================================
set -e

echo "===================================================================="
echo "  🧰 ตัวติดตั้งแอปพลิเคชัน rusthost.app สำหรับ macOS"
echo "===================================================================="

BACKUP_DIR="$HOME/Documents/rusthost-migrated-projects"

check_and_prompt_legacy() {
  local NAME="$1"
  local TARGET_PATH="$2"
  local WWW_PATH="$3"

  if [ -d "$TARGET_PATH" ]; then
    echo "⚠️  ตรวจพบ $NAME ติดตั้งอยู่ที่: $TARGET_PATH"
    echo -n "ต้องการสำรองโปรเจกต์ใน $WWW_PATH แล้วลบ $NAME ออกก่อนลงโปรแกรมหรือไม่? [y/N]: "
    read -r REPLY
    if [[ "$REPLY" =~ ^[Yy]$ ]]; then
      if [ -d "$WWW_PATH" ]; then
        mkdir -p "$BACKUP_DIR"
        cp -R "$WWW_PATH/"* "$BACKUP_DIR/" 2>/dev/null || true
        echo "  ✓ สำรองโปรเจกต์ไว้ที่ $BACKUP_DIR เรียบร้อยแล้ว"
      fi
      rm -rf "$TARGET_PATH" 2>/dev/null || sudo rm -rf "$TARGET_PATH"
      echo "  ✓ ลบ $NAME ($TARGET_PATH) เรียบร้อยแล้ว"
    else
      echo "  • เก็บ $NAME ไว้ตามเดิม"
    fi
  else
    echo "✓ ไม่พบ $NAME ($TARGET_PATH) ในเครื่อง"
  fi
}

check_and_prompt_legacy "XAMPP" "/Applications/XAMPP" "/Applications/XAMPP/xamppfiles/htdocs"
check_and_prompt_legacy "MAMP"  "/Applications/MAMP"  "/Applications/MAMP/htdocs"

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BIN_PATH="$SCRIPT_DIR/target/release/rusthost"
if [ ! -f "$BIN_PATH" ]; then
  BIN_PATH="$SCRIPT_DIR/target/debug/rusthost"
fi
ICNS_PATH="$SCRIPT_DIR/ui/AppIcon.icns"

rm -f "$HOME/Desktop/rusthost.command"

create_app_bundle() {
  local APP_DIR="$1"
  rm -rf "$APP_DIR"
  mkdir -p "$APP_DIR/Contents/MacOS"
  mkdir -p "$APP_DIR/Contents/Resources"
  cp "$BIN_PATH" "$APP_DIR/Contents/MacOS/rusthost"
  chmod +x "$APP_DIR/Contents/MacOS/rusthost"

  if [ -f "$ICNS_PATH" ]; then
    cp "$ICNS_PATH" "$APP_DIR/Contents/Resources/AppIcon.icns"
  fi

  cat <<EOF > "$APP_DIR/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>rusthost</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIdentifier</key>
    <string>dev.rusthost.app</string>
    <key>CFBundleName</key>
    <string>rusthost</string>
    <key>CFBundleDisplayName</key>
    <string>rusthost</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF
  xattr -cr "$APP_DIR" 2>/dev/null || true
  codesign --force --deep --sign - "$APP_DIR" 2>/dev/null || true
  touch "$APP_DIR"
}

create_app_bundle "$HOME/Desktop/rusthost.app"
create_app_bundle "/Applications/rusthost.app" 2>/dev/null || true

echo "✅ ติดตั้งแอปพลิเคชัน Native Mac ($HOME/Desktop/rusthost.app) พร้อมไอคอน AppIcon.icns สำเร็จ!"
