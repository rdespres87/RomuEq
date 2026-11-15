# AUDIT ROMUEQ — Plan d'Actions

Projet : RomuEq — 9 crates, 25 642 LOC, 157 tests, build clean, clippy clean.
Date : 2026-11-15

---

## P0 — BLOQUANT (fixer avant toute autre chose)

### A2-1 : Remplacer `mpsc::sync_channel` par un ring buffer lock-free dans le callback audio

**Fichier** : `crates/equaform-gui/src/audio.rs`
**Lignes** : 16, 524, 619

**Problème** : `std::sync::mpsc::SyncSender<EqParameters>` avec `sync_channel(256)` est utilisé dans le closure du callback CPAL. Si le canal est plein, le thread audio bloque → clics, crackling, xrun.

**Actions :**
1. Remplacer le `sync_channel` par l'`SpscRingBuffer` déjà existant dans `equaform-common/src/lib.rs`.
2. Dans le closure du callback, remplacer `param_rx.recv()` par `param_rx.try_dequeue()` avec fallback sur les paramètres courants (`self.delivered_params`).
3. Ajouter un compteur atomique `dropped_param_updates` pour le telemetry.
4. Vérifier que le drain du ring buffer se fait sur le thread GUI, pas sur le callback.

**Vérification après** : `cargo test --workspace` + test de charge audio (1h de playback sans xrun).

---

### A2-2 : Supprimer `eprintln!` des handlers d'erreur du callback audio

**Fichier** : `crates/equaform-gui/src/audio.rs`
**Lignes** : 606, 707, 724, 734, 741

**Problème** : `eprintln!` est appelé dans le callback audio CPAL en cas d'erreur. I/O synchrone = risque de xrun.

**Actions :**
1. Remplacer chaque `|err| eprintln!(...)` par un closure qui incrémente un `AtomicUsize` de compteur d'erreurs.
2. Exposer ce compteur via un `Arc<AtomicUsize>` partagé avec la GUI pour affichage dans le UI.
3. Ou utiliser le ring buffer de logs existant si un tel mécanisme existe.

**Vérification après** : `cargo clippy --workspace` + test que les erreurs audio sont toujours trackées (compteur atomique).

---

## P1 — IMPORTANT

### A1-1 : Ajouter `equaform-standalone` au workspace

**Fichier** : `Cargo.toml`
**Ligne** : 3

**Problème** : `equaform-standalone` n'est pas dans `members = [...]` du workspace. Donc `cargo test --workspace` et `cargo clippy --workspace` ne le couvrent pas.

**Actions :**
1. Ajouter `"crates/equaform-standalone"` à la liste `members` dans `Cargo.toml`.
2. Vérifier que `cargo test --workspace` passe.
3. Vérifier que `cargo clippy --workspace` passe.

---

### A1-2 : Corriger `#[allow(unsafe_code)]` dans `audio.rs`

**Fichier** : `crates/equaform-gui/src/audio.rs`
**Ligne** : 970

**Problème** : Le crate a `#![deny(unsafe_code)]` mais la section FFI macOS fait `#[allow(unsafe_code)]` sur le module entier. Si ce module est compilé (feature `standalone`), le `deny` est neutralisé pour tout le crate.

**Actions :**
1. Remplacer `#[allow(unsafe_code)]` par `#[allow(unsafe_code)]` uniquement sur les fonctions FFI individuelles (pas sur le module).
2. Ou déplacer la section FFI dans un module séparé `ffi` avec son propre `mod ffi` et appliquer `#[allow(unsafe_code)]` uniquement sur ce sous-module.
3. Vérifier que `#![deny(unsafe_code)]` reste actif sur tout le reste.

---

### A1-3 : Refactor `process_output_block` — 17 arguments

**Fichier** : `crates/equaform-gui/src/audio.rs`
**Ligne** : 820

**Problème** : `process_output_block` a 17 arguments (`#[allow(clippy::too_many_arguments)]`). C'est une god function qui fait drift compensation, DSP, meters, spectrum.

