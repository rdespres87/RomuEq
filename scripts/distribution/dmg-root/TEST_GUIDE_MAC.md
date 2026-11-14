# Equaform-8 — macOS DAW Plugins Beta Testing Guide

Welcome to the **Equaform-8** beta test! This guide walks you through the installation, DAW rescan procedures, security/Gatekeeper handling, and testing checklist for macOS.

> **System Requirements:** Apple Silicon Mac (M1/M2/M3/M4) running macOS 12.0 Monterey or later. (This beta package compiles thin `arm64` binaries optimized for Apple Silicon).

---

## 1. 3-Minute QuickStart

1. **Mount the disk image:** Double-click `Equaform-8-Plugins-v1.0.0.dmg`.
2. **Launch the Installer:** Double-click `Equaform-8-Plugins-Installer.pkg`.
   - *If blocked by macOS Gatekeeper, see Section 2 below.*
3. **Open your DAW:** Start your digital audio workstation (Logic Pro, Ableton Live, Bitwig Studio, Reaper, Cubase) and insert **Equaform-8** on any track.

---

## 2. macOS Gatekeeper & Security Notice

Because this is a private beta build without a commercial Apple Developer ID notarization, macOS security will flag the installer package as unverified on first open.

### Method A: macOS 13 (Ventura) & macOS 14 (Sonoma)
1. **Right-click** (or hold `Control` and click) on `Equaform-8-Plugins-Installer.pkg`.
2. Select **Open** from the context menu.
3. In the warning dialog that appears, click the **Open** button.

### Method B: macOS 15 (Sequoia)
1. Attempt to open `Equaform-8-Plugins-Installer.pkg` normally (it will be blocked).
2. Open **System Settings** > **Privacy & Security**.
3. Scroll down to the **Security** section.
4. You will see: *"Equaform-8-Plugins-Installer.pkg was blocked from use because it is not from an identified developer"*.
5. Click **Open Anyway** and confirm with your password or Touch ID.

### Method C: One-Click Helper Script
Double-click `Open-Installer-Gatekeeper.command` located at the root of the DMG. It will automatically strip the quarantine flag from the installer and launch it immediately.

> **Note on Plugin Quarantine:** The installer package automatically removes all quarantine flags from the installed plugins (`.component`, `.vst3`, `.clap`) inside `/Library/Audio/Plug-Ins/`. Your DAW will **not** block them once installed.

---

## 3. Installed Components & Paths

The installer deploys to industry-standard system directories:

| Component | Format | Installation Path | Target DAWs |
| :--- | :--- | :--- | :--- |
| **Equaform-8.component** | Audio Unit v2 (AUv2) | `/Library/Audio/Plug-Ins/Components/` | Apple Logic Pro, GarageBand, Ableton Live |
| **Equaform-8.vst3** | VST3 | `/Library/Audio/Plug-Ins/VST3/` | Ableton Live, Steinberg Cubase, Studio One, Reaper |
| **Equaform-8.clap** | CLAP | `/Library/Audio/Plug-Ins/CLAP/` | Bitwig Studio, Reaper, FL Studio |
| **Factory Presets (15)** | `.req` State Files | `~/.equaform/presets/`<br/>`/Library/Application Support/Equaform/presets/` | All Equaform-8 formats |

---

## 4. DAW Rescan & Verification Procedures

### Apple Logic Pro (AUv2)
1. Launch Logic Pro. Logic should automatically scan the new component.
2. If it does not appear under **Audio Units > Equaform Audio > Equaform-8**:
   - Go to **Logic Pro** > **Settings** (or **Preferences**) > **Plug-in Manager...**
   - In the search bar at the top-right, type `Equaform`.
   - Select the row for `Equaform-8` (Manufacturer: `Eqfm`, Type: `aufx`).
   - Click **Reset & Rescan Selection** at the bottom left.
   - Verify that the Compatibility column shows **successfully validated**.

#### Terminal Validation (Logic / CoreAudio):
You can test the Audio Unit directly using Apple's official validator tool:
```bash
auval -v aufx Eqf8 Eqfm
```
Look for `* * PASS` at the end of the validation output.

---

### Ableton Live (VST3 or AUv2)
1. Open Ableton Live.
2. Open **Preferences** (`Cmd + ,`) and navigate to the **Plug-Ins** tab.
3. Ensure the following settings are turned **ON**:
   - **Use Audio Units v2**
   - **Use VST3 Plug-In System Folders**
4. If Equaform-8 does not appear in your plugin list:
   - Hold down the **Option (Alt)** key and click **Rescan** (this forces a deep rescan of all plugins).
