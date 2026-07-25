# Phone-TV — Benchmark complet

_Mesuré le 2026-07-25 sur Linux x86_64 (Debian 13, kernel 6.12.96, Rust stable, GPU
NVIDIA). Appareil de test pour les mesures ADB : moto g14, Android 14, USB._

_Comparaisons « mai » = campagne du 2026-05-26, même machine._

## Résumé en une ligne

26,8 MB de binaire release, 161 MB de RSS au repos, fonctions pures sous les 30 µs.
Le coût réel de l'application n'est pas son code : il est dans les allers-retours ADB,
de 2 à 110 ms selon la commande.

## Build

| Métrique | Valeur | mai |
|---|---|---|
| Release cold (deps + 2 binaires, profil par défaut) | **105 s** | 78 s |
| Release cold avec le profil actuel (LTO thin + CGU=1) | **119 s** | — |
| Release incrémental (`touch src/main.rs`) | **22,2 s** | 5,7 s |
| Crates directs | 12 | 12 |
| Crates transitifs | 333 | 309 |

L'incrémental passe de 5,7 s à 22,2 s : c'est le prix de `codegen-units = 1` et du
link LTO, payé uniquement sur le profil release. `cargo build` (dev), celui que la CI
utilise, n'est pas concerné.

## Taille du binaire

| Build | Taille |
|---|---|
| Release, profil Cargo par défaut | 38,6 MB |
| … + `strip` a posteriori | 29,4 MB |
| **Release avec le profil du dépôt** (`lto="thin"`, `codegen-units=1`, `strip=true`) | **26,8 MB** |

Soit **−31 %** par rapport au profil par défaut. La campagne de mai mesurait 24,4 MB :
l'écart restant vient de la stack GUI, `.text` étant passé de 12,0 à 19,0 MB avec
egui/wgpu 0.35 et reqwest 0.13.

### Répartition par section (`objdump -h`)

| Section | Avant profil | Après profil |
|---|---|---|
| `.text` | 19,64 MB | **18,98 MB** |
| `.rodata` | 3,88 MB | **3,72 MB** |
| `.data.rel.ro` | 0,90 MB | — |
| `.data` / `.bss` | 0,02 MB chacun | — |

Le gros du reste vient de la stack GUI (eframe/egui/wgpu/winit), de TLS
(rustls + ring) et d'accesskit. Le code applicatif reste ~3 % du binaire.

## Runtime

Mesuré après ~10 s de vie idle, fenêtre ouverte, aucun appareil sélectionné.

| Métrique | mai | Profil par défaut | **Profil du dépôt** |
|---|---|---|---|
| RSS | 176 MB | 199,9 MB | **161,2 MB** |
| VSZ | 766 MB | 1 298 MB | — |
| Threads | 5 | 10 | 10 |
| Descripteurs ouverts | 46 | 77 | — |

Le `strip` fait gagner 39 MB de RSS : les tables de debug étaient mappées en mémoire
pour rien. On repasse ainsi *sous* le chiffre de mai malgré une stack GUI plus lourde.

## Latences ADB (moto g14, USB)

10 itérations après warmup, 5 pour les commandes lentes.

| Commande | Temps/op |
|---|---|
| `adb devices -l` | **2,0 ms** |
| `adb shell getprop ro.product.model` | 29,5 ms |
| `adb shell dumpsys battery` | 32,2 ms |
| `adb shell ls -lp /sdcard/` | 33,9 ms |
| `adb shell pm list packages -3 -f` | 57,9 ms |
| `adb shell pm list packages` | 59,4 ms |
| `adb shell dumpsys media.camera` | **109,2 ms** |

### Lecture

