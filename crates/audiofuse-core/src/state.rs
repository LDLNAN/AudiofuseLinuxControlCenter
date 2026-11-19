//! Complete hardware state model and 78-byte (`0x4e`) bulk state / VU meter payload
//! encoder and decoder for the Arturia AudioFuse 8Pre (`1c75:af0e`).

use crate::protocol::*;
use serde::{Deserialize, Serialize};

/// Analog Input Channel Strip state (`Input 1` .. `Input 8`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalogInputChannel {
    /// 1-based channel index (`1..=8`).
    pub channel_number: u8,
    /// `Input N Mic` (`0` or `1`, default `1`).
    pub mic_active: bool,
    /// `Input N Jack` (`0` or `1`, default `1`).
    pub jack_active: bool,
    /// `Input N Inst` (`0` or `1`, default `0`, supported on channels 1 & 2 only).
    pub inst_hi_z: bool,
    /// `Input N Type` (`0`: Input, `1`: Mic, `2`: Line, `3`: Inst on Ch 1-2).
    pub input_type: InputType,
    /// `Input N Phantom` (`+48V` phantom power, default `false`).
    pub phantom_48v: bool,
    /// `Input N Phase Invert` (`Ø` polarity invert, default `false`).
    pub phase_invert: bool,
    /// `Input N PAD Line` (`Off` / `-20dB`, default `Off`).
    pub pad_line: LinePad,
    /// `Input N PAD Mic` (`OFF` / `PAD (-20dB)` / `BOOST`, default `OFF`).
    pub pad_mic: MicPadBoost,
    /// Real-time input VU meter (`0..=127`).
    pub vu: u8,
}

impl AnalogInputChannel {
    pub fn new(channel_number: u8) -> Self {
        Self {
            channel_number,
            mic_active: true,
            jack_active: true,
            inst_hi_z: false,
            input_type: InputType::ComboAuto,
            phantom_48v: false,
            phase_invert: false,
            pad_line: LinePad::Off,
            pad_mic: MicPadBoost::Off,
            vu: 0,
        }
    }

    /// Returns whether this channel supports Hi-Z Instrument input (`In 1` & `In 2` only).
    pub fn supports_inst(&self) -> bool {
        self.channel_number == 1 || self.channel_number == 2
    }
}

/// Hardware Low-Latency Cue Mixer Channel (`24` channels total: `0..7` Analog, `8..15` ADAT, `16..23` USB).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CueMixerChannel {
    /// Global 0-based mixer index (`0..=23`).
    pub index: u8,
    /// Bank (`Analog`, `Adat`, `Usb`).
    pub bank: MixerBank,
    /// 1-based channel within the bank (`1..=8`).
    pub bank_channel: u8,
    /// `Mixer Ch Level` in dB (`-61.0..=+12.0`, default `0.0` dB).
    pub level_db: f32,
    /// `Mixer Ch Pan` (`-64..=+64`, default `0` = Center; when stereo-linked, odd = `-64`, even = `+64`).
    pub pan: i8,
    /// `Mixer Ch Mute` (`false` / `true`).
    pub mute: bool,
    /// `Mixer Ch Solo` (`false` / `true`).
    pub solo: bool,
    /// `Mixer Ch Peak` (read-only clip/peak indicator from 78-byte bulk state).
    pub peak: bool,
    /// `Mixer Ch Name` (16 chars max, persisted to device strings / preferences).
    pub name: String,
    /// `Mixer Ch Group` (`0` = None, `1..=4` = Group 1..4).
    pub group: u8,
    /// `Mixer Ch Show` (`true` for Analog by default, `false` for ADAT/USB).
    pub show: bool,
    /// Real-time channel input VU meter (`0..=127`).
    pub vu: u8,
}

