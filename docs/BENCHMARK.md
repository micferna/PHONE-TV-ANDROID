# Phone-TV — Benchmark complet

_Mesuré le 2026-05-26 sur Linux x86_64 (Debian 13, kernel 6.12, Rust stable)._

## Résumé en une ligne

24 MB binaire release, 19 MB strippé, ~76 s pour build cold, 176 MB RSS au démarrage idle,
fonctions critiques sous la barre des 30 µs. Pas de hot path identifié.

## Build

| Métrique | Valeur |
|---|---|
| Build release cold | **77.7 s** |
| Build release incrémental (touch main.rs) | **5.7 s** |
| Build debug cold | **45.2 s** |
| Crates uniques (transitives) | 309 |
| Crates directs | 12 |

## Taille du binaire

| Build | Taille |
|---|---|
| `cargo build --release` | **24.4 MB** |
| `strip` sur le release | **19.1 MB** |
| `cargo build` (debug) | 289.1 MB |

### Répartition par section (`objdump -h`)

| Section | Taille | Rôle |
|---|---|---|
| `.text` | 12.0 MB | Code exécutable |
| `.rodata` | 3.0 MB | Constantes / strings |
| `.data.rel.ro` | 0.6 MB | Const tables relocatables |
| `.data` | 27 KB | Variables statiques |
| `.bss` | 8 KB | Variables zero-init |

### Plus gros symboles (`nm --size-sort`)

| Symbole | Taille | Origine |
|---|---|---|
| `ecp_nistz256_precomputed` | 148 KB | `ring` (TLS) |
| `kBrotliDictionary` | 120 KB | `brotli-decompressor` (reqwest) |
| `x11_dl::xlib::Xlib::open` | 74 KB | X11 bindings (Linux GUI) |
| `BIG5_LOW_BITS` | 37 KB | `encoding_rs` |
| `zbus::connection::builder::build` | 34 KB | accesskit (a11y) |
| `epaint::text::text_layout::layout` | 24 KB | egui text shaping |
| `k25519Precomp` + `x25519_ge_frombytes_vartime` | 45 KB | `ring` (TLS) |

→ La majorité du poids vient de la stack GUI (eframe/egui/wgpu/winit), TLS (rustls+ring),
et accesskit. Le code applicatif est ~3% du binaire.

## Code

| Métrique | Valeur |
|---|---|
| Fichiers `.rs` | **37** |
| LOC totales | **11 617** |
| Plus gros module | `ui/security.rs` (2 649 LOC) |
| Modules > 1000 LOC | `ui/security.rs`, `ui/wizard.rs`, `app.rs` |

### Top fichiers par LOC

| Fichier | LOC |
|---|---|
| `ui/security.rs` | 2 649 |
| `ui/wizard.rs` | 1 481 |
| `app.rs` | 1 041 |
| `ui/phone.rs` | 670 |
| `adb.rs` | 605 |
| `ui/tv.rs` | 488 |
| `security/monitoring.rs` | 369 |
| `ui/audit.rs` | 337 |
| `types.rs` | 329 |

## Runtime

Démarré avec `target/release/phone-tv` sur un GPU NVIDIA, mesuré après 3 s de vie idle.

| Métrique | Valeur |
|---|---|
| RSS (mémoire résidente) | **176 MB** |
| VSZ (mémoire virtuelle) | **766 MB** |
| Threads | 5 |
| File descriptors ouverts | 46 |

176 MB RSS au démarrage idle est dans la moyenne pour un binaire eframe/wgpu — c'est
dominé par les drivers GL/Vulkan et les textures de fonts par défaut. Comparable
à OBS Studio idle (~150-200 MB).

## Micro-benchmarks (`cargo run --release --bin bench-micro`)

Warmup 100 itérations, mesures sur N itérations après warmup.

| Opération | Itérations | Temps/op |
|---|---|---|
| `bulletins_behind("2025-12-05")` (recent patch) | 50 000 | **1.6 µs** |
| `bulletins_behind("2024-01-05")` (old patch) | 50 000 | **1.5 µs** |
| `bulletins_behind("not-a-date")` (parse fail) | 50 000 | **38 ns** |
| `reappeared_packages` (10 sessions × 300 apps) | 10 000 | **12.3 µs** |
| `reappeared_packages` (10 sessions × 1000 apps) | 5 000 | **23.3 µs** |
| `serde_json::from_str` (50-app verdict array) | 5 000 | **29.5 µs** |

### Lecture

- **Lookup bulletin** : 1.5 µs pour parcourir 12 bulletins et calculer le gap. Pas de
  scaling problématique tant que la table reste sous quelques centaines d'entrées.
- **Diff historique** : ~25 µs pour comparer 1000 apps contre 10 sessions de 30 packages.
  Linéaire en `apps × packages_supprimés_total`. Pas un hot path puisque appelé une
  fois après chaque scan.
- **Parsing JSON LLM** : 30 µs pour 50 verdicts. Négligeable face au temps réseau
  (1-3 s pour la réponse OpenRouter).

## Dépendances par poids estimé

D'après les symboles dans le binaire (proxy pour cargo-bloat indisponible) :

| Famille | Part estimée |
|---|---|
| eframe + egui + wgpu + winit (GUI) | ~55% |
| reqwest + rustls + ring + h2 + brotli (HTTP/TLS) | ~25% |
| accesskit + zbus (a11y Linux) | ~8% |
| image + resvg + image-webp | ~5% |
| encoding_rs + chrono + serde + autres | ~4% |
| **Code applicatif phone-tv** | **~3%** |

## Audit sécurité

```
cargo audit
```

- **0 vulnérabilité actionnable** dans les dépendances directes
- 3 warnings transitifs non corrigeables sans bump upstream :
  - `paste` (unmaintained) via wgpu→metal
  - `rand` (unsound RUSTSEC-2026-0097) via deux chemins : `mime_guess2→phf` et `reqwest→quinn`

## Lint

```
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Tous les deux passent au strict.

## Pistes d'optimisation (si nécessaire)

Aucune n'est critique vu les chiffres ci-dessus, mais si on cherchait à réduire le binaire :

1. **Compiler en LTO** (`[profile.release] lto = "thin"`) — typiquement -10 à -15%
2. **`panic = "abort"`** dans release — économise ~500 KB de tables d'unwind
3. **`strip = true`** dans `[profile.release]` — élimine les symboles automatiquement (gain 20%)
4. **Désactiver features inutilisées** dans `image` (déjà `default-features = false`)
5. **`reqwest` sans gzip/brotli si on n'a besoin que de JSON** — économise brotli (~120 KB)

Pour les perfs runtime, le seul "hot path" identifiable serait la liste d'apps dans
`security/apps.rs::list_packages` (ADB shell call). C'est dominé par les ms d'I/O ADB,
pas par notre code.