**Actions :**
1. Créer un struct `AudioProcessorContext` regroupant : `engine`, `param_rx`, `audio_consumer`, `drift`, `is_running`, `source_mode`, `peak_l`, `peak_r`, `spectrum_samples`, `phase`, `left_in`, `right_in`, `left_out`, `right_out`.
2. Remplacer les 17 arguments par `&mut AudioProcessorContext`.
3. Ajouter un `#[inline]` sur la fonction.
4. Vérifier que `cargo clippy` ne signale plus `too_many_arguments`.

---

## P2 — AMÉLIORATION

### A2-1 : Unifier les 3 ring buffers lock-free

**Fichiers** : `crates/equaform-common/src/lib.rs` (SpscRingBuffer, DoubleBufferedSpsc), `crates/equaform-common/src/audio_ring.rs` (AudioRingProducer/Consumer)

**Problème** : 3 implémentations de ring buffer lock-free dans le même projet. Maintenance triple, risque de divergence.

**Actions :**
1. Garder `SpscRingBuffer<T>` de `common/src/lib.rs` comme implémentation principale.
2. Adapter `AudioRingProducer`/`AudioRingConsumer` pour utiliser `SpscRingBuffer<(f32, f32)>` au lieu de sa propre logique atomique.
3. Supprimer `DoubleBufferedSpsc` si non utilisé, ou le documenter comme expérimental.
4. Vérifier que tous les tests passent.

---

### A2-2 : Pré-calcul des valeurs d'automation dans `process_dsp`

**Fichier** : `crates/equaform-engine/src/lifecycle.rs`
**Lignes** : 575-610

**Problème** : Double boucle sur `block_size` par band pour interpoler gain ET frequency. O(bands × block_size × 2) appels à `value_at_sample`.

**Actions :**
1. Pré-calculer les valeurs automatisées pour tout le block dans un tableau `[f32; block_size]` avant la boucle samples.
2. Utiliser interpolation linéaire directe sur les clés d'automation.
3. Réduire le nombre d'appels à `value_at_sample` de O(block_size × 2) à O(2 × num_clés).

---

### A2-3 : SIMD pour la cascade biquad

**Fichier** : `crates/equaform-engine/src/lifecycle.rs`
**Lignes** : 648-656

**Problème** : Boucle sample-by-sample avec variables locales empêche l'auto-vectorisation.

**Actions :**
1. Utiliser `std::simd` (nightly) ou `bytemuck` pour aligner les coefficients.
2. Ou utiliser une boucle `chunks_exact(4)` avec des scalaires alignés.
3. Mesurer le gain avec `cargo bench`.

---

### A2-4 : `PresetManager` Vec → HashMap

**Fichier** : `crates/equaform-gui/src/preset_manager.rs`
**Ligne** : 67

**Problème** : `presets: Vec<NamedPreset>` avec recherche linéaire dans `get()`.

**Actions :**
1. Remplacer `Vec<NamedPreset>` par `HashMap<String, NamedPreset>`.
2. Mettre à jour `get()`, `add()`, `remove()`, `iter()`.
3. Vérifier que les tests passent.

---

### A2-5 : Générer `parameter_info()` au lieu du match de 1676 lignes

**Fichier** : `crates/equaform-common/src/parameter.rs`

**Problème** : 51 variantes match explicites, copie-colle propice aux erreurs.

**Actions :**
1. Créer un macro `param_info! { id, name, type, range, unit }` qui génère les entrées du match.
2. Ou utiliser un script de build (`build.rs`) pour générer un fichier `generated_params.rs`.
3. Réduire la taille du match à ~20 lignes de macro calls.

---

## P3 — NETTOYAGE

### A3-1 : Supprimer les `#[allow(dead_code)]` superflus

**Fichiers** : `lifecycle.rs:378,403`, `common/src/lib.rs:66,704,729`, `plugin/src/ext_gui.rs:15`, `plugin/src/lib.rs:989`, `ext_ports.rs:14`, `ext_params.rs:20`, `macos_audio.rs:277,293,307,354`

**Actions :**
1. Pour chaque `#[allow(dead_code)]`, vérifier si le code est réellement mort ou utilisé via une feature/condition.
2. Si utilisé, retirer le allow et activer la feature correspondante dans les tests.
3. Si mort, supprimer le code.

---

### A3-2 : Supprimer les noms de devices hardcodés en français