Tout ce qui traverse `adb shell` coûte ~30 ms de plancher, indépendamment du travail
demandé : c'est le coût d'un aller-retour et d'un `fork` sur le téléphone. `adb
devices -l`, qui ne parle qu'au serveur adb local, coûte 15× moins.

Deux coûts se cumulent donc, et aucun n'est le travail lui-même : l'aller-retour
(~30 ms) **et** le spawn de chaque `getprop` sur le téléphone (~20 ms). D'où ce
classement, mesuré pour lire trois propriétés :

| Stratégie | Temps |
|---|---|
| 3 × `adb shell getprop <prop>` | 89,9 ms |
| 3 × `getprop` dans un seul `adb shell` | 67,5 ms |
| **1 × `adb shell getprop` (table entière, 40 KB)** | **36,3 ms** |

Vider toute la table ne paie chaque coût qu'une fois, et les 953 lignes renvoyées se
parsent en microsecondes. `get_all_devices()` lit donc désormais le dump complet :

| `get_all_devices()`, 1 appareil | Temps |
|---|---|
| Avant (`devices -l` + 3 `getprop`) | ~92 ms |
| **Après (`devices -l` + 1 dump)** | **37 ms** |

Le rafraîchissement tourne hors du thread UI et derrière un garde `refreshing`, donc
rien ne bloque ; seul le constructeur `PhoneTvApp::new()` le fait en synchrone, ce qui
retarde d'autant l'ouverture de la fenêtre — 37 ms désormais au lieu de 92 ms.

## Scan réseau

Scan du `/24` local à la recherche du port 5555, budget de 400 ms par hôte.

| Variante | Temps | Pic de threads |
|---|---|---|
| Un thread OS par hôte (avant) | 0,46 s | 255 |
| Pool 32 workers | 3,20 s | 33 |
| Pool 64 workers | 1,60 s | 65 |
| **Pool 96 workers (actuel)** | **1,20 s** | **97** |

Le timeout domine : sur un `/24` calme, presque chaque adresse coûte les 400 ms pleins,
donc le temps total vaut à peu près `254 / workers × 400 ms`. Le choix retenu troque
0,74 s contre 62 % de threads en moins, sur une action déjà asynchrone derrière un
indicateur d'activité — et évite d'envoyer 254 SYN simultanés, rafale que les box
grand public limitent volontiers.

## Micro-benchmarks (`cargo run --release --bin bench-micro`)

Warmup 100 itérations.

| Opération | Itérations | Temps/op | mai |
|---|---|---|---|
| `bulletins_behind` (patch récent) | 50 000 | **1,4 µs** | 1,6 µs |
| `bulletins_behind` (patch ancien) | 50 000 | **1,5 µs** | 1,5 µs |
| `bulletins_behind` (date invalide) | 50 000 | **41 ns** | 38 ns |
| `reappeared_packages` (10 × 300 apps) | 10 000 | **13,0 µs** | 12,3 µs |
| `reappeared_packages` (10 × 1000 apps) | 5 000 | **25,7 µs** | 23,3 µs |
| `serde_json::from_str` (50 verdicts) | 5 000 | **26,9 µs** | 29,5 µs |

Stable au bruit de mesure près. Mis en regard des latences ADB ci-dessus, ces fonctions
sont trois à quatre ordres de grandeur sous le coût d'une seule commande sur le
téléphone : aucune ne mérite d'être optimisée.

## Code

| Métrique | Valeur | mai |
|---|---|---|
| Fichiers `.rs` | 39 | 37 |
| LOC totales | 13 095 | 11 617 |

### Top fichiers par LOC

| Fichier | LOC |
|---|---|
| `ui/security.rs` | 2 649 |
| `ui/wizard.rs` | 1 481 |
| `adb.rs` | 1 314 |
| `app.rs` | 1 303 |
| `ui/phone.rs` | 660 |
| `ui/video.rs` | 537 |
| `ui/tv.rs` | 488 |

## Lint et tests

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Les trois passent au strict.

## Pistes restantes

Les pistes de la campagne de mai (LTO, `strip`) sont appliquées et chiffrées plus
haut, tout comme la fusion des `getprop`. Ce qui reste :

1. **Sortir le `get_all_devices()` synchrone de `PhoneTvApp::new()`** — la fenêtre
   s'ouvrirait ~37 ms plus tôt, la liste se remplissant ensuite comme au refresh.
   Gain devenu marginal depuis le passage au dump unique.
2. **`panic = "abort"`** — ~500 KB de tables d'unwind. Non retenu pour l'instant :
   l'application fait tourner plusieurs threads de travail, et un `abort` global
   changerait leur comportement en cas de panique.
3. **`reqwest` sans `gzip`/`brotli`** — ~120 KB, si l'API OpenRouter reste la seule
   consommatrice HTTP.
