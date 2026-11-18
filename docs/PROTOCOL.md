# Arturia AudioFuse 8Pre (`1c75:af0e`) Hardware & Protocol Reference

This document summarizes the USB descriptors, UAC2 entities, vendor control protocol, and parameter mappings extracted from hardware inspection of an attached **Arturia AudioFuse 8Pre** (`1c75:af0e`, serial `8810400336011205`, `bcdDevice 1.01`) and clean-room analysis of the original Arturia AudioFuse Control Center (`v2.5.0.1665`) and firmware update package (`v1.0.4`).

---

## 1. USB Device Topology & Interfaces

- **Vendor ID**: `0x1c75` (Arturia)
- **Product ID**: `0xaf0e` (`AudioFuse 8Pre`, internal C++ class `Arturia::AudioFuseLib::AFEXDevice` / `AudiofuseExStateController`)
- **Firmware Archive Format (`*.af8pre`)**: Standard ZIP archive containing `info.json` and the raw firmware image (e.g., `audiofuse_expander__fw1_0_4_8385329__2020_10_20.bin`, 94,768 bytes).

### USB Interfaces

| Interface | Class / Subclass / Protocol | Kernel Driver | Endpoints & Purpose |
| :--- | :--- | :--- | :--- |
| **Interface 0** | `0x01` / `0x01` / `0x20` (UAC2 Control) | `snd-usb-audio` | `0x82` (Interrupt IN, 6 bytes, interval 64): UAC2 status & hardware state change notifications; EP0 vendor/class control transfers |
| **Interface 1** | `0x01` / `0x02` / `0x20` (UAC2 Streaming OUT) | `snd-usb-audio` | Alt 1: `0x01` Isochronous OUT (18 channels, 24-bit in 4-byte slot, Async) |
| **Interface 2** | `0x01` / `0x02` / `0x20` (UAC2 Streaming IN) | `snd-usb-audio` | Alt 1: `0x81` Isochronous IN (18 channels, 24-bit in 4-byte slot), `0x81` / `0x11` explicit feedback |
| **Interface 3** | `0xfe` / `0x01` / `0x01` (Application Specific: DFU) | *(unbound)* | USB DFU 1.10 (`bmAttributes = 0x0d`: WillDetach, ManifestationTolerant, CanDnload; `wTransferSize = 64` bytes) |

### UAC2 Audio Control Entities (Interface 0)

| Entity ID | Type | Description |
| :--- | :--- | :--- |
| `1` (`0x01`) | Input Terminal | USB Streaming Input (`0x0101`), 18 channels (`FL, FR, FC, LFE, BL, BR, FLC, FRC, BC, SL, SR, TC, TFL, TFC, TFR, TBL, TBC, TBR`), Clock `40` |
| `2` (`0x02`) | Input Terminal | Analog / Hardware Inputs (`0x0201` Microphone), 18 channels, Clock `40` |
| `20` (`0x14`) | Output Terminal | Speaker / Hardware Outputs (`0x0301`), Source `1`, Clock `40` |
| `22` (`0x16`) | Output Terminal | USB Streaming Output (`0x0101`), Source `30`, Clock `40` |
| `30` (`0x1E`) | Extension Unit | Vendor Extension Unit (`wExtensionCode = 0x0000`, 1 input pin from Entity `2`, 18 channels, `bmControls = 0x03`) |
| `40` (`0x28`) | Clock Selector | `"ARTURIA Clock Selector"`, 3 pins: `1 -> 41 (0x29)`, `2 -> 43 (0x2B)`, `3 -> 44 (0x2C)` |
| `41` (`0x29`) | Clock Source | `"Internal Clock"` (Internal programmable clock, synced to SOF, Read/Write freq control) |
| `43` (`0x2B`) | Clock Source | `"ADAT Clock"` (External clock, validity control exposed) |
| `44` (`0x2C`) | Clock Source | `"WORD Clock"` (External clock, validity control exposed) |

