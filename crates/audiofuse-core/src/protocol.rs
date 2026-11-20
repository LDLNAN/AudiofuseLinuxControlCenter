//! Clean-room verified protocol constants, USB/UAC2 control requests,
//! AFEX unit/parameter IDs, Q8.8 constant-power pan math, 1-hot routing masks,
//! and 24-bit VU meter scaling for the Arturia AudioFuse 8Pre (`1c75:af0e`).

use serde::{Deserialize, Serialize};
use std::f32::consts::FRAC_PI_2;
use std::fmt;

/// Arturia USB Vendor ID (`0x1c75`).
pub const ARTURIA_VENDOR_ID: u16 = 0x1c75;

/// Arturia AudioFuse 8Pre Product ID (`0xaf0e`).
pub const AUDIOFUSE_8PRE_PRODUCT_ID: u16 = 0xaf0e;

/// AFEX parameter write/read unit ID (`0x0a` / `10`).
pub const AFEX_UNIT_ID: u8 = 0x0a;

/// AFEX hardware state snapshot unit ID (`0x0d` / `13`).
pub const AFEX_STATE_UNIT_ID: u8 = 0x0d;

/// AFEX real-time VU meter telemetry unit ID (`0x11` / `17`).
pub const AFEX_VU_UNIT_ID: u8 = 0x11;

/// USB Interface 0: UAC2 Audio Control + AFEX EP0 Vendor Control Transfers.
pub const UAC2_CONTROL_INTERFACE: u8 = 0x00;

/// USB Interface 3: USB DFU (`bInterfaceClass 0xfe`, `bInterfaceSubClass 0x01`).
/// Blocked at runtime to protect active audio streaming.
pub const DFU_INTERFACE: u8 = 0x03;

/// DFU transfer block size (`wTransferSize = 64` bytes).
pub const DFU_TRANSFER_SIZE: usize = 64;

/// Standard scalar AFEX parameter payload size (`u16`/`i16` little-endian = 2 bytes).
pub const PARAM_VALUE_SIZE: usize = 2;

/// Verified full hardware state snapshot payload size from Unit `0x0d` (`wValue = 0x0002`): `266` (`0x010a`) bytes.
pub const AFEX_STATE_SIZE: usize = 266;

/// Verified real-time VU telemetry payload size from Unit `0x11` (`wValue = 0x0000`): `152` (`0x98`) bytes (`38 × i32` LE).
pub const AFEX_VU_SIZE: usize = 152;

/// Number of signed 32-bit peak meter slots in the 152-byte Unit `0x11` VU payload.
pub const AFEX_VU_CHANNELS: usize = 38;

/// Verified 6-byte magic header at offset `0x00..0x06` of the 266-byte Unit `0x0d` state snapshot.
pub const AFEX_STATE_MAGIC: [u8; 6] = [0xca, 0xde, 0xca, 0xde, 0x01, 0x00];

/// `wValue` selector for reading the full 266-byte state snapshot from Unit `0x0d`.
pub const AFEX_STATE_WVALUE_FULL: u16 = 0x0002;

/// `wValue` selector for reading the 56-byte front-panel hardware sub-block from Unit `0x0d`.
pub const AFEX_STATE_WVALUE_HW_BLOCK: u16 = 0x0000;

/// `wValue` selector for reading the 204-byte DSP sub-block from Unit `0x0d`.
pub const AFEX_STATE_WVALUE_DSP_BLOCK: u16 = 0x0001;

/// Channel byte used in `wValue` for global (non-indexed) AFEX parameters (`0xff`).
pub const AFEX_GLOBAL_CHANNEL: u8 = 0xff;

/// Legacy 78-byte (`0x4e`) simulator preset-slot buffer size (not a hardware USB transfer).
pub const BULK_STATE_VU_SIZE: usize = 0x4e; // 78

/// Maximum ASCII string length stored on device (`Device Name`) and host channel labels.
pub const MAX_DEVICE_STRING_LEN: usize = 16;

/// USB `bmRequestType` for Class Interface IN (`0xa1`: Device-to-Host, Class, Interface).
pub const USB_REQ_CLASS_INTERFACE_IN: u8 = 0xa1;

/// USB `bmRequestType` for Class Interface OUT (`0x21`: Host-to-Device, Class, Interface).
pub const USB_REQ_CLASS_INTERFACE_OUT: u8 = 0x21;

/// UAC2 Request Code: `CUR` (`0x01` - Current setting attribute, used for UAC2 Clock entities).
pub const UAC2_REQ_CUR: u8 = 0x01;

/// UAC2 Request Code: `RANGE` (`0x02` - Range attribute).
pub const UAC2_REQ_RANGE: u8 = 0x02;

/// UAC2 Request Code: `MEM` (`0x03` - Memory request, used for all AFEX state/VU/parameter transfers).
pub const UAC2_REQ_MEM: u8 = 0x03;

/// String type `0`: Global device name (backed on hardware by `param_id::DEVICE_NAME = 0x68`, 16 bytes).
pub const STRING_TYPE_DEVICE_NAME: u8 = 0x00;

/// String type `1`: Mixer channel name (`idx = 0..23`, host-side only on AudioFuse 8Pre).
pub const STRING_TYPE_CHANNEL_NAME: u8 = 0x01;

// --- UAC2 Audio Control Entities (Interface 0, verified from live USB descriptors & ALSA) ---

