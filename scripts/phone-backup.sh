#!/usr/bin/env bash
set -euo pipefail

# ── Config ──────────────────────────────────────────────────────────
DEVICE_SERIAL="ZY22JVMJWL"
BACKUP_ROOT="$HOME/Backups/Phone"
LATEST_DIR="$BACKUP_ROOT/latest"
EXPORTS_DIR="$BACKUP_ROOT/exports"
ARCHIVES_DIR="$BACKUP_ROOT/archives"
LOG_FILE="$BACKUP_ROOT/backup.log"
LOCK_FILE="/tmp/phone-backup.lock"
COOLDOWN_SECONDS=3600
ADB_WAIT_SECONDS=15
TODAY=$(date +%Y-%m-%d)

# Directories to sync from phone
PHONE_DIRS=(
    "DCIM"
    "Pictures"
    "Movies"
    "Downloads"
    "Music"
    "Android/media/com.whatsapp"
    "Android/data/com.snapchat.android"
)

# Counters
files_copied=0
errors=0

# ── Logging ─────────────────────────────────────────────────────────
log() {
    local msg="[$(date '+%Y-%m-%d %H:%M:%S')] $1"
    echo "$msg" >> "$LOG_FILE"
}

# ── Guards ──────────────────────────────────────────────────────────
acquire_lock() {
    if [ -f "$LOCK_FILE" ]; then
        local lock_pid
        lock_pid=$(cat "$LOCK_FILE" 2>/dev/null || echo "")
        if [ -n "$lock_pid" ] && kill -0 "$lock_pid" 2>/dev/null; then
            log "SKIP: backup already running (PID $lock_pid)"
            exit 0
        fi
        rm -f "$LOCK_FILE"
    fi
    echo $$ > "$LOCK_FILE"
    trap 'rm -f "$LOCK_FILE"' EXIT
}

check_cooldown() {
    if [ -f "$LOG_FILE" ]; then
        local last_success
        last_success=$(grep "DONE:" "$LOG_FILE" | tail -1 | grep -oP '^\[\K[0-9-]+ [0-9:]+' || echo "")
        if [ -n "$last_success" ]; then
            local last_ts
            last_ts=$(date -d "$last_success" +%s 2>/dev/null || echo 0)
            local now_ts
            now_ts=$(date +%s)
            if (( now_ts - last_ts < COOLDOWN_SECONDS )); then
                log "SKIP: cooldown active (last backup $(( (now_ts - last_ts) / 60 ))m ago)"
                exit 0
            fi
        fi
    fi
}

wait_for_device() {
    local waited=0
    while (( waited < ADB_WAIT_SECONDS )); do
        if adb -s "$DEVICE_SERIAL" get-state 2>/dev/null | grep -q "device"; then
            return 0
        fi
        sleep 1
        (( waited++ ))
    done
    log "ERROR: device $DEVICE_SERIAL not ready after ${ADB_WAIT_SECONDS}s"
    exit 1
}

# ── Init ────────────────────────────────────────────────────────────
init_dirs() {
    mkdir -p "$LATEST_DIR" "$EXPORTS_DIR" "$ARCHIVES_DIR"
}

# ── File Sync ───────────────────────────────────────────────────────
sync_files() {
    log "sync_files: not implemented"
}

# ── SMS Export ──────────────────────────────────────────────────────
export_sms() {
    log "export_sms: not implemented"
}

# ── Contacts Export ─────────────────────────────────────────────────
export_contacts() {
    log "export_contacts: not implemented"
}

# ── Call Log Export ─────────────────────────────────────────────────
export_call_log() {
    log "export_call_log: not implemented"
}

# ── Weekly Archive ──────────────────────────────────────────────────
maybe_archive() {
    log "maybe_archive: not implemented"
}

# ── Main ────────────────────────────────────────────────────────────
main() {
    init_dirs
    acquire_lock
    check_cooldown
    wait_for_device

    log "START: backup for $DEVICE_SERIAL"

    sync_files
    export_sms
    export_contacts
    export_call_log
    maybe_archive

    log "DONE: $TODAY — files_copied=$files_copied errors=$errors"
}

main "$@"
