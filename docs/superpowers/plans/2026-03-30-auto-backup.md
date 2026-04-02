# Auto-backup incrémental Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automatically backup phone media, SMS, contacts, and call logs when the Moto G14 is plugged in via USB, using incremental sync to avoid duplicates.

**Architecture:** A standalone bash script handles all backup logic (incremental file sync via `adb pull`, content provider exports for SMS/contacts/calls, weekly archive compression). A udev rule triggers the script on USB plug-in. Lock file + cooldown prevent duplicate runs.

**Tech Stack:** Bash, ADB, jq, zstd, udev, systemd-run

---

### Task 1: Create backup directory structure and script skeleton

**Files:**
- Create: `scripts/phone-backup.sh`

- [ ] **Step 1: Create scripts directory and skeleton script**

```bash
mkdir -p scripts
```

Write `scripts/phone-backup.sh`:

```bash
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

# ── Placeholders for subsequent tasks ───────────────────────────────
sync_files() {
    log "sync_files: not implemented"
}

export_sms() {
    log "export_sms: not implemented"
}

export_contacts() {
    log "export_contacts: not implemented"
}

export_call_log() {
    log "export_call_log: not implemented"
}

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
    local files_copied=0
    local errors=0

    sync_files
    export_sms
    export_contacts
    export_call_log
    maybe_archive

    log "DONE: $TODAY — files_copied=$files_copied errors=$errors"
}

main "$@"
```

- [ ] **Step 2: Make executable and test skeleton runs**

Run:
```bash
chmod +x scripts/phone-backup.sh
```

Test the skeleton (should create dirs, check lock, check cooldown, wait for device, then log "not implemented" for each function):
```bash
bash scripts/phone-backup.sh
cat ~/Backups/Phone/backup.log
```

Expected: log shows START, each "not implemented" line, and DONE.

- [ ] **Step 3: Commit**

```bash
git add scripts/phone-backup.sh
git commit -m "feat: backup script skeleton with guards and logging"
```

---

### Task 2: Implement incremental file sync

**Files:**
- Modify: `scripts/phone-backup.sh` (replace `sync_files` function)

- [ ] **Step 1: Replace sync_files with incremental pull logic**

Replace the `sync_files` placeholder in `scripts/phone-backup.sh` with:

```bash
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
        # Format: "size path" per line
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
                (( dir_copied++ ))
                (( files_copied++ ))
            else
                log "  ERROR pulling $rpath"
                (( errors++ ))
            fi
        done <<< "$remote_list"

        log "  $phone_dir: $dir_copied new/updated files"
    done

    log "File sync complete: $files_copied files copied"
}
```

- [ ] **Step 2: Test incremental sync**

Run:
```bash
bash scripts/phone-backup.sh
```

Expected: log shows files being scanned and copied for existing directories. Run a second time — should copy 0 files (all already synced).

```bash
grep "files copied" ~/Backups/Phone/backup.log | tail -2
```

First run: `files_copied=N` (some number). Second run: `files_copied=0`.

- [ ] **Step 3: Commit**

```bash
git add scripts/phone-backup.sh
git commit -m "feat: incremental file sync via adb pull"
```

---

### Task 3: Implement SMS export

**Files:**
- Modify: `scripts/phone-backup.sh` (replace `export_sms` function)

- [ ] **Step 1: Replace export_sms with content provider parser**

Replace the `export_sms` placeholder in `scripts/phone-backup.sh` with:

```bash
export_sms() {
    log "Exporting SMS..."

    local raw
    raw=$(adb -s "$DEVICE_SERIAL" shell content query --uri content://sms 2>/dev/null || echo "")

    if [ -z "$raw" ]; then
        log "  No SMS data or access denied"
        return
    fi

    local json_file="$EXPORTS_DIR/sms_${TODAY}.json"
    local csv_file="$EXPORTS_DIR/sms_${TODAY}.csv"
    local count=0

    # Parse ADB content query output into JSON
    # Each row starts with "Row: N "
    echo "[" > "$json_file"
    local first=true

    while IFS= read -r line; do
        if [[ "$line" != Row:* ]]; then
            continue
        fi

        local date_val="" address="" body="" type_val="" read_val=""

        # Extract fields using parameter expansion
        if [[ "$line" =~ date=([0-9]+) ]]; then
            date_val="${BASH_REMATCH[1]}"
        fi
        if [[ "$line" =~ address=([^,]+) ]]; then
            address="${BASH_REMATCH[1]}"
            address="${address## }"
        fi
        if [[ "$line" =~ body=([^,]*)(, read=|, type=|$) ]]; then
            body="${BASH_REMATCH[1]}"
        fi
        if [[ "$line" =~ type=([0-9]+) ]]; then
            type_val="${BASH_REMATCH[1]}"
        fi
        if [[ "$line" =~ read=([0-9]+) ]]; then
            read_val="${BASH_REMATCH[1]}"
        fi

        # Convert epoch ms to human-readable
        local date_human=""
        if [ -n "$date_val" ]; then
            date_human=$(date -d "@$(( date_val / 1000 ))" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || echo "$date_val")
        fi

        # Type labels
        local type_label="unknown"
        case "$type_val" in
            1) type_label="received" ;;
            2) type_label="sent" ;;
            3) type_label="draft" ;;
            4) type_label="outbox" ;;
        esac

        # Escape JSON special chars in body
        local body_escaped
        body_escaped=$(printf '%s' "$body" | jq -Rs '.' 2>/dev/null || echo "\"$body\"")

        if [ "$first" = true ]; then
            first=false
        else
            echo "," >> "$json_file"
        fi

        cat >> "$json_file" <<JSONEOF
  {"date": "$date_human", "date_epoch_ms": $date_val, "address": "$address", "body": $body_escaped, "type": "$type_label", "read": $read_val}
JSONEOF
        (( count++ ))
    done <<< "$raw"

    echo "]" >> "$json_file"

    # Generate CSV
    echo "date,address,body,type,read" > "$csv_file"
    jq -r '.[] | [.date, .address, (.body | gsub(","; " ") | gsub("\n"; " ")), .type, .read] | @csv' "$json_file" >> "$csv_file" 2>/dev/null

    log "  SMS exported: $count messages → $json_file + $csv_file"
}
```

- [ ] **Step 2: Test SMS export**

Run:
```bash
bash scripts/phone-backup.sh
ls -la ~/Backups/Phone/exports/sms_*
head -5 ~/Backups/Phone/exports/sms_$(date +%Y-%m-%d).json
head -3 ~/Backups/Phone/exports/sms_$(date +%Y-%m-%d).csv
```

Expected: JSON file with array of SMS objects, CSV with header row + data rows.

- [ ] **Step 3: Commit**

```bash
git add scripts/phone-backup.sh
git commit -m "feat: SMS export to JSON+CSV"
```

---

### Task 4: Implement contacts export

**Files:**
- Modify: `scripts/phone-backup.sh` (replace `export_contacts` function)

- [ ] **Step 1: Replace export_contacts**

Replace the `export_contacts` placeholder in `scripts/phone-backup.sh` with:

```bash
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
            7) type_label="other" ;;
        esac

        local name_escaped
        name_escaped=$(printf '%s' "$display_name" | jq -Rs '.' 2>/dev/null || echo "\"$display_name\"")

        if [ "$first" = true ]; then
            first=false
        else
            echo "," >> "$json_file"
        fi

        cat >> "$json_file" <<JSONEOF
  {"display_name": $name_escaped, "number": "$number", "type": "$type_label"}
JSONEOF
        (( count++ ))
    done <<< "$raw"

    echo "]" >> "$json_file"

    echo "display_name,number,type" > "$csv_file"
    jq -r '.[] | [.display_name, .number, .type] | @csv' "$json_file" >> "$csv_file" 2>/dev/null

    log "  Contacts exported: $count entries → $json_file + $csv_file"
}
```

- [ ] **Step 2: Test contacts export**

Run:
```bash
bash scripts/phone-backup.sh
head -10 ~/Backups/Phone/exports/contacts_$(date +%Y-%m-%d).json
head -5 ~/Backups/Phone/exports/contacts_$(date +%Y-%m-%d).csv
```

Expected: JSON array of contacts, CSV with header + rows.

- [ ] **Step 3: Commit**

```bash
git add scripts/phone-backup.sh
git commit -m "feat: contacts export to JSON+CSV"
```

---

### Task 5: Implement call log export

**Files:**
- Modify: `scripts/phone-backup.sh` (replace `export_call_log` function)

- [ ] **Step 1: Replace export_call_log**

Replace the `export_call_log` placeholder in `scripts/phone-backup.sh` with:

```bash
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
        name_escaped=$(printf '%s' "$name" | jq -Rs '.' 2>/dev/null || echo "\"$name\"")

        if [ "$first" = true ]; then
            first=false
        else
            echo "," >> "$json_file"
        fi

        cat >> "$json_file" <<JSONEOF
  {"number": "$number", "name": $name_escaped, "date": "$date_human", "date_epoch_ms": ${date_val:-0}, "duration_sec": ${duration:-0}, "type": "$type_label"}
JSONEOF
        (( count++ ))
    done <<< "$raw"

    echo "]" >> "$json_file"

    echo "number,name,date,duration_sec,type" > "$csv_file"
    jq -r '.[] | [.number, .name, .date, .duration_sec, .type] | @csv' "$json_file" >> "$csv_file" 2>/dev/null

    log "  Call log exported: $count entries → $json_file + $csv_file"
}
```

