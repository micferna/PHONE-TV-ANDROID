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
        # Check if PID is alive AND is actually our backup script
        if [ -n "$lock_pid" ] && kill -0 "$lock_pid" 2>/dev/null; then
            local cmdline
            cmdline=$(cat "/proc/$lock_pid/cmdline" 2>/dev/null | tr '\0' ' ' || echo "")
            if [[ "$cmdline" == *"phone-backup"* ]]; then
                log "SKIP: backup already running (PID $lock_pid)"
                exit 0
            fi
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
    log "Exporting SMS..."

    local json_file="$EXPORTS_DIR/sms_${TODAY}.json"
    local csv_file="$EXPORTS_DIR/sms_${TODAY}.csv"

    # Use projection for clean field separation, pipe through a python one-liner
    # that reliably parses the content query output
    adb -s "$DEVICE_SERIAL" shell content query --uri content://sms --projection _id:address:body:date:type:read 2>/dev/null | \
    python3 -c "
import sys, json, re
from datetime import datetime

msgs = []
for line in sys.stdin:
    line = line.strip()
    if not line.startswith('Row:'):
        continue
    # Parse key=value pairs — body can contain commas so we parse carefully
    m = {}
    # Extract simple numeric fields first
    for key in ('_id', 'date', 'type', 'read'):
        match = re.search(key + r'=(\d+)', line)
        if match:
            m[key] = match.group(1)
    # Extract address
    match = re.search(r'address=([^,]+)', line)
    if match:
        m['address'] = match.group(1).strip()
    # Extract body: everything between 'body=' and ', date=' (or ', type=')
    match = re.search(r'body=(.*?)(?:, date=|, type=|, read=)', line)
    if match:
        m['body'] = match.group(1).strip()
    else:
        m['body'] = ''

    date_ms = int(m.get('date', 0))
    try:
        date_str = datetime.fromtimestamp(date_ms / 1000).strftime('%Y-%m-%d %H:%M:%S')
    except:
        date_str = str(date_ms)

    type_map = {'1': 'received', '2': 'sent', '3': 'draft', '4': 'outbox'}
    msgs.append({
        'date': date_str,
        'date_epoch_ms': date_ms,
        'address': m.get('address', ''),
        'body': m.get('body', ''),
        'type': type_map.get(m.get('type', ''), 'unknown'),
        'read': int(m.get('read', 0))
    })

json.dump(msgs, open('$json_file', 'w'), ensure_ascii=False, indent=1)
print(len(msgs))
" 2>/dev/null
    local count
    count=$(jq length "$json_file" 2>/dev/null || echo 0)

    # CSV
    echo "date,address,body,type,read" > "$csv_file"
    jq -r '.[] | [.date, .address, (.body | gsub(","; " ") | gsub("\n"; " ")), .type, .read] | @csv' "$json_file" >> "$csv_file" 2>/dev/null

    log "  SMS exported: $count messages"
}

# ── Apps Export ─────────────────────────────────────────────────────
export_apps() {
    log "Exporting installed apps..."

    local json_file="$EXPORTS_DIR/apps_${TODAY}.json"

    adb -s "$DEVICE_SERIAL" shell pm list packages -3 -f 2>/dev/null | \
    python3 -c "
import sys, json, subprocess

apps = []
for line in sys.stdin:
    line = line.strip()
    if not line.startswith('package:'):
        continue
    # format: package:/path/to.apk=com.package.name
    rest = line[8:]  # strip 'package:'
    if '=' in rest:
        path, pkg = rest.rsplit('=', 1)
    else:
        pkg = rest
        path = ''
    apps.append({'package': pkg.strip(), 'path': path.strip()})

json.dump(sorted(apps, key=lambda a: a['package']), open('$json_file', 'w'), ensure_ascii=False, indent=1)
print(len(apps))
" 2>/dev/null

    local count
    count=$(jq length "$json_file" 2>/dev/null || echo 0)
    log "  Apps exported: $count packages"
}

