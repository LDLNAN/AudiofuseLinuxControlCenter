//! Protocol constants, USB control requests, UAC2 entities, parameter IDs,
//! and piecewise-linear dB scaling for the Arturia AudioFuse 8Pre (`1c75:af0e`).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Arturia USB Vendor ID (`0x1c75`).
pub const ARTURIA_VENDOR_ID: u16 = 0x1c75;

/// Arturia AudioFuse 8Pre Product ID (`0xaf0e`).
pub const AUDIOFUSE_8PRE_PRODUCT_ID: u16 = 0xaf0e;

/// Vendor control unit ID (`AFEXDevice` unit `0x0a` / `10`).
pub const AFEX_UNIT_ID: u8 = 0x0a;

/// USB Interface 0: UAC2 Audio Control + Vendor Control Transfers.
pub const UAC2_CONTROL_INTERFACE: u8 = 0x00;

/// USB Interface 3: USB DFU (`bInterfaceClass 0xfe`, `bInterfaceSubClass 0x01`).
pub const DFU_INTERFACE: u8 = 0x03;

/// DFU transfer block size (`wTransferSize = 64` bytes).
pub const DFU_TRANSFER_SIZE: usize = 64;

/// Parameter payload size (`uint16_t` little-endian = 2 bytes).
pub const PARAM_VALUE_SIZE: usize = 2;

/// Bulk hardware state & real-time VU meter payload size (`0x4e` = 78 bytes).
pub const BULK_STATE_VU_SIZE: usize = 0x4e; // 78

/// Maximum ASCII string length stored on device (`Device Name`, `Mixer Ch Name`).
pub const MAX_DEVICE_STRING_LEN: usize = 16;

/// USB `bmRequestType` for Class Interface IN (`0xa1`: Device-to-Host, Class, Interface).
pub const USB_REQ_CLASS_INTERFACE_IN: u8 = 0xa1;

/// USB `bmRequestType` for Class Interface OUT (`0x21`: Host-to-Device, Class, Interface).
pub const USB_REQ_CLASS_INTERFACE_OUT: u8 = 0x21;

/// UAC2 Request Code: `CUR` (`0x01` - Current setting attribute).
pub const UAC2_REQ_CUR: u8 = 0x01;

/// Vendor Request Code base for `GetString(type, index)` (`0x98 + type`).
pub const REQ_GET_STRING_BASE: u8 = 0x98;

/// Vendor Request Code base for `SetString(type, index, str)` (`0x18 + type`).
pub const REQ_SET_STRING_BASE: u8 = 0x18;

/// Vendor Request Code for `PresetLoad(slot, buf, len)` (`0x48`).
pub const REQ_PRESET_LOAD: u8 = 0x48;

/// Vendor Request Code for `PresetSave(slot, buf, len)` (`0x49`).
pub const REQ_PRESET_SAVE: u8 = 0x49;

/// String type `0`: Global device name (`"AudioFuse 8Pre"`).
pub const STRING_TYPE_DEVICE_NAME: u8 = 0x00;

/// String type `1`: Mixer channel name (`idx = 0..23`).
pub const STRING_TYPE_CHANNEL_NAME: u8 = 0x01;

// --- UAC2 Audio Control Entities (Interface 0) ---

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
/// Entity `40` (`0x28`): `"ARTURIA Clock Selector"` (3 pins: 1->41, 2->43, 3->44).
pub const UAC2_ENTITY_CLOCK_SELECTOR: u8 = 0x28;
/// Entity `41` (`0x29`): `"Internal Clock"` (Programmable frequency + validity).
pub const UAC2_ENTITY_CLOCK_INTERNAL: u8 = 0x29;
/// Entity `43` (`0x2B`): `"ADAT Clock"` (External clock validity).
pub const UAC2_ENTITY_CLOCK_ADAT: u8 = 0x2b;
/// Entity `44` (`0x2C`): `"WORD Clock"` (External clock validity).
pub const UAC2_ENTITY_CLOCK_WORD: u8 = 0x2c;

