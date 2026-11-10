# Deep Review — RomuEq

## CRITICAL: Audio Output Chopping (BlackHole → Hardware)

### C1. Sample Rate Mismatch entre Input et Output (ROOT CAUSE PRINCIPALE)

**Fichier:** `crates/romueq-gui/src/audio.rs`, lignes 346-347 (output) vs 464-468 (input)

L'output stream récupère son sample rate depuis le device hardware (`supported_config.sample_rate().0`). L'input stream est forcé au même sample rate (ligne 466: `sample_rate: cpal::SampleRate(sample_rate as u32)`).

**Problème:** Si BlackHole tourne à 48000 Hz (sa fréquence native par défaut) et que la Komplete Audio 6 MK2 tourne à 44100 Hz, CPAL va:
- Créer l'output à 44100 Hz
- Forcer l'input BlackHole à 44100 Hz (mismatch avec sa fréquence native)
- CPAL fait du resampling interne, ce qui introduit du jitter et des artefacts
- Le jitter buffer tente de compenser mais ne gère pas un mismatch de fréquence fondamentale

**Le guard** (`macos_audio.rs` ligne 281-283) synchronise BlackHole avec le device physique, mais:
1. Le sleep de 1500ms après `killall -9 coreaudiod` est insuffisant (coreaudiod peut prendre 2-5 secondes)
2. La synchronisation se fait AVANT que les streams CPAL ne soient créés, mais si le user change de device entre l'activation du guard et le start du stream, le mismatch réapparaît

**Fix:** Vérifier que `get_device_sample_rate(blackhole_id) == output_sample_rate` au moment du `start()`, et refuser de démarrer si mismatch. Ou mieux: forcer BlackHole à la fréquence du device de sortie ET vérifier que CPAL accepte ce rate.

### C2. Jitter Buffer — Drift Compensation Trop Aggressive

**Fichier:** `crates/romueq-gui/src/audio.rs`, lignes 111-121

```rust
let rate = if cur_len > 4096 {
    1.02
} else if cur_len > Self::MAX_FRAMES {
    1.005
} else if cur_len < 1024 {
    0.98
} else if cur_len < Self::MIN_FRAMES {
    0.995
} else {
    1.000
};
```

Les sauts de 1.0 à 1.02 (ou 0.98) créent des discontinuités de taux de lecture. Avec un buffer de 2048 frames TARGET, le buffer peut osciller entre les seuils à chaque block, causant des variations brutales de vitesse de lecture → "chops" perceptibles.

**Le problème fondamental:** Le jitter buffer utilise une interpolation linéaire (lignes 128-132) avec un read pointer fractionnaire, MAIS les corrections de rate sont en paliers discrets au lieu d'être une courbe continue. Un drift graduel entre deux fréquences d'horloge différentes devrait être compensé par un rate progressif, pas par des paliers.

**Fix:** Remplacer les paliers par un contrôleur PID ou une moyenne mobile du drift. Par exemple: mesurer le delta entre `push_count` et `pop_count` sur N blocks et calculer un rate continu.

### C3. Lock Contention dans le Callback Audio

**Fichier:** `crates/romueq-gui/src/audio.rs`, lignes 620-630, 657-662, 668-682

Le callback output CPAL effectue:
1. `try_lock()` sur `internal` (ligne 621) → si échec, remplit de zéros
2. `jitter_buffer.lock()` (ligne 657) → bloque si l'input callback tient le lock
3. `internal.lock()` (ligne 668) → bloque à nouveau

L'input callback (lignes 474-485) tient aussi `jitter_buffer.lock()`. Sur macOS avec CPAL, les callbacks input et output peuvent se chevaucher. Si l'input tient le lock du jitter buffer pendant que l'output attend, le callback output peut être retardé → CPAL signale un underrun → silence.

**Le `try_lock()` ligne 621** est particulièrement dangereux: si l'input callback ou le GUI thread tient le lock, l'output remplit le buffer avec des zéros → coupure audible.

