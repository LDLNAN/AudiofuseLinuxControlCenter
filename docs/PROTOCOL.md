# Arturia AudioFuse 8Pre (`1c75:af0e`) Hardware & Protocol Reference

This document records the clean-room verified USB descriptors, UAC2 clock entities, AFEX vendor control transfers, 266-byte hardware state layout, 152-byte live VU telemetry layout, and Linux `snd-usb-audio` coexistence design for the **Arturia AudioFuse 8Pre** (`1c75:af0e`).

Every transfer, entity ID, parameter ID, byte offset, and scaling formula below has been verified both from clean-room analysis of the Arturia AudioFuse Control Center specification and live against a physically connected AudioFuse 8Pre (`1c75:af0e`, `bcdDevice 1.01`, ALSA card `A8Pre`) running active PipeWire/ALSA audio streams.

---

## 1. USB Device Topology & Interfaces

- **Vendor ID**: `0x1c75` (Arturia)
- **Product ID**: `0xaf0e` (`AudioFuse 8Pre`)
- **Firmware Archive Format (`*.af8pre`)**: Standard ZIP archive containing `info.json` and the raw firmware image (parsed offline only).

### USB Interfaces

| Interface | Class / Subclass / Protocol | Kernel Driver | Endpoints & Purpose |
| :--- | :--- | :--- | :--- |
| **Interface 0** | `0x01` / `0x01` / `0x20` (UAC2 Control) | `snd-usb-audio` | `0x82` (Interrupt IN, 6 bytes): UAC2 status notifications; **EP0 control transfers** for UAC2 clock entities and AFEX units `0x0a`, `0x0d`, `0x11` |
| **Interface 1** | `0x01` / `0x02` / `0x20` (UAC2 Streaming OUT) | `snd-usb-audio` | Alt 1: `0x01` Isochronous OUT (18 ch, 24-bit in 32-bit slot, Async) — **never touched** |
| **Interface 2** | `0x01` / `0x02` / `0x20` (UAC2 Streaming IN) | `snd-usb-audio` | Alt 1: `0x81` Isochronous IN (18 ch, 24-bit in 32-bit slot) — **never touched** |
| **Interface 3** | `0xfe` / `0x01` / `0x01` (USB DFU 1.10) | *(unbound)* | `wTransferSize = 64` bytes — **blocked at runtime** (runtime `DFU_GETSTATUS` can cause the hub/firmware to re-enumerate and disrupt ALSA) |

### UAC2 Audio Control Entities (Interface 0)

Verified from live USB descriptors (`lsusb -v`) and ALSA mixer enumeration (`amixer -c A8Pre contents`):

| Entity ID | Type | Description |
| :--- | :--- | :--- |
| `1` (`0x01`) | Input Terminal | USB Streaming Input (`0x0101`), 18 channels, Clock `40` |
| `2` (`0x02`) | Input Terminal | Analog / Hardware Inputs (`0x0201`), 18 channels, Clock `40` |
| `20` (`0x14`) | Output Terminal | Speaker / Hardware Outputs (`0x0301`), Source `1`, Clock `40` |
| `22` (`0x16`) | Output Terminal | USB Streaming Output (`0x0101`), Source `30`, Clock `40` |
| `30` (`0x1E`) | Extension Unit | Vendor Extension Unit (18 channels) |
| `40` (`0x28`) | Clock Selector | `"ARTURIA Clock Selector"` — **3 pins: Pin `1` -> `41` (`0x29` Internal), Pin `2` -> `43` (`0x2B` WORD Clock), Pin `3` -> `44` (`0x2C` ADAT Clock)** |
| `41` (`0x29`) | Clock Source | `"Internal Clock"` (programmable frequency `44100/48000/88200/96000` Hz + validity) |
| `43` (`0x2B`) | Clock Source | `"WORD Clock"` (external BNC clock validity) |
| `44` (`0x2C`) | Clock Source | `"ADAT Clock"` (external optical clock validity) |