/// UAC2 Clock Selector Control Selector (`CS_CLOCK_SELECTOR_CONTROL = 0x01`).
pub const UAC2_CS_CLOCK_SELECTOR_CONTROL: u8 = 0x01;
/// UAC2 Clock Sampling Frequency Control Selector (`CS_SAM_FREQ_CONTROL = 0x01`).
pub const UAC2_CS_SAM_FREQ_CONTROL: u8 = 0x01;
/// UAC2 Clock Validity Control Selector (`CS_CLOCK_VALID_CONTROL = 0x02`).
pub const UAC2_CS_CLOCK_VALID_CONTROL: u8 = 0x02;

// --- Parameter IDs (`AudiofuseExStateController` table) ---

/// Global parameter IDs (`idx = 0`).
pub mod param_id {
    /// `0x05` (`5`): `Device Mode` (`0` = USB, `1` = ADAT).
    pub const DEVICE_MODE: u8 = 0x05;
    /// `0x06` (`6`): `ADAT On` (`0` = Off, `1` = On).
    pub const ADAT_ON: u8 = 0x06;
    /// `0x08` (`8`): `Wordclock Impedance` (`0` = 75 Ohms On, `1` = HiZ Off).
    pub const WORDCLOCK_IMPEDANCE: u8 = 0x08;
    /// `0x09` (`9`): `Wordclock Thru` (`0` = Off, `1` = On).
    pub const WORDCLOCK_THRU: u8 = 0x09;
    /// `0x0a` (`10`): `SMUX Mode` (`0` = Normal, `1` = Double Speed).
    pub const SMUX_MODE: u8 = 0x0a;

    /// `0x10` (`16`), `idx = 0..7`: Input Type / Mic / Jack state.
    pub const INPUT_TYPE: u8 = 0x10;
    /// `0x11` (`17`), `idx = 0..7`: Input PAD / Boost (Line PAD in bit/byte or enum, Mic PAD/Boost).
    pub const INPUT_PAD_BOOST: u8 = 0x11;
    /// `0x12` (`18`), `idx = 0..7`: Input +48V Phantom Power (`0` = Off, `1` = +48V On).
    pub const INPUT_PHANTOM: u8 = 0x12;
    /// `0x13` (`19`), `idx = 0..7`: Input Phase Invert (`0` = Normal, `1` = Inverted).
    pub const INPUT_PHASE_INVERT: u8 = 0x13;
    /// `0x14` (`20`), `idx = 0..7`: Input Line Pad (`0` = Off, `1` = -20dB).
    pub const INPUT_PAD_LINE: u8 = 0x14;
    /// `0x18` (`24`), `idx = 0..1`: Input 1-2 Instrument (`Hi-Z`) mode.
    pub const INPUT_INST: u8 = 0x18;

    /// `0x20` (`32`), `idx = 0..23`: Cue Mixer Channel Mute (`0..7` Analog, `8..15` ADAT, `16..23` USB).
    pub const MIXER_CH_MUTE: u8 = 0x20;
    /// `0x21` (`33`), `idx = 0..23`: Cue Mixer Channel Solo (`0..7` Analog, `8..15` ADAT, `16..23` USB).
    pub const MIXER_CH_SOLO: u8 = 0x21;
    /// `0x22` (`34`), `idx = 0..23`: Cue Mixer Channel Pan (`-64..+64` encoded as `0..128` or signed `i16`).
    pub const MIXER_CH_PAN: u8 = 0x22;
    /// `0x23` (`35`), `idx = 0..23`: Cue Mixer Channel Level (`PiecewiseLinear(0.842105263)` encoded `0..1000`).
    pub const MIXER_CH_LEVEL: u8 = 0x23;

    /// `0x30` (`48`), `idx = 0`: `Mixer Output Mute`.
    pub const MIXER_OUT_MUTE: u8 = 0x30;
    /// `0x31` (`49`), `idx = 0`: `Mixer Output Level` (`-61.0..0.0` dB).
    pub const MIXER_OUT_LEVEL: u8 = 0x31;
    /// `0x32` (`50`), `idx = 0`: `Mixer Output Solo`.
    pub const MIXER_OUT_SOLO: u8 = 0x32;
    /// `0x33` (`51`), `idx = 0`: `Direct/Computer Mix` (`0.0..1.0` -> `0..1000`).
    pub const DIRECT_COMPUTER_MIX: u8 = 0x33;