---

## 2. Vendor USB Control Protocol (`AFEXDevice`)

In `Arturia::AudioFuseLib::AFEXDevice` (inheriting from `Arturia::AudioFuseLib::GenericUsbDevice` and communicating via `Arturia::USB::LibusbAudioInterface`):

### Device Constants (`AFEXDevice` vtable)

- **Product ID**: `0xaf0e`
- **Target Control Unit ID**: `0x0a` (`10`)
- **Parameter Value Size**: `2` bytes (`uint16_t`, little-endian)
- **Bulk State / VU Meter Payload Size**: `0x4e` (`78` bytes)
- **Firmware File Glob**: `*.af8pre`

### Control Transfer Requests (`GenericUsbDevice`)

All vendor/class control transfers target **Interface 0** (`wIndex` low byte = `0x00`, high byte = Unit ID `0x0a` -> `wIndex = 0x0a00` for unit-addressed requests):

1. **Get / Set Parameter (`AFEXDevice::GetParameter` / `SetParameter`)**:
   - `wValue = ((param_id & 0xff) << 8) | (channel_index & 0xff)`
   - `wIndex = (unit_id << 8) | interface_num` (`0x0a00`)
   - `wLength = 2` (2-byte little-endian value)
   - Uses `LibusbAudioInterface` parameter control transfer (`bmRequestType = 0xa1` Class Interface IN for Get, `0x21` Class Interface OUT for Set; UAC2 `CS_SAM_FREQ_CONTROL` / vendor control selector in `wValue`).
2. **Bulk State & VU Meter Read**:
   - Reads `0x4e` (`78`) bytes containing real-time hardware state and channel VU levels.
3. **String Read / Write (`GenericUsbDevice::GetString` / `SetString`)**:
   - Used for `Device Name` and per-channel mixer `Name` (16 bytes max, ASCII).
   - `GetString(type, index)`: `bmRequestType = 0xa1` (IN), `bRequest = 0x98 + type`, `wValue = 0`, `wIndex = index`, `wLength = 16` (`0x10`).
   - `SetString(type, index, str)`: `bmRequestType = 0x21` (OUT), `bRequest = 0x18 + type`, `wValue = 0`, `wIndex = index`, `wLength = 16` (`0x10`), zero-padded 16-byte buffer.
4. **Preset Load / Save (`GenericUsbDevice::PresetLoad` / `PresetSave`)**:
   - `PresetLoad(slot, buf, len)`: `bmRequestType = 0xa1` (IN), `bRequest = 0x48`, `wValue = 0`, `wIndex = slot`, `wLength = len`.
   - `PresetSave(slot, buf, len)`: `bmRequestType = 0x21` (OUT), `bRequest = 0x49`, `wValue = 0`, `wIndex = slot`, `wLength = len`.
5. **UAC2 Clock Source & Sample Rate**:
   - Clock Source (`Clock Selector`, Entity `40` / `0x28`): UAC2 `CS_CLOCK_SELECTOR_CONTROL` (`0x01`), 1-byte selector index (`1` = Internal `0x29`, `2` = ADAT `0x2B`, `3` = Word Clock `0x2C`), or via ALSA control `numid=4` (`ARTURIA Clock Selector Clock Source`).
   - Clock Lock Status: UAC2 `CS_CLOCK_VALID_CONTROL` (`0x02`) on Entities `41`, `43`, `44` (also exposed via ALSA `numid=1..3`).
   - Sample Rate (`44100`, `48000`, `88200`, `96000`): UAC2 `CS_SAM_FREQ_CONTROL` (`0x01`) on Entity `41` (`0x29`).

---

## 3. Complete Parameter Schema (`AudioFuse 8Pre/param.xml`)

### 3.1 Global & Clock Parameters