/// Entity `1` (`0x01`): USB Streaming Input Terminal (18 ch).
pub const UAC2_ENTITY_USB_IN_TERMINAL: u8 = 0x01;
/// Entity `2` (`0x02`): Analog / Hardware Input Terminal (18 ch).
pub const UAC2_ENTITY_HW_IN_TERMINAL: u8 = 0x02;
/// Entity `20` (`0x14`): Speaker / Hardware Output Terminal.
pub const UAC2_ENTITY_HW_OUT_TERMINAL: u8 = 0x14;
/// Entity `22` (`0x16`): USB Streaming Output Terminal.
pub const UAC2_ENTITY_USB_OUT_TERMINAL: u8 = 0x16;
/// Entity `30` (`0x1E`): Vendor Extension Unit.
pub const UAC2_ENTITY_EXTENSION_UNIT: u8 = 0x1e;
/// Entity `40` (`0x28`): `"ARTURIA Clock Selector"` (3 pins: `1 -> 41 Internal`, `2 -> 43 WORD`, `3 -> 44 ADAT`).
pub const UAC2_ENTITY_CLOCK_SELECTOR: u8 = 0x28;
/// Entity `41` (`0x29`): `"Internal Clock"` (Programmable frequency + validity).
pub const UAC2_ENTITY_CLOCK_INTERNAL: u8 = 0x29;
/// Entity `43` (`0x2B`): `"WORD Clock"` (External BNC clock validity, Selector Pin 2).
pub const UAC2_ENTITY_CLOCK_WORD: u8 = 0x2b;
/// Entity `44` (`0x2C`): `"ADAT Clock"` (External optical clock validity, Selector Pin 3).
pub const UAC2_ENTITY_CLOCK_ADAT: u8 = 0x2c;

/// UAC2 Clock Selector Control Selector (`CS_CLOCK_SELECTOR_CONTROL = 0x01`).
pub const UAC2_CS_CLOCK_SELECTOR_CONTROL: u8 = 0x01;
/// UAC2 Clock Sampling Frequency Control Selector (`CS_SAM_FREQ_CONTROL = 0x01`).
pub const UAC2_CS_SAM_FREQ_CONTROL: u8 = 0x01;
/// UAC2 Clock Validity Control Selector (`CS_CLOCK_VALID_CONTROL = 0x02`).
pub const UAC2_CS_CLOCK_VALID_CONTROL: u8 = 0x02;

// --- Verified Hardware Parameter IDs (AFEX Unit 0x0a, offset in 266-byte Unit 0x0d struct = param_id * 2) ---

/// Clean-room verified AFEX parameter IDs for the AudioFuse 8Pre (`1c75:af0e`).
/// Every ID `p` maps directly to byte offset `p * 2` in the 266-byte Unit `0x0d` state snapshot.
pub mod param_id {
    /// `0x03` (`+0x006`, `u16`, read-only): Front-panel 56-byte hardware block generation counter.
    pub const HW_BLOCK_GEN: u8 = 0x03;

    /// `0x04` (`+0x008..0x017`, `8 × u16`, `ch = 0..7`): `Input 1..8 PAD Mic` (`0 = OFF`, `1 = PAD -20dB`, `2 = BOOST`).
    pub const INPUT_PAD_MIC: u8 = 0x04;
    /// Backwards-compatible alias for [`INPUT_PAD_MIC`].
    pub const INPUT_PAD_BOOST: u8 = INPUT_PAD_MIC;

    /// `0x0c` (`+0x018`, `u16` bitmask, `ch = 0..7`): `Input 1..8 PAD Line` (`0 = Off`, `1 = -20dB`).
    pub const INPUT_PAD_LINE: u8 = 0x0c;

    /// `0x0d` (`+0x01a`, `u16` bitmask, `ch = 0..1`): `Input 1..2 Inst` (`0 = Off`, `1 = Hi-Z Instrument`).
    pub const INPUT_INST: u8 = 0x0d;

    /// `0x0e` (`+0x01c`, `u16` bitmask, `ch = 0..7`): `Input 1..8 Phase Invert` (`0 = Normal`, `1 = Inverted`).
    pub const INPUT_PHASE_INVERT: u8 = 0x0e;

    /// `0x0f` (`+0x01e`, `u16`, `ch = 0xff`): `Speakers Source` (`0 = Main 1-2`, `1 = Cue Mix`).
    pub const SPEAKERS_SOURCE: u8 = 0x0f;

    /// `0x10` (`+0x020`, `u16`, `ch = 0xff`): `Phones Source` (`0 = Main 1-2`, `1 = Cue Mix`, `2 = Follow Speakers`).
    pub const PHONES_SOURCE: u8 = 0x10;

    /// `0x11` (`+0x022`, `u16`, `ch = 0xff`): `Speakers Mono` (`0 = Stereo`, `1 = Mono`).
    pub const SPEAKERS_MONO: u8 = 0x11;

    /// `0x12` (`+0x024`, `u16`, `ch = 0xff`): `Phones Mono` (`0 = Stereo`, `1 = Mono`).
    pub const PHONES_MONO: u8 = 0x12;

    /// `0x13` (`+0x026`, `u16` bitmask, `ch = 0..7`): `Input 1..8 Phantom (+48V)` (`0 = Off`, `1 = +48V`).
    pub const INPUT_PHANTOM: u8 = 0x13;

    /// `0x14` (`+0x028`, `u16` bitmask, read-only): Front-panel jack sense for Ch 1-2.
    pub const INPUT_FRONT_JACK_SENSE: u8 = 0x14;

    /// `0x15` (`+0x02a..0x03d`, `10 × u16`, read-only): Hardware connector sense (`0 = Unplugged`, `1 = Mic XLR`, `2 = Line TRS`).
    pub const INPUT_JACK_SENSE: u8 = 0x15;

    /// `0x1f` (`+0x03e`, `u16`, read-only): DSP 204-byte block generation counter.
    pub const DSP_BLOCK_GEN: u8 = 0x1f;

    /// `0x20` (`+0x040..0x06f`, `24 × i16`, `ch = 0..23`): Cue Mixer Channel Left Gain in Q8.8 dB (`-32768 = -inf`).
    pub const MIXER_CH_GAIN_L: u8 = 0x20;

