#!/bin/bash
# Script pour configurer la webcam virtuelle Phone-Cam et le micro virtuel

echo "=== Configuration Webcam Virtuelle Phone-Cam ==="

# v4l2loopback n'accorde qu'un seul jeton de capture par périphérique : une seule
# application peut lire un /dev/videoN à la fois. On crée donc une source
# (Phone-Cam-SRC, alimentée par scrcpy) et plusieurs sorties, une par application,
# que l'app recopie depuis la source. Voir scripts/webcam-fanout.sh.
if lsmod | grep -q v4l2loopback; then
    echo "[OK] v4l2loopback déjà chargé"
else
    echo "[...] Chargement de v4l2loopback..."
    if sudo modprobe v4l2loopback \
        video_nr=10,12,13,14,15 \
        card_label="Phone-Cam-SRC,Phone-Cam-1,Phone-Cam-2,Phone-Cam-3,Phone-Cam-4" \
        exclusive_caps=1,1,1,1,1; then
        echo "[OK] v4l2loopback chargé"
    else
        echo "[ERREUR] Impossible de charger v4l2loopback"
        echo "Installez avec: sudo apt install v4l2loopback-dkms linux-headers-\$(uname -r)"
        exit 1
    fi
fi

# Vérifier les périphériques
missing=0
for dev in /dev/video10 /dev/video12 /dev/video13 /dev/video14 /dev/video15; do
    if [ -e "$dev" ]; then
        echo "[OK] $dev disponible"
    else
        echo "[ERREUR] $dev non trouvé"
        missing=1
    fi
done
[ "$missing" -eq 0 ] || exit 1

echo ""
echo "=== Configuration Micro Virtuel (PipeWire) ==="

# Créer un micro virtuel pour les apps de chat (Lovo, Azar, Discord)
# Ce micro virtuel capture l'audio de scrcpy et le rend disponible comme entrée
if command -v pactl &> /dev/null; then
    # Supprimer l'ancien sink virtuel s'il existe
    pactl unload-module module-null-sink 2>/dev/null || true

    # Créer le sink virtuel "Phone-Mic"
    # L'audio de scrcpy ira vers ce sink, et le .monitor sera utilisable comme micro
    if pactl load-module module-null-sink sink_name=phone_mic sink_properties=device.description="Phone-Mic" &>/dev/null; then
        echo "[OK] Micro virtuel 'Phone-Mic' créé"
        echo "    Dans Lovo/Azar/Discord, sélectionnez 'Monitor of Phone-Mic' comme micro"
    else
        echo "[INFO] Micro virtuel déjà configuré ou erreur PipeWire"
    fi

    # Créer aussi une source virtuelle combinée si possible
    pactl load-module module-remap-source source_name=phone_mic_source master=phone_mic.monitor source_properties=device.description="Phone-Mic-Input" 2>/dev/null && \
        echo "[OK] Source micro 'Phone-Mic-Input' créée (plus simple à utiliser)"
else
    echo "[INFO] pactl non disponible, micro virtuel non configuré"
    echo "       Installez PipeWire ou PulseAudio pour l'audio des apps de chat"
fi

echo ""
echo "=== Configuration terminée! ==="
echo ""
echo "Source (scrcpy y écrit, ne la choisissez pas): /dev/video10 (Phone-Cam-SRC)"
echo "Caméras à utiliser: Phone-Cam-1 à Phone-Cam-4 (/dev/video12 à /dev/video15)"
echo ""
echo "Pour les apps de chat (Lovo, Azar, Discord):"
echo "  1. Lance l'app Phone-TV et active la webcam avec le micro coché"
echo "  2. Dans chaque app de chat, choisis:"
echo "     - Caméra: 'Phone-Cam-1', puis 'Phone-Cam-2' dans la suivante, etc."
echo "       (une caméra par application: un périphérique ne sert qu'un lecteur)"
echo "     - Micro: 'Monitor of Phone-Mic' ou 'Phone-Mic-Input'"
echo ""
echo "IMPORTANT: Pour router l'audio de scrcpy vers le micro virtuel:"
echo "  1. Ouvre pavucontrol (ou les paramètres audio)"
echo "  2. Va dans l'onglet 'Lecture' (Playback)"
echo "  3. Trouve 'scrcpy' et change sa sortie vers 'Phone-Mic'"
echo ""
echo "Pour charger automatiquement au démarrage, créez /etc/modprobe.d/v4l2loopback.conf:"
echo "  options v4l2loopback video_nr=10,12,13,14,15 \\"
echo "    card_label=Phone-Cam-SRC,Phone-Cam-1,Phone-Cam-2,Phone-Cam-3,Phone-Cam-4 \\"
echo "    exclusive_caps=1,1,1,1,1"
