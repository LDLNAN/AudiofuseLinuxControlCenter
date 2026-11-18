# AudioFuse Linux Control Center

A native, open-source Linux replacement for the **Arturia AudioFuse 8Pre** Control Center, which is officially only available on Windows and macOS.

## Project Purpose & Scope

While the Arturia AudioFuse 8Pre (`1c75:af0e`) enumerates as a USB Audio Class 2.0 (UAC2) interface on Linux (`snd-usb-audio`), standard ALSA mixer controls only expose the four clock-source validity and selector entities. All hardware channel strip controls, cue mixing, output routing, metering, and device configuration require Arturia's vendor-specific USB control protocol—leaving Linux users without access to the interface's core studio functionality unless they boot a Windows/macOS host or pass the USB device into a VM.

**AudioFuse Linux Control Center** aims to provide a complete, first-class Linux replacement with:

1. **Full Feature Parity with the Windows AudioFuse Control Center** for the AudioFuse 8Pre:
   - **8 Analog Input Channel Strips**: Mic/Line/Instrument source selection (`In 1–2` Instrument Hi-Z), +48V Phantom Power (`1–4` / `5–8` and per-channel indicators), Phase Invert (`Ø`), Line Pad (`-20 dB`), and Mic Pad/Boost (`OFF` / `-20 dB PAD` / `BOOST`).
   - **Hardware Low-Latency Cue Mixer**: 24 mixer channels (8 Analog In, 8 ADAT In, 8 USB Playback) with per-channel Fader (`-61 dB` to `+12 dB`), Pan, Mute, Solo, Peak indicator, custom 16-character channel naming (persisted to device memory), channel grouping (`Groups 1–4`), and stereo linking (`1–2`, `3–4`, `5–6`, `7–8`).
   - **Master Cue & Monitor Section**: Direct/Computer Mix balance, Master Cue Output Level/Mute/Solo, Speakers routing (`Main 1-2` / `Cue Mix`) + Mono sum, and Headphones routing (`Main 1-2` / `Cue Mix` / `Follow Spkrs`) + Mono sum.
   - **Output & Loopback Routing Matrix**: Independent stereo pair routing for `Analog Out 1–2`, `3–4`, `5–6`, `7–8` (`Cue Mix`, `USB`, `Analog In`, `ADAT In`), `ADAT Out 1–2`, `3–4`, `5–6`, `7–8` (`Cue Mix`, `USB 9–16`, `Analog In 1–8`), and recording `Loopback` (`Disabled`, `Cue Mix`, `USB 17–18`).
   - **Clocking & Hardware Configuration**: Device Mode (`USB` interface vs. standalone `ADAT` 8-channel expander), Sample Rate (`44.1` / `48` / `88.2` / `96 kHz`), Clock Source (`Internal`, `Word Clock`, `ADAT`) with lock status, Word Clock `75 Ω` termination (`On` / `Hi-Z`), Word Clock Thru (`Off` / `On`), ADAT Enable, and S/MUX mode (`Normal` / `Double Speed`).
   - **Real-Time Hardware VU Metering**: Live 78-byte bulk state / VU polling across all Analog, ADAT, USB, Cue Mix, Speaker, Headphone, and Loopback channels.
   - **Device Presets & Persistence**: On-device preset load/save (`PresetLoad` / `PresetSave`), local preset file import/export, and CLI/daemon state restoration on boot/hotplug.
   - **Firmware Inspection & Update Support**: Firmware version readout (`bcdDevice` / `.af8pre` archive inspection) and USB DFU (Interface 3) update workflow.
2. **Universal Linux Distribution Packaging**:
   - Portable standalone binaries and cross-distro distribution formats (`AppImage`, `Flatpak`, `.deb`, `.rpm`, Arch `PKGBUILD`) alongside non-root `udev` rules (`1c75:af0e`) so any Linux studio setup works out of the box without root privileges.
   - Both a full desktop GUI and a scriptable CLI (`audiofuse-cli`) for headless/PipeWire/ALSA session automation.
3. **Defined Project Lifecycle**:
   - **End-Goal**: 100% feature parity with the official Windows AudioFuse Control Center for the AudioFuse 8Pre.
   - **Post-Parity Maintenance**: Once feature parity and cross-distro packaging are complete, feature development is finished; only bug fixes and kernel/distro compatibility fixes will be worked on if necessary.

---

## UI / UX Philosophy (No "Generic AI" UI)

Like [mousetool](https://github.com/LDLNAN/mousetool), this project enforces a strict **hardware-utility studio design language** and explicitly rejects "Generic AI" UI patterns:

- **Utilitarian Studio Hardware Density**: All channel strips, routing selectors, and meters are arranged like physical studio rack gear and mixing consoles—compact, high-density, scannable at a glance, with zero wasted viewport space.
- **No AI Visual Tropes**:
  - No neon purple/cyan gradients, glassmorphism, floating decorative cards, or glowing drop shadows.
  - No oversized hero banners, chat-like onboarding wizards, rounded pill cards, or emojis in the interface.
  - Crisp `1px solid` structural borders, tight `3px–4px` corner radii, flat hardware surface hierarchy (`#141413` carbon dark / `#f6f5f2` warm studio light), and purposeful hardware indicator colors (`#e05d30` / `#c84b20` active state / +48V, standard green/amber/red dBFS metering).
- **Precision Typography**: System geometric sans for compact labels paired with tabular monospace (`JetBrains Mono` / `monospace` with `tabular-nums`) for all dB readouts, sample rates, pan positions, and channel indices so values never jitter during live metering.

---

## Hardware & Protocol Summary (`Arturia AudioFuse 8Pre`)

| Property | Value |
| :--- | :--- |
| **USB Vendor / Product ID** | `1c75:af0e` (`Arturia AudioFuse 8Pre`) |
| **Interface 0** | USB Audio Class 2.0 Control + Vendor Control Transfers (`ep_82` interrupt IN) |
| **Interface 1 & 2** | UAC2 Audio Streaming OUT (`ep_01`, 18 ch 24-bit) & IN (`ep_81`, 18 ch 24-bit) |
| **Interface 3** | USB DFU (`bInterfaceClass 0xfe`, `bInterfaceSubClass 0x01`, 64-byte transfer size) |
| **UAC2 Clock Entities** | Clock Selector `40` (`0x28`): `41` (`0x29` Internal), `43` (`0x2B` ADAT), `44` (`0x2C` Word Clock); Extension Unit `30` (`0x1E`) |
| **Vendor Control Unit** | `AFEXDevice` unit ID `0x0a` (`10`), 16-bit parameter values (`2` bytes), `0x4e` (`78` bytes) bulk state/VU payload |

See [`docs/PROTOCOL.md`](./docs/PROTOCOL.md) for the reverse-engineered parameter map, USB control transfer specification, and UI layout mapping.

---

## Repository Structure

- [`docs/PROTOCOL.md`](./docs/PROTOCOL.md) — Reverse-engineered Arturia AudioFuse 8Pre (`AFEXDevice`) USB protocol, parameter definitions, scaling curves, and GUI structure.
- [`AGENTS.md`](./AGENTS.md) — Engineering, UI/UX, hardware testing, and verification guidelines for contributors and autonomous agents.
- `Original/` *(git-ignored)* — Local-only directory containing original Windows/macOS installers and `.af8pre` firmware archives used for clean-room protocol analysis.

## License

MIT