    /// `0x38` (`+0x070..0x09f`, `24 × i16`, `ch = 0..23`): Cue Mixer Channel Right Gain in Q8.8 dB (`-32768 = -inf`).
    pub const MIXER_CH_GAIN_R: u8 = 0x38;

    /// `0x50` (`+0x0a0`, `u16` bitmask, `ch = 0..15`): Cue Mixer Mute for Analog 1..8 (`0..7`) + ADAT 1..8 (`8..15`).
    pub const MIXER_MUTE_ANALOG_ADAT: u8 = 0x50;

    /// `0x51` (`+0x0a2`, `u16` bitmask, `ch = 0..7`): Cue Mixer Mute for USB 1..8 (`0..7`).
    pub const MIXER_MUTE_USB: u8 = 0x51;

    /// `0x52` (`+0x0a4`, `u16` bitmask, `ch = 0..15`): Cue Mixer Solo for Analog 1..8 (`0..7`) + ADAT 1..8 (`8..15`).
    pub const MIXER_SOLO_ANALOG_ADAT: u8 = 0x52;

    /// `0x53` (`+0x0a6`, `u16` bitmask, `ch = 0..7`): Cue Mixer Solo for USB 1..8 (`0..7`).
    pub const MIXER_SOLO_USB: u8 = 0x53;

    /// `0x55` (`+0x0aa`, `i16`, `ch = 0xff`): `Mixer Output Level` (Master Cue Level) in Q8.8 dB (`-61.0..=0.0 dB`, `-32768 = -inf`).
    pub const MIXER_OUT_LEVEL: u8 = 0x55;

    /// `0x56` (`+0x0ac`, `u16` bitmask, `ch = 0..7`): `Mixer USB 1..8 Show`.
    pub const MIXER_SHOW_USB: u8 = 0x56;

    /// `0x57` (`+0x0ae`, `u16` bitmask, `ch = 0..15`): `Mixer Analog 1..8 + ADAT 1..8 Show`.
    pub const MIXER_SHOW_ANALOG_ADAT: u8 = 0x57;

    /// `0x58` (`+0x0b0`, `u16` bitmask, `ch = 0..11`): `Mixer Stereo Show / Link` (`0..3` Analog, `4..7` ADAT, `8..11` USB).
    pub const MIXER_STEREO_SHOW: u8 = 0x58;

    /// `0x66` (`+0x0cc`, `u16`, `ch = 0xff`): `Device Mode` (`0 = USB`, `1 = ADAT`).
    pub const DEVICE_MODE: u8 = 0x66;

    /// `0x68` (`+0x0d0..0x0df`, 16 bytes ASCII space-padded, `ch = 0xff`, `wLength = 16`): `Device Name`.
    pub const DEVICE_NAME: u8 = 0x68;

    /// `0x70` (`+0x0e0`, `u16`, `ch = 0xff`): `Wordclock Impedance` (`0 = 75 Ohms On`, `1 = HiZ Off`).
    pub const WORDCLOCK_IMPEDANCE: u8 = 0x70;

    /// `0x71` (`+0x0e2`, `u16`, `ch = 0xff`): `Wordclock Thru` (`0 = Off`, `1 = On`).
    pub const WORDCLOCK_THRU: u8 = 0x71;

    /// `0x72` (`+0x0e4..0x0f3`, 16 bytes ASCII, read-only): Hardware Serial Number.
    pub const SERIAL_NUMBER: u8 = 0x72;

    /// `0x7a` (`+0x0f4..0x0fb`, `4 × u16`, `ch = 0..3`): `Analog Out 1-2..7-8 Source` (1-hot bitmask `1/2/4/8`).
    pub const ANALOG_OUT_SOURCE: u8 = 0x7a;

    /// `0x7e` (`+0x0fc..0x103`, `4 × u16`, `ch = 0..3`): `ADAT Out 1-2..7-8 Source` (1-hot bitmask `1/2/4`).
    pub const ADAT_OUT_SOURCE: u8 = 0x7e;

    /// `0x82` (`+0x104`, `u16`, `ch = 0xff`): `Loopback Source` (`0 = Disabled`, `1 = Cue Mix`, `2 = USB 17-18`).
    pub const LOOPBACK_SOURCE: u8 = 0x82;

    /// `0x83` (`+0x106`, `u16`, `ch = 0x00`): `SMUX Mode` (`0 = Normal`, `1 = Double Speed`).
    pub const SMUX_MODE: u8 = 0x83;

    /// `0x84` (`+0x108`, `u16`, `ch = 0xff`): `ADAT On` / status.
    pub const ADAT_ON: u8 = 0x84;
}

/// Construct the 16-bit `wValue` for `AFEXDevice::GetParameter` / `SetParameter`:
/// `wValue = ((param_id as u16) << 8) | (channel_index as u16)`
#[inline]
pub const fn encode_param_wvalue(param_id: u8, channel_index: u8) -> u16 {
    ((param_id as u16) << 8) | (channel_index as u16)
}

/// Decode a 16-bit `wValue` into `(param_id, channel_index)`.
#[inline]
pub const fn decode_param_wvalue(wvalue: u16) -> (u8, u8) {
    (((wvalue >> 8) & 0xff) as u8, (wvalue & 0xff) as u8)
}

/// Construct the 16-bit `wIndex` for unit-addressed control transfers on Interface 0:
/// `wIndex = ((unit_id as u16) << 8) | (interface_num as u16)`.
#[inline]
pub const fn encode_unit_windex(unit_id: u8, interface_num: u8) -> u16 {
    ((unit_id as u16) << 8) | (interface_num as u16)
}