5. In the browser under **Plug-ins**, find:
   - `VST3 > Equaform Audio > Equaform-8` OR
   - `Audio Units > Equaform Audio > Equaform-8`.

---

### Bitwig Studio (CLAP or VST3)
*Note: Bitwig Studio supports CLAP natively with full parameter modulation and process isolation.*
1. Go to **Settings** > **Locations** > **Plug-in Locations**.
2. Click **Rescan**.
3. In your device browser, search for `Equaform-8`.
4. Prefer the **CLAP** version for lowest latency and best CPU efficiency.

---

### Cockos Reaper (CLAP, VST3, AUv2)
1. Open **Preferences** (`Cmd + ,`) > **Plug-ins** > **VST** (or **CLAP**).
2. Click **Re-scan...** > **Clear cache and re-scan**.
3. Insert an FX on any track and search for `Equaform-8`.

---

### Steinberg Cubase / Nuendo (VST3)
1. Go to **Studio** > **VST Plug-in Manager**.
2. Click the **Rescan All** icon (circular arrows).
3. Check the **Equalizers** or **Other** category for `Equaform-8`.

---

## 5. Factory Presets

Equaform-8 comes bundled with 15 engineered factory presets covering standard mixing, mastering, voice, and electronic music use cases:

1. **Flat** — Default pristine response (all bands at 0 dB)
2. **Bass Boost (+6 dB)** — Low-end reinforcement for modern genres
3. **Electronic / EDM** — Punchy sub + crisp air curve
4. **Rock / Metal** — Tight bass, scooped low-mids, bite at 3 kHz
5. **Hip-Hop / R&B** — Warm low-end, scooped boxiness, smooth highs
6. **Jazz / Warmth** — Gentle musical warming curve
7. **Modern Pop** — Controlled proximity, glossy top end
8. **Acoustic Clarity** — High-pass rumble filter, sparkle at 10 kHz
9. **Vocal Air** — Gentle high-shelf presence for lead vocals
10. **Podcast / Voice** — Mud reduction at 300 Hz, speech intelligibility boost
11. **Cinema / Dialog** — Balanced dialog equalization
12. **Gaming / FPS** — Footstep clarity & spatial awareness curve
13. **Audiophile (Harman)** — Target tuning inspired by the Harman target curve
14. **Night Listening** — Dynamic range taming (compressed perceptual bass/highs)
15. **Loudness** — Equal-loudness contour (Fletcher-Munson compensation)

Presets can be switched from the top preset selector inside the plugin UI. You can also save your own custom presets using the **Save Preset** button.

---

## 6. Tester Feedback Checklist

Please test the following aspects and share your notes with us:

### A. Interface & Visuals
- [ ] Does the UI open cleanly without graphical glitches or flickering?
- [ ] Is text and curve rendering sharp on Retina / High-DPI screens?
- [ ] Does resizing the plugin window work smoothly (if supported by your DAW)?

### B. EQ Curve & Parameter Interaction
- [ ] Can you select and drag frequency/gain handles on the spectrum display?
- [ ] Does adjusting Q via mouse wheel or right-click feel intuitive?
- [ ] Do the band toggle buttons (Bypass, Solo, Filter Types: Bell, Low Shelf, High Shelf, High Pass, Low Pass, Notch) respond accurately?

### C. Sound & DSP Quality
- [ ] Is the audio output clean, transparent, and artifact-free?
- [ ] Does sweeping narrow notch or boost bands introduce any zipper noise or clicks?
- [ ] Does the global **Auto-Gain** feature keep perceived volume balanced when applying heavy boosts?

### D. DAW Integration & Recall
- [ ] Does saving your DAW project and reopening it restore all band frequencies, gains, and Q values accurately?
- [ ] Can you automate parameters (e.g., Band 1 Gain or Frequency) in your DAW timeline?
- [ ] Does preset switching occur seamlessly without audio pops?

### E. CPU & Performance
- [ ] What is the average CPU usage on your track?
- [ ] Does duplicating the plugin across 8+ tracks remain lightweight and stable?

---

## 7. Troubleshooting & FAQ

#### Q: Logic Pro validation failed or didn't show the plugin.
Run this terminal command to test the AUv2 directly:
```bash
auval -v aufx Eqf8 Eqfm
```
If it reports cached state, run:
```bash
touch /Library/Audio/Plug-Ins/Components
killall -9 AudioComponentRegistrar
```
Then restart Logic Pro.

#### Q: How do I cleanly uninstall Equaform-8?
Double-click `Uninstall-Equaform-Plugins.command` on the DMG. It will remove the components and system presets cleanly. Your personal presets in `~/.equaform/presets/` will remain safe.

---

*Thank you for being part of the Equaform-8 beta program!*
