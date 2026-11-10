#!/bin/zsh
# ==============================================================================
# rusthost — macOS Uninstaller
# ==============================================================================
pkill -f "rusthost" 2>/dev/null || true
rm -f "$HOME/Desktop/rusthost.command"
rm -rf "$HOME/Library/Application Support/rusthost"
echo "✅ ถอนการติดตั้ง rusthost ออกจากระบบเรียบร้อยแล้ว"
