#!/usr/bin/env bash
# Naurix: one command to check, run, build and install.
#
#   ./run.sh           run in development mode (default)
#   ./run.sh app       build the app and open it
#   ./run.sh install   build the app and copy it to ~/Applications
#   ./run.sh dmg       build a Naurix .dmg installer (target/release/bundle/dmg)
#   ./run.sh sandbox   run a second copy with throwaway data (never touches your workspaces)
#   ./run.sh test      run all tests and lint checks
#   ./run.sh doctor    check that the tools you need are installed
#   ./run.sh help
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

APP_NAME="Naurix"
APP_PATH="target/release/bundle/macos/${APP_NAME}.app"

red()   { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }
bold()  { printf '\033[1m%s\033[0m\n' "$*"; }

# Rust installed by rustup lives in ~/.cargo/bin; make it visible even in a fresh shell.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

doctor() {
  local ok=0
  check() { # name, command-to-test, fix hint
    if eval "$2" >/dev/null 2>&1; then green "  ✓ $1"; else red "  ✗ $1"; echo "      fix: $3"; ok=1; fi
  }
  bold "Checking your setup…"
  [ "$(uname -s)" = "Darwin" ] || { red "  ✗ macOS is required"; ok=1; }
  check "Xcode command line tools" "xcode-select -p" "xcode-select --install"
  check "Rust (cargo)" "command -v cargo" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   (then open a new terminal)"
  check "Node.js 22 or newer" "node -e 'process.exit(+process.versions.node.split(\".\")[0] >= 22 ? 0 : 1)'" "brew install node"
  check "pnpm" "command -v pnpm" "npm install -g pnpm   (or: corepack enable)"
  [ "$ok" -eq 0 ] && green "All good. Architecture: $(uname -m)" || { red "Fix the items above, then run ./run.sh again."; return 1; }
}

deps() {
  if [ ! -d node_modules ] || [ pnpm-lock.yaml -nt node_modules/.modules.yaml ]; then
    bold "Installing JavaScript dependencies…"
    pnpm install --frozen-lockfile
  fi
}

build_app() {
  deps
  bold "Building ${APP_NAME}.app (the first build takes a few minutes)…"
  pnpm tauri build --bundles app
  [ -d "$APP_PATH" ] || { red "Build finished but $APP_PATH was not found."; exit 1; }
}

cmd="${1:-dev}"
case "$cmd" in
  dev)
    doctor
    deps
    bold "Starting Naurix in development mode (the first run compiles Rust; later runs are fast)…"
    exec pnpm tauri dev
    ;;
  app)
    doctor
    build_app
    green "Built: $APP_PATH"
    open "$APP_PATH"
    ;;
  install)
    doctor
    build_app
    dest="${NAURIX_INSTALL_DIR:-$HOME/Applications}"
    mkdir -p "$dest"
    # Replace only a previous copy of this app, never anything else.
    rm -rf "${dest:?}/${APP_NAME}.app"
    cp -R "$APP_PATH" "$dest/"
    green "Installed to $dest/${APP_NAME}.app"
    echo "Open it from Finder/Spotlight, or: open \"$dest/${APP_NAME}.app\""
    echo "It is unsigned. If macOS blocks it, right-click the app and choose Open."
    ;;
  dmg)
    doctor
    deps
    bold "Building the .dmg installer (the first build takes a few minutes)…"
    # The pretty window layout is set through Finder (AppleScript). If that is blocked or
    # times out (no Automation permission, headless session), retry without it (CI=true).
    pnpm tauri build --bundles dmg || {
      red "Finder styling failed; retrying a plain .dmg (no custom window layout)…"
      CI=true pnpm tauri build --bundles dmg
    }
    dmg="$(ls -t target/release/bundle/dmg/*.dmg 2>/dev/null | head -n1 || true)"
    [ -n "$dmg" ] || { red "Build finished but no .dmg was found in target/release/bundle/dmg."; exit 1; }
    green "Built: $dmg"
    open -R "$dmg"
    ;;
  sandbox)
    doctor
    deps
    # Separate data folder and dev-server port, so it can run next to your normal copy.
    data="${NAURIX_DATA_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/naurix-sandbox.XXXXXX")}"
    port="${NAURIX_DEV_PORT:-1520}"
    bold "Sandbox: data in $data, dev server on port $port. Your real workspaces are not used."
    export NAURIX_DATA_DIR="$data" NAURIX_DEV_PORT="$port"
    exec pnpm tauri dev --config "{\"build\":{\"devUrl\":\"http://localhost:${port}\"}}"
    ;;
  test)
    doctor
    deps
    pnpm typecheck
    pnpm test
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    green "All checks passed."
    ;;
  doctor)
    doctor
    ;;
  help|-h|--help)
    sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'
    ;;
  *)
    red "Unknown command: $cmd"
    sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
    ;;
esac