/// Default `wIndex` for AFEX parameter unit `0x0a` on Interface `0x00` (`0x0a00`).
pub const AFEX_DEFAULT_WINDEX: u16 = encode_unit_windex(AFEX_UNIT_ID, UAC2_CONTROL_INTERFACE);

/// `wIndex` for AFEX state snapshot unit `0x0d` on Interface `0x00` (`0x0d00`).
pub const AFEX_STATE_WINDEX: u16 = encode_unit_windex(AFEX_STATE_UNIT_ID, UAC2_CONTROL_INTERFACE);

/// `wIndex` for AFEX live VU meter telemetry unit `0x11` on Interface `0x00` (`0x1100`).
pub const AFEX_VU_WINDEX: u16 = encode_unit_windex(AFEX_VU_UNIT_ID, UAC2_CONTROL_INTERFACE);

// --- PiecewiseLinear(0.842105263) UI Fader Scaling ---

/// Knee position for `PiecewiseLinear(0.842105263)` mixer fader law (`0.842105263` = `16/19` ≈ `0.0 dB` unity knee).
pub const MIXER_FADER_KNEE: f32 = 0.842_105_27;
/// Minimum mixer channel level in dB (`-61.0 dB`).
pub const MIXER_FADER_MIN_DB: f32 = -61.0;
/// Unity mixer channel level in dB (`0.0 dB`).
pub const MIXER_FADER_UNITY_DB: f32 = 0.0;
/// Maximum mixer channel level in dB (`+12.0 dB`).
pub const MIXER_FADER_MAX_DB: f32 = 12.0;
/// Threshold below which mixer levels are formatted as `-inf` (`-60.5 dB`).
pub const MIXER_FADER_NEG_INF_THRESHOLD_DB: f32 = -60.5;

/// Convert normalized fader position `t` (`0.0..=1.0`) to decibels (`-61.0..=+12.0` dB)
/// using `PiecewiseLinear(0.842105263)`.
pub fn fader_normalized_to_db(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t <= MIXER_FADER_KNEE {
        let ratio = t / MIXER_FADER_KNEE;
        MIXER_FADER_MIN_DB + ratio * (MIXER_FADER_UNITY_DB - MIXER_FADER_MIN_DB)
    } else {
        let ratio = (t - MIXER_FADER_KNEE) / (1.0 - MIXER_FADER_KNEE);
        MIXER_FADER_UNITY_DB + ratio * (MIXER_FADER_MAX_DB - MIXER_FADER_UNITY_DB)
    }
}

/// Convert decibels (`-61.0..=+12.0` dB) to normalized fader position `t` (`0.0..=1.0`)
/// using `PiecewiseLinear(0.842105263)`.
pub fn fader_db_to_normalized(db: f32) -> f32 {
    let db = db.clamp(MIXER_FADER_MIN_DB, MIXER_FADER_MAX_DB);
    if db <= MIXER_FADER_UNITY_DB {
        let ratio = (db - MIXER_FADER_MIN_DB) / (MIXER_FADER_UNITY_DB - MIXER_FADER_MIN_DB);
        (ratio * MIXER_FADER_KNEE).clamp(0.0, 1.0)
    } else {
        let ratio = (db - MIXER_FADER_UNITY_DB) / (MIXER_FADER_MAX_DB - MIXER_FADER_UNITY_DB);
        (MIXER_FADER_KNEE + ratio * (1.0 - MIXER_FADER_KNEE)).clamp(0.0, 1.0)
    }
}

/// Legacy 0..=1000 fader helper (used in simulator preset slot serialization).
pub fn encode_mixer_fader_db(db: f32) -> u16 {
    let norm = fader_db_to_normalized(db);
    (norm * 1000.0).round() as u16
}

/// Legacy 0..=1000 fader helper (used in simulator preset slot serialization).
pub fn decode_mixer_fader_db(raw: u16) -> f32 {
    let norm = (raw.min(1000) as f32) / 1000.0;
    let db = fader_normalized_to_db(norm);
    (db * 10.0).round() / 10.0
}

/// Legacy 0..=1000 master level helper (used in simulator preset slot serialization).
pub fn encode_master_level_db(db: f32) -> u16 {
    let db = db.clamp(MIXER_FADER_MIN_DB, 0.0);
    let norm = (db - MIXER_FADER_MIN_DB) / (0.0 - MIXER_FADER_MIN_DB);
    (norm * 1000.0).round() as u16
}

/// Legacy 0..=1000 master level helper (used in simulator preset slot serialization).
pub fn decode_master_level_db(raw: u16) -> f32 {
    let norm = (raw.min(1000) as f32) / 1000.0;
    let db = MIXER_FADER_MIN_DB + norm * (0.0 - MIXER_FADER_MIN_DB);
    (db * 10.0).round() / 10.0
}

// --- Verified Hardware Q8.8 dB & Constant-Power Pan Law (0x20, 0x38, 0x55) ---

/// Encode a dB level (`-61.0..=0.0` dB) into hardware signed Q8.8 fixed-point (`1/256 dB`)
/// for `Mixer Output Level` (`param_id = 0x55`). Values `<= -60.5 dB` encode as `-32768` (`-inf`).
pub fn encode_master_level_q88(db: f32) -> i16 {
    let clamped = db.clamp(MIXER_FADER_MIN_DB, 0.0);
    if clamped < MIXER_FADER_NEG_INF_THRESHOLD_DB {
        i16::MIN
    } else {
        (clamped * 256.0).round().clamp(-32767.0, 0.0) as i16
    }
}

/// Decode a hardware signed Q8.8 fixed-point master level (`param_id = 0x55`) into dB (`-61.0..=0.0`).
pub fn decode_master_level_q88(raw: i16) -> f32 {
    if raw == i16::MIN {
        MIXER_FADER_MIN_DB
    } else {
        let db = (raw as f32 / 256.0).clamp(MIXER_FADER_MIN_DB, 0.0);
        (db * 10.0).round() / 10.0
    }
}

