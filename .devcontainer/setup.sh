#!/usr/bin/env bash
set -euo pipefail

# SQLite
sudo apt-get update
sudo apt-get install -y sqlite3

# Node.js / npm (via NodeSource LTS)
if ! command -v node &>/dev/null; then
    curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
    sudo apt-get install -y nodejs
fi

# Cursor CLI (idempotent — skip if already installed)
if ! command -v cursor &>/dev/null; then
    curl https://cursor.com/install -fsS | bash
fi
