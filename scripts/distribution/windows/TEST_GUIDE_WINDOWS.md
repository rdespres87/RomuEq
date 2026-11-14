# Equaform-8 — Windows DAW Plugins Beta Testing Guide

Welcome to the **Equaform-8** Windows beta test! This guide provides comprehensive instructions for installation, DAW plugin rescanning, graphics acceleration, and feedback testing on 64-bit Windows 10 and 11.

---

## 1. System Requirements

- **Operating System:** Windows 10 (Build 1809 or newer) or Windows 11 (64-bit `x64`).
- **Graphics Acceleration:** Dedicated or integrated GPU supporting **DirectX 12** (Feature Level 11_0+), **Vulkan**, or **Direct3D 11**.
- **Display:** 1080p minimum resolution; high-DPI scaling (125%, 150%, 200%) is natively supported via Per-Monitor V2 DPI awareness.
- **DAW Hosts:** Any 64-bit digital audio workstation supporting **VST3** or **CLAP** (e.g., Ableton Live, Bitwig Studio, Reaper, Cubase, FL Studio, Studio One).

---

## 2. 3-Minute QuickStart

1. **Run Installer:** Double-click `Equaform-8-Plugins-Windows-v1.0.0.exe`.
2. **Select Components:** Choose **VST3**, **CLAP**, and **15 Factory Presets**.
3. **Launch DAW:** Open your workstation and insert **Equaform-8** onto an audio or master bus.

---

## 3. Standard Windows Plugin Locations

Equaform-8 installs strictly to industry-standard 64-bit common file locations:

| Format | Target System Path | Supported DAWs |
| :--- | :--- | :--- |
| **VST3** | `C:\Program Files\Common Files\VST3\Equaform-8.vst3\` | Cubase, Ableton Live, Studio One, Reaper, FL Studio |
| **CLAP** | `C:\Program Files\Common Files\CLAP\Equaform-8.clap` | Bitwig Studio, Cockos Reaper, FL Studio |
| **Presets (Shared)** | `C:\ProgramData\Equaform\presets\` | Global reference presets |
| **Presets (User)** | `%APPDATA%\Equaform\presets\` | User-customized presets |

---

## 4. DAW Rescan & Verification Procedures

### Ableton Live (Windows VST3)
1. Open **Preferences** (`Ctrl + ,`) and select the **Plug-Ins** tab.
2. Verify that **Use VST3 Plug-In System Folders** is turned **ON**.
3. If Equaform-8 does not appear in your plugin list:
   - Hold down the `Alt` key and click **Rescan** (forces a deep full rescan).
4. In the Live browser under **Plug-ins > VST3 > Equaform Audio**, load **Equaform-8**.

---

### Bitwig Studio (Windows CLAP & VST3)
*Note: Bitwig Studio supports CLAP natively with full per-voice modulation, fast parameter updates, and crash sandboxing.*
1. Go to **Settings > Locations > Plug-in Locations**.
2. Click **Rescan**.
3. In your device browser, search for `Equaform-8`.
4. Prefer the **CLAP** format for maximum performance.

---

### Cockos Reaper (Windows CLAP & VST3)
1. Open **Preferences** (`Ctrl + P`) > **Plug-ins > VST** (or **CLAP**).
2. Click **Re-scan... > Clear cache and re-scan**.
3. Insert FX on any track and type `Equaform-8`.

---

### Steinberg Cubase / Nuendo (Windows VST3)
1. Go to **Studio > VST Plug-in Manager**.
2. Click the **Rescan All** icon (circular arrows).
3. If blocked or marked red, check the **Blocklist** tab and click **Reactivate**.
4. In your track inspector or channel strip, insert `Equaform-8`.

---

### Image-Line FL Studio (Windows VST3 & CLAP)
1. Go to **Options > Manage Plugins**.
2. Under **Plugin search paths**, ensure `C:\Program Files\Common Files\VST3\` and `C:\Program Files\Common Files\CLAP\` are listed.
3. Turn **ON** `Find installed plugins` and click **Find plugins** (Verify plugins).
4. Once scanned in green/orange, open the mixer and insert `Equaform-8`.

---

### PreSonus Studio One (Windows VST3)
1. Go to **Studio One > Options > Locations > VST Plug-Ins**.
2. Click **Reset Blocklist**, then click **Scan at startup** or **Refresh**.

---

## 5. Factory Presets

Equaform-8 includes 15 built-in factory presets covering essential mixing, mastering, vocal, and sound-design requirements:

1. **Flat** — Neutral response (all 8 bands at 0 dB)
2. **Bass Boost (+6 dB)** — Low-end reinforcement
3. **Electronic / EDM** — Punchy low-end scoop and airy highs
4. **Rock / Metal** — Tight bass, 400 Hz boxiness cut, 3 kHz presence
5. **Hip-Hop / R&B** — Sub warmth with smooth treble roll-off
6. **Jazz / Warmth** — Gentle musical contour
7. **Modern Pop** — Controlled proximity with 10 kHz sheen
8. **Acoustic Clarity** — Rumble high-pass filter and sparkle
9. **Vocal Air** — High-shelf presence for vocal clarity
10. **Podcast / Voice** — Mud reduction at 300 Hz and speech intelligibility
11. **Cinema / Dialog** — Clean dialog clarity
12. **Gaming / FPS** — Footstep clarity and spatial awareness
13. **Audiophile (Harman)** — Reference curve inspired by the Harman target
14. **Night Listening** — Compressed perceptual loudness contour
15. **Loudness** — Fletcher-Munson equal-loudness contour

---

## 6. Tester Feedback Checklist

Please test the following aspects and share your results:

### A. Window & Graphics Acceleration
- [ ] Does the UI open quickly without white flashing or black screen artifacts?
- [ ] Does dragging the window across multiple monitors with different DPI scalings (e.g., 100% and 150%) resize crisply?
- [ ] Are curve animations and spectrum visualizations smooth (target 60 FPS)?

### B. Parameter Interaction
- [ ] Can you grab and move EQ band points using the mouse?
- [ ] Does adjusting the Q-factor via mouse wheel work smoothly?
- [ ] Do filter type dropdowns (Bell, Shelves, High/Low Pass, Notch) switch instantly?

### C. Audio Performance
- [ ] Is audio output clean and transparent without glitches or distortion?
- [ ] Does rapid band automation (gain or frequency sweeps) cause audio dropouts?
- [ ] Does the **Auto-Gain** feature stabilize overall track volume?

### D. DAW Recall & Automation
- [ ] Does saving and reopening your DAW project restore all 8 bands and preset names accurately?
- [ ] Does DAW timeline parameter automation (Master Gain, Band 1 Frequency, etc.) track smoothly?

---

## 7. Troubleshooting & FAQ

#### Q: The plugin window opens with a black screen or fails to render.
- Update your GPU graphics drivers (NVIDIA GeForce, AMD Adrenalin, or Intel Arc/Iris).
- Ensure your GPU supports DirectX 12 or Vulkan.

#### Q: The plugin does not appear after running the installer.
- Verify that your DAW is scanning 64-bit `C:\Program Files\Common Files\VST3\`.
- In your DAW preferences, perform a "Clear cache and rescan".

#### Q: How do I cleanly uninstall Equaform-8?
- Open **Windows Settings > Apps > Installed apps**, find **Equaform-8 DAW Plugins**, and select **Uninstall**.
- Your personal presets in `%APPDATA%\Equaform\presets\` will remain untouched.

---

*Thank you for helping test Equaform-8 on Windows!*