> **Important correction:** Pin `2` (`Entity 43` / `0x2B`) is **WORD Clock** and Pin `3` (`Entity 44` / `0x2C`) is **ADAT Clock**, matching ALSA `amixer -c A8Pre` (`numid=2` WORD Clock Validity, `numid=3` ADAT Clock Validity, `numid=4` Clock Source `0=Internal, 1=WORD, 2=ADAT`).

---

## 2. Verified AFEX Wire Protocol (EP0 on Interface 0)

All vendor hardware control, state snapshot, and VU telemetry transfers use **USB Class Interface EP0 control transfers** targeting **Interface `0`** (`wIndex` low byte = `0x00`):

- **`bmRequestType`**: `0xa1` (Device-to-Host, Class, Interface) for reads; `0x21` (Host-to-Device, Class, Interface) for writes.
- **`bRequest`**:
  - `0x03` (`UAC2_REQ_MEM`): Used for **all** AFEX state reads, VU meter reads, and parameter gets/sets.
  - `0x01` (`UAC2_REQ_CUR`): Used for standard UAC2 Clock entities (`0x28`, `0x29`, `0x2b`, `0x2c`).
- **`wIndex`**: `((unit_id as u16) << 8) | 0x00`.

### 2.1 Full Hardware State Snapshot (`Unit 0x0d`, 266 bytes)

| Field | Value |
| :--- | :--- |
| `bmRequestType` | `0xa1` (Class Interface IN) |
| `bRequest` | `0x03` (`MEM`) |
| `wValue` | `0x0002` (full 266-byte struct; `0x0000` reads 56-byte hardware sub-block at `+0x06`, `0x0001` reads 204-byte DSP sub-block at `+0x3e`) |
| `wIndex` | `0x0d00` (`Unit 0x0d`, Interface `0`) |
| `wLength` | `266` (`0x010a`) bytes |

Header bytes `0x00..0x05` are always `[0xca, 0xde, 0xca, 0xde, 0x01, 0x00]` (`0xcade, 0xcade, 0x0001`). Every verified AFEX `param_id` maps directly into this 266-byte snapshot at **byte offset `param_id * 2`**!

### 2.2 Live VU Meter Telemetry (`Unit 0x11`, 152 bytes)

| Field | Value |
| :--- | :--- |
| `bmRequestType` | `0xa1` (Class Interface IN) |
| `bRequest` | `0x03` (`MEM`) |
| `wValue` | `0x0000` |
| `wIndex` | `0x1100` (`Unit 0x11`, Interface `0`) |
| `wLength` | `152` (`0x98`) bytes = `38 × i32` little-endian |

> **Correction vs. legacy `0x4e` (78-byte) assumption:** `0x4e` is not a USB VU meter packet size on AudioFuse 8Pre. Real live hardware VU telemetry is **152 bytes (`38 × i32` LE) on Unit `0x11`**.

Each `i32` sample is a **24-bit signed left-aligned linear peak amplitude**:

```text
linear = abs(raw_i32 >> 8) / 8388607.0   // 0.0 .. 1.0
dbfs   = 20.0 * log10(linear)            // clamped to -60.0 .. 0.0 dBFS
vu_u8  = round(((dbfs + 60.0) / 60.0) * 127.0)  // 0 .. 127 UI/CLI scale
```

#### Exact 38-Slot Meter Map

| Slot Range | Channel Group | Notes |
| :--- | :--- | :--- |
| `0..=7` | Analog Inputs `1..8` | Live pre-ADC/input levels |
| `8..=15` | ADAT Inputs `1..8` | Live optical input levels |
| `16..=23` | USB Playback `1..8` | `16, 17` (`USB 1-2`) also feed Speakers/Phones when source is `Main 1-2` |
| `24..=31` | USB Playback `9..16` | Feeds ADAT Outputs when routed from USB `9-16` |
| `32..=33` | USB Playback `17..18` | Feeds Loopback when source is `USB 17-18` |
| `34..=35` | Pre-Master Cue Bus L / R | Internal cue bus tap |
| `36..=37` | Cue Mix Main Output L / R | Live Cue Mix master output (feeds Speakers/Phones/Outputs/Loopback when routed from `Cue Mix`) |