impl CueMixerChannel {
    pub fn new(index: u8) -> Self {
        let (bank, bank_channel, show) = if index < 8 {
            (MixerBank::Analog, index + 1, true)
        } else if index < 16 {
            (MixerBank::Adat, index - 8 + 1, false)
        } else {
            (MixerBank::Usb, index - 16 + 1, false)
        };
        let name = format!("{} {}", bank.prefix(), bank_channel);
        Self {
            index,
            bank,
            bank_channel,
            level_db: 0.0,
            pan: 0,
            mute: false,
            solo: false,
            peak: false,
            name,
            group: 0,
            show,
            vu: 0,
        }
    }
}

/// Stereo link state for a mixer bank (`1-2`, `3-4`, `5-6`, `7-8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BankStereoLinks {
    pub pair_1_2: bool,
    pub pair_3_4: bool,
    pub pair_5_6: bool,
    pub pair_7_8: bool,
}

impl BankStereoLinks {
    pub fn get_pair(&self, pair_idx: usize) -> bool {
        match pair_idx {
            0 => self.pair_1_2,
            1 => self.pair_3_4,
            2 => self.pair_5_6,
            3 => self.pair_7_8,
            _ => false,
        }
    }

    pub fn set_pair(&mut self, pair_idx: usize, linked: bool) {
        match pair_idx {
            0 => self.pair_1_2 = linked,
            1 => self.pair_3_4 = linked,
            2 => self.pair_5_6 = linked,
            3 => self.pair_7_8 = linked,
            _ => {}
        }
    }
}

/// Master Cue Mixer & Monitor Section (`Speakers`, `Phones`, `Direct/Computer Mix`, `Master Cue Output`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MasterMonitorSection {
    /// `Direct/Computer Mix` (`0.0` = Direct Cue, `1.0` = Computer USB, default `0.5`).
    pub direct_computer_mix: f32,
    /// `Mixer Output Level` (`-61.0` to `0.0` dB, Linear, `< -60.5` displayed as `-inf`, default `0.0` dB).
    pub mixer_output_level_db: f32,
    /// `Mixer Output Mute` (`false` / `true`).
    pub mixer_output_mute: bool,
    /// `Mixer Output Solo` (`false` / `true`).
    pub mixer_output_solo: bool,
    /// `Mixer Output Main Vu L` / `R` (`0..=127`).
    pub mixer_main_vu_l: u8,
    pub mixer_main_vu_r: u8,
    /// `Speakers Source` (`Main 1-2` / `Cue Mix`, default `Main 1-2`).
    pub speakers_source: SpeakersSource,
    /// `Speakers Mono` (`false` / `true`).
    pub speakers_mono: bool,
    /// `Speakers Left Vu` / `Right Vu` (`0..=127`).
    pub speakers_vu_l: u8,
    pub speakers_vu_r: u8,
    /// `Phones Source` (`Main 1-2` / `Cue Mix` / `Follow Spkrs`, default `Main 1-2`).
    pub phones_source: PhonesSource,
    /// `Phones Mono` (`false` / `true`).
    pub phones_mono: bool,
    /// `Phones Left Vu` / `Right Vu` (`0..=127`).
    pub phones_vu_l: u8,
    pub phones_vu_r: u8,
}

impl Default for MasterMonitorSection {
    fn default() -> Self {
        Self {
            direct_computer_mix: 0.5,
            mixer_output_level_db: 0.0,
            mixer_output_mute: false,
            mixer_output_solo: false,
            mixer_main_vu_l: 0,
            mixer_main_vu_r: 0,
            speakers_source: SpeakersSource::Main1_2,
            speakers_mono: false,
            speakers_vu_l: 0,
            speakers_vu_r: 0,
            phones_source: PhonesSource::Main1_2,
            phones_mono: false,
            phones_vu_l: 0,
            phones_vu_r: 0,
        }
    }
}