# ── Device Info Export ──────────────────────────────────────────────
export_device_info() {
    log "Exporting device info..."

    local json_file="$EXPORTS_DIR/device_info_${TODAY}.json"

    python3 -c "
import subprocess, json

def adb_prop(prop):
    try:
        r = subprocess.run(['adb', '-s', '$DEVICE_SERIAL', 'shell', 'getprop', prop],
                           capture_output=True, text=True, timeout=5)
        return r.stdout.strip()
    except:
        return ''

def adb_cmd(args):
    try:
        r = subprocess.run(['adb', '-s', '$DEVICE_SERIAL'] + args,
                           capture_output=True, text=True, timeout=10)
        return r.stdout.strip()
    except:
        return ''

# Battery
battery = {}
for line in adb_cmd(['shell', 'dumpsys', 'battery']).splitlines():
    line = line.strip()
    if ':' in line:
        k, v = line.split(':', 1)
        battery[k.strip().lower()] = v.strip()

# Storage
storage = {}
df_out = adb_cmd(['shell', 'df', '/data'])
for line in df_out.splitlines()[1:]:
    parts = line.split()
    if len(parts) >= 4:
        storage = {'total': parts[1], 'used': parts[2], 'available': parts[3], 'percent': parts[4] if len(parts)>4 else ''}

info = {
    'model': adb_prop('ro.product.model'),
    'brand': adb_prop('ro.product.brand'),
    'android_version': adb_prop('ro.build.version.release'),
    'sdk': adb_prop('ro.build.version.sdk'),
    'serial': '$DEVICE_SERIAL',
    'battery_level': battery.get('level', ''),
    'battery_status': battery.get('status', ''),
    'storage': storage,
    'security_patch': adb_prop('ro.build.version.security_patch'),
    'build_date': adb_prop('ro.build.date'),
}

json.dump(info, open('$json_file', 'w'), ensure_ascii=False, indent=2)
print('OK')
" 2>/dev/null

    log "  Device info exported"
}

# ── Contacts Export ─────────────────────────────────────────────────
export_contacts() {
    log "Exporting contacts..."

    local raw
    raw=$(adb -s "$DEVICE_SERIAL" shell content query --uri content://contacts/phones --projection display_name:number:type 2>/dev/null || echo "")

    if [ -z "$raw" ]; then
        log "  No contacts data or access denied"
        return
    fi

    local json_file="$EXPORTS_DIR/contacts_${TODAY}.json"
    local csv_file="$EXPORTS_DIR/contacts_${TODAY}.csv"
    local count=0

    echo "[" > "$json_file"
    local first=true

    while IFS= read -r line; do
        if [[ "$line" != Row:* ]]; then
            continue
        fi

        local display_name="" number="" type_val=""

        if [[ "$line" =~ display_name=([^,]+) ]]; then
            display_name="${BASH_REMATCH[1]}"
            display_name="${display_name## }"
        fi
        if [[ "$line" =~ number=([^,]+) ]]; then
            number="${BASH_REMATCH[1]}"
            number="${number## }"
        fi
        if [[ "$line" =~ type=([0-9]+) ]]; then
            type_val="${BASH_REMATCH[1]}"
        fi

        local type_label="other"
        case "$type_val" in
            1) type_label="home" ;;
            2) type_label="mobile" ;;
            3) type_label="work" ;;
        esac

        local name_escaped
        name_escaped=$(printf '%s' "$display_name" | jq -Rs '.' 2>/dev/null || echo "\"\"")

        if [ "$first" = true ]; then
            first=false
        else
            echo "," >> "$json_file"
        fi

        cat >> "$json_file" <<JSONEOF
  {"display_name": $name_escaped, "number": "$number", "type": "$type_label"}
JSONEOF
        count=$(( count + 1 ))
    done <<< "$raw"

    echo "]" >> "$json_file"

    echo "display_name,number,type" > "$csv_file"
    jq -r '.[] | [.display_name, .number, .type] | @csv' "$json_file" >> "$csv_file" 2>/dev/null

    log "  Contacts exported: $count entries"
}