/// Encode `(level_db, pan)` into hardware Q8.8 dB `(gain_l, gain_r)` pairs (`0x20`, `0x38`)
/// using the verified constant-power cosine/sine pan law.
pub fn encode_mixer_channel_gains_q88(level_db: f32, pan: i8) -> (i16, i16) {
    let clamped_db = level_db.clamp(MIXER_FADER_MIN_DB, MIXER_FADER_MAX_DB);
    if clamped_db < MIXER_FADER_NEG_INF_THRESHOLD_DB {
        return (i16::MIN, i16::MIN);
    }
    let pan_clamped = pan.clamp(-64, 64);
    let pan_norm = ((pan_clamped as f32) + 64.0) / 128.0;
    let theta = pan_norm * FRAC_PI_2;
    let amp = 10.0_f32.powf(clamped_db / 20.0);

    let l_lin = amp * theta.cos();
    let r_lin = amp * theta.sin();

    let to_q88 = |lin: f32| -> i16 {
        if lin <= 1e-6 {
            i16::MIN
        } else {
            let db = 20.0 * lin.log10();
            if db < -127.0 {
                i16::MIN
            } else {
                (db * 256.0).round().clamp(-32767.0, 32767.0) as i16
            }
        }
    };

    (to_q88(l_lin), to_q88(r_lin))
}

/// Decode hardware Q8.8 dB `(gain_l, gain_r)` pairs (`0x20`, `0x38`) back into `(level_db, pan)`
/// using the inverse constant-power pan law (`hypot` + `atan2`).
pub fn decode_mixer_channel_gains_q88(gain_l: i16, gain_r: i16) -> (f32, i8) {
    let q88_to_lin = |raw: i16| -> f32 {
        if raw == i16::MIN {
            0.0
        } else {
            10.0_f32.powf((raw as f32 / 256.0) / 20.0)
        }
    };
    let l_amp = q88_to_lin(gain_l);
    let r_amp = q88_to_lin(gain_r);
    let total = l_amp.hypot(r_amp);
    if total <= 1e-5 {
        return (MIXER_FADER_MIN_DB, 0);
    }
    let level_db = (20.0 * total.log10()).clamp(MIXER_FADER_MIN_DB, MIXER_FADER_MAX_DB);
    let level_rounded = (level_db * 10.0).round() / 10.0;
    let theta = r_amp.atan2(l_amp);
    let pan_norm = (theta / FRAC_PI_2).clamp(0.0, 1.0);
    let pan = (pan_norm * 128.0 - 64.0).round().clamp(-64.0, 64.0) as i8;
    (level_rounded, pan)
}

/// Format a fader or master level in dB according to the AudioFuse Control Center specification:
/// values `< -60.5 dB` display as `"-inf dB"`, otherwise `"+X.X dB"` or `"-X.X dB"` or `"0.0 dB"`.
pub fn format_db(db: f32) -> String {
    if db < MIXER_FADER_NEG_INF_THRESHOLD_DB {
        "-inf dB".to_string()
    } else if db > 0.05 {
        format!("+{:.1} dB", db)
    } else if db.abs() <= 0.05 {
        "0.0 dB".to_string()
    } else {
        format!("{:.1} dB", db)
    }
}

/// Format a pan value (`-64..=+64`) according to the AudioFuse Control Center specification:
/// `-64` -> `"<64"`, `0` -> `"C"`, `+64` -> `"64>"`.
pub fn format_pan(pan: i8) -> String {
    let pan = pan.clamp(-64, 64);
    if pan < 0 {
        format!("<{}", -pan)
    } else if pan > 0 {
        format!("{}>", pan)
    } else {
        "C".to_string()
    }
}

// --- Verified 24-bit Left-Aligned VU Telemetry Conversions (Unit 0x11, 152 bytes) ---

/// Convert a hardware 24-bit signed left-aligned linear peak `i32` (from Unit `0x11`) into dBFS (`-60.0..=0.0`).
pub fn vu_i32_to_dbfs(raw: i32) -> f32 {
    let sample24 = (raw >> 8).unsigned_abs() as f32;
    let linear = (sample24 / 8_388_607.0).clamp(0.0, 1.0);
    if linear <= 1e-3 {
        -60.0
    } else {
        (20.0 * linear.log10()).clamp(-60.0, 0.0)
    }
}

/// Convert a hardware 24-bit signed left-aligned linear peak `i32` (from Unit `0x11`) into the `0..=127` VU scale
/// matching [`vu_to_dbfs`] (`0 = -60.0 dBFS`, `127 = 0.0 dBFS`).
pub fn vu_i32_to_u8(raw: i32) -> u8 {
    let dbfs = vu_i32_to_dbfs(raw);
    if dbfs <= -59.9 {
        0
    } else {
        (((dbfs + 60.0) / 60.0) * 127.0).round().clamp(0.0, 127.0) as u8
    }
}

/// Convert a `0..=127` VU meter level into a hardware-format 24-bit left-aligned peak `i32` (for simulator).
pub fn vu_u8_to_i32(vu: u8) -> i32 {
    let clamped = vu.min(127);
    if clamped == 0 {
        return 0;
    }
    let dbfs = vu_to_dbfs(clamped);
    let linear = 10.0_f32.powf(dbfs / 20.0).clamp(0.0, 1.0);
    let sample24 = (linear * 8_388_607.0).round() as i32;
    sample24 << 8
}

/// Convert a normalized VU meter value (`0..=127`) to dBFS (`-60.0..=0.0` dBFS).
pub fn vu_to_dbfs(vu: u8) -> f32 {
    let clamped = vu.min(127);
    if clamped == 0 {
        -60.0
    } else {
        let norm = (clamped as f32) / 127.0;
        (-60.0 + norm * 60.0).clamp(-60.0, 0.0)
    }
}

