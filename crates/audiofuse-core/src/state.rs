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

    /// Encode this state into the verified 266-byte (`0x010a`) Unit `0x0d` hardware state snapshot.
    pub fn encode_afex_state_266(&self) -> [u8; AFEX_STATE_SIZE] {
        let mut buf = [0u8; AFEX_STATE_SIZE];
        buf[0..6].copy_from_slice(&AFEX_STATE_MAGIC);

        let write_u16 = |b: &mut [u8], pid: u8, val: u16| {
            let off = (pid as usize) * 2;
            b[off..off + 2].copy_from_slice(&val.to_le_bytes());
        };
        let write_i16 = |b: &mut [u8], pid: u8, val: i16| {
            let off = (pid as usize) * 2;
            b[off..off + 2].copy_from_slice(&val.to_le_bytes());
        };

        let mut pad_line_mask = 0u16;
        let mut inst_mask = 0u16;
        let mut phase_mask = 0u16;
        let mut phantom_mask = 0u16;
        for i in 0..8 {
            let inp = &self.inputs[i];
            let pad_mic_off = (param_id::INPUT_PAD_MIC as usize) * 2 + i * 2;
            buf[pad_mic_off..pad_mic_off + 2].copy_from_slice(&(inp.pad_mic as u16).to_le_bytes());
            if inp.pad_line == LinePad::Pad20dB {
                pad_line_mask |= 1 << i;
            }
            if i < 2 && inp.inst_hi_z {
                inst_mask |= 1 << i;
            }
            if inp.phase_invert {
                phase_mask |= 1 << i;
            }
            if inp.phantom_48v {
                phantom_mask |= 1 << i;
            }
            let sense: u16 = match inp.input_type {
                InputType::Mic => 1,
                InputType::Line | InputType::Inst => 2,
                InputType::ComboAuto => 0,
            };
            let sense_off = (param_id::INPUT_JACK_SENSE as usize) * 2 + i * 2;
            buf[sense_off..sense_off + 2].copy_from_slice(&sense.to_le_bytes());
        }

        write_u16(&mut buf, param_id::INPUT_PAD_LINE, pad_line_mask);
        write_u16(&mut buf, param_id::INPUT_INST, inst_mask);
        write_u16(&mut buf, param_id::INPUT_PHASE_INVERT, phase_mask);
        write_u16(
            &mut buf,
            param_id::SPEAKERS_SOURCE,
            self.monitor.speakers_source as u16,
        );
        write_u16(
            &mut buf,
            param_id::PHONES_SOURCE,
            self.monitor.phones_source as u16,
        );
        write_u16(
            &mut buf,
            param_id::SPEAKERS_MONO,
            u16::from(self.monitor.speakers_mono),
        );
        write_u16(
            &mut buf,
            param_id::PHONES_MONO,
            u16::from(self.monitor.phones_mono),
        );
        write_u16(&mut buf, param_id::INPUT_PHANTOM, phantom_mask);

        let mut mute_aa = 0u16;
        let mut mute_usb = 0u16;
        let mut solo_aa = 0u16;
        let mut solo_usb = 0u16;
        let mut show_aa = 0u16;
        let mut show_usb = 0u16;
        for (i, ch) in self.mixer_channels.iter().take(24).enumerate() {
            let (gl, gr) = encode_mixer_channel_gains_q88(ch.level_db, ch.pan);
            let off_l = (param_id::MIXER_CH_GAIN_L as usize) * 2 + i * 2;
            let off_r = (param_id::MIXER_CH_GAIN_R as usize) * 2 + i * 2;
            buf[off_l..off_l + 2].copy_from_slice(&gl.to_le_bytes());
            buf[off_r..off_r + 2].copy_from_slice(&gr.to_le_bytes());
            if i < 16 {
                if ch.mute {
                    mute_aa |= 1 << i;
                }
                if ch.solo {
                    solo_aa |= 1 << i;
                }
                if ch.show {
                    show_aa |= 1 << i;
                }
            } else {
                let u = i - 16;
                if ch.mute {
                    mute_usb |= 1 << u;
                }
                if ch.solo {
                    solo_usb |= 1 << u;
                }
                if ch.show {
                    show_usb |= 1 << u;
                }
            }
        }
        write_u16(&mut buf, param_id::MIXER_MUTE_ANALOG_ADAT, mute_aa);
        write_u16(&mut buf, param_id::MIXER_MUTE_USB, mute_usb);
        write_u16(&mut buf, param_id::MIXER_SOLO_ANALOG_ADAT, solo_aa);
        write_u16(&mut buf, param_id::MIXER_SOLO_USB, solo_usb);
        write_i16(
            &mut buf,
            param_id::MIXER_OUT_LEVEL,
            encode_master_level_q88(self.monitor.mixer_output_level_db),
        );
        write_u16(&mut buf, param_id::MIXER_SHOW_USB, show_usb);
        write_u16(&mut buf, param_id::MIXER_SHOW_ANALOG_ADAT, show_aa);

        let mut stereo_mask = 0u16;
        for bank in 0..3 {
            for pair in 0..4 {
                if self.stereo_links[bank].get_pair(pair) {
                    stereo_mask |= 1 << (bank * 4 + pair);
                }
            }
        }
        write_u16(&mut buf, param_id::MIXER_STEREO_SHOW, stereo_mask);

        write_u16(
            &mut buf,
            param_id::DEVICE_MODE,
            self.config.device_mode as u16,
        );
        let name_off = (param_id::DEVICE_NAME as usize) * 2;
        buf[name_off..name_off + MAX_DEVICE_STRING_LEN]
            .copy_from_slice(&encode_device_name_16(&self.config.device_name));

        write_u16(
            &mut buf,
            param_id::WORDCLOCK_IMPEDANCE,
            self.config.wordclock_impedance as u16,
        );
        write_u16(
            &mut buf,
            param_id::WORDCLOCK_THRU,
            u16::from(self.config.wordclock_thru),
        );
        let ser_off = (param_id::SERIAL_NUMBER as usize) * 2;
        buf[ser_off..ser_off + MAX_DEVICE_STRING_LEN]
            .copy_from_slice(&encode_device_string_16(&self.config.serial_number));

        for pair in 0..4 {
            let ao_off = (param_id::ANALOG_OUT_SOURCE as usize) * 2 + pair * 2;
            buf[ao_off..ao_off + 2].copy_from_slice(
                &self.routing.analog_out_sources[pair]
                    .to_wire_mask()
                    .to_le_bytes(),
            );
            let ad_off = (param_id::ADAT_OUT_SOURCE as usize) * 2 + pair * 2;
            buf[ad_off..ad_off + 2].copy_from_slice(
                &self.routing.adat_out_sources[pair]
                    .to_wire_mask()
                    .to_le_bytes(),
            );
        }
        write_u16(
            &mut buf,
            param_id::LOOPBACK_SOURCE,
            self.routing.loopback_source.to_wire_mask(),
        );
        write_u16(&mut buf, param_id::SMUX_MODE, self.config.smux_mode as u16);
        write_u16(&mut buf, param_id::ADAT_ON, u16::from(self.config.adat_on));

        buf
    }

    /// Apply a verified 266-byte (`0x010a`) Unit `0x0d` hardware state snapshot to this state.
    pub fn apply_afex_state_266(&mut self, buf: &[u8; AFEX_STATE_SIZE]) -> Result<(), String> {
        if buf[0..6] != AFEX_STATE_MAGIC {
            return Err(format!(
                "Invalid AFEX Unit 0x0d header: {:02x?}",
                &buf[0..6]
            ));
        }
        let read_u16 = |pid: u8| -> u16 {
            let off = (pid as usize) * 2;
            u16::from_le_bytes([buf[off], buf[off + 1]])
        };
        let read_i16 = |pid: u8| -> i16 {
            let off = (pid as usize) * 2;
            i16::from_le_bytes([buf[off], buf[off + 1]])
        };

        let pad_line_mask = read_u16(param_id::INPUT_PAD_LINE);
        let inst_mask = read_u16(param_id::INPUT_INST);
        let phase_mask = read_u16(param_id::INPUT_PHASE_INVERT);
        let phantom_mask = read_u16(param_id::INPUT_PHANTOM);

        for i in 0..8 {
            let inp = &mut self.inputs[i];
            let pad_mic_off = (param_id::INPUT_PAD_MIC as usize) * 2 + i * 2;
            let pad_mic_raw = u16::from_le_bytes([buf[pad_mic_off], buf[pad_mic_off + 1]]);
            inp.pad_mic = MicPadBoost::from_raw(pad_mic_raw);
            inp.pad_line = if ((pad_line_mask >> i) & 1) != 0 {
                LinePad::Pad20dB
            } else {
                LinePad::Off
            };
            inp.inst_hi_z = i < 2 && ((inst_mask >> i) & 1) != 0;
            inp.phase_invert = ((phase_mask >> i) & 1) != 0;
            inp.phantom_48v = ((phantom_mask >> i) & 1) != 0;

            let sense_off = (param_id::INPUT_JACK_SENSE as usize) * 2 + i * 2;
            let sense = u16::from_le_bytes([buf[sense_off], buf[sense_off + 1]]);
            match sense {
                1 => {
                    inp.mic_active = true;
                    inp.jack_active = false;
                    inp.input_type = InputType::Mic;
                }
                2 => {
                    inp.mic_active = false;
                    inp.jack_active = true;
                    inp.input_type = if inp.inst_hi_z {
                        InputType::Inst
                    } else {
                        InputType::Line
                    };
                }
                _ => {
                    if inp.inst_hi_z {
                        inp.mic_active = false;
                        inp.jack_active = true;
                        inp.input_type = InputType::Inst;
                    } else {
                        inp.mic_active = false;
                        inp.jack_active = false;
                        inp.input_type = InputType::ComboAuto;
                    }
                }
            }
        }

        self.monitor.speakers_source =
            SpeakersSource::from_raw(read_u16(param_id::SPEAKERS_SOURCE));
        self.monitor.phones_source = PhonesSource::from_raw(read_u16(param_id::PHONES_SOURCE));
        self.monitor.speakers_mono = read_u16(param_id::SPEAKERS_MONO) != 0;
        self.monitor.phones_mono = read_u16(param_id::PHONES_MONO) != 0;

        let mute_aa = read_u16(param_id::MIXER_MUTE_ANALOG_ADAT);
        let mute_usb = read_u16(param_id::MIXER_MUTE_USB);
        let solo_aa = read_u16(param_id::MIXER_SOLO_ANALOG_ADAT);
        let solo_usb = read_u16(param_id::MIXER_SOLO_USB);
        let show_usb = read_u16(param_id::MIXER_SHOW_USB);
        let show_aa = read_u16(param_id::MIXER_SHOW_ANALOG_ADAT);

        for (i, ch) in self.mixer_channels.iter_mut().take(24).enumerate() {
            let off_l = (param_id::MIXER_CH_GAIN_L as usize) * 2 + i * 2;
            let off_r = (param_id::MIXER_CH_GAIN_R as usize) * 2 + i * 2;
            let gl = i16::from_le_bytes([buf[off_l], buf[off_l + 1]]);
            let gr = i16::from_le_bytes([buf[off_r], buf[off_r + 1]]);
            let (lvl, pan) = decode_mixer_channel_gains_q88(gl, gr);
            ch.level_db = lvl;
            ch.pan = pan;
            if i < 16 {
                ch.mute = ((mute_aa >> i) & 1) != 0;
                ch.solo = ((solo_aa >> i) & 1) != 0;
                ch.show = ((show_aa >> i) & 1) != 0;
            } else {
                let u = i - 16;
                ch.mute = ((mute_usb >> u) & 1) != 0;
                ch.solo = ((solo_usb >> u) & 1) != 0;
                ch.show = ((show_usb >> u) & 1) != 0;
            }
        }

        self.monitor.mixer_output_level_db =
            decode_master_level_q88(read_i16(param_id::MIXER_OUT_LEVEL));

        let stereo_mask = read_u16(param_id::MIXER_STEREO_SHOW);
        for bank in 0..3 {
            for pair in 0..4 {
                self.stereo_links[bank].set_pair(pair, ((stereo_mask >> (bank * 4 + pair)) & 1) != 0);
            }
        }

        self.config.device_mode = DeviceMode::from_raw(read_u16(param_id::DEVICE_MODE));
        let name_off = (param_id::DEVICE_NAME as usize) * 2;
        let name = decode_device_string_16(&buf[name_off..name_off + MAX_DEVICE_STRING_LEN]);
        if !name.is_empty() {
            self.config.device_name = name;
        }
        self.config.wordclock_impedance =
            WordclockImpedance::from_raw(read_u16(param_id::WORDCLOCK_IMPEDANCE));
        self.config.wordclock_thru = read_u16(param_id::WORDCLOCK_THRU) != 0;
        let ser_off = (param_id::SERIAL_NUMBER as usize) * 2;
        let ser = decode_device_string_16(&buf[ser_off..ser_off + MAX_DEVICE_STRING_LEN]);
        if !ser.is_empty() {
            self.config.serial_number = ser;
        }

        for pair in 0..4 {
            let ao_off = (param_id::ANALOG_OUT_SOURCE as usize) * 2 + pair * 2;
            let ao_mask = u16::from_le_bytes([buf[ao_off], buf[ao_off + 1]]);
            self.routing.analog_out_sources[pair] = AnalogOutSource::from_wire_mask(ao_mask);

            let ad_off = (param_id::ADAT_OUT_SOURCE as usize) * 2 + pair * 2;
            let ad_mask = u16::from_le_bytes([buf[ad_off], buf[ad_off + 1]]);
            self.routing.adat_out_sources[pair] = AdatOutSource::from_wire_mask(ad_mask);
        }
        self.routing.loopback_source =
            LoopbackSource::from_wire_mask(read_u16(param_id::LOOPBACK_SOURCE));
        self.config.smux_mode = SmuxMode::from_raw(read_u16(param_id::SMUX_MODE));
        self.config.adat_on = read_u16(param_id::ADAT_ON) != 0;

        Ok(())
    }

    /// Encode current VU levels into the verified 152-byte (`38 × i32` LE) Unit `0x11` VU payload.
    pub fn encode_afex_vu_152(&self) -> [u8; AFEX_VU_SIZE] {
        let mut slots = [0i32; AFEX_VU_CHANNELS];
        for i in 0..8 {
            slots[i] = vu_u8_to_i32(self.inputs[i].vu);
            slots[8 + i] = vu_u8_to_i32(self.adat_in_vu[i]);
            slots[16 + i] = vu_u8_to_i32(self.usb_in_vu[i]);
            slots[24 + i] = vu_u8_to_i32(self.routing.adat_out_vu[i]);
        }
        slots[32] = vu_u8_to_i32(self.routing.loopback_vu_l);
        slots[33] = vu_u8_to_i32(self.routing.loopback_vu_r);
        slots[34] = vu_u8_to_i32(self.monitor.mixer_main_vu_l);
        slots[35] = vu_u8_to_i32(self.monitor.mixer_main_vu_r);
        slots[36] = vu_u8_to_i32(self.monitor.mixer_main_vu_l);
        slots[37] = vu_u8_to_i32(self.monitor.mixer_main_vu_r);

        let mut buf = [0u8; AFEX_VU_SIZE];
        for (i, val) in slots.iter().enumerate() {
            buf[i * 4..i * 4 + 4].copy_from_slice(&val.to_le_bytes());
        }
        buf
    }

    /// Decode a verified 152-byte (`38 × i32` LE) Unit `0x11` live VU payload into all input,
    /// cue mixer, monitor, and routed output VU meters.
    pub fn apply_afex_vu_152(&mut self, buf: &[u8; AFEX_VU_SIZE]) {
        let mut vu = [0u8; AFEX_VU_CHANNELS];
        for i in 0..AFEX_VU_CHANNELS {
            let raw = i32::from_le_bytes([
                buf[i * 4],
                buf[i * 4 + 1],
                buf[i * 4 + 2],
                buf[i * 4 + 3],
            ]);
            vu[i] = vu_i32_to_u8(raw);
        }

        for i in 0..8 {
            self.inputs[i].vu = vu[i];
            self.adat_in_vu[i] = vu[8 + i];
            self.usb_in_vu[i] = vu[16 + i];
        }
        for (idx, ch) in self.mixer_channels.iter_mut().take(24).enumerate() {
            ch.vu = vu[idx];
            ch.peak = vu[idx] >= 126;
        }

        let cue_l = vu[36];
        let cue_r = vu[37];
        self.monitor.mixer_main_vu_l = cue_l;
        self.monitor.mixer_main_vu_r = cue_r;

        let (spk_l, spk_r) = match self.monitor.speakers_source {
            SpeakersSource::Main1_2 => (vu[16], vu[17]),
            SpeakersSource::CueMix => (cue_l, cue_r),
        };
        self.monitor.speakers_vu_l = spk_l;
        self.monitor.speakers_vu_r = spk_r;

        let (ph_l, ph_r) = match self.monitor.phones_source {
            PhonesSource::Main1_2 => (vu[16], vu[17]),
            PhonesSource::CueMix => (cue_l, cue_r),
            PhonesSource::FollowSpeakers => (spk_l, spk_r),
        };
        self.monitor.phones_vu_l = ph_l;
        self.monitor.phones_vu_r = ph_r;

        for pair in 0..4 {
            let ch_l = pair * 2;
            let ch_r = ch_l + 1;
            let (ao_l, ao_r) = match self.routing.analog_out_sources[pair] {
                AnalogOutSource::CueMix => (cue_l, cue_r),
                AnalogOutSource::UsbPair => (vu[16 + ch_l], vu[16 + ch_r]),
                AnalogOutSource::AnalogInPair => (vu[ch_l], vu[ch_r]),
                AnalogOutSource::AdatInPair => (vu[8 + ch_l], vu[8 + ch_r]),
            };
            self.routing.analog_out_vu[ch_l] = ao_l;
            self.routing.analog_out_vu[ch_r] = ao_r;

            let (ad_l, ad_r) = match self.routing.adat_out_sources[pair] {
                AdatOutSource::CueMix => (cue_l, cue_r),
                AdatOutSource::UsbPair => (vu[24 + ch_l], vu[24 + ch_r]),
                AdatOutSource::AnalogInPair => (vu[ch_l], vu[ch_r]),
            };
            self.routing.adat_out_vu[ch_l] = ad_l;
            self.routing.adat_out_vu[ch_r] = ad_r;
        }

        let (lb_l, lb_r) = match self.routing.loopback_source {
            LoopbackSource::Disabled => (0, 0),
            LoopbackSource::CueMix => (cue_l, cue_r),
            LoopbackSource::Usb17_18 => (vu[32], vu[33]),
        };
        self.routing.loopback_vu_l = lb_l;
        self.routing.loopback_vu_r = lb_r;
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