**Fix:** Utiliser un lock-free ring buffer pour le jitter, ou au minimum un `Mutex` avec une politique de fallback qui hold/skip plutôt que zéro.

### C4. Channel Count Mismatch — Silently Returns False

**Fichier:** `crates/romueq-engine/src/lifecycle.rs`, ligne 364-366

```rust
if inputs.len() != self.channels {
    return false;
}
```

**Fichier:** `crates/romueq-gui/src/audio.rs`, lignes 671-680

Le code construit TOUJOURS `in_slices: [&[f32]; 2]` (stéréo) et `out_slices: [&mut [f32]; 2]`, mais si le device BlackHole input est détecté comme mono (1 channel), CPAL appelle le callback avec des slices de taille différente. Le code pad le channel manquant (ligne 480-482), donc les slices sont toujours de taille 2... mais si le device rapporte un channel count différent de 2 au niveau de CPAL, le buffer `data` peut avoir une taille incompatible.

De plus, dans `crates/romueq-gui/src/lib.rs` ligne 1787:
```rust
let channels = 2; // stereo default
```
C'est hardcoded. Si le device de sortie a un channel count différent (ex: 6 channels pour la Komplete Audio), la conversion interleaved→planar ne couvre que les 2 premiers channels.

**Fix:** Récupérer le channel count réel du device et l'utiliser partout, pas de hardcoded `2`.

---

## HIGH PRIORITY

### H1. Coefficient Recalculation Every Block

**Fichier:** `crates/romueq-engine/src/lifecycle.rs`, lignes 491-498

`compute_coefficients()` est appelé à CHAQUE block processing pour CHAQUE bande active. Pour 8 bands × N blocks/sec (48000/512 = ~93 blocks/sec), ça fait 744 appels/sec à `compute_coefficients()`.

Les coefficients ne changent que quand les paramètres changent (via UI), pas à chaque block. Cette recalculation est un gaspillage CPU significatif.

**Fix:** Recalculer les coefficients uniquement quand `set_parameters()` est appelé, les stocker dans le struct, et les réutiliser dans `process_dsp()`.

### H2. Bypass Transition Bug — Direction Inversée

**Fichier:** `crates/romueq-engine/src/lifecycle.rs`, lignes 546-552

```rust
// When bypassing (prev_bypass=false → curr_bypass=true):
//   fade_out = processed signal, fade_in = dry (input) signal.
```

La logique inverse fade_in/fade_out quand on passe de bypass=false à bypass=true:
- `fade_out = cos(t)` diminue de 1→0 → le signal processed diminue
- `fade_in = sin(t)` augmente de 0→1 → le signal dry augmente