**Fichier** : `crates/equaform-gui/src/audio.rs`
**Lignes** : 1464, 1492

**Problème** : `"Haut-parleurs MacBook Pro"`, `"Komplete Audio"` — noms macOS en français.

**Actions :**
1. Utiliser les noms retournés par CPAL `device.name()` directement, sans filtre textuel.
2. Ou utiliser une comparaison insensitive à la casse sur le nom brut.

---

### A3-3 : Remplacer `thread::sleep` par `yield_now` dans le polling macOS

**Fichier** : `crates/equaform-gui/src/audio.rs`
**Ligne** : 1135

**Problème** : `sleep(25ms)` dans `sync_virtual_rate_sync` — bloquant.

**Actions :**
1. Remplacer `std::thread::sleep(Duration::from_millis(25))` par `std::thread::yield_now()`.
2. Ajouter un compteur de retries avec timeout (max 10 itérations).

---

### A3-4 : Tests audio — remplacer `sleep` par mocks

**Fichier** : `crates/equaform-gui/src/audio.rs`
**Lignes** : 1238, 1288

**Problème** : Tests de drift compensation utilisent `sleep(11610µs)` et `join()`. Lent et non-déterministe.

**Actions :**
1. Isoler la logique de drift compensation dans un trait `DriftController` avec une implémentation mock.
2. Remplacer `sleep` par des timers simulés dans les tests.

---

### A3-5 : Vérifier l'utilité de `rustfft`

**Fichier** : `crates/equaform-analysis/Cargo.toml`

**Actions :**
1. Vérifier si `rustfft` est utilisé dans le pipeline DSP principal.
2. Si non utilisé, le retirer des dépendances.
3. Si utilisé, mesurer son impact sur le temps de build.

---

### A3-6 : `GuiCommandBatch` et `ParamSnapshot` — recherche linéaire

**Fichier** : `crates/equaform-gui/src/lib.rs`
**Lignes** : 72-80, 132-145

**Problème** : Recherche linéaire dans Vec de taille ≤ 51. Acceptable mais anti-pattern.

**Actions :**
1. Si le nombre de paramètres reste ≤ 51, laisser tel quel (micro-optimisation).
2. Si augmentation prévue, passer à `BTreeMap<ParamId, f32>`.

---

## VÉRIFICATION POST-FIX

Après chaque groupe de fixes P0/P1 :

```bash
cd /Users/rdespres/Projets/RomuEq

# Format
cargo fmt --all -- --check

# Lint
cargo clippy --workspace --all-features

# Build
cargo check --workspace --all-features

# Tests
cargo test --workspace --all-features

# Tree (vérifier que standalone est inclus)
cargo tree --depth 3 --edges normal
```

---

## DÉPENDANCES À SURVÉLER

| Dépendance | Crate | Note |
|------------|-------|------|
| `cpal 0.15.3` | gui, standalone | FFI audio, vérifier version stable |
| `eframe 0.31` | gui | ~500 Mo transitifs, lourd pour MVP |
| `rustfft 6.4` | analysis | Vérifier usage réel |
| `clack-plugin 0.1` | plugin | Bindings CLAP, version mineure |
| `bincode 1.3` | engine | Version 1.x — envisager v2 pour futures compat |
| `serde 1.0` | common, plugin | Standard, pas de souci |

---

## RÉCAPITULATIF DES FICHIERS À MODIFIER

| Fichier | Lignes | Priorité |
|---------|--------|----------|
| `crates/equaform-gui/src/audio.rs` | 16, 524, 606, 707, 724, 734, 741, 820, 970, 1135, 1464, 1492 | P0-P3 |
| `Cargo.toml` | 3 | P1 |
| `crates/equaform-common/src/lib.rs` | 92-401 | P2 |
| `crates/equaform-common/src/audio_ring.rs` | 1-300 | P2 |
| `crates/equaform-engine/src/lifecycle.rs` | 575-610, 648-656 | P2 |
| `crates/equaform-gui/src/preset_manager.rs` | 67 | P2 |
| `crates/equaform-common/src/parameter.rs` | 1-1676 | P2 |
| `crates/equaform-gui/src/workspace_ui.rs` | 73 | P1 |