Output meters (`Speakers`, `Phones`, `Analog Out 1-8`, `ADAT Out 1-8`, `Loopback`) are derived cleanly from their active routing source slots (`Cue Mix` -> `36..37`, `USB` -> `16..23`/`24..31`/`32..33`, `Analog In` -> `0..7`, `ADAT In` -> `8..15`).

### 2.3 Parameter Write / Read (`Unit 0x0a`)

| Field | Write (`SetParameter`) | Read (`GetParameter`) |
| :--- | :--- | :--- |
| `bmRequestType` | `0x21` (Class Interface OUT) | `0xa1` (Class Interface IN) |
| `bRequest` | `0x03` (`MEM`) | `0x03` (`MEM`) |
| `wValue` | `((param_id as u16) << 8) \| (channel as u16)` | `((param_id as u16) << 8) \| (channel as u16)` |
| `wIndex` | `0x0a00` (`Unit 0x0a`, Interface `0`) | `0x0a00` (`Unit 0x0a`, Interface `0`) |
| `wLength` | `2` bytes (`u16`/`i16` LE), or `16` bytes for `Device Name` (`0x68`) | `2` bytes (or prefer atomic 266-byte read on Unit `0x0d`) |

- For **per-channel** parameters (including bitmask fields), `channel` is the 0-based channel index (`0..7`, `0..15`, `0..23`, `0..3`, `0..1`) and the 2-byte payload is the single channel value (e.g., `0x0000` or `0x0001` — the firmware updates the corresponding bit in the bitmask).
- For **global** parameters, `channel = 0xff`, **except** `SMUX Mode` (`0x83`) which uses `channel = 0x00`.

### 2.4 UAC2 Clock Requests (Interface 0, `bRequest = 0x01` `CUR`)

- **Clock Selector (`Entity 40` / `0x28`)**: `wValue = 0x0100`, `wIndex = 0x2800`, `wLength = 1` (`1 = Internal`, `2 = WORD Clock`, `3 = ADAT Clock`). When `snd-usb-audio` is active, prefer syncing clock source via ALSA `amixer -c A8Pre cset numid=4 <0|1|2>` so ALSA's cached control state stays consistent.
- **Clock Validity (`Entities 41/43/44`)**: `wValue = 0x0200`, `wIndex = (entity << 8) | 0x00`, `wLength = 1` (`0 = invalid`, `1 = valid`).
- **Sample Rate (`Entity 41` / `0x29`)**: `wValue = 0x0100`, `wIndex = 0x2900`, `wLength = 4` (`u32` LE Hz: `44100`, `48000`, `88200`, `96000`). Note: changing sample rate while PipeWire/ALSA streams are open may be rejected or overridden by the audio server's active clock rate.

---

## 3. Complete Verified Parameter & 266-Byte State Table

Every `param_id` corresponds to offset `param_id * 2` in the 266-byte snapshot from Unit `0x0d`:

| `param_id` | Offset (`0x0d`) | Type / Size | `channel` | Name & Verified Encoding |
| :--- | :--- | :--- | :--- | :--- |
| `0x03` | `+0x006` | `u16` | RO | Front-panel 56-byte block generation counter |
| `0x04` | `+0x008..0x017` | `8 × u16` | `0..7` | **Input 1..8 PAD Mic**: `0 = OFF`, `1 = PAD (-20dB)`, `2 = BOOST` *(hardware-gated when XLR Mic is unplugged)* |
| `0x0c` | `+0x018` | `u16` bitmask | `0..7` | **Input 1..8 PAD Line**: `0 = Off`, `1 = -20dB` *(hardware-gated when TRS Line jack is unplugged)* |
| `0x0d` | `+0x01a` | `u16` bitmask | `0..1` | **Input 1..2 Inst (Hi-Z)**: `0 = Off`, `1 = Hi-Z Instrument` |
| `0x0e` | `+0x01c` | `u16` bitmask | `0..7` | **Input 1..8 Phase Invert (`Ø`)**: `0 = Normal`, `1 = Inverted` |
| `0x0f` | `+0x01e` | `u16` | `0xff` | **Speakers Source**: `0 = Main 1-2`, `1 = Cue Mix` |
| `0x10` | `+0x020` | `u16` | `0xff` | **Phones Source**: `0 = Main 1-2`, `1 = Cue Mix`, `2 = Follow Speakers` |
| `0x11` | `+0x022` | `u16` | `0xff` | **Speakers Mono**: `0 = Stereo`, `1 = Mono` |
| `0x12` | `+0x024` | `u16` | `0xff` | **Phones Mono**: `0 = Stereo`, `1 = Mono` |
| `0x13` | `+0x026` | `u16` bitmask | `0..7` | **Input 1..8 Phantom (+48V)**: `0 = Off`, `1 = +48V On` |
| `0x14` | `+0x028` | `u16` bitmask | RO | Front-panel jack sense for Ch 1-2 *(read-only)* |
| `0x15` | `+0x02a..0x03d` | `10 × u16` | RO | Hardware jack/mic sense (`0..7` per channel, `8..9` extra ch 1-2): `0 = Unplugged`, `1 = Mic XLR`, `2 = Line TRS` |
| `0x1f` | `+0x03e` | `u16` | RO | DSP 204-byte block generation counter |
| `0x20` | `+0x040..0x06f` | `24 × i16` | `0..23` | **Cue Mixer Channel Left Gain** in Q8.8 dB (`1/256 dB`, `-32768 = -inf`): `0..7` Analog, `8..15` ADAT, `16..23` USB |
| `0x38` | `+0x070..0x09f` | `24 × i16` | `0..23` | **Cue Mixer Channel Right Gain** in Q8.8 dB (`1/256 dB`, `-32768 = -inf`): `0..7` Analog, `8..15` ADAT, `16..23` USB |
| `0x50` | `+0x0a0` | `u16` bitmask | `0..15` | **Mixer Mute (Analog 1..8 + ADAT 1..8)**: bit `i` = channel `i` (`0..15`) |
| `0x51` | `+0x0a2` | `u16` bitmask | `0..7` | **Mixer Mute (USB 1..8)**: bit `i` = USB channel `i` (`0..7`, mixer idx `16..23`) |
| `0x52` | `+0x0a4` | `u16` bitmask | `0..15` | **Mixer Solo (Analog 1..8 + ADAT 1..8)**: bit `i` = channel `i` (`0..15`) |
| `0x53` | `+0x0a6` | `u16` bitmask | `0..7` | **Mixer Solo (USB 1..8)**: bit `i` = USB channel `i` (`0..7`, mixer idx `16..23`) |
| `0x55` | `+0x0aa` | `i16` | `0xff` | **Mixer Output Level (Cue Master)** in Q8.8 dB (`-61.0..=0.0 dB`, `<= -61.0` -> `-32768`) |
| `0x56` | `+0x0ac` | `u16` bitmask | `0..7` | **Mixer USB 1..8 Show** |
| `0x57` | `+0x0ae` | `u16` bitmask | `0..15` | **Mixer Analog 1..8 + ADAT 1..8 Show** |
| `0x58` | `+0x0b0` | `u16` bitmask | `0..11` | **Mixer Stereo Link / Show**: bits `0..3` Analog pairs, `4..7` ADAT pairs, `8..11` USB pairs |
| `0x61..0x65` | `+0x0c2..0x0ca` | `5 × u16` | RO | Firmware / hardware version fields |
| `0x66` | `+0x0cc` | `u16` | `0xff` | **Device Mode**: `0 = USB`, `1 = ADAT` |
| `0x68` | `+0x0d0..0x0df` | `16 × u8` | `0xff` | **Device Name**: 16-byte space-padded ASCII (`wLength = 16`) |
| `0x70` | `+0x0e0` | `u16` | `0xff` | **Wordclock Impedance**: `0 = 75 Ohms (On)`, `1 = HiZ (Off)` |
| `0x71` | `+0x0e2` | `u16` | `0xff` | **Wordclock Thru**: `0 = Off`, `1 = On` |
| `0x72` | `+0x0e4..0x0f3` | `16 × u8` | RO | Hardware Serial Number (ASCII, e.g. `"8810400336011205"`) |
| `0x7a` | `+0x0f4..0x0fb` | `4 × u16` | `0..3` | **Analog Out 1-2..7-8 Source** (**1-hot bitmask**): `0x0001 = Cue Mix`, `0x0002 = USB`, `0x0004 = Analog In`, `0x0008 = ADAT In` |
| `0x7e` | `+0x0fc..0x103` | `4 × u16` | `0..3` | **ADAT Out 1-2..7-8 Source** (**1-hot bitmask**): `0x0001 = Cue Mix`, `0x0002 = USB 9-16`, `0x0004 = Analog In` |
| `0x82` | `+0x104` | `u16` | `0xff` | **Loopback Source**: `0x0000 = Disabled`, `0x0001 = Cue Mix`, `0x0002 = USB 17-18` |
| `0x83` | `+0x106` | `u16` | `0x00` | **SMUX Mode**: `0 = Normal`, `1 = Double Speed` *(uses `channel = 0x00`)* |
| `0x84` | `+0x108` | `u16` | `0xff` | **ADAT On / Status** |

