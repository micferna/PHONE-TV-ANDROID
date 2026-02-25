#!/bin/bash
# Script pour configurer la webcam virtuelle Phone-Cam et le micro virtuel

echo "=== Configuration Webcam Virtuelle Phone-Cam ==="

# Vérifier si v4l2loopback est chargé
if lsmod | grep -q v4l2loopback; then
    echo "[OK] v4l2loopback déjà chargé"
else
    echo "[...] Chargement de v4l2loopback..."
    sudo modprobe v4l2loopback video_nr=10 card_label="Phone-Cam" exclusive_caps=1
    if [ $? -eq 0 ]; then
        echo "[OK] v4l2loopback chargé"
    else
        echo "[ERREUR] Impossible de charger v4l2loopback"
        echo "Installez avec: sudo apt install v4l2loopback-dkms linux-headers-\$(uname -r)"
        exit 1
    fi
fi

# Vérifier le périphérique
if [ -e /dev/video10 ]; then
    echo "[OK] /dev/video10 disponible"
else
    echo "[ERREUR] /dev/video10 non trouvé"
    exit 1
fi

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
echo "Webcam virtuelle prête: /dev/video10 (Phone-Cam)"
echo ""
echo "Pour les apps de chat (Lovo, Azar, Discord):"
echo "  1. Lance l'app Phone-TV et active la webcam avec le micro coché"
echo "  2. Dans l'app de chat, choisis:"
echo "     - Caméra: 'Phone-Cam' ou '/dev/video10'"
echo "     - Micro: 'Monitor of Phone-Mic' ou 'Phone-Mic-Input'"
echo ""
echo "IMPORTANT: Pour router l'audio de scrcpy vers le micro virtuel:"
echo "  1. Ouvre pavucontrol (ou les paramètres audio)"
echo "  2. Va dans l'onglet 'Lecture' (Playback)"
echo "  3. Trouve 'scrcpy' et change sa sortie vers 'Phone-Mic'"
echo ""
echo "Pour charger automatiquement au démarrage, ajoutez à /etc/modules:"
echo "  v4l2loopback video_nr=10 card_label=Phone-Cam exclusive_caps=1"