| Parameter Name | Type | Range / Enum Values | Default | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `Device Mode` | Enum | `0`: `USB`, `1`: `ADAT` | `0` (`USB`) | Switches between USB audio interface and standalone 8-ch ADAT expander |
| `Device Name` | String | 16 chars max | `"AudioFuse 8Pre"` | Stored on device via `GetString`/`SetString` |
| `Clock Source` | Enum | `0`: `Internal Clock` (`0x29`), `1`: `ADAT Clock` (`0x2B`), `2`: `WORD Clock` (`0x2C`) | `0` | UAC2 Entity `40` (`0x28`) |
| `Clock Locked` | Bool | `0`: Unlocked, `1`: Locked | `0` | Read-only status from UAC2 Clock Validity |
| `Sample Rate` | Enum | `0`: `44100`, `1`: `48000`, `2`: `88200`, `3`: `96000` | `0` | UAC2 Clock Entity `41` (`0x29`) |
| `Wordclock Impedance` | Enum | `0`: `75 Ohms (On)`, `1`: `HiZ (Off)` | `0` | 75-ohm BNC termination switch |
| `Wordclock Thru` | Enum | `0`: `Off`, `1`: `On` | `0` | Routes Word Clock In to Word Clock Out |
| `ADAT On` | Bool | `0`: Off, `1`: On | `1` | Enables optical ADAT I/O |
| `SMUX Mode` | Enum | `0`: `Normal`, `1`: `Double Speed` | `0` | S/MUX optical framing mode at 88.2/96 kHz |

### 3.2 Analog Input Channel Strips (`Input 1` .. `Input 8`)

| Parameter Name | Channels | Type | Values | Default | Notes |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `Input N Mic` | `1..8` | Bool | `0`, `1` | `1` | Mic preamp active |
| `Input N Jack` | `1..8` | Bool | `0`, `1` | `1` | TRS Line/Inst jack active |
| `Input N Inst` | `1..2` | Bool | `0`, `1` | `0` | Hi-Z Instrument input mode (Channels 1 & 2 only) |
| `Input N Type` | `1..2`: 4 vals<br>`3..8`: 3 vals | Enum | `0`: `Input`, `1`: `Mic`, `2`: `Line`, `3`: `Inst` (`1..2` only) | `0` | Input connector/impedance selector |
| `Input N Phantom` | `1..8` | Bool | `0`: Off, `1`: +48V On | `0` | +48V phantom power |
| `Input N Phase Invert` | `1..8` | Bool | `0`: Normal, `1`: Inverted (`Ø`) | `0` | Polarity inversion |
| `Input N PAD Line` | `1..8` | Enum | `0`: `Off`, `1`: `-20dB` | `0` | Line input attenuation |
| `Input N PAD Mic` | `1..8` | Enum | `0`: `OFF`, `1`: `PAD` (`-20 dB`), `2`: `BOOST` | `0` | Mic preamp pad / gain boost |
| `Input Analog N Vu` | `1..8` | Int | `0..127` | `0` | Real-time input VU meter |

### 3.3 ADAT & USB Input Metering

- `Input ADAT 1 Vu` .. `Input ADAT 8 Vu`: `0..127`
- `Input USB 1 Vu` .. `Input USB 8 Vu`: `0..127`

### 3.4 Cue Mixer Channels (`Analog 1..8`, `ADAT 1..8`, `USB 1..8`)

Each of the 24 mixer channels (`Ch` in `Analog 1..8`, `ADAT 1..8`, `USB 1..8`) exposes:

| Parameter Suffix | Type | Range / Scaling | Default | Persisted |
| :--- | :--- | :--- | :--- | :--- |
| `Mixer Ch Level` | Float | `-61.0` to `+12.0` dB (`PiecewiseLinear(0.842105263)`, display `< -60.5` as `-inf`) | `0.0` dB | Device |
| `Mixer Ch Pan` | Int | `-64` (`<64`) to `+64` (`64>`), `0` = `C` | `0` (odd `-64`, even `+64` when stereo linked) | Device |
| `Mixer Ch Mute` | Bool | `0` / `1` | `0` | Device |
| `Mixer Ch Solo` | Bool | `0` / `1` | `0` | Device |
| `Mixer Ch Peak` | Bool | `0` / `1` | `0` | Read-only |
| `Mixer Ch Name` | String | 16 chars max | `"Analog N"` / `"ADAT N"` / `"USB N"` | Device Pref |
| `Mixer Ch Group` | Int | `0` (None), `1` (`Grp 1`), `2` (`Grp 2`), `3` (`Grp 3`), `4` (`Grp 4`) | `0` | Device Pref |
| `Mixer Ch Show` | Bool | `0` / `1` | `1` (Analog), `0` (ADAT/USB) | Device Pref |

Stereo link toggles per bank (`Analog`, `ADAT`, `USB`):
- `Mixer <Bank> 1-2 Stereo Show` (`Bool`, default `0`)
- `Mixer <Bank> 3-4 Stereo Show` (`Bool`, default `0`)
- `Mixer <Bank> 5-6 Stereo Show` (`Bool`, default `0`)
- `Mixer <Bank> 7-8 Stereo Show` (`Bool`, default `0`)

### 3.5 Master Cue Mixer & Monitor Section

| Parameter Name | Type | Range / Values | Default |
| :--- | :--- | :--- | :--- |
| `Direct/Computer Mix` | Float | `0.0` (Direct Cue) to `1.0` (Computer USB) | `0.5` |
| `Mixer Output Level` | Float | `-61.0` to `0.0` dB (`Linear`, `< -60.5` as `-inf`) | `0.0` dB |
| `Mixer Output Mute` | Bool | `0` / `1` | `0` |
| `Mixer Output Solo` | Bool | `0` / `1` | `0` |
| `Mixer Output Main Vu L` / `R` | Int | `0..127` | `0` |
| `Speakers Source` | Enum | `0`: `Main 1-2`, `1`: `Cue Mix` | `0` |
| `Speakers Mono` | Bool | `0` / `1` | `0` |
| `Speakers Left Vu` / `Right Vu` | Int | `0..127` | `0` |
| `Phones Source` | Enum | `0`: `Main 1-2`, `1`: `Cue Mix`, `2`: `Follow Spkrs` | `0` |
| `Phones Mono` | Bool | `0` / `1` | `0` |
| `Phones Left Vu` / `Right Vu` | Int | `0..127` | `0` |

### 3.6 Output & Loopback Routing Matrix

| Parameter Name | Type | Enum Values | Default |
| :--- | :--- | :--- | :--- |
| `Analog 1-2 Source` | Enum | `0`: `Cue Mix`, `1`: `USB 1-2`, `2`: `Analog In 1-2`, `3`: `ADAT In 1-2` | `1` (`USB 1-2`) |
| `Analog 3-4 Source` | Enum | `0`: `Cue Mix`, `1`: `USB 3-4`, `2`: `Analog In 3-4`, `3`: `ADAT In 3-4` | `1` (`USB 3-4`) |
| `Analog 5-6 Source` | Enum | `0`: `Cue Mix`, `1`: `USB 5-6`, `2`: `Analog In 5-6`, `3`: `ADAT In 5-6` | `1` (`USB 5-6`) |
| `Analog 7-8 Source` | Enum | `0`: `Cue Mix`, `1`: `USB 7-8`, `2`: `Analog In 7-8`, `3`: `ADAT In 7-8` | `1` (`USB 7-8`) |
| `ADAT 1-2 Source` | Enum | `0`: `Cue Mix`, `1`: `USB 9-10`, `2`: `Analog In 1-2` | `1` (`USB 9-10`) |
| `ADAT 3-4 Source` | Enum | `0`: `Cue Mix`, `1`: `USB 11-12`, `2`: `Analog In 3-4` | `1` (`USB 11-12`) |
| `ADAT 5-6 Source` | Enum | `0`: `Cue Mix`, `1`: `USB 13-14`, `2`: `Analog In 5-6` | `1` (`USB 13-14`) |
| `ADAT 7-8 Source` | Enum | `0`: `Cue Mix`, `1`: `USB 15-16`, `2`: `Analog In 7-8` | `1` (`USB 15-16`) |
| `Loopback Source` | Enum | `0`: `Disabled`, `1`: `Cue Mix`, `2`: `USB 17-18` | `1` (`Cue Mix`) |