### 3.1 Constant-Power Cue Mixer Gain & Pan Law (`0x20` & `0x38`)

The hardware DSP does not store separate `level` and `pan` registers; instead each cue channel has signed Q8.8 dB **Left Gain (`0x20`)** and **Right Gain (`0x38`)** values computed with a constant-power cosine/sine pan law:

```text
pan_norm = (pan + 64.0) / 128.0       // pan in -64..=+64 -> 0.0..=1.0 (0.5 = Center)
theta    = pan_norm * (pi / 2.0)
amp      = (level_db <= -60.5) ? 0.0 : 10^(level_db / 20.0)
gain_l   = clamp(round(20 * log10(amp * cos(theta)) * 256), -32768, 32767)  // -32768 if <= 0
gain_r   = clamp(round(20 * log10(amp * sin(theta)) * 256), -32768, 32767)  // -32768 if <= 0
```

At `0.0 dB` centered (`pan = 0`), `cos(pi/4) = sin(pi/4) = -3.01 dB` (`~ -771` / `-768` Q8.8). Hard-left (`pan = -64`) sets `L = 0` (`0.0 dB`), `R = -32768` (`-inf`); hard-right (`pan = +64`) sets `L = -32768`, `R = 0`. Decoding recovers `(level_db, pan)` via `hypot(l_amp, r_amp)` and `atan2(r_amp, l_amp) / (pi / 2)`.

---

## 4. Unsupported, Hardware-Gated, and Host-Only Controls

To guarantee honest behavior against real `1c75:af0e` hardware:

1. **Not backed by hardware registers on AudioFuse 8Pre** (native transport returns `TransportError::Unsupported`):
   - `Mixer Output Mute`, `Mixer Output Solo`, and `Direct/Computer Mix` (previously speculated as `0x30`, `0x32`, `0x33` — none exist in AudioFuse 8Pre firmware; the 8Pre has only `Mixer Output Level` at `0x55`).
   - On-device preset slots (`0x48`/`0x49` `PresetLoad`/`PresetSave`) — AudioFuse 8Pre stores presets as host-side JSON files (`preset export` / `preset import`).
   - Legacy vendor string opcodes (`0x98`/`0x18`) — `Device Name` uses `param_id = 0x68` (16 bytes on Unit `0x0a`), while `Mixer Channel Name` and `Mixer Channel Group` are host-side UI labels (`send-to-device="0"` in `param.xml`).
   - `Input Type` (`Mic`/`Line`) is determined physically by the XLR/TRS combo jack sense (`0x15` read-only); only `Input 1-2 Inst (Hi-Z)` (`0x0d`) is software-switchable.
2. **Hardware-gated controls**:
   - `Input PAD Mic` (`0x04`) and `Input PAD Line` (`0x0c`) are accepted by the firmware only when the corresponding XLR Mic or TRS Line connector is physically detected in `0x15`. After writing, the controller verifies read-back from Unit `0x0d` and returns an explicit error if the hardware ignored/gated the write.
3. **Analog-only controls**:
   - Preamp gain knobs, speaker volume, and headphone volume are physical analog pots and are not USB-controllable.
4. **Runtime DFU blocked**:
   - Interface 3 (`USB DFU`) is never probed or claimed at runtime. `.af8pre` firmware archives are inspected strictly offline.

---

## 5. Linux `snd-usb-audio` / PipeWire Coexistence (`kmod/audiofuse8pre_ctl.c`)

On Linux, `snd-usb-audio` claims Interface 0 (AudioControl) alongside Interfaces 1 & 2 (Isochronous Streaming). Because Linux `drivers/usb/core/devio.c:check_ctrlrecip()` rejects `USBDEVFS_CONTROL` ioctls with `bmRequestType = 0xa1/0x21` (`USB_TYPE_CLASS | USB_RECIP_INTERFACE`) whenever another kernel driver owns Interface 0 (returning `-EBUSY`), and the AudioFuse 8Pre XMOS firmware STALLs (`-EPIPE`) all non-`0xa1/0x21` request types, pure user-space `libusb`/`usbfs` cannot perform AFEX transfers without detaching `snd-usb-audio` (which would kill PipeWire/ALSA audio).

To preserve **100% uninterrupted ALSA/PipeWire audio**:

- `kmod/audiofuse8pre_ctl.c` provides a minimal, non-intrusive kernel misc device (`/dev/audiofuse8pre-ctl`) that:
  - **NEVER** registers a `usb_driver`, **NEVER** claims or detaches any USB interface, and **NEVER** interferes with `snd-usb-audio` or Interfaces 1 & 2.
  - Strictly enforces a hardware safety whitelist: **VID `0x1c75` / PID `0xaf0e`**, **Interface `0x00` only** (`(wIndex & 0xff) == 0`), **`bmRequestType` in `{0xa1, 0x21}`**, **`bRequest` in `{0x01, 0x02, 0x03}`**, and **`entity_id` in `{0x0a, 0x0d, 0x11, 0x1e, 0x28, 0x29, 0x2b, 0x2c}`** — hard-blocking Interfaces 1, 2, and 3 (DFU) inside the kernel.
  - Dispatches whitelisted EP0 control transfers safely via `usb_control_msg()` under a module mutex while PipeWire/ALSA streaming continues uninterrupted on Interfaces 1 & 2.
- `UsbHardwareTransport::open()` automatically opens `/dev/audiofuse8pre-ctl` first whenever available; if `/dev/audiofuse8pre-ctl` is absent and Interface 0 is unclaimed, it falls back to `rusb`; if `snd-usb-audio` owns Interface 0 and the helper module is not loaded, it returns a clear, actionable error explaining how to load `kmod/audiofuse8pre_ctl.ko` (or pass `--simulator`).