// --- Enumerated Hardware Parameters ---

/// `Device Mode` (`param_id = 0x66`): USB interface vs. standalone 8-channel ADAT expander.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum DeviceMode {
    #[default]
    Usb = 0,
    Adat = 1,
}

impl DeviceMode {
    pub fn from_raw(v: u16) -> Self {
        match v {
            1 => Self::Adat,
            _ => Self::Usb,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Usb => "USB",
            Self::Adat => "ADAT",
        }
    }
}

impl fmt::Display for DeviceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `Clock Source` (UAC2 Clock Selector Entity `40` / `0x28`).
///
/// Verified USB descriptor & ALSA pin mapping:
/// - Pin `1` -> Entity `41` (`0x29`): `Internal Clock`
/// - Pin `2` -> Entity `43` (`0x2B`): `WORD Clock`
/// - Pin `3` -> Entity `44` (`0x2C`): `ADAT Clock`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum ClockSource {
    #[default]
    Internal = 0,
    Adat = 1,
    WordClock = 2,
}

impl ClockSource {
    pub fn from_raw(v: u16) -> Self {
        match v {
            1 => Self::Adat,
            2 => Self::WordClock,
            _ => Self::Internal,
        }
    }

    /// Convert to UAC2 Clock Selector Entity `40` (`0x28`) 1-based pin index:
    /// `1` = Internal (`0x29`), `2` = WORD Clock (`0x2B`), `3` = ADAT Clock (`0x2C`).
    pub fn to_uac2_pin(&self) -> u8 {
        match self {
            Self::Internal => 1,
            Self::WordClock => 2,
            Self::Adat => 3,
        }
    }

    /// Decode from UAC2 Clock Selector Entity `40` (`0x28`) 1-based pin index.
    pub fn from_uac2_pin(pin: u8) -> Self {
        match pin {
            2 => Self::WordClock,
            3 => Self::Adat,
            _ => Self::Internal,
        }
    }

    /// Convert to the named ALSA clock selector enum value (`0 = Internal, 1 = WORD, 2 = ADAT`).
    pub fn to_alsa_enum_index(&self) -> u8 {
        match self {
            Self::Internal => 0,
            Self::WordClock => 1,
            Self::Adat => 2,
        }
    }

    /// Return the corresponding UAC2 Clock Source Entity ID (`41` Internal, `43` WORD, `44` ADAT).
    pub fn uac2_clock_entity(&self) -> u8 {
        match self {
            Self::Internal => UAC2_ENTITY_CLOCK_INTERNAL,
            Self::WordClock => UAC2_ENTITY_CLOCK_WORD,
            Self::Adat => UAC2_ENTITY_CLOCK_ADAT,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Internal => "Internal Clock",
            Self::Adat => "ADAT Clock",
            Self::WordClock => "WORD Clock",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Internal => "INT",
            Self::Adat => "ADAT",
            Self::WordClock => "WC",
        }
    }
}

impl fmt::Display for ClockSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `Sample Rate` (`44100`, `48000`, `88200`, `96000` Hz) on UAC2 Entity `41` (`0x29`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum SampleRate {
    Rate44100 = 0,
    #[default]
    Rate48000 = 1,
    Rate88200 = 2,
    Rate96000 = 3,
}

impl SampleRate {
    pub const ALL: [SampleRate; 4] = [
        SampleRate::Rate44100,
        SampleRate::Rate48000,
        SampleRate::Rate88200,
        SampleRate::Rate96000,
    ];

    pub fn from_enum_idx(idx: u16) -> Self {
        match idx {
            0 => Self::Rate44100,
            1 => Self::Rate48000,
            2 => Self::Rate88200,
            3 => Self::Rate96000,
            _ => Self::Rate48000,
        }
    }

    pub fn from_hz(hz: u32) -> Option<Self> {
        match hz {
            44_100 | 441 => Some(Self::Rate44100),
            48_000 | 48 => Some(Self::Rate48000),
            88_200 | 882 => Some(Self::Rate88200),
            96_000 | 96 => Some(Self::Rate96000),
            _ => None,
        }
    }

    pub fn to_hz(&self) -> u32 {
        match self {
            Self::Rate44100 => 44_100,
            Self::Rate48000 => 48_000,
            Self::Rate88200 => 88_200,
            Self::Rate96000 => 96_000,
        }
    }

    pub fn khz_label(&self) -> &'static str {
        match self {
            Self::Rate44100 => "44.1 kHz",
            Self::Rate48000 => "48.0 kHz",
            Self::Rate88200 => "88.2 kHz",
            Self::Rate96000 => "96.0 kHz",
        }
    }

    pub fn is_double_speed(&self) -> bool {
        matches!(self, Self::Rate88200 | Self::Rate96000)
    }
}

impl fmt::Display for SampleRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} Hz ({})", self.to_hz(), self.khz_label())
    }
}

/// `Wordclock Impedance` (`param_id = 0x70`): `0` = `75 Ohms (On)`, `1` = `HiZ (Off)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum WordclockImpedance {
    #[default]
    Ohms75 = 0,
    HiZ = 1,
}

impl WordclockImpedance {
    pub fn from_raw(v: u16) -> Self {
        match v {
            1 => Self::HiZ,
            _ => Self::Ohms75,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ohms75 => "75 Ohms (On)",
            Self::HiZ => "HiZ (Off)",
        }
    }
}