/// Output & Loopback Routing Matrix (`Analog Out 1-8`, `ADAT Out 1-8`, `Loopback`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingMatrix {
    /// `Analog 1-2`, `3-4`, `5-6`, `7-8` Source (default `UsbPair`).
    pub analog_out_sources: [AnalogOutSource; 4],
    /// `ADAT 1-2`, `3-4`, `5-6`, `7-8` Source (default `UsbPair`).
    pub adat_out_sources: [AdatOutSource; 4],
    /// `Loopback Source` (default `CueMix`).
    pub loopback_source: LoopbackSource,
    /// `Analog Output 1 Vu` .. `Analog Output 8 Vu` (`0..=127`).
    pub analog_out_vu: [u8; 8],
    /// `ADAT Output 1 Vu` .. `ADAT Output 8 Vu` (`0..=127`).
    pub adat_out_vu: [u8; 8],
    /// `Loopback Left Vu`, `Loopback Right Vu` (`0..=127`).
    pub loopback_vu_l: u8,
    pub loopback_vu_r: u8,
}

impl Default for RoutingMatrix {
    fn default() -> Self {
        Self {
            analog_out_sources: [AnalogOutSource::UsbPair; 4],
            adat_out_sources: [AdatOutSource::UsbPair; 4],
            loopback_source: LoopbackSource::CueMix,
            analog_out_vu: [0; 8],
            adat_out_vu: [0; 8],
            loopback_vu_l: 0,
            loopback_vu_r: 0,
        }
    }
}

/// Global Clock & Device Preferences (`Device Mode`, `Clock Source`, `Sample Rate`, `Wordclock`, `ADAT`, `SMUX`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceConfig {
    /// `Device Name` (16 chars max, default `"AudioFuse 8Pre"`).
    pub device_name: String,
    /// Hardware serial number (e.g. `"8810400336011205"`).
    pub serial_number: String,
    /// Firmware version string (`bcdDevice` / `.af8pre`, e.g. `"1.0.1"` or `"1.0.4"`).
    pub firmware_version: String,
    /// `Device Mode` (`USB` vs `ADAT`).
    pub device_mode: DeviceMode,
    /// `Clock Source` (`Internal`, `ADAT`, `WordClock`).
    pub clock_source: ClockSource,
    /// `Clock Locked` (read-only validity of current clock source).
    pub clock_locked: bool,
    /// Per-source clock validity (`[Internal, ADAT, WordClock]`).
    pub clock_validity: [bool; 3],
    /// `Sample Rate` (`44100`, `48000`, `88200`, `96000`).
    pub sample_rate: SampleRate,
    /// `Wordclock Impedance` (`75 Ohms (On)` vs `HiZ (Off)`).
    pub wordclock_impedance: WordclockImpedance,
    /// `Wordclock Thru` (`false` = Off, `true` = On).
    pub wordclock_thru: bool,
    /// `ADAT On` (`false` = Off, `true` = On, default `true`).
    pub adat_on: bool,
    /// `SMUX Mode` (`Normal` vs `Double Speed`).
    pub smux_mode: SmuxMode,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            device_name: "AudioFuse 8Pre".to_string(),
            serial_number: "8810400336011205".to_string(),
            firmware_version: "1.0.1".to_string(),
            device_mode: DeviceMode::Usb,
            clock_source: ClockSource::Internal,
            clock_locked: true,
            clock_validity: [true, false, false],
            sample_rate: SampleRate::Rate48000,
            wordclock_impedance: WordclockImpedance::Ohms75,
            wordclock_thru: false,
            adat_on: true,
            smux_mode: SmuxMode::Normal,
        }
    }
}

/// Complete state of the Arturia AudioFuse 8Pre (`1c75:af0e`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioFuseState {
    pub config: DeviceConfig,
    pub inputs: [AnalogInputChannel; 8],
    pub adat_in_vu: [u8; 8],
    pub usb_in_vu: [u8; 8],
    pub mixer_channels: Vec<CueMixerChannel>,
    pub stereo_links: [BankStereoLinks; 3],
    pub monitor: MasterMonitorSection,
    pub routing: RoutingMatrix,
    pub active_preset_slot: u8,
}