    /// `0x38` (`56`), `idx = 0`: `Speakers Source` (`0` = Main 1-2, `1` = Cue Mix).
    pub const SPEAKERS_SOURCE: u8 = 0x38;
    /// `0x39` (`57`), `idx = 0`: `Speakers Mono` (`0` = Stereo, `1` = Mono).
    pub const SPEAKERS_MONO: u8 = 0x39;

    /// `0x40` (`64`), `idx = 0`: `Phones Source` (`0` = Main 1-2, `1` = Cue Mix, `2` = Follow Spkrs).
    pub const PHONES_SOURCE: u8 = 0x40;
    /// `0x41` (`65`), `idx = 0`: `Phones Mono` (`0` = Stereo, `1` = Mono).
    pub const PHONES_MONO: u8 = 0x41;

    /// `0x48` (`72`), `idx = 0..3`: `Analog 1-2..7-8 Source`.
    pub const ANALOG_OUT_SOURCE: u8 = 0x48;
    /// `0x49` (`73`), `idx = 0..3`: `ADAT 1-2..7-8 Source`.
    pub const ADAT_OUT_SOURCE: u8 = 0x49;
    /// `0x4a` (`74`), `idx = 0`: `Loopback Source`.
    pub const LOOPBACK_SOURCE: u8 = 0x4a;
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
/// `wIndex = ((unit_id as u16) << 8) | (interface_num as u16)` (`0x0a00` for `AFEXDevice`).
#[inline]
pub const fn encode_unit_windex(unit_id: u8, interface_num: u8) -> u16 {
    ((unit_id as u16) << 8) | (interface_num as u16)
}

/// Default `wIndex` for `AFEXDevice` unit `0x0a` on Interface `0x00` (`0x0a00`).
pub const AFEX_DEFAULT_WINDEX: u16 = encode_unit_windex(AFEX_UNIT_ID, UAC2_CONTROL_INTERFACE);

// --- PiecewiseLinear(0.842105263) Fader Scaling ---

/// Knee position for `PiecewiseLinear(0.842105263)` mixer fader law (`0.842105263` = `16/19` ≈ `0.0 dB` unity knee).
///
/// The fader range is `-61.0 dB` to `+12.0 dB` (`73.0 dB` total span):
/// - Normalized `t = 0.0` -> `-61.0 dB` (displayed as `-inf` when `< -60.5 dB`)
/// - Normalized `t = 0.842105263` -> `0.0 dB` (unity gain)
/// - Normalized `t = 1.0` -> `+12.0 dB`
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

/// Encode mixer channel level in dB (`-61.0..=+12.0`) to raw 16-bit parameter value (`0..=1000`).
pub fn encode_mixer_fader_db(db: f32) -> u16 {
    let norm = fader_db_to_normalized(db);
    (norm * 1000.0).round() as u16
}

/// Decode raw 16-bit parameter value (`0..=1000`) to mixer channel level in dB (`-61.0..=+12.0`).
pub fn decode_mixer_fader_db(raw: u16) -> f32 {
    let norm = (raw.min(1000) as f32) / 1000.0;
    let db = fader_normalized_to_db(norm);
    (db * 10.0).round() / 10.0
}

/// Encode master output level in dB (`-61.0..=0.0`, Linear) to raw 16-bit parameter value (`0..=1000`).
pub fn encode_master_level_db(db: f32) -> u16 {
    let db = db.clamp(MIXER_FADER_MIN_DB, 0.0);
    let norm = (db - MIXER_FADER_MIN_DB) / (0.0 - MIXER_FADER_MIN_DB);
    (norm * 1000.0).round() as u16
}

/// Decode raw 16-bit parameter value (`0..=1000`) to master output level in dB (`-61.0..=0.0`).
pub fn decode_master_level_db(raw: u16) -> f32 {
    let norm = (raw.min(1000) as f32) / 1000.0;
    let db = MIXER_FADER_MIN_DB + norm * (0.0 - MIXER_FADER_MIN_DB);
    (db * 10.0).round() / 10.0
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

/// Convert raw hardware VU meter value (`0..=127`) to approximate dBFS (`-60.0..=0.0` dBFS).
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

/// `Device Mode` (`param_id = 0x05`): USB interface vs. standalone 8-channel ADAT expander.
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

/// `Clock Source` (`param.xml` enum `0..2`, UAC2 Entity `40` pins `1..3`).
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
    /// `1` = Internal (`0x29`), `2` = ADAT (`0x2B`), `3` = Word Clock (`0x2C`).
    pub fn to_uac2_pin(&self) -> u8 {
        match self {
            Self::Internal => 1,
            Self::Adat => 2,
            Self::WordClock => 3,
        }
    }

    /// Decode from UAC2 Clock Selector Entity `40` (`0x28`) 1-based pin index.
    pub fn from_uac2_pin(pin: u8) -> Self {
        match pin {
            2 => Self::Adat,
            3 => Self::WordClock,
            _ => Self::Internal,
        }
    }

    /// Return the corresponding UAC2 Clock Source Entity ID (`41`, `43`, or `44`).
    pub fn uac2_clock_entity(&self) -> u8 {
        match self {
            Self::Internal => UAC2_ENTITY_CLOCK_INTERNAL,
            Self::Adat => UAC2_ENTITY_CLOCK_ADAT,
            Self::WordClock => UAC2_ENTITY_CLOCK_WORD,
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

/// `Wordclock Impedance` (`param_id = 0x08`): `0` = `75 Ohms (On)`, `1` = `HiZ (Off)`.
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

/// `SMUX Mode` (`param_id = 0x0a`): `0` = `Normal`, `1` = `Double Speed`.
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

/// Analog Input Connector / Impedance Mode (`Input N Type`, `param_id = 0x10`).
/// Channels 1 & 2 support `Inst` (`Hi-Z`, value `3`); Channels 3..8 support `0..2`.
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

/// `Input N PAD Line` (`0`: `Off`, `1`: `-20dB`).
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

/// `Input N PAD Mic` (`0`: `OFF`, `1`: `PAD (-20 dB)`, `2`: `BOOST`).
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

/// `Speakers Source` (`param_id = 0x38`): `0` = `Main 1-2`, `1` = `Cue Mix`.
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

/// `Phones Source` (`param_id = 0x40`): `0` = `Main 1-2`, `1` = `Cue Mix`, `2` = `Follow Spkrs`.
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

/// `Analog 1-2..7-8 Source` (`param_id = 0x48`, `idx = 0..3`).
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

    pub fn from_raw(v: u16) -> Self {
        match v {
            0 => Self::CueMix,
            1 => Self::UsbPair,
            2 => Self::AnalogInPair,
            3 => Self::AdatInPair,
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

/// `ADAT 1-2..7-8 Source` (`param_id = 0x49`, `idx = 0..3`).
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

    pub fn from_raw(v: u16) -> Self {
        match v {
            0 => Self::CueMix,
            1 => Self::UsbPair,
            2 => Self::AnalogInPair,
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

/// `Loopback Source` (`param_id = 0x4a`, `idx = 0`): `0` = `Disabled`, `1` = `Cue Mix`, `2` = `USB 17-18`.
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
            _ => Self::CueMix,
        }
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

/// Decode a 16-byte zero-padded buffer into a clean Rust `String`.
pub fn decode_device_string_16(buf: &[u8]) -> String {
    let len = buf
        .iter()
        .take(MAX_DEVICE_STRING_LEN)
        .position(|&b| b == 0)
        .unwrap_or_else(|| buf.len().min(MAX_DEVICE_STRING_LEN));
    String::from_utf8_lossy(&buf[..len]).trim_end().to_string()
}
