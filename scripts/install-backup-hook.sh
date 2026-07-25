#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKUP_SCRIPT="$SCRIPT_DIR/phone-backup.sh"
UDEV_RULE="/etc/udev/rules.d/99-phone-backup.rules"
RUN_USER="${SUDO_USER:-$(whoami)}"

if [ "$EUID" -ne 0 ]; then
    echo "Ce script doit être lancé avec sudo"
    echo "Usage: sudo bash $0"
    exit 1
fi

# Which device the udev rule should match: PHONE_SERIAL, else the one plugged in
# right now. A serial baked into a public repository identifies a personal handset
# and makes the script useless to anyone else. Probed as the invoking user, so it
# reaches their adb server instead of starting a second one as root.
DEVICE_SERIAL="${PHONE_SERIAL:-$(runuser -u "$RUN_USER" -- adb devices 2>/dev/null | awk 'NR>1 && $2=="device" {print $1}' | head -1)}"
if [ -z "$DEVICE_SERIAL" ]; then
    echo "Aucun appareil ADB détecté." >&2
    echo "Branchez le téléphone, ou lancez: sudo PHONE_SERIAL=<serial> bash $0" >&2
    exit 1
fi

echo "Installation de la règle udev pour backup automatique..."
echo "  Device: $DEVICE_SERIAL"
echo "  Script: $BACKUP_SCRIPT"
echo "  User:   $RUN_USER"
echo ""

cat > "$UDEV_RULE" <<EOF
# Auto-backup on USB plug-in
ACTION=="add", SUBSYSTEM=="usb", ATTR{serial}=="$DEVICE_SERIAL", RUN+="/bin/systemd-run --no-block --uid=$RUN_USER --setenv=HOME=/home/$RUN_USER --setenv=PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin --unit=phone-backup $BACKUP_SCRIPT"
EOF

udevadm control --reload-rules
echo "Règle udev installée: $UDEV_RULE"
echo ""
echo "Test: débranche et rebranche ton téléphone."
echo "Logs: tail -f ~/Backups/Phone/backup.log"
