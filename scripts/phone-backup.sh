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
    log "Syncing files..."
    files_copied=0

    for phone_dir in "${PHONE_DIRS[@]}"; do
        local remote="/sdcard/$phone_dir"
        local local_dir="$LATEST_DIR/$phone_dir"
        mkdir -p "$local_dir"

        # Check if remote dir exists
        if ! adb -s "$DEVICE_SERIAL" shell "[ -d '$remote' ] && echo ok" 2>/dev/null | grep -q ok; then
            log "  SKIP: $remote does not exist on device"
            continue
        fi

        log "  Scanning $remote ..."

        # Get remote file list with sizes in one shot
        local remote_list
        remote_list=$(adb -s "$DEVICE_SERIAL" shell "find '$remote' -type f -exec stat -c '%s %n' {} +" 2>/dev/null || echo "")

        if [ -z "$remote_list" ]; then
            log "  No files in $remote"
            continue
        fi

        local dir_copied=0

        while IFS= read -r line; do
            # Parse "size /sdcard/DCIM/path/file.jpg"
            local rsize="${line%% *}"
            local rpath="${line#* }"
            # Strip trailing \r from adb output
            rpath="${rpath%$'\r'}"
            rsize="${rsize%$'\r'}"

            # Convert remote absolute path to local relative path
            local relpath="${rpath#/sdcard/}"
            local lpath="$LATEST_DIR/$relpath"

            # Check if local file exists and has same size
            if [ -f "$lpath" ]; then
                local lsize
                lsize=$(stat -c '%s' "$lpath" 2>/dev/null || echo 0)
                if [ "$lsize" = "$rsize" ]; then
                    continue
                fi
            fi

            # Pull the file
            local ldir
            ldir=$(dirname "$lpath")
            mkdir -p "$ldir"

            if adb -s "$DEVICE_SERIAL" pull "$rpath" "$lpath" > /dev/null 2>&1; then
                dir_copied=$(( dir_copied + 1 ))
                files_copied=$(( files_copied + 1 ))
            else
                log "  ERROR pulling $rpath"
                errors=$(( errors + 1 ))
            fi
        done <<< "$remote_list"

        log "  $phone_dir: $dir_copied new/updated files"
    done

    log "File sync complete: $files_copied files copied"
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
