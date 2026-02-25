# Phone-TV - Contrôle Android Phone & TV via ADB

Application de bureau en Rust pour contrôler vos téléphones et TV Android depuis votre PC Linux via ADB (Android Debug Bridge).

## Fonctionnalités

**Contrôle Téléphone :**
- Streaming webcam (caméra avant/arrière) vers un périphérique virtuel v4l2loopback (`/dev/video10`) pour Discord, OBS, etc.
- Capture micro optionnelle (micro du téléphone ou audio des apps)
- Mirroring d'écran via scrcpy
- Transfert de fichiers vidéo vers le téléphone avec suivi de progression
- Streaming vidéo par URL
- Boutons rapides : Caméra, Vidéo, Micro, Home, Back
- Mode "Stay Awake" pour empêcher la mise en veille

**Télécommande TV :**
- Navigation D-Pad complète (haut, bas, gauche, droite, OK)
- Boutons : Home, Back, Menu, Power
- Contrôles média : Play/Pause, Rewind, Fast Forward, Previous, Next
- Contrôle du volume et Mute
- Lanceurs rapides : YouTube TV, Netflix, Plex, Spotify

**Gestion des appareils :**
- Détection automatique des appareils connectés en USB
- Scan réseau pour trouver les appareils Android (port 5555)
- Connexion manuelle par IP

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

## Installation

```bash
git clone git@github.com:micferna/PHONE-TV-ANDROID.git
cd PHONE-TV-ANDROID
cargo build --release
```

Le binaire se trouvera dans `target/release/phone-tv`.

## Configuration de la webcam et du micro virtuel

Avant d'utiliser les fonctions webcam/micro, lancez le script de configuration :

```bash
chmod +x setup-webcam.sh
sudo ./setup-webcam.sh
```

Ce script :
- Charge le module `v4l2loopback` avec `/dev/video10` comme "Phone-Cam"
- Crée un sink audio virtuel "Phone-Mic" via PipeWire/PulseAudio
- Crée une source remappée "Phone-Mic-Input"

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
5. Dans Discord, sélectionnez **"Phone-Cam"** comme caméra et **"Phone-Mic-Input"** comme micro

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
| eframe | 0.29 | Framework GUI (basé sur egui) |
| anyhow | 1.0 | Gestion d'erreurs |
| rfd | 0.14 | Boîtes de dialogue fichier |

## Licence

MIT
