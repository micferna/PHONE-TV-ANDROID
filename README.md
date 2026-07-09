# Phone-TV - Contrôle Android Phone & TV via ADB

[![CI](https://github.com/micferna/PHONE-TV-ANDROID/actions/workflows/ci.yml/badge.svg)](https://github.com/micferna/PHONE-TV-ANDROID/actions/workflows/ci.yml)
[![CodeQL](https://github.com/micferna/PHONE-TV-ANDROID/actions/workflows/codeql.yml/badge.svg)](https://github.com/micferna/PHONE-TV-ANDROID/actions/workflows/codeql.yml)
[![Supply chain](https://github.com/micferna/PHONE-TV-ANDROID/actions/workflows/supply-chain.yml/badge.svg)](https://github.com/micferna/PHONE-TV-ANDROID/actions/workflows/supply-chain.yml)
[![Secret scan](https://github.com/micferna/PHONE-TV-ANDROID/actions/workflows/gitleaks.yml/badge.svg)](https://github.com/micferna/PHONE-TV-ANDROID/actions/workflows/gitleaks.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)

Application de bureau en Rust pour contrôler vos téléphones et TV Android depuis votre PC Linux via ADB (Android Debug Bridge).

## Fonctionnalités

**Contrôle Téléphone :**
- Streaming webcam (caméra avant/arrière) vers des périphériques virtuels v4l2loopback, jusqu'à 4 applications simultanées (Discord, OBS, navigateur…)
- Capture micro optionnelle (micro du téléphone ou audio des apps)
- Mirroring d'écran via scrcpy
- Transfert de fichiers vidéo vers le téléphone avec suivi de progression
- Streaming vidéo par URL
- Boutons rapides : Caméra, Vidéo, Micro, Home, Back
- Mode "Stay Awake" pour empêcher la mise en veille
- Capture d'écran (PNG sauvegardable)
- Installation d'APK depuis le PC
- Sonnerie / "retrouver mon téléphone"
- Niveau de batterie en temps réel

**Télécommande TV :**
- Navigation D-Pad complète (haut, bas, gauche, droite, OK)
- Boutons : Home, Back, Menu, Power
- Contrôles média : Play/Pause, Rewind, Fast Forward, Previous, Next
- Contrôle du volume et Mute
- Lanceurs rapides : YouTube TV, Netflix, Plex, Spotify, Oqee
- Saisie texte depuis le clavier PC
- Capture d'écran TV

**Audit & sécurité (IA) :**
- Score de sécurité, posture système (SELinux, patch level, etc.)
- Pentest : root check, vulnérabilités, ports ouverts
- Wizard guidé : détection → scan → pentest → profil → analyse IA → nettoyage → rapport
- Analyse IA via OpenRouter de toutes les apps (bloatware, trackers, suspectes)
- Sauvegarde APK avant suppression + restauration en un clic
- Détection des apps réinstallées entre deux audits
- Historique par appareil (sessions, scores, profils)
- Export du rapport en Markdown

**Gestion des appareils :**
- Détection automatique des appareils connectés en USB
- Scan réseau pour trouver les appareils Android (port 5555)
- Connexion manuelle par IP
- Appairage sans fil (Android 11+) avec code à 6 chiffres

## Prérequis

- **Rust** (edition 2021+)
- **ADB** (Android Debug Bridge)
- **Flatpak** avec [aurynk (scrcpy)](https://flathub.org/apps/io.github.IshuSinghSE.aurynk)
- **v4l2loopback-dkms** et **linux-headers** (pour la webcam virtuelle)
- **PipeWire** ou **PulseAudio** (pour le micro virtuel)

### Installation des dépendances (Debian/Ubuntu)

```bash
# ADB
sudo apt install adb

# v4l2loopback pour la webcam virtuelle
sudo apt install v4l2loopback-dkms linux-headers-$(uname -r)

# Flatpak + scrcpy (aurynk)
sudo apt install flatpak
flatpak install flathub io.github.IshuSinghSE.aurynk
```

### Installation des dépendances (Arch Linux)

```bash
sudo pacman -S android-tools v4l2loopback-dkms linux-headers flatpak
flatpak install flathub io.github.IshuSinghSE.aurynk
```

### Installation sur Windows

Phone-TV fonctionne sous Windows 10/11 (x86_64). La webcam virtuelle utilise
OBS Virtual Camera plutôt que v4l2loopback.

**Dépendances minimum :**
1. **Android Platform Tools** (adb.exe) — [télécharger](https://developer.android.com/tools/releases/platform-tools), extraire et ajouter au PATH
2. **scrcpy** (pour mirror + webcam) — [télécharger](https://github.com/Genymobile/scrcpy/releases), extraire et ajouter au PATH

**Webcam virtuelle (équivalent v4l2loopback) :**
3. **OBS Studio** — [télécharger](https://obsproject.com/download) (inclut OBS Virtual Camera depuis la v26)
   - Une fois OBS lancé : Sources → + → Capture de fenêtre → choisir la fenêtre `scrcpy`
   - Cliquer "Démarrer la caméra virtuelle" (bouton en bas à droite)
   - Dans Discord/Teams/Zoom : sélectionner "OBS Virtual Camera"
   - Alternative open source : [Unity Capture](https://github.com/schellingb/UnityCapture)

Télécharger le `.zip` depuis [Releases](../../releases/latest), extraire,
et lancer `phone-tv.exe`.

## Installation

```bash
git clone git@github.com:micferna/PHONE-TV-ANDROID.git
cd PHONE-TV-ANDROID
cargo build --release
```

Le binaire se trouvera dans `target/release/phone-tv` (ou `.exe` sous Windows).

### Builds prêts à l'emploi

Voir [GitHub Releases](../../releases/latest) :
- `phone-tv_*.deb` — Debian/Ubuntu
- `phone-tv-*.AppImage` — distros Linux (portable)
- `phone-tv-*-windows-x86_64.zip` — Windows 10/11

## Configuration de la webcam et du micro virtuel

Avant d'utiliser les fonctions webcam/micro, lancez le script de configuration :

```bash
chmod +x setup-webcam.sh
sudo ./setup-webcam.sh
```

Ce script :
- Charge le module `v4l2loopback` avec une source `Phone-Cam-SRC` (`/dev/video10`) et
  quatre sorties `Phone-Cam-1` à `Phone-Cam-4` (`/dev/video12` à `/dev/video15`)
- Crée un sink audio virtuel "Phone-Mic" via PipeWire/PulseAudio
- Crée une source remappée "Phone-Mic-Input"

### Pourquoi plusieurs périphériques ?

`v4l2loopback` n'accorde qu'un **seul jeton de capture par périphérique** : une seule
application peut lire un `/dev/videoN` donné, toutes les autres reçoivent `-EBUSY`.
Le paramètre `max_openers` ne change rien, il ne borne que `open()`, ce qui suffit à
*énumérer* la caméra mais pas à la lire.

Phone-TV contourne cela en lisant la source une fois et en recopiant les images vers
un périphérique par application :

```
scrcpy ──> Phone-Cam-SRC ──> ffmpeg ──┬──> Phone-Cam-1   (Discord)
           /dev/video10               ├──> Phone-Cam-2   (Firefox)
                                      ├──> Phone-Cam-3   (OBS)
                                      └──> Phone-Cam-4
```

Ce « fan-out » démarre et s'arrête avec la webcam. Il n'y a pas de ré-encodage, les
images sont copiées telles quelles. **Ne choisissez jamais `Phone-Cam-SRC` dans une
application** : elle confisquerait le jeton de la source. Le nombre de sorties fixe le
nombre d'applications simultanées ; pour en ajouter, étendez `video_nr` dans
`/etc/modprobe.d/v4l2loopback.conf` *et* `FANOUT_SINKS` dans `src/adb.rs`.

Le script `scripts/webcam-fanout.sh` fait la même chose à la main, hors de l'app.

## Utilisation

```bash
cargo run --release
```

Ou directement le binaire :

```bash
./target/release/phone-tv
```

### Streaming webcam pour Discord

1. Connectez votre téléphone en USB
2. Lancez `sudo ./setup-webcam.sh`
3. Lancez l'application et sélectionnez votre appareil
4. Cochez les options micro si nécessaire, puis cliquez **"Démarrer Webcam"**
5. Dans Discord, sélectionnez **"Phone-Cam-1"** comme caméra et **"Phone-Mic-Input"** comme micro

Pour une seconde application en même temps (navigateur, OBS…), choisissez-y
**"Phone-Cam-2"**, puis **"Phone-Cam-3"** pour une troisième : une caméra par
application. Une application lancée avant la webcam doit ré-énumérer ses périphériques
(rouvrir ses paramètres vidéo, ou `Ctrl+R` dans Discord) pour les voir apparaître.

### Télécommande TV

1. L'application détecte automatiquement les TV Android connectées
2. Sélectionnez la TV dans la liste
3. Utilisez le D-Pad et les boutons pour naviguer

### Transfert vidéo

1. Sélectionnez votre téléphone
2. Parcourez et sélectionnez un fichier vidéo
3. Cliquez **"Envoyer+Lire"** pour transférer et lancer automatiquement la lecture
4. La barre de progression affiche l'état du transfert en temps réel

## Dépendances Rust

| Crate | Version | Description |
|-------|---------|-------------|
| eframe | 0.33 | Framework GUI (basé sur egui) |
| egui_extras | 0.33 | Chargeurs d'images pour egui |
| anyhow | 1.0 | Gestion d'erreurs |
| rfd | 0.17 | Boîtes de dialogue fichier |
| reqwest | 0.12 | Client HTTP (OpenRouter) |
| serde / serde_json | 1.0 | Sérialisation des configs et historique |
| toml | 0.8 | Settings |
| chrono | 0.4 | Horodatage des sessions et backups |
| dirs | 6.0 | Chemins XDG (config, backups) |
| image | 0.25 | Décodage PNG (captures) |

## Licence

MIT