C'est CORRECT pour le cas bypass=false→true. Mais pour bypass=true→false (réactivation):
- `fade_out = cos(t)` diminue → le signal processed DIMINUE (c'est l'inverse de ce qu'il faut)
- `fade_in = sin(t)` augmente → le signal dry AUGMENTE (c'est l'inverse)

La même formule est utilisée dans les deux sens, mais elle devrait être inversée.

**Fix:** Ajouter un flag `is_bypassing` et inverser fade_in/fade_out selon le sens de transition.

### H3. Automation Frequency Interpolation Bug

**F fichier:** `crates/romueq-engine/src/lifecycle.rs`, ligne 484

```rust
last_freq = freq_min.powf(t) * (freq_max / freq_min).powf(t);
```

Mathématiquement: `freq_min^t * (freq_max/freq_min)^t = freq_min^t * freq_max^t / freq_min^t = freq_max^t`.

Ce n'est PAS une interpolation log entre `freq_min` et `freq_max`. C'est `freq_max^t` où `t ∈ [0,1]`, ce qui donne toujours une fréquence très proche de `freq_max` (10kHz/2 = 24000 Hz pour t=1, et 20^0.5 ≈ 4.47 pour t=0).

**Fix:** Utiliser `freq_min.powf(t) * freq_max.powf(1.0 - t)` pour une interpolation log correcte.

### H4. `process()` Return Value Ignored

**Fichier:** `crates/romueq-gui/src/audio.rs`, ligne 680

```rust
let _ = g.engine.process(&in_slices, &mut out_slices);
```

Le return value de `engine.process()` est ignoré. Si `process()` retourne `false` (channel mismatch, bad block size, engine not ready), le buffer de sortie n'est pas modifié et contient des données périmées du block précédent.

**De même dans:** `crates/romueq-gui/src/lib.rs` ligne 1793:
```rust
let _ = eng.process(&input_planar, &mut output_planar);
```

**Fix:** Si `process()` retourne false, remplir les output avec des zéros ou le dernier output connu.

---

## MEDIUM PRIORITY

### M1. Lock Contention — Snapshot et Analysis dans le Callback

**Fichier:** `crates/romueq-gui/src/lib.rs`, lignes 1097-1098

```rust
let _snapshot = self.client.snapshot();
let mut analysis = self.client.analysis_snapshot();
```

Ces appels acquièrent des locks sur `EngineClient` qui contient `engine: Option<Box<EngineLifecycle>>`. Le GUI thread et le audio thread accèdent tous les deux au engine via des locks. Si le audio callback tient le lock pendant que le GUI thread attend, le GUI freeze (et inversement).

### M2. Bypass Buffer Indexing dans `process_dsp`

**Fichier:** `crates/romueq-engine/src/lifecycle.rs`, ligne 440

```rust
self.bypass_buffer[buffer_offset..buffer_offset + block_size].copy_from_slice(samples);
```

`buffer_offset = ch_idx * self.max_block_size`. `samples` est `&mut self.buffer[buffer_offset..buffer_offset + block_size]`. C'est correct tant que `block_size <= max_block_size`. Mais si `block_size` est proche de `max_block_size` et que `ch_idx` est élevé, il n'y a pas de bounds check explicite (les borrows Rust gèrent ça, mais c'est fragile).

### M3. Hardcoded Values

Plusieurs valeurs magiques devraient être des constantes nommées:
- `channels = 2` dans `gui/lib.rs:1787`
- `block_size = 4096` dans `audio.rs:350`
- `TARGET_FRAMES = 2048` dans `audio.rs:64`
- `num_bins = 128` dans `gui/lib.rs:1836`
- `MAX_BANDS` utilisé directement au lieu d'une constante locale

### M4. Memory Layout — EngineLifecycle Est Gros

**Fichier:** `crates/romueq-engine/src/lifecycle.rs`, lignes 135-139

```rust
buffer: [f32; FLAT_BUFFER_SIZE],      // 8 * 8192 * 4 = 262144 bytes
bypass_buffer: [f32; FLAT_BUFFER_SIZE], // 262144 bytes
```

Le struct fait ~526 KiB inline. C'est alloué dans `EngineClient` qui est dans `StandaloneRunner` qui est dans le GUI thread. Si le GUI thread fait du locking fréquent, ce gros struct passe par des clones (`*eng.eq_parameters()` ligne 1779) qui copient ~200 bytes à chaque block.

### M5. Input Stream Channel Handling

**Fichier:** `crates/romueq-gui/src/audio.rs`, lignes 476-484

```rust
let l = frame.first().copied().unwrap_or(0.0);
let r = if in_channels > 1 {
    frame.get(1).copied().unwrap_or(l)
} else {
    l
};
```

Si `in_channels == 1`, `r = l`. Le jitter buffer reçoit du stéréo symétrique même pour une source mono. Ce n'est pas un bug en soi, mais c'est inutile et peut masquer un problème de configuration device.

---

## LOW PRIORITY / CODE QUALITY

### L1. French Strings in Error Messages

Plusieurs messages d'erreur sont en français alors que la convention du projet est l'anglais:
- `macos_audio.rs:73`: `"Échec de configuration du périphérique..."`
- `macos_audio.rs:220`: `"Impossible d'écrire le paquet..."`
- `macos_audio.rs:223`: `"RomuEq a besoin d'installer..."`
- `audio.rs:531`: `"Aucun périphérique d'entrée loopback..."`
- `gui/lib.rs`: nombreux messages UI en français

### L2. Dead / Unused Code

- `processing_mode_to_f32` dans `gui/lib.rs:1617` retourne toujours `0.0`
- `stereo_mode_to_f32` dans `gui/lib.rs:1622` retourne toujours `0.0`
- `_spectrum_processor` dans `EngineClient` (gui/lib.rs:1509) — préfixé `_` car non utilisé
- `SpectrumProcessor` dans `StandaloneRunner` (gui/lib.rs:1005) — créé mais usage limité

### L3. `#[allow(dead_code)]` Abuse

**Fichier:** `crates/romueq-engine/src/lifecycle.rs`, lignes 272, 297, etc.

Plusieurs méthodes publiques ont `#[allow(dead_code)]` car elles ne sont pas appelées depuis d'autres crates. C'est acceptable pour une crate lib, mais certaines de ces méthodes (comme `query_automation`) ne sont appelées nulle part.

### L4. Test Coverage Gaps

- Aucun test pour le jitter buffer avec drift réel (push rate != pop rate)
- Aucun test pour la transition bypass avec signal non-nul
- Aucun test pour le cas où `process()` retourne false
- Les tests de `process_dsp` n'utilisent que des signaux DC (tous les échantillons identiques)

### L5. No Real-Time Safety Verification

Le commentaire `lifecycle.rs:28` dit "process() performs no allocations, no syscalls, no blocking." Mais:
- `compute_coefficients()` (appelé dans `process_dsp`) utilise des fonctions mathématiques (`sin`, `cos`, `pow`, `sqrt`) qui peuvent varier en temps d'exécution
- `AutomationIngestor::process_block()` peut faire des allocations internes
- `bincode::serialize/deserialize` (lignes 660, 665) fait des allocations

---

## ARCHITECTURE OBSERVATIONS

### A1. Double Engine Instance

L'application maintient DEUX instances de `EngineLifecycle`:
1. `EngineClient.engine` (GUI thread) — initialisé à 48kHz, 512 blocks
2. `AudioInternal.engine` (audio thread via CPAL) — initialisé séparément

Les paramètres sont synchronisés manuellement entre les deux (`audio.set_parameters()` ligne 1128, 1161, 1395). Si la synchronisation rate (lock timeout, race condition), le GUI montre des paramètres différents de ce qui est traité.

### A2. ParamSnapshot Redondant

`ParamSnapshot` est recréé à chaque block dans `process()` (gui/lib.rs:1802) via `encode_snapshot()`, qui parcourt tous les bands et encode chaque paramètre. C'est un gaspillage CPU dans le callback GUI.

### A3. Command Batch Processé 3 Fois

Dans `StandaloneRunner::update()` (gui/lib.rs):
1. Ligne 1121-1131: batch appliqué + params sync audio
2. Ligne 1390-1398: batch appliqué + params sync audio (dans le panel render)

Le même batch est appliqué deux fois, et les paramètres sont synchronisés avec l'audio stream deux fois.

---

## RECOMMANDATIONS PRIORITAIRES

1. **Fix sample rate mismatch** — Le plus probable responsable du "son haché". Ajouter une vérification stricte et un log explicite.
2. **Fix jitter buffer drift compensation** — Remplacer les paliers par une courbe continue.
3. **Fix lock contention** — Lock-free buffer ou fallback sans zéros.
4. **Fix bypass transition direction** — Inverser fade_in/fade_out pour bypass=true→false.
5. **Cache coefficients** — Calculer uniquement sur changement de paramètre.
6. **Handle `process()` return value** — Ne pas ignorer false.