impl Default for AudioFuseState {
    fn default() -> Self {
        let inputs = std::array::from_fn(|i| AnalogInputChannel::new((i + 1) as u8));
        let mixer_channels = (0..24).map(CueMixerChannel::new).collect();
        Self {
            config: DeviceConfig::default(),
            inputs,
            adat_in_vu: [0; 8],
            usb_in_vu: [0; 8],
            mixer_channels,
            stereo_links: [BankStereoLinks::default(); 3],
            monitor: MasterMonitorSection::default(),
            routing: RoutingMatrix::default(),
            active_preset_slot: 0,
        }
    }
}

impl AudioFuseState {
    /// Synchronize VU meter values across input channels and cue mixer channels.
    pub fn sync_mixer_vu_from_inputs(&mut self) {
        for i in 0..8 {
            if let Some(ch) = self.mixer_channels.get_mut(i) {
                ch.vu = self.inputs[i].vu;
            }
            if let Some(ch) = self.mixer_channels.get_mut(8 + i) {
                ch.vu = self.adat_in_vu[i];
            }
            if let Some(ch) = self.mixer_channels.get_mut(16 + i) {
                ch.vu = self.usb_in_vu[i];
            }
        }
    }

    /// Encode the real-time hardware state and VU meters into the 78-byte (`0x4e`) bulk payload.
    ///
    /// Layout of the 78-byte (`0x4e`) hardware bulk state / VU meter packet:
    /// - `0..8`: `Input Analog 1..8 Vu` (`0..=127`)
    /// - `8..16`: `Input ADAT 1..8 Vu` (`0..=127`)
    /// - `16..24`: `Input USB 1..8 Vu` (`0..=127`)
    /// - `24..32`: `Analog Output 1..8 Vu` (`0..=127`)
    /// - `32..40`: `ADAT Output 1..8 Vu` (`0..=127`)
    /// - `40`: `Mixer Output Main Vu L` (`0..=127`)
    /// - `41`: `Mixer Output Main Vu R` (`0..=127`)
    /// - `42`: `Speakers Left Vu` (`0..=127`)
    /// - `43`: `Speakers Right Vu` (`0..=127`)
    /// - `44`: `Phones Left Vu` (`0..=127`)
    /// - `45`: `Phones Right Vu` (`0..=127`)
    /// - `46`: `Loopback Left Vu` (`0..=127`)
    /// - `47`: `Loopback Right Vu` (`0..=127`)
    /// - `48..51`: 24-bit bitmask of `Mixer Ch Peak` (`Analog 1..8`, `ADAT 1..8`, `USB 1..8`)
    /// - `51`: Hardware status flags (`bit 0` = Clock Locked, `bit 1` = Internal Valid, `bit 2` = ADAT Valid, `bit 3` = WordClock Valid, `bit 4` = ADAT On, `bit 5` = SMUX Double, `bit 6` = WC 75Ohm, `bit 7` = WC Thru)
    /// - `52`: `(sample_rate_idx & 0x03) | ((clock_source_idx & 0x03) << 2) | ((device_mode & 0x01) << 4)`
    /// - `53`: Phantom +48V bitmask (`Input 1..8`)
    /// - `54`: Phase Invert bitmask (`Input 1..8`)
    /// - `55`: Line Pad (`-20dB`) bitmask (`Input 1..8`)
    /// - `56..64`: Input Type & Mic Pad/Boost packed per channel (`low nibble = InputType`, `high nibble = MicPadBoost`, `bit 6 = Inst Hi-Z`)
    /// - `64..67`: 24-bit bitmask of `Mixer Ch Mute` (`0..23`)
    /// - `67..70`: 24-bit bitmask of `Mixer Ch Solo` (`0..23`)
    /// - `70`: Master Monitor flags (`bit 0 = Out Mute`, `bit 1 = Out Solo`, `bit 2 = Spkrs Source`, `bit 3 = Spkrs Mono`, `bits 4..5 = Phones Source`, `bit 6 = Phones Mono`)
    /// - `71`: Analog Out 1-8 Source packed (`2 bits` per stereo pair `0..3`)
    /// - `72`: ADAT Out 1-8 Source packed (`2 bits` per stereo pair `0..3`)
    /// - `73`: Loopback Source (`0..2`)
    /// - `74..76`: Direct/Computer Mix (`u16` LE `0..1000`)
    /// - `76..78`: Master Output Level (`u16` LE `0..1000`)
    pub fn encode_bulk_state_78(&self) -> [u8; BULK_STATE_VU_SIZE] {
        let mut buf = [0u8; BULK_STATE_VU_SIZE];

        for i in 0..8 {
            buf[i] = self.inputs[i].vu.min(127);
            buf[8 + i] = self.adat_in_vu[i].min(127);
            buf[16 + i] = self.usb_in_vu[i].min(127);
            buf[24 + i] = self.routing.analog_out_vu[i].min(127);
            buf[32 + i] = self.routing.adat_out_vu[i].min(127);
        }

        buf[40] = self.monitor.mixer_main_vu_l.min(127);
        buf[41] = self.monitor.mixer_main_vu_r.min(127);
        buf[42] = self.monitor.speakers_vu_l.min(127);
        buf[43] = self.monitor.speakers_vu_r.min(127);
        buf[44] = self.monitor.phones_vu_l.min(127);
        buf[45] = self.monitor.phones_vu_r.min(127);
        buf[46] = self.routing.loopback_vu_l.min(127);
        buf[47] = self.routing.loopback_vu_r.min(127);

        let mut peak_mask: u32 = 0;
        let mut mute_mask: u32 = 0;
        let mut solo_mask: u32 = 0;
        for (idx, ch) in self.mixer_channels.iter().take(24).enumerate() {
            if ch.peak {
                peak_mask |= 1 << idx;
            }
            if ch.mute {
                mute_mask |= 1 << idx;
            }
            if ch.solo {
                solo_mask |= 1 << idx;
            }
        }
        buf[48] = (peak_mask & 0xff) as u8;
        buf[49] = ((peak_mask >> 8) & 0xff) as u8;
        buf[50] = ((peak_mask >> 16) & 0xff) as u8;

        let mut status_flags: u8 = 0;
        if self.config.clock_locked {
            status_flags |= 1 << 0;
        }
        if self.config.clock_validity[0] {
            status_flags |= 1 << 1;
        }
        if self.config.clock_validity[1] {
            status_flags |= 1 << 2;
        }
        if self.config.clock_validity[2] {
            status_flags |= 1 << 3;
        }
        if self.config.adat_on {
            status_flags |= 1 << 4;
        }
        if self.config.smux_mode == SmuxMode::DoubleSpeed {
            status_flags |= 1 << 5;
        }
        if self.config.wordclock_impedance == WordclockImpedance::Ohms75 {
            status_flags |= 1 << 6;
        }
        if self.config.wordclock_thru {
            status_flags |= 1 << 7;
        }
        buf[51] = status_flags;

        buf[52] = (self.config.sample_rate as u8 & 0x03)
            | ((self.config.clock_source as u8 & 0x03) << 2)
            | ((self.config.device_mode as u8 & 0x01) << 4);

        let mut phantom_mask = 0u8;
        let mut phase_mask = 0u8;
        let mut line_pad_mask = 0u8;
        for i in 0..8 {
            let inp = &self.inputs[i];
            if inp.phantom_48v {
                phantom_mask |= 1 << i;
            }
            if inp.phase_invert {
                phase_mask |= 1 << i;
            }
            if inp.pad_line == LinePad::Pad20dB {
                line_pad_mask |= 1 << i;
            }
            let mut ch_byte = (inp.input_type as u8 & 0x03) | ((inp.pad_mic as u8 & 0x03) << 4);
            if inp.inst_hi_z {
                ch_byte |= 1 << 6;
            }
            buf[56 + i] = ch_byte;
        }
        buf[53] = phantom_mask;
        buf[54] = phase_mask;
        buf[55] = line_pad_mask;

        buf[64] = (mute_mask & 0xff) as u8;
        buf[65] = ((mute_mask >> 8) & 0xff) as u8;
        buf[66] = ((mute_mask >> 16) & 0xff) as u8;

        buf[67] = (solo_mask & 0xff) as u8;
        buf[68] = ((solo_mask >> 8) & 0xff) as u8;
        buf[69] = ((solo_mask >> 16) & 0xff) as u8;

        let mut mon_flags = 0u8;
        if self.monitor.mixer_output_mute {
            mon_flags |= 1 << 0;
        }
        if self.monitor.mixer_output_solo {
            mon_flags |= 1 << 1;
        }
        if self.monitor.speakers_source == SpeakersSource::CueMix {
            mon_flags |= 1 << 2;
        }
        if self.monitor.speakers_mono {
            mon_flags |= 1 << 3;
        }
        mon_flags |= (self.monitor.phones_source as u8 & 0x03) << 4;
        if self.monitor.phones_mono {
            mon_flags |= 1 << 6;
        }
        buf[70] = mon_flags;

        let mut analog_src_packed = 0u8;
        let mut adat_src_packed = 0u8;
        for pair in 0..4 {
            analog_src_packed |= (self.routing.analog_out_sources[pair] as u8 & 0x03) << (pair * 2);
            adat_src_packed |= (self.routing.adat_out_sources[pair] as u8 & 0x03) << (pair * 2);
        }
        buf[71] = analog_src_packed;
        buf[72] = adat_src_packed;
        buf[73] = self.routing.loopback_source as u8;

        let mix_raw = (self.monitor.direct_computer_mix.clamp(0.0, 1.0) * 1000.0).round() as u16;
        let out_lvl_raw = encode_master_level_db(self.monitor.mixer_output_level_db);
        buf[74..76].copy_from_slice(&mix_raw.to_le_bytes());
        buf[76..78].copy_from_slice(&out_lvl_raw.to_le_bytes());

        buf
    }

