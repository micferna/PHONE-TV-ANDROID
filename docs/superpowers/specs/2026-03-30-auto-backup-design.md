# Auto-backup incrémental au branchement USB

## Objectif

Quand le Moto G14 (serial `ZY22JVMJWL`) est branché en USB, lancer automatiquement une backup incrémentale vers `~/Backups/Phone/`. Ne copie que les fichiers nouveaux ou modifiés. Exporte SMS, contacts et journal d'appels en JSON+CSV.

## Structure des backups

```
~/Backups/Phone/
├── latest/                    # miroir courant (sync incrémental)
│   ├── DCIM/
│   ├── Pictures/
│   ├── Movies/
│   ├── Downloads/
│   ├── WhatsApp/
│   ├── Snapchat/
│   └── Music/
├── exports/                   # dumps SMS/contacts/appels datés
│   ├── sms_YYYY-MM-DD.json
│   ├── sms_YYYY-MM-DD.csv
│   ├── contacts_YYYY-MM-DD.json
│   ├── contacts_YYYY-MM-DD.csv
│   ├── call_log_YYYY-MM-DD.json
│   └── call_log_YYYY-MM-DD.csv
├── archives/                  # snapshots compressés hebdo
│   └── YYYY-MM-DD_full.tar.zst
└── backup.log
```

## Composants

### 1. Script `phone-backup.sh`

Placé dans le repo à `scripts/phone-backup.sh`.

#### Sync incrémental des fichiers

Pour chaque dossier source (`DCIM`, `Pictures`, `Movies`, `Downloads`, `Music`, WhatsApp media, Snapchat media) :
- Lister les fichiers sur le téléphone avec `adb shell find <path> -type f`
- Pour chaque fichier, comparer taille (`adb shell stat -c '%s'`) avec le fichier local
- Ne pull que si le fichier n'existe pas localement ou si la taille diffère
- Optimisation : lister tous les fichiers + tailles en une seule commande shell, comparer en batch

#### Export SMS/MMS

```bash
adb shell content query --uri content://sms
```

Parser la sortie, convertir en JSON et CSV. Colonnes : `date`, `address`, `body`, `type` (1=reçu, 2=envoyé), `read`.

#### Export Contacts

```bash
adb shell content query --uri content://contacts/phones
```

Colonnes : `display_name`, `number`, `type`.

#### Export Journal d'appels

```bash
adb shell content query --uri content://call_log/calls
```

Colonnes : `number`, `name`, `date`, `duration`, `type` (1=entrant, 2=sortant, 3=manqué).

#### Archive hebdomadaire

Si aucune archive n'a été créée dans les 7 derniers jours, créer `archives/YYYY-MM-DD_full.tar.zst` à partir de `latest/` + `exports/`.

#### Protections

- **Lock file** : `/tmp/phone-backup.lock` — si présent, skip (backup déjà en cours)
- **Cooldown 1h** : si `backup.log` montre une backup réussie il y a moins de 60 minutes, skip
- **Attente ADB** : après détection udev, attendre jusqu'à 15s que `adb devices` montre le device en état `device` (pas `offline`)

### 2. Udev rule

Fichier : `/etc/udev/rules.d/99-phone-backup.rules`

```
ACTION=="add", SUBSYSTEM=="usb", ATTR{serial}=="ZY22JVMJWL", RUN+="/bin/systemd-run --no-block --uid=ocb /path/to/phone-backup.sh"
```

Utilise `systemd-run` pour lancer en tant que l'utilisateur `ocb` (pas root), en non-bloquant.

### 3. Logging

Toute l'activité est loggée dans `~/Backups/Phone/backup.log` :
- Timestamp de début/fin
- Nombre de fichiers copiés
- Nombre de SMS/contacts/appels exportés
- Erreurs éventuelles

## Ce qui n'est PAS couvert

- Messages texte WhatsApp/Discord/Snap (nécessite root, dans `/data/data/`)
- Données d'apps (inaccessible sans root)
- Notifications desktop (non souhaitées)

## Dépendances

- `adb` (déjà installé)
- `zstd` pour la compression des archives (à installer si absent)
- `jq` pour le formatage JSON (à installer si absent)