Associated Output VU Meters (`0..127`):
- `Analog Output 1 Vu` .. `Analog Output 8 Vu`
- `ADAT Output 1 Vu` .. `ADAT Output 8 Vu`
- `Loopback Left Vu`, `Loopback Right Vu`

---

## 4. `AudiofuseExStateController` Parameter ID Table

In `AudioFuse Control Center` (`0x1006685d0..0x100669e70`), `AudiofuseExStateController` registers each hardware parameter with a `(param_id, channel_idx)` pair passed to `AFEXDevice::GetParameter` / `SetParameter`:

- **Per-channel loops (`idx = 0..7`)**:
  - `param_id = 0x10` (`16`), `idx = 0..7`: Input Type / Mic / Jack state
  - `param_id = 0x11` (`17`), `idx = 0..7`: Input PAD / Boost
  - `param_id = 0x12` (`18`), `idx = 0..7`: Input +48V Phantom Power
  - `param_id = 0x13` (`19`), `idx = 0..7`: Input Phase Invert (`Ø`)
  - `param_id = 0x18` (`24`), `idx = 0..1`: Input 1–2 Instrument (`Hi-Z`) mode
- **Cue Mixer per-channel loops**:
  - `param_id = 0x20` (`32`), `idx = 0..23`: Cue Mixer Channel Mute (`0..7` Analog, `8..15` ADAT, `16..23` USB)
  - `param_id = 0x21` (`33`), `idx = 0..23`: Cue Mixer Channel Solo (`0..7` Analog, `8..15` ADAT, `16..23` USB)
  - `param_id = 0x22` (`34`), `idx = 0..23`: Cue Mixer Channel Pan (`0..7` Analog, `8..15` ADAT, `16..23` USB)
  - `param_id = 0x23` (`35`), `idx = 0..23`: Cue Mixer Channel Level (`0..7` Analog, `8..15` ADAT, `16..23` USB)
- **Master Cue & Monitor Controls (`idx = 0`)**:
  - `param_id = 0x30` (`48`): `Mixer Output Mute`
  - `param_id = 0x31` (`49`): `Mixer Output Level`
  - `param_id = 0x38` (`56`): `Speakers Source`
  - `param_id = 0x39` (`57`): `Speakers Mono`
  - `param_id = 0x40` (`64`): `Phones Source`
  - `param_id = 0x41` (`65`): `Phones Mono`
- **Output Routing & Global Hardware Settings**:
  - `param_id = 0x48` (`72`), `idx = 0..3`: `Analog 1-2..7-8 Source`
  - `param_id = 0x49` (`73`), `idx = 0..3`: `ADAT 1-2..7-8 Source`
  - `param_id = 0x4a` (`74`), `idx = 0`: `Loopback Source`
  - `param_id = 0x05` (`5`), `idx = 0`: `Device Mode` (`USB` / `ADAT`)
  - `param_id = 0x06` (`6`), `idx = 0`: `ADAT On`
  - `param_id = 0x08` (`8`), `idx = 0`: `Wordclock Impedance` (`75 Ohms` / `HiZ`)
  - `param_id = 0x09` (`9`), `idx = 0`: `Wordclock Thru`
  - `param_id = 0x0a` (`10`), `idx = 0`: `SMUX Mode`