    pub fn apply_meter_bytes(&mut self, buf: &[u8; 78]) {
        for i in 0..8 {
            self.inputs[i].vu = buf[i] & 0x7f;
            self.adat_in_vu[i] = buf[8 + i] & 0x7f;
            self.usb_in_vu[i] = buf[16 + i] & 0x7f;
            self.routing.analog_out_vu[i] = buf[24 + i] & 0x7f;
            self.routing.adat_out_vu[i] = buf[32 + i] & 0x7f;
        }

        self.monitor.mixer_main_vu_l = buf[40] & 0x7f;
        self.monitor.mixer_main_vu_r = buf[41] & 0x7f;
        self.monitor.speakers_vu_l = buf[42] & 0x7f;
        self.monitor.speakers_vu_r = buf[43] & 0x7f;
        self.monitor.phones_vu_l = buf[44] & 0x7f;
        self.monitor.phones_vu_r = buf[45] & 0x7f;
        self.routing.loopback_vu_l = buf[46] & 0x7f;
        self.routing.loopback_vu_r = buf[47] & 0x7f;

        let peak_mask = (buf[48] as u32) | ((buf[49] as u32) << 8) | ((buf[50] as u32) << 16);
        for (idx, ch) in self.mixer_channels.iter_mut().take(24).enumerate() {
            ch.peak = ((peak_mask >> idx) & 1) != 0;
        }

        for ch in &mut self.mixer_channels {
            ch.vu = buf[ch.index as usize] & 127;
        }
    }
    /// Apply a 78-byte (`0x4e`) hardware bulk state / VU meter packet to this state.
    pub fn apply_bulk_state_78(&mut self, buf: &[u8; BULK_STATE_VU_SIZE]) {
        self.apply_meter_bytes(buf);
        let status_flags = buf[51];
        {
            self.config.clock_locked = (status_flags & (1 << 0)) != 0;
            self.config.clock_validity[0] = (status_flags & (1 << 1)) != 0;
            self.config.clock_validity[1] = (status_flags & (1 << 2)) != 0;
            self.config.clock_validity[2] = (status_flags & (1 << 3)) != 0;
            self.config.adat_on = (status_flags & (1 << 4)) != 0;
            self.config.smux_mode = if (status_flags & (1 << 5)) != 0 {
                SmuxMode::DoubleSpeed
            } else {
                SmuxMode::Normal
            };
            self.config.wordclock_impedance = if (status_flags & (1 << 6)) != 0 {
                WordclockImpedance::Ohms75
            } else {
                WordclockImpedance::HiZ
            };
            self.config.wordclock_thru = (status_flags & (1 << 7)) != 0;

            self.config.sample_rate = SampleRate::from_enum_idx((buf[52] & 0x03) as u16);
            self.config.clock_source = ClockSource::from_raw(((buf[52] >> 2) & 0x03) as u16);
            self.config.device_mode = DeviceMode::from_raw(((buf[52] >> 4) & 0x01) as u16);

            let phantom_mask = buf[53];
            let phase_mask = buf[54];
            let line_pad_mask = buf[55];
            for i in 0..8 {
                let inp = &mut self.inputs[i];
                inp.phantom_48v = ((phantom_mask >> i) & 1) != 0;
                inp.phase_invert = ((phase_mask >> i) & 1) != 0;
                inp.pad_line = if ((line_pad_mask >> i) & 1) != 0 {
                    LinePad::Pad20dB
                } else {
                    LinePad::Off
                };
                let ch_byte = buf[56 + i];
                inp.input_type = InputType::from_raw((ch_byte & 0x03) as u16, i);
                inp.pad_mic = MicPadBoost::from_raw(((ch_byte >> 4) & 0x03) as u16);
                inp.inst_hi_z = if i < 2 {
                    ((ch_byte >> 6) & 1) != 0 || inp.input_type == InputType::Inst
                } else {
                    false
                };
            }

            let mute_mask = (buf[64] as u32) | ((buf[65] as u32) << 8) | ((buf[66] as u32) << 16);
            let solo_mask = (buf[67] as u32) | ((buf[68] as u32) << 8) | ((buf[69] as u32) << 16);
            for (idx, ch) in self.mixer_channels.iter_mut().take(24).enumerate() {
                ch.mute = ((mute_mask >> idx) & 1) != 0;
                ch.solo = ((solo_mask >> idx) & 1) != 0;
            }

            let mon_flags = buf[70];
            self.monitor.mixer_output_mute = (mon_flags & (1 << 0)) != 0;
            self.monitor.mixer_output_solo = (mon_flags & (1 << 1)) != 0;
            self.monitor.speakers_source = if (mon_flags & (1 << 2)) != 0 {
                SpeakersSource::CueMix
            } else {
                SpeakersSource::Main1_2
            };
            self.monitor.speakers_mono = (mon_flags & (1 << 3)) != 0;
            self.monitor.phones_source = PhonesSource::from_raw(((mon_flags >> 4) & 0x03) as u16);
            self.monitor.phones_mono = (mon_flags & (1 << 6)) != 0;

            for pair in 0..4 {
                self.routing.analog_out_sources[pair] =
                    AnalogOutSource::from_raw(((buf[71] >> (pair * 2)) & 0x03) as u16);
                self.routing.adat_out_sources[pair] =
                    AdatOutSource::from_raw(((buf[72] >> (pair * 2)) & 0x03) as u16);
            }
            self.routing.loopback_source = LoopbackSource::from_raw(buf[73] as u16);

            let mix_raw = u16::from_le_bytes([buf[74], buf[75]]);
            let out_lvl_raw = u16::from_le_bytes([buf[76], buf[77]]);
            self.monitor.direct_computer_mix =
                ((mix_raw.min(1000) as f32) / 1000.0 * 100.0).round() / 100.0;
            self.monitor.mixer_output_level_db = decode_master_level_db(out_lvl_raw);
        }

        self.sync_mixer_vu_from_inputs();
    }
}
