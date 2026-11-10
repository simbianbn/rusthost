#!/usr/bin/env bash
set -e
export PATH="/opt/homebrew/bin:$PATH"
cd "$(dirname "$0")"

if ! gh auth status >/dev/null 2>&1; then
  echo "==> กรุณายืนยันสิทธิ์ GitHub CLI (คัดลอกรหัส One-time code ไปวางในเบราว์เซอร์)..."
  gh auth login --hostname github.com --git-protocol https --web -s repo,project
fi

gh auth setup-git

if ! git remote get-url origin >/dev/null 2>&1; then
  gh repo create simbianbn/rusthost \
    --public \
    --description "Minimal Native Desktop Local Development Stack Manager in Rust" \
    --source=. \
    --remote=origin \
    --push
else
  git push -u origin main
fi

gh project link 1 --owner simbianbn --repo simbianbn/rusthost || true
echo "✅ อัปโหลดโค้ดขึ้น https://github.com/simbianbn/rusthost และผูกกับ Project #1 เรียบร้อยแล้ว!"
