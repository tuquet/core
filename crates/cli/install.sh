#!/bin/sh
# ==============================================================================
# Operon CLI - Universal Installer (macOS & Linux)
# ==============================================================================
# Usage:
#   curl -fsSL https://github.com/tuquet/operon/releases/latest/download/install.sh | sh
#   ... | sh -s -- --no-daemon
# ==============================================================================

set -e

NO_DAEMON=0
for arg in "$@"; do
    case "$arg" in
        --no-daemon) NO_DAEMON=1 ;;
    esac
done

REPO="${OPERON_REPO:-tuquet/operon}"
BASE_URL="https://github.com/${REPO}/releases/latest/download"
OPERON_DIR="${OPERON_HOME:-${HOME}/.operon}"
BIN_DIR="${OPERON_DIR}/bin"
TARGET_EXE="${BIN_DIR}/operon"
ALIAS_EXE="${BIN_DIR}/opn"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin) PLATFORM="macos" ;;
    Linux)  PLATFORM="linux" ;;
    *)
        echo "[operon] ERROR: Unsupported OS: $OS" >&2
        exit 1
        ;;
esac

case "$ARCH" in
    x86_64|amd64)  TARGET_ARCH="x64" ;;
    arm64|aarch64) TARGET_ARCH="arm64" ;;
    *)
        echo "[operon] ERROR: Unsupported CPU architecture: $ARCH" >&2
        exit 1
        ;;
esac

ASSET_NAME="operon-${PLATFORM}-${TARGET_ARCH}"
echo "[operon] Installing Operon CLI (${ASSET_NAME}) to ${BIN_DIR}..."
mkdir -p "${BIN_DIR}"

TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'operon')"
trap 'rm -rf "${TMP_DIR}"' EXIT

fetch_file() {
    url="$1"
    out="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$out"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$url" -O "$out"
    else
        echo "[operon] ERROR: curl or wget is required." >&2
        exit 1
    fi
}

fetch_file "${BASE_URL}/${ASSET_NAME}" "${TMP_DIR}/${ASSET_NAME}"

if fetch_file "${BASE_URL}/SHA256SUMS.txt" "${TMP_DIR}/SHA256SUMS.txt" 2>/dev/null; then
    EXPECTED_HASH="$(grep -E "[[:space:]]\*?${ASSET_NAME}$" "${TMP_DIR}/SHA256SUMS.txt" | awk '{print $1}' | head -n 1)"
    if [ -z "$EXPECTED_HASH" ]; then
        echo "[operon] ERROR: SHA256 entry missing for ${ASSET_NAME}." >&2
        exit 1
    fi
    if command -v sha256sum >/dev/null 2>&1; then
        ACTUAL_HASH="$(sha256sum "${TMP_DIR}/${ASSET_NAME}" | awk '{print $1}')"
    elif command -v shasum >/dev/null 2>&1; then
        ACTUAL_HASH="$(shasum -a 256 "${TMP_DIR}/${ASSET_NAME}" | awk '{print $1}')"
    else
        ACTUAL_HASH=""
    fi
    if [ -n "$ACTUAL_HASH" ] && [ "$ACTUAL_HASH" != "$EXPECTED_HASH" ]; then
        echo "[operon] ERROR: SHA256 mismatch (expected $EXPECTED_HASH, got $ACTUAL_HASH)" >&2
        exit 1
    fi
    [ -n "$ACTUAL_HASH" ] && echo "[operon] SHA256 verified."
fi

for candidate in "${TARGET_EXE}" "${ALIAS_EXE}"; do
    if [ -x "${candidate}" ]; then
        "${candidate}" daemon stop --force >/dev/null 2>&1 || true
    fi
done

mv -f "${TMP_DIR}/${ASSET_NAME}" "${TARGET_EXE}"
chmod +x "${TARGET_EXE}"
ln -sf "operon" "${ALIAS_EXE}"
echo "[operon] Installed binary: ${TARGET_EXE} (alias: opn)"

# Configure PATH idempotently
PATH_LINE='export PATH="$HOME/.operon/bin:$PATH"'
for PROFILE in "${HOME}/.bashrc" "${HOME}/.zshrc" "${HOME}/.profile"; do
    if [ -f "$PROFILE" ] && ! grep -q '\.operon/bin' "$PROFILE" 2>/dev/null; then
        printf "\n# Operon CLI\n%s\n" "${PATH_LINE}" >> "$PROFILE"
    fi
done

if [ "$NO_DAEMON" -eq 0 ]; then
    if [ "$PLATFORM" = "macos" ]; then
        PLIST_DIR="${HOME}/Library/LaunchAgents"
        PLIST_PATH="${PLIST_DIR}/com.operon.daemon.plist"
        rm -f "${PLIST_DIR}/com.operon.local-bridge.plist" # legacy plist cleanup
        mkdir -p "${PLIST_DIR}" "${OPERON_DIR}/logs"
        cat > "${PLIST_PATH}" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.operon.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>${TARGET_EXE}</string>
        <string>daemon</string>
        <string>start</string>
        <string>-H</string><string>127.0.0.1</string>
        <string>-p</string><string>8765</string>
    </array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
    <key>StandardOutPath</key><string>${OPERON_DIR}/logs/daemon.stdout.log</string>
    <key>StandardErrorPath</key><string>${OPERON_DIR}/logs/daemon.stderr.log</string>
</dict>
</plist>
EOF
        launchctl unload "${PLIST_PATH}" >/dev/null 2>&1 || true
        launchctl load "${PLIST_PATH}" >/dev/null 2>&1 || "${TARGET_EXE}" daemon start -H 127.0.0.1 -p 8765 -d
    elif command -v systemctl >/dev/null 2>&1 && systemctl --user status >/dev/null 2>&1; then
        UNIT_DIR="${HOME}/.config/systemd/user"
        rm -f "${UNIT_DIR}/operon-bridge.service"
        mkdir -p "${UNIT_DIR}"
        cat > "${UNIT_DIR}/operon-daemon.service" <<EOF
[Unit]
Description=Operon Local Daemon
After=network.target

[Service]
ExecStart=${TARGET_EXE} daemon start -H 127.0.0.1 -p 8765
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
EOF
        systemctl --user daemon-reload
        systemctl --user enable --now operon-daemon.service >/dev/null 2>&1 || "${TARGET_EXE}" daemon start -H 127.0.0.1 -p 8765 -d
    else
        "${TARGET_EXE}" daemon start -H 127.0.0.1 -p 8765 -d
    fi
    echo "[operon] Daemon started at http://127.0.0.1:8765"
fi

echo "[operon] Ready: $("${TARGET_EXE}" --version 2>/dev/null || echo operon) (run 'opn desk' to open host shell)"
