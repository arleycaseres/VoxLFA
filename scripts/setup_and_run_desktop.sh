#!/usr/bin/env bash
# setup_and_run_desktop.sh
# Prep and run VoxLFA desktop (Linux Debian/Ubuntu-based).
# Usage: sudo ./scripts/setup_and_run_desktop.sh  (will request sudo when needed)

set -u
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DESKTOP_DIR="$ROOT_DIR/desktop"

log(){ printf "[info] %s\n" "$1"; }
err(){ printf "[error] %s\n" "$1" >&2; }

# 1) Ensure we run from repo root
cd "$ROOT_DIR" || exit 1
log "Working directory: $ROOT_DIR"

# 2) Update apt and install system deps (ask sudo when needed)
read -r -p "Install/upgrade system packages (requires sudo)? [Y/n] " REPLY
REPLY=${REPLY:-Y}
if [[ "$REPLY" =~ ^[Yy] ]]; then
  sudo apt update || { err "apt update failed"; exit 1; }
  sudo apt --fix-broken install -y || true
  sudo apt install -y curl build-essential pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libasound2-dev libjack-jackd2-dev libssl-dev || true
  log "System packages installation attempted. If some packages failed, inspect output and retry manually."
fi

# 3) Ensure Rust toolchain (rustup)
if ! command -v cargo >/dev/null 2>&1; then
  log "Installing rustup + toolchain..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y || { err "rustup install failed"; exit 1; }
  # shell integration for this session
  . "$HOME/.cargo/env"
else
  log "Rust already installed: $(rustc --version 2>/dev/null || echo 'unknown')"
fi

# 4) Ensure Node.js (>=20)
NEEDED_NODE_MAJOR=20
if command -v node >/dev/null 2>&1; then
  NODE_VER=$(node -v | sed 's/^v//')
  NODE_MAJOR=$(echo "$NODE_VER" | cut -d. -f1)
else
  NODE_MAJOR=0
fi
if [[ "$NODE_MAJOR" -lt $NEEDED_NODE_MAJOR ]]; then
  log "Installing Node.js ${NEEDED_NODE_MAJOR} (NodeSource)..."
  curl -fsSL https://deb.nodesource.com/setup_${NEEDED_NODE_MAJOR}.x | sudo -E bash - || { err "nodesource setup failed"; }
  sudo apt-get install -y nodejs || { err "nodejs install failed"; }
else
  log "Node.js present: $(node -v)"
fi

# 5) Build Rust workspace (try default, fallback to --no-default-features)
log "Building Rust workspace (core)..."
if ! cargo build --workspace; then
  err "cargo build failed; attempting without default features (--no-default-features)"
  if ! cargo build --workspace --no-default-features; then
    err "cargo build still failing. Inspect linker errors (missing system libs like libjack/libssl)."
    exit 1
  fi
fi

# 6) Frontend: install deps and typecheck
if [[ -d "$DESKTOP_DIR" ]]; then
  cd "$DESKTOP_DIR" || exit 1
  if ! command -v npm >/dev/null 2>&1; then
    err "npm not found; ensure Node.js installation succeeded"
  else
    log "Installing desktop/frontend dependencies (npm)..."
    npm install --no-audit --no-fund || { err "npm install failed"; }
    log "Running TypeScript check (npx tsc --noEmit)..."
    if command -v npx >/dev/null 2>&1; then
      npx tsc --noEmit || log "TypeScript reported errors (non-fatal here)."
    fi
  fi
else
  err "Desktop folder not found: $DESKTOP_DIR"
fi

# 7) Run the app: quick UI (Vite) and option to run Tauri dev
echo
log "Now you can run the UI quickly (Vite) or the full desktop (Tauri)."
cat <<'EOF'
Recommended next steps (choose one):

# Quick UI (no native audio):
cd desktop
npm run dev -- --port 1421
# open http://localhost:1421 in browser

# Full desktop (native window + audio):
cd desktop
npm run tauri dev
EOF

log "Script finished. Follow the recommended next steps above."
exit 0