impl fmt::Display for WordclockImpedance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `SMUX Mode` (`param_id = 0x83`, `ch = 0x00`): `0` = `Normal`, `1` = `Double Speed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum SmuxMode {
    #[default]
    Normal = 0,
    DoubleSpeed = 1,
}

impl SmuxMode {
    pub fn from_raw(v: u16) -> Self {
        match v {
            1 => Self::DoubleSpeed,
            _ => Self::Normal,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::DoubleSpeed => "Double Speed",
        }
    }
}

impl fmt::Display for SmuxMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Analog Input Connector / Impedance Mode.
/// On physical hardware, Mic vs. Line is sensed via `param_id::INPUT_JACK_SENSE` (`0x15` read-only);
/// Channels 1 & 2 support software-switchable Hi-Z `Inst` via `param_id::INPUT_INST` (`0x0d`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum InputType {
    #[default]
    ComboAuto = 0,
    Mic = 1,
    Line = 2,
    Inst = 3,
}

impl InputType {
    pub fn from_raw(v: u16, channel_index: usize) -> Self {
        match v {
            1 => Self::Mic,
            2 => Self::Line,
            3 if channel_index < 2 => Self::Inst,
            3 => Self::Line,
            _ => Self::ComboAuto,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ComboAuto => "Input",
            Self::Mic => "Mic",
            Self::Line => "Line",
            Self::Inst => "Inst",
        }
    }
}

impl fmt::Display for InputType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `Input N PAD Line` (`param_id = 0x0c`): `0` = `Off`, `1` = `-20dB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum LinePad {
    #[default]
    Off = 0,
    Pad20dB = 1,
}

impl LinePad {
    pub fn from_raw(v: u16) -> Self {
        match v {
            1 => Self::Pad20dB,
            _ => Self::Off,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Pad20dB => "-20dB",
        }
    }
}

impl fmt::Display for LinePad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `Input N PAD Mic` (`param_id = 0x04`): `0` = `OFF`, `1` = `PAD (-20dB)`, `2` = `BOOST`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum MicPadBoost {
    #[default]
    Off = 0,
    Pad20dB = 1,
    Boost = 2,
}

impl MicPadBoost {
    pub fn from_raw(v: u16) -> Self {
        match v {
            1 => Self::Pad20dB,
            2 => Self::Boost,
            _ => Self::Off,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Pad20dB => "PAD (-20dB)",
            Self::Boost => "BOOST",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Pad20dB => "-20dB",
            Self::Boost => "BOOST",
        }
    }
}

impl fmt::Display for MicPadBoost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Mixer Bank selector (`Analog 1-8`, `ADAT 1-8`, `USB 1-8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum MixerBank {
    #[default]
    Analog = 0,
    Adat = 1,
    Usb = 2,
}

impl MixerBank {
    pub const ALL: [MixerBank; 3] = [MixerBank::Analog, MixerBank::Adat, MixerBank::Usb];

    pub fn base_index(&self) -> usize {
        match self {
            Self::Analog => 0,
            Self::Adat => 8,
            Self::Usb => 16,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Analog => "Analog In 1-8",
            Self::Adat => "ADAT In 1-8",
            Self::Usb => "USB Play 1-8",
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Analog => "Analog",
            Self::Adat => "ADAT",
            Self::Usb => "USB",
        }
    }
}

/// `Speakers Source` (`param_id = 0x0f`, `ch = 0xff`): `0` = `Main 1-2`, `1` = `Cue Mix`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum SpeakersSource {
    #[default]
    Main1_2 = 0,
    CueMix = 1,
}

impl SpeakersSource {
    pub fn from_raw(v: u16) -> Self {
        match v {
            1 => Self::CueMix,
            _ => Self::Main1_2,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Main1_2 => "Main 1-2",
            Self::CueMix => "Cue Mix",
        }
    }
}

impl fmt::Display for SpeakersSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `Phones Source` (`param_id = 0x10`, `ch = 0xff`): `0` = `Main 1-2`, `1` = `Cue Mix`, `2` = `Follow Spkrs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum PhonesSource {
    #[default]
    Main1_2 = 0,
    CueMix = 1,
    FollowSpeakers = 2,
}

impl PhonesSource {
    pub fn from_raw(v: u16) -> Self {
        match v {
            1 => Self::CueMix,
            2 => Self::FollowSpeakers,
            _ => Self::Main1_2,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Main1_2 => "Main 1-2",
            Self::CueMix => "Cue Mix",
            Self::FollowSpeakers => "Follow Spkrs",
        }
    }
}

impl fmt::Display for PhonesSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `Analog 1-2..7-8 Source` (`param_id = 0x7a`, `ch = 0..3`).
/// Hardware wire encoding is a 1-hot bitmask: `0x0001 = CueMix`, `0x0002 = UsbPair`,
/// `0x0004 = AnalogInPair`, `0x0008 = AdatInPair`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum AnalogOutSource {
    CueMix = 0,
    #[default]
    UsbPair = 1,
    AnalogInPair = 2,
    AdatInPair = 3,
}

impl AnalogOutSource {
    pub const ALL: [AnalogOutSource; 4] = [
        AnalogOutSource::CueMix,
        AnalogOutSource::UsbPair,
        AnalogOutSource::AnalogInPair,
        AnalogOutSource::AdatInPair,
    ];

    /// Decode from enum index (`0..=3`, used in simulator preset format).
    pub fn from_raw(v: u16) -> Self {
        match v {
            0 => Self::CueMix,
            1 => Self::UsbPair,
            2 => Self::AnalogInPair,
            3 => Self::AdatInPair,
            _ => Self::UsbPair,
        }
    }

    /// Encode to verified hardware 1-hot bitmask (`0x0001`, `0x0002`, `0x0004`, `0x0008`).
    pub fn to_wire_mask(&self) -> u16 {
        match self {
            Self::CueMix => 0x0001,
            Self::UsbPair => 0x0002,
            Self::AnalogInPair => 0x0004,
            Self::AdatInPair => 0x0008,
        }
    }

