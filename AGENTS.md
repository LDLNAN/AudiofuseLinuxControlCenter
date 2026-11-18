# Engineering & Agent Guidelines (`AudiofuseLinuxControlCenter`)

## 1. Mission & End-Goal
- Build a complete, native Linux replacement for the **Arturia AudioFuse 8Pre Control Center** (`1c75:af0e`).
- Target **100% feature parity** with the official Windows AudioFuse Control Center, shipped as portable binaries and packages for all Linux distributions (`AppImage`, `Flatpak`, `.deb`, `.rpm`, Arch `PKGBUILD`, plus `udev` rules and CLI).
- Once full feature parity and packaging are verified, the project enters **bug-fix-only** maintenance mode.

## 2. Never Commit `Original/`
- The `Original/` directory contains proprietary vendor installers and firmware binaries for local reverse-engineering and reference only.
- Always verify `git status` before committing to ensure `Original/` or any extracted proprietary binaries are excluded via `.gitignore`.

## 3. UI / UX Design Rules — Strictly No "Generic AI" UI
Follow the exact utilitarian design principles established in `/home/lucasdonald/Documents/GitHub/mousetool`:
- **Hardware Studio Utility Aesthetic**: Dense, console-inspired layout modeled after physical pro-audio gear and compact studio interfaces.
- **Palette & Surfaces**:
  - Dark mode (default for studio use): Carbon `#141413` background, `#1e1e1c` panel surface, `#282826` recessed controls, `#2e2e2b` `1px solid` crisp dividers.
  - Light mode: Warm paper `#f6f5f2` background, `#ffffff` panel surface, `#eeede8` recessed controls, `#dcdad3` `1px solid` dividers.
  - Functional accents only: `#e05d30` / `#c84b20` for active/+48V/engaged states, `#2a9d8f` for lock/sync, `#e9c46a` for Solo/Pad/warn, `#e63946` for Mute/Clip/Peak.
- **Banned Patterns**:
  - No purple/blue/cyan neon gradients, glassmorphism, backdrop blurs, or glowing shadows.
  - No rounded pill cards (`border-radius` must be `2px–4px`), bloated padding, or empty marketing/hero chrome.
  - No emojis anywhere in the UI, CLI output, or documentation.
- **Typography**:
  - Geometric sans for labels (`11px–13px`, uppercase section headers with `0.05em` tracking).
  - Monospace with `tabular-nums` (`JetBrains Mono` / system monospace) for all dB values, pan positions, sample rates, and channel numbers.

## 4. Hardware Verification Environment
- An **Arturia AudioFuse 8Pre** (`1c75:af0e`) is physically connected to this Linux host (`lsusb -d 1c75:af0e`, ALSA card `A8Pre`).
- When testing USB control transfers on Interface 0, ensure non-destructive restore of pre-test settings and never detach the active UAC2 audio streaming interfaces (`Interface 1` & `Interface 2`) unless explicitly testing standalone device detachment.