- [ ] **Step 2: Test call log export**

Run:
```bash
bash scripts/phone-backup.sh
head -10 ~/Backups/Phone/exports/call_log_$(date +%Y-%m-%d).json
head -5 ~/Backups/Phone/exports/call_log_$(date +%Y-%m-%d).csv
```

Expected: JSON array of calls, CSV with header + rows.

- [ ] **Step 3: Commit**

```bash
git add scripts/phone-backup.sh
git commit -m "feat: call log export to JSON+CSV"
```

---

### Task 6: Implement weekly archive

**Files:**
- Modify: `scripts/phone-backup.sh` (replace `maybe_archive` function)

- [ ] **Step 1: Replace maybe_archive**

Replace the `maybe_archive` placeholder in `scripts/phone-backup.sh` with:

```bash
maybe_archive() {
    # Check if any archive was created in the last 7 days
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
        (( errors++ ))
    fi
}
```

- [ ] **Step 2: Test archive creation**

Run:
```bash
bash scripts/phone-backup.sh
ls -lh ~/Backups/Phone/archives/
```

Expected: a `.tar.zst` file. Run again — should skip ("recent archive").

- [ ] **Step 3: Commit**

```bash
git add scripts/phone-backup.sh
git commit -m "feat: weekly tar.zst archive"
```

---

### Task 7: Create udev rule and installer

**Files:**
- Create: `scripts/install-backup-hook.sh`

- [ ] **Step 1: Write the udev installer script**

Write `scripts/install-backup-hook.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKUP_SCRIPT="$SCRIPT_DIR/phone-backup.sh"
UDEV_RULE="/etc/udev/rules.d/99-phone-backup.rules"
DEVICE_SERIAL="ZY22JVMJWL"
RUN_USER="$(whoami)"

if [ "$EUID" -ne 0 ]; then
    echo "Ce script doit être lancé avec sudo"
    echo "Usage: sudo bash $0"
    exit 1
fi

echo "Installation de la règle udev pour backup automatique..."
echo "  Device: $DEVICE_SERIAL"
echo "  Script: $BACKUP_SCRIPT"
echo "  User:   $RUN_USER"
echo ""

cat > "$UDEV_RULE" <<EOF
# Auto-backup Moto G14 on USB plug-in
ACTION=="add", SUBSYSTEM=="usb", ATTR{serial}=="$DEVICE_SERIAL", RUN+="/bin/systemd-run --no-block --uid=$RUN_USER --setenv=HOME=/home/$RUN_USER --setenv=PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin --unit=phone-backup $BACKUP_SCRIPT"
EOF

udevadm control --reload-rules
echo "Règle udev installée: $UDEV_RULE"
echo ""
echo "Test: débranche et rebranche ton téléphone."
echo "Logs: tail -f ~/Backups/Phone/backup.log"
```

- [ ] **Step 2: Make executable**

Run:
```bash
chmod +x scripts/install-backup-hook.sh
```

- [ ] **Step 3: Test the installer**

Run:
```bash
sudo bash scripts/install-backup-hook.sh
cat /etc/udev/rules.d/99-phone-backup.rules
```

Expected: rule file exists with correct serial and paths.

- [ ] **Step 4: Commit**

```bash
git add scripts/install-backup-hook.sh
git commit -m "feat: udev rule installer for auto-backup on USB"
```

---

### Task 8: End-to-end test

- [ ] **Step 1: Clean test — remove log and run fresh**

```bash
rm -f ~/Backups/Phone/backup.log
bash scripts/phone-backup.sh
```

Check log shows full cycle: START, file sync counts, SMS/contacts/calls export counts, archive, DONE.

```bash
cat ~/Backups/Phone/backup.log
```

- [ ] **Step 2: Test incremental — run again immediately**

```bash
bash scripts/phone-backup.sh
tail -3 ~/Backups/Phone/backup.log
```

Expected: SKIP due to cooldown (< 1h since last backup).

- [ ] **Step 3: Test lock protection**

```bash
echo $$ > /tmp/phone-backup.lock
bash scripts/phone-backup.sh
tail -1 ~/Backups/Phone/backup.log
rm /tmp/phone-backup.lock
```

Expected: SKIP due to lock file with active PID.

- [ ] **Step 4: Verify backup contents**

```bash
echo "=== Files ==="
find ~/Backups/Phone/latest -type f | wc -l
echo "=== Exports ==="
ls ~/Backups/Phone/exports/
echo "=== Archives ==="
ls ~/Backups/Phone/archives/
echo "=== Log ==="
cat ~/Backups/Phone/backup.log
```

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: phone auto-backup system complete"
```