    /// Decode from verified hardware 1-hot bitmask (`0x0001`, `0x0002`, `0x0004`, `0x0008`).
    pub fn from_wire_mask(mask: u16) -> Self {
        match mask {
            0x0001 => Self::CueMix,
            0x0002 => Self::UsbPair,
            0x0004 => Self::AnalogInPair,
            0x0008 => Self::AdatInPair,
            _ => Self::UsbPair,
        }
    }

    pub fn label_for_pair(&self, pair_idx: usize) -> String {
        let ch_start = pair_idx * 2 + 1;
        let ch_end = ch_start + 1;
        match self {
            Self::CueMix => "Cue Mix".to_string(),
            Self::UsbPair => format!("USB {}-{}", ch_start, ch_end),
            Self::AnalogInPair => format!("Analog In {}-{}", ch_start, ch_end),
            Self::AdatInPair => format!("ADAT In {}-{}", ch_start, ch_end),
        }
    }
}

/// `ADAT 1-2..7-8 Source` (`param_id = 0x7e`, `ch = 0..3`).
/// Hardware wire encoding is a 1-hot bitmask: `0x0001 = CueMix`, `0x0002 = UsbPair (9-16)`,
/// `0x0004 = AnalogInPair`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum AdatOutSource {
    CueMix = 0,
    #[default]
    UsbPair = 1,
    AnalogInPair = 2,
}

impl AdatOutSource {
    pub const ALL: [AdatOutSource; 3] = [
        AdatOutSource::CueMix,
        AdatOutSource::UsbPair,
        AdatOutSource::AnalogInPair,
    ];

    /// Decode from enum index (`0..=2`, used in simulator preset format).
    pub fn from_raw(v: u16) -> Self {
        match v {
            0 => Self::CueMix,
            1 => Self::UsbPair,
            2 => Self::AnalogInPair,
            _ => Self::UsbPair,
        }
    }

    /// Encode to verified hardware 1-hot bitmask (`0x0001`, `0x0002`, `0x0004`).
    pub fn to_wire_mask(&self) -> u16 {
        match self {
            Self::CueMix => 0x0001,
            Self::UsbPair => 0x0002,
            Self::AnalogInPair => 0x0004,
        }
    }

    /// Decode from verified hardware 1-hot bitmask (`0x0001`, `0x0002`, `0x0004`).
    pub fn from_wire_mask(mask: u16) -> Self {
        match mask {
            0x0001 => Self::CueMix,
            0x0002 => Self::UsbPair,
            0x0004 => Self::AnalogInPair,
            _ => Self::UsbPair,
        }
    }

    pub fn label_for_pair(&self, pair_idx: usize) -> String {
        let ch_start = pair_idx * 2 + 1;
        let ch_end = ch_start + 1;
        let usb_start = 8 + ch_start;
        let usb_end = 8 + ch_end;
        match self {
            Self::CueMix => "Cue Mix".to_string(),
            Self::UsbPair => format!("USB {}-{}", usb_start, usb_end),
            Self::AnalogInPair => format!("Analog In {}-{}", ch_start, ch_end),
        }
    }
}

/// `Loopback Source` (`param_id = 0x82`, `ch = 0xff`):
/// `0x0000` = `Disabled`, `0x0001` = `Cue Mix`, `0x0002` = `USB 17-18`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum LoopbackSource {
    Disabled = 0,
    #[default]
    CueMix = 1,
    Usb17_18 = 2,
}

impl LoopbackSource {
    pub const ALL: [LoopbackSource; 3] = [
        LoopbackSource::Disabled,
        LoopbackSource::CueMix,
        LoopbackSource::Usb17_18,
    ];

    pub fn from_raw(v: u16) -> Self {
        match v {
            0 => Self::Disabled,
            1 => Self::CueMix,
            2 => Self::Usb17_18,
            _ => Self::Disabled,
        }
    }

    /// Encode to verified hardware bitmask (`0x0000`, `0x0001`, `0x0002`).
    pub fn to_wire_mask(&self) -> u16 {
        *self as u16
    }

    /// Decode from verified hardware bitmask (`0x0000`, `0x0001`, `0x0002`).
    pub fn from_wire_mask(mask: u16) -> Self {
        Self::from_raw(mask)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::CueMix => "Cue Mix",
            Self::Usb17_18 => "USB 17-18",
        }
    }
}

impl fmt::Display for LoopbackSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Truncate or pad an ASCII string to at most 16 characters, returning a 16-byte zero-padded buffer.
pub fn encode_device_string_16(s: &str) -> [u8; MAX_DEVICE_STRING_LEN] {
    let mut buf = [0u8; MAX_DEVICE_STRING_LEN];
    for (i, b) in s.bytes().take(MAX_DEVICE_STRING_LEN).enumerate() {
        buf[i] = if b.is_ascii() && !b.is_ascii_control() {
            b
        } else {
            b'_'
        };
    }
    buf
}

/// Truncate or space-pad an ASCII string to 16 bytes for hardware `Device Name` (`param_id = 0x68`).
pub fn encode_device_name_16(s: &str) -> [u8; MAX_DEVICE_STRING_LEN] {
    let mut buf = [b' '; MAX_DEVICE_STRING_LEN];
    for (i, b) in s.bytes().take(MAX_DEVICE_STRING_LEN).enumerate() {
        buf[i] = if b.is_ascii() && !b.is_ascii_control() {
            b
        } else {
            b'_'
        };
    }
    buf
}

/// Decode a 16-byte zero- or space-padded buffer into a clean Rust `String`.
pub fn decode_device_string_16(buf: &[u8]) -> String {
    let len = buf
        .iter()
        .take(MAX_DEVICE_STRING_LEN)
        .position(|&b| b == 0)
        .unwrap_or_else(|| buf.len().min(MAX_DEVICE_STRING_LEN));
    String::from_utf8_lossy(&buf[..len]).trim_end().to_string()
}
