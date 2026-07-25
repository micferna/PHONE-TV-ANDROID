#!/usr/bin/env bash
# Fan-out de la webcam virtuelle Phone-Cam vers un device par application.
#
# v4l2loopback >= 0.15 ne distribue qu'un seul jeton de capture par device
# (V4L2L_TOKEN_CAPTURE) : le premier lecteur qui appelle S_FMT/REQBUFS le prend,
# les suivants reçoivent -EBUSY. max_openers ne change rien, il ne gouverne que
# open(). Une seule application peut donc lire /dev/video10 à la fois.
#
# On lit la source une fois et on recopie les images vers plusieurs devices :
#
#   scrcpy -> /dev/video10 -> ffmpeg -+-> /dev/video12  (Firefox)
#                                     `-> /dev/video13  (Discord)
#
# Chaque application prend alors le jeton de *son* device. Pas de ré-encodage,
# uniquement de la copie d'images.

set -euo pipefail

SRC=${SRC:-/dev/video10}
# Un device ne sert qu'une application : autant de sinks que d'applications
# simultanées voulues.
# shellcheck disable=SC2206  # découpage sur les espaces voulu
SINKS=(${SINKS:-/dev/video12 /dev/video13 /dev/video14 /dev/video15})
# scrcpy retente jusqu'à ~30 s quand le HAL de face unlock tient le capteur.
WAIT_TIMEOUT=${WAIT_TIMEOUT:-40}

die() { echo "webcam-fanout: $*" >&2; exit 1; }

command -v ffmpeg >/dev/null || die "ffmpeg introuvable"
command -v v4l2-ctl >/dev/null || die "v4l2-ctl introuvable (paquet v4l-utils)"

for sink in "${SINKS[@]}"; do
	[[ -e $sink ]] || die "$sink absent. Ajoutez-le au fichier
  /etc/modprobe.d/v4l2loopback.conf puis rechargez le module, ou créez-le à
  chaud : sudo v4l2loopback-ctl add -n Phone-Cam-1 -x 1 $sink"
	[[ -w $sink ]] || die "$sink non accessible en écriture (groupe 'video' ?)"
done

# Avec exclusive_caps=1 un device loopback n'annonce « Video Capture » que
# pendant qu'un writer y écrit ; sinon il n'expose que « Video Output » et
# G_FMT échoue. C'est donc le test « est-ce que scrcpy a démarré ».
source_is_streaming() {
	v4l2-ctl -d "$SRC" -D 2>/dev/null | grep -q 'Video Capture'
}

wait_for_source() {
	local deadline=$((SECONDS + WAIT_TIMEOUT))
	while ((SECONDS < deadline)); do
		source_is_streaming && return 0
		sleep 1
	done
	return 1
}

echo "webcam-fanout: attente d'un flux sur $SRC..."
wait_for_source || die "aucun flux sur $SRC après ${WAIT_TIMEOUT}s (scrcpy tourne ?)"

# Le format est imposé par le writer ; on le relaie tel quel aux sorties.
echo "webcam-fanout: $SRC -> ${SINKS[*]}"

args=(-loglevel warning -f v4l2 -i "$SRC")
for sink in "${SINKS[@]}"; do
	args+=(-map 0:v -f v4l2 -pix_fmt yuv420p "$sink")
done

# $SRC n'a qu'un jeton de capture. Si une application le tient encore (elle a été
# pointée sur la source au lieu d'un sink), ffmpeg sort aussitôt sur -EBUSY ;
# on retente pour s'accrocher dès qu'elle le relâche.
deadline=$((SECONDS + WAIT_TIMEOUT))
while :; do
	ffmpeg "${args[@]}" && exit 0
	((SECONDS < deadline)) ||
		die "$SRC reste occupé. Une application lit-elle la source au lieu de ${SINKS[0]} ?"
	echo "webcam-fanout: $SRC occupé, nouvel essai..."
	sleep 2
done