# ── Call Log Export ─────────────────────────────────────────────────
export_call_log() {
    log "Exporting call log..."

    local raw
    raw=$(adb -s "$DEVICE_SERIAL" shell content query --uri content://call_log/calls --projection number:name:date:duration:type 2>/dev/null || echo "")

    if [ -z "$raw" ]; then
        log "  No call log data or access denied"
        return
    fi

    local json_file="$EXPORTS_DIR/call_log_${TODAY}.json"
    local csv_file="$EXPORTS_DIR/call_log_${TODAY}.csv"
    local count=0

    echo "[" > "$json_file"
    local first=true

    while IFS= read -r line; do
        if [[ "$line" != Row:* ]]; then
            continue
        fi

        local number="" name="" date_val="" duration="" type_val=""

        if [[ "$line" =~ number=([^,]+) ]]; then
            number="${BASH_REMATCH[1]}"
            number="${number## }"
        fi
        if [[ "$line" =~ name=([^,]+) ]]; then
            name="${BASH_REMATCH[1]}"
            name="${name## }"
            [ "$name" = "NULL" ] && name=""
        fi
        if [[ "$line" =~ date=([0-9]+) ]]; then
            date_val="${BASH_REMATCH[1]}"
        fi
        if [[ "$line" =~ duration=([0-9]+) ]]; then
            duration="${BASH_REMATCH[1]}"
        fi
        if [[ "$line" =~ type=([0-9]+) ]]; then
            type_val="${BASH_REMATCH[1]}"
        fi

        local date_human=""
        if [ -n "$date_val" ]; then
            date_human=$(date -d "@$(( date_val / 1000 ))" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || echo "$date_val")
        fi

        local type_label="unknown"
        case "$type_val" in
            1) type_label="incoming" ;;
            2) type_label="outgoing" ;;
            3) type_label="missed" ;;
            4) type_label="voicemail" ;;
            5) type_label="rejected" ;;
            6) type_label="blocked" ;;
        esac

        local name_escaped
        name_escaped=$(printf '%s' "$name" | jq -Rs '.' 2>/dev/null || echo "\"\"")

        if [ "$first" = true ]; then
            first=false
        else
            echo "," >> "$json_file"
        fi

        cat >> "$json_file" <<JSONEOF
  {"number": "$number", "name": $name_escaped, "date": "$date_human", "date_epoch_ms": ${date_val:-0}, "duration_sec": ${duration:-0}, "type": "$type_label"}
JSONEOF
        count=$(( count + 1 ))
    done <<< "$raw"

    echo "]" >> "$json_file"

    echo "number,name,date,duration_sec,type" > "$csv_file"
    jq -r '.[] | [.number, .name, .date, .duration_sec, .type] | @csv' "$json_file" >> "$csv_file" 2>/dev/null

    log "  Call log exported: $count entries"
}

# ── Weekly Archive ──────────────────────────────────────────────────
maybe_archive() {
    local recent_archive
    recent_archive=$(find "$ARCHIVES_DIR" -name "*.tar.zst" -mtime -7 -print -quit 2>/dev/null || echo "")

    if [ -n "$recent_archive" ]; then
        log "Archive skipped (recent: $(basename "$recent_archive"))"
        return
    fi

    local archive_file="$ARCHIVES_DIR/${TODAY}_full.tar.zst"
    log "Creating weekly archive: $archive_file"

    tar -cf - -C "$BACKUP_ROOT" latest exports 2>/dev/null | zstd -3 -T0 -o "$archive_file" 2>/dev/null

    if [ -f "$archive_file" ]; then
        local size
        size=$(du -h "$archive_file" | cut -f1)
        log "  Archive created: $size"
    else
        log "  ERROR: archive creation failed"
        errors=$(( errors + 1 ))
    fi
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
    export_apps
    export_device_info
    maybe_archive

    log "DONE: $TODAY — files_copied=$files_copied errors=$errors"
}

main "$@"
