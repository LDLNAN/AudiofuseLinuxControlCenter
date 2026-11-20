//! High-level `AudioFuseController` (`AudiofuseExStateController` equivalent)
//! coordinating verified AFEX USB parameter transfers (`Unit 0x0a`), atomic 266-byte
//! hardware state snapshots (`Unit 0x0d`), 152-byte live VU telemetry (`Unit 0x11`),
//! UAC2 clock entities (`0x28`, `0x29`, `0x2b`, `0x2c`), stereo linking, channel groups,
//! and local JSON preset import/export.

use crate::firmware::DfuStatusReport;
use crate::preset::{AudioFusePresetFile, DEVICE_PRESET_PAYLOAD_SIZE, PresetError};
use crate::protocol::*;
use crate::state::AudioFuseState;
use crate::transport::{AudioFuseTransport, HardwareDiscoveryInfo, TransportError};
use std::path::Path;
use std::sync::Arc;

/// Telemetry payload emitted by the background polling worker (`subscribe_telemetry`).
#[derive(Debug, Clone)]
pub struct TelemetryFrame {
    pub clock: Option<(ClockSource, [bool; 3], SampleRate)>,
    pub vu_152: [u8; AFEX_VU_SIZE],
    pub state_266: Option<[u8; AFEX_STATE_SIZE]>,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct ApplyReport {
    pub applied: Vec<String>,
    pub skipped: Vec<String>,
}
impl ApplyReport {
    pub fn skipped_summary(&self) -> String {
        if self.skipped.is_empty() {
            "none".into()
        } else {
            self.skipped.join(", ")
        }
    }

    fn record(
        &mut self,
        field: impl Into<String>,
        result: Result<(), TransportError>,
    ) -> Result<(), TransportError> {
        let field = field.into();
        match result {
            Ok(()) => {
                self.applied.push(field);
                Ok(())
            }
            Err(error) => Err(TransportError::Partial {
                applied: self.applied.clone(),
                skipped: self.skipped.clone(),
                failed: field,
                cause: Box::new(error),
            }),
        }
    }
}

/// High-level state controller for the Arturia AudioFuse 8Pre (`1c75:af0e`).
pub struct AudioFuseController {
    transport: Arc<dyn AudioFuseTransport>,
    state: AudioFuseState,
}

impl AudioFuseController {
    /// Create a new `AudioFuseController` over the given transport and synchronize full hardware state.
    pub fn new(transport: Arc<dyn AudioFuseTransport>) -> Result<Self, TransportError> {
        let mut state = AudioFuseState::default();
        let disc = transport.discovery_info();
        if disc.serial_number != "unknown" {
            state.config.serial_number = disc.serial_number.clone();
        }
        if disc.bcd_device != "unknown" {
            state.config.firmware_version = disc.bcd_device.clone();
        }

        let mut controller = Self { transport, state };
        controller.refresh_all()?;
        Ok(controller)
    }

    /// Return a reference to the current `AudioFuseState`.
    pub fn state(&self) -> &AudioFuseState {
        &self.state
    }

    /// Return a mutable reference to the current `AudioFuseState`.
    pub fn state_mut(&mut self) -> &mut AudioFuseState {
        &mut self.state
    }

    /// Return the underlying transport handle (e.g. for off-thread GUI dispatch).
    pub fn transport(&self) -> Arc<dyn AudioFuseTransport> {
        self.transport.clone()
    }

    /// Return the active transport backend name.
    pub fn backend_name(&self) -> &str {
        self.transport.backend_name()
    }

    /// Return whether the physical USB device `1c75:af0e` is detected on the host.
    pub fn is_hardware_detected(&self) -> bool {
        self.transport.is_hardware_detected()
    }

    /// Return whether the active backend is the offline deterministic simulator.
    pub fn is_simulated(&self) -> bool {
        self.transport.is_simulated()
    }

    /// Return host hardware discovery info.
    pub fn discovery_info(&self) -> HardwareDiscoveryInfo {
        self.transport.discovery_info()
    }

    /// Poll the verified 152-byte (`38 × i32`) live VU meters from Unit `0x11` and update `self.state`.
    pub fn poll_vu_meters(&mut self) -> Result<[u8; AFEX_VU_SIZE], TransportError> {
        let vu = self.transport.read_vu_152()?;
        self.state.apply_afex_vu_152(&vu);
        Ok(vu)
    }

    pub fn apply_state_snapshot(
        &mut self,
        snapshot: &[u8; AFEX_STATE_SIZE],
    ) -> Result<(), TransportError> {
        self.state
            .apply_afex_state_266(snapshot)
            .map_err(TransportError::Other)?;
        if self.is_simulated() {
            for input in &mut self.state.inputs {
                if input.input_type == InputType::ComboAuto {
                    input.mic_active = true;
                    input.jack_active = true;
                }
            }
        }
        Ok(())
    }

    /// Poll both the 266-byte state snapshot (`Unit 0x0d`) and 152-byte live VU meters (`Unit 0x11`).
    pub fn poll_state_and_meters(
        &mut self,
    ) -> Result<([u8; AFEX_STATE_SIZE], [u8; AFEX_VU_SIZE]), TransportError> {
        let snap = self.transport.read_state_266()?;
        self.apply_state_snapshot(&snap)?;
        let vu = self.transport.read_vu_152()?;
        self.state.apply_afex_vu_152(&vu);
        Ok((snap, vu))
    }

    /// Spawn a background worker thread that polls live 152-byte VU telemetry every ~30 ms
    /// and a full 266-byte hardware state sync every ~300 ms without blocking the GUI thread.
    pub fn subscribe_telemetry(
        &self,
    ) -> std::sync::mpsc::Receiver<Result<TelemetryFrame, TransportError>> {
        let transport = self.transport.clone();
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut tick: u64 = 0;
            loop {
                let frame = (|| {
                    let (state_266, clock) = if tick % 10 == 0 {
                        (
                            Some(transport.read_state_266()?),
                            Some((
                                transport.get_clock_source()?,
                                transport.get_clock_validity()?,
                                transport.get_sample_rate()?,
                            )),
                        )
                    } else {
                        (None, None)
                    };
                    Ok(TelemetryFrame {
                        vu_152: transport.read_vu_152()?,
                        state_266,
                        clock,
                    })
                })();
                if sender.send(frame).is_err() {
                    break;
                }
                tick = tick.wrapping_add(1);
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        });
        receiver
    }

    /// Perform a full state refresh from the transport (266-byte Unit `0x0d` snapshot + 152-byte VU + UAC2 clock).
    pub fn refresh_all(&mut self) -> Result<(), TransportError> {
        let _ = self.poll_state_and_meters()?;

        let validity = self.transport.get_clock_validity()?;
        self.state.config.clock_validity = validity;

        let src = self.transport.get_clock_source()?;
        self.state.config.clock_source = src;
        self.state.config.clock_locked = match src {
            ClockSource::Internal => self.state.config.clock_validity[0],
            ClockSource::Adat => self.state.config.clock_validity[1],
            ClockSource::WordClock => self.state.config.clock_validity[2],
        };

        let sr = self.transport.get_sample_rate()?;
        self.state.config.sample_rate = sr;

        if self.transport.is_simulated() {
            for i in 0..24 {
                if let Ok(ch_name) = self
                    .transport
                    .get_string(STRING_TYPE_CHANNEL_NAME, i as u16)
                {
                    if !ch_name.is_empty() {
                        self.state.mixer_channels[i].name = ch_name;
                    }
                }
            }
        }
        Ok(())
    }

    fn verify_hardware_state_readback(&mut self) -> Result<(), TransportError> {
        if !self.transport.is_simulated() {
            let snap = self.transport.read_state_266()?;
            self.state
                .apply_afex_state_266(&snap)
                .map_err(TransportError::Other)?;
        }
        Ok(())
    }

    fn confirm_field(
        &mut self,
        label: &str,
        matches: impl FnOnce(&AudioFuseState) -> bool,
    ) -> Result<(), TransportError> {
        self.verify_hardware_state_readback()?;
        if matches(&self.state) {
            Ok(())
        } else {
            Err(TransportError::HardwareRejected(format!(
                "{label} differs on readback"
            )))
        }
    }

    // --- Global & Clock Controls ---

    pub fn set_device_name(&mut self, name: &str) -> Result<(), TransportError> {
        self.transport
            .set_string(STRING_TYPE_DEVICE_NAME, 0, name)?;
        let encoded = encode_device_name_16(name);
        let expected = decode_device_string_16(&encoded);
        if self.is_simulated() {
            self.state.config.device_name.clone_from(&expected);
        }
        self.confirm_field("device name", |s| s.config.device_name == expected)
    }

    pub fn set_device_mode(&mut self, mode: DeviceMode) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::DEVICE_MODE, AFEX_GLOBAL_CHANNEL, mode as u16)?;
        if self.is_simulated() {
            self.state.config.device_mode = mode;
        }
        self.confirm_field("device_mode", |s| s.config.device_mode == mode)
    }

    pub fn set_clock_source(&mut self, source: ClockSource) -> Result<(), TransportError> {
        self.transport.set_clock_source(source)?;
        self.state.config.clock_source = self.transport.get_clock_source()?;
        if self.state.config.clock_source != source {
            return Err(TransportError::HardwareRejected(
                "Clock source differs on readback".into(),
            ));
        }
        if let Ok(v) = self.transport.get_clock_validity() {
            self.state.config.clock_validity = v;
        }
        self.state.config.clock_locked = match source {
            ClockSource::Internal => self.state.config.clock_validity[0],
            ClockSource::Adat => self.state.config.clock_validity[1],
            ClockSource::WordClock => self.state.config.clock_validity[2],
        };
        Ok(())
    }

    /// `Some(reason)` when the sample rate is read-only for this user (live bridge without privilege).
    pub fn sample_rate_read_only_reason(&self) -> Option<String> {
        self.transport.sample_rate_read_only_reason()
    }

    pub fn set_sample_rate(&mut self, rate: SampleRate) -> Result<(), TransportError> {
        if let Some(reason) = self.sample_rate_read_only_reason() {
            return Err(TransportError::ReadOnly(reason));
        }
        self.transport.set_sample_rate(rate)?;
        self.state.config.sample_rate = rate;
        Ok(())
    }

    pub fn set_wordclock_impedance(
        &mut self,
        imp: WordclockImpedance,
    ) -> Result<(), TransportError> {
        self.transport.set_parameter(
            param_id::WORDCLOCK_IMPEDANCE,
            AFEX_GLOBAL_CHANNEL,
            imp as u16,
        )?;
        if self.is_simulated() {
            self.state.config.wordclock_impedance = imp;
        }
        self.confirm_field("wordclock_impedance", |s| {
            s.config.wordclock_impedance == imp
        })
    }

    pub fn set_wordclock_thru(&mut self, enabled: bool) -> Result<(), TransportError> {
        self.transport.set_parameter(
            param_id::WORDCLOCK_THRU,
            AFEX_GLOBAL_CHANNEL,
            u16::from(enabled),
        )?;
        if self.is_simulated() {
            self.state.config.wordclock_thru = enabled;
        }
        self.confirm_field("wordclock_thru", |s| s.config.wordclock_thru == enabled)
    }

    pub fn set_adat_on(&mut self, enabled: bool) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::ADAT_ON, AFEX_GLOBAL_CHANNEL, u16::from(enabled))?;
        if self.is_simulated() {
            self.state.config.adat_on = enabled;
        }
        self.confirm_field("adat_on", |s| s.config.adat_on == enabled)
    }

    pub fn set_smux_mode(&mut self, mode: SmuxMode) -> Result<(), TransportError> {
        // Verified: SMUX Mode (0x83) uses channel = 0x00 on hardware.
        self.transport
            .set_parameter(param_id::SMUX_MODE, 0x00, mode as u16)?;
        if self.is_simulated() {
            self.state.config.smux_mode = mode;
        }
        self.confirm_field("smux_mode", |s| s.config.smux_mode == mode)
    }

    // --- 8 Analog Input Channel Strip Controls ---

    /// Set input connector / impedance mode.
    /// On physical hardware, Mic/Line is sensed by the combo XLR/TRS jack (`0x15`);
    /// only Ch 1-2 `Inst` (Hi-Z) is software-switchable via `param_id::INPUT_INST` (`0x0d`).
    pub fn set_input_type(
        &mut self,
        channel_idx: usize,
        input_type: InputType,
    ) -> Result<(), TransportError> {
        if channel_idx >= 8 {
            return Err(TransportError::InvalidArgument(format!(
                "channel_idx {channel_idx} is outside 0..8"
            )));
        }
        if !self.transport.is_simulated() {
            if channel_idx < 2 && input_type == InputType::Inst {
                return self.set_input_inst(channel_idx, true);
            }
            if channel_idx < 2
                && input_type == InputType::Line
                && self.state.inputs[channel_idx].inst_hi_z
            {
                return self.set_input_inst(channel_idx, false);
            }
            return self
                .transport
                .set_simulated_input_type(channel_idx, input_type);
        }

        if channel_idx >= 2 && input_type == InputType::Inst {
            return Err(TransportError::InvalidArgument(
                "Instrument mode requires input 1 or 2".into(),
            ));
        }
        if !matches!(input_type, InputType::ComboAuto | InputType::Mic)
            && self.state.inputs[channel_idx].pad_mic != MicPadBoost::Off
        {
            self.set_input_mic_pad_boost(channel_idx, MicPadBoost::Off)?;
        }
        if !matches!(input_type, InputType::ComboAuto | InputType::Line)
            && self.state.inputs[channel_idx].pad_line != LinePad::Off
        {
            self.set_input_line_pad(channel_idx, LinePad::Off)?;
        }
        self.transport
            .set_simulated_input_type(channel_idx, input_type)?;
        let inp = &mut self.state.inputs[channel_idx];
        inp.input_type = input_type;
        inp.mic_active = matches!(input_type, InputType::ComboAuto | InputType::Mic);
        inp.jack_active = matches!(
            input_type,
            InputType::ComboAuto | InputType::Line | InputType::Inst
        );
        if channel_idx < 2 {
            let inst_flag = input_type == InputType::Inst;
            self.transport.set_parameter(
                param_id::INPUT_INST,
                channel_idx as u8,
                u16::from(inst_flag),
            )?;
            inp.inst_hi_z = inst_flag;
        }
        Ok(())
    }

    /// Toggle Hi-Z Instrument input mode on Channels 1 & 2 (`param_id = 0x0d`, `ch = 0..1`).
    pub fn set_input_inst(&mut self, channel_idx: usize, hi_z: bool) -> Result<(), TransportError> {
        if channel_idx >= 2 {
            return Err(TransportError::Unsupported(
                "Hi-Z Instrument mode is only available on Analog Inputs 1 and 2".into(),
            ));
        }
        if self.transport.is_simulated() {
            let new_type = if hi_z {
                InputType::Inst
            } else if self.state.inputs[channel_idx].input_type == InputType::Inst {
                InputType::Line
            } else {
                self.state.inputs[channel_idx].input_type
            };
            return self.set_input_type(channel_idx, new_type);
        }

        self.transport
            .set_parameter(param_id::INPUT_INST, channel_idx as u8, u16::from(hi_z))?;
        self.verify_hardware_state_readback()?;
        if self.state.inputs[channel_idx].inst_hi_z != hi_z {
            return Err(TransportError::HardwareRejected(format!(
                "Input {} Inst (Hi-Z) write was ignored by hardware (requires a 1/4\" TRS/TS jack plugged into Input {})",
                channel_idx + 1,
                channel_idx + 1
            )));
        }
        Ok(())
    }

    /// Set +48V Phantom Power for a single analog input (`0..7`, `param_id = 0x13`).
    pub fn set_input_phantom(
        &mut self,
        channel_idx: usize,
        enabled: bool,
    ) -> Result<(), TransportError> {
        if channel_idx >= 8 {
            return Err(TransportError::InvalidArgument(format!(
                "channel_idx {channel_idx} is outside 0..8"
            )));
        }
        self.transport.set_parameter(
            param_id::INPUT_PHANTOM,
            channel_idx as u8,
            u16::from(enabled),
        )?;
        if self.is_simulated() {
            self.state.inputs[channel_idx].phantom_48v = enabled;
        }
        self.confirm_field("phantom_48v", |s| {
            s.inputs[channel_idx].phantom_48v == enabled
        })
    }

    /// Toggle +48V Phantom Power for a bank of 4 channels (`bank = 0` for `1-4`, `bank = 1` for `5-8`).
    pub fn set_phantom_bank_4(&mut self, bank: usize, enabled: bool) -> Result<(), TransportError> {
        if bank > 1 {
            return Err(TransportError::InvalidArgument(
                "Phantom bank must be 0 or 1".into(),
            ));
        }
        let start = bank * 4;
        for ch in start..start + 4 {
            self.set_input_phantom(ch, enabled).map_err(|e| {
                TransportError::Other(format!(
                    "Phantom bank: {} of 4 confirmed; input {} failed: {e}",
                    ch - start,
                    ch + 1
                ))
            })?;
        }
        Ok(())
    }

    /// Set Phase Invert (`Ø`) for an analog input (`0..7`, `param_id = 0x0e`).
    pub fn set_input_phase_invert(
        &mut self,
        channel_idx: usize,
        inverted: bool,
    ) -> Result<(), TransportError> {
        if channel_idx >= 8 {
            return Err(TransportError::InvalidArgument(format!(
                "channel_idx {channel_idx} is outside 0..8"
            )));
        }
        self.transport.set_parameter(
            param_id::INPUT_PHASE_INVERT,
            channel_idx as u8,
            u16::from(inverted),
        )?;
        if self.is_simulated() {
            self.state.inputs[channel_idx].phase_invert = inverted;
        }
        self.confirm_field("phase_invert", |s| {
            s.inputs[channel_idx].phase_invert == inverted
        })
    }

    /// Set `-20dB` Line Pad for an analog input (`0..7`, `param_id = 0x0c`).
    /// Hardware silently gates writes unless a TRS Line jack is physically connected;
    /// verified via read-back on real hardware.
    pub fn set_input_line_pad(
        &mut self,
        channel_idx: usize,
        pad: LinePad,
    ) -> Result<(), TransportError> {
        if channel_idx >= 8 {
            return Err(TransportError::InvalidArgument(format!(
                "channel_idx {channel_idx} is outside 0..8"
            )));
        }
        if self.transport.is_simulated()
            && pad != LinePad::Off
            && !matches!(
                self.state.inputs[channel_idx].input_type,
                InputType::ComboAuto | InputType::Line
            )
        {
            return Err(TransportError::Other(
                "Line pad requires Line or Auto".into(),
            ));
        }
        self.transport
            .set_parameter(param_id::INPUT_PAD_LINE, channel_idx as u8, pad as u16)?;
        if !self.transport.is_simulated() {
            self.verify_hardware_state_readback()?;
            if self.state.inputs[channel_idx].pad_line != pad {
                return Err(TransportError::HardwareRejected(format!(
                    "Input {} Line Pad was ignored by hardware (requires a TRS Line cable plugged into Input {})",
                    channel_idx + 1,
                    channel_idx + 1
                )));
            }
        } else {
            self.state.inputs[channel_idx].pad_line = pad;
        }
        Ok(())
    }

    /// Set Mic Pad / Boost (`OFF` / `PAD (-20dB)` / `BOOST`) for an analog input (`0..7`, `param_id = 0x04`).
    /// Hardware silently gates writes unless an XLR Mic is physically connected;
    /// verified via read-back on real hardware.
    pub fn set_input_mic_pad_boost(
        &mut self,
        channel_idx: usize,
        mode: MicPadBoost,
    ) -> Result<(), TransportError> {
        if channel_idx >= 8 {
            return Err(TransportError::InvalidArgument(format!(
                "channel_idx {channel_idx} is outside 0..8"
            )));
        }
        if self.transport.is_simulated()
            && mode != MicPadBoost::Off
            && !self.state.inputs[channel_idx].mic_active
        {
            return Err(TransportError::Other(
                "Simulator mic pad/boost requires a mic-capable input (Mic or Auto)".into(),
            ));
        }
        self.transport
            .set_parameter(param_id::INPUT_PAD_MIC, channel_idx as u8, mode as u16)?;
        if !self.transport.is_simulated() {
            self.verify_hardware_state_readback()?;
            if self.state.inputs[channel_idx].pad_mic != mode {
                return Err(TransportError::HardwareRejected(format!(
                    "Input {} Mic Pad/Boost was ignored by hardware (requires an XLR microphone plugged into Input {})",
                    channel_idx + 1,
                    channel_idx + 1
                )));
            }
        } else {
            self.state.inputs[channel_idx].pad_mic = mode;
        }
        Ok(())
    }

    // --- 24-Channel Low-Latency Cue Mixer Controls ---

    /// Return the stereo partner channel index (`0..23`) if the given mixer channel is currently stereo-linked.
    pub fn stereo_partner(&self, mixer_idx: usize) -> Option<usize> {
        if mixer_idx >= 24 {
            return None;
        }
        let bank_idx = mixer_idx / 8;
        let in_bank = mixer_idx % 8;
        let pair_idx = in_bank / 2;
        if self.state.stereo_links[bank_idx].get_pair(pair_idx) {
            if in_bank % 2 == 0 {
                Some(mixer_idx + 1)
            } else {
                Some(mixer_idx - 1)
            }
        } else {
            None
        }
    }

    fn linked_and_grouped_channels(&self, mixer_idx: usize) -> Vec<usize> {
        let mut targets = vec![mixer_idx];
        if let Some(partner) = self.stereo_partner(mixer_idx) {
            if !targets.contains(&partner) {
                targets.push(partner);
            }
        }
        let grp = self.state.mixer_channels[mixer_idx].group;
        if (1..=4).contains(&grp) {
            for (i, ch) in self.state.mixer_channels.iter().enumerate() {
                if ch.group == grp && !targets.contains(&i) {
                    targets.push(i);
                }
            }
        }
        targets
    }

    fn write_channel_gain_pan(
        &mut self,
        idx: usize,
        level_db: f32,
        pan: i8,
    ) -> Result<(), TransportError> {
        let clamped_db = level_db.clamp(MIXER_FADER_MIN_DB, MIXER_FADER_MAX_DB);
        let clamped_pan = pan.clamp(-64, 64);
        let (gl, gr) = encode_mixer_channel_gains_q88(clamped_db, clamped_pan);
        self.transport
            .set_parameter(param_id::MIXER_CH_GAIN_L, idx as u8, gl as u16)?;
        self.transport
            .set_parameter(param_id::MIXER_CH_GAIN_R, idx as u8, gr as u16)
            .map_err(|e| {
                TransportError::Other(format!(
                    "Channel {} left gain submitted; right gain failed: {e}",
                    idx + 1
                ))
            })?;
        if !self.is_simulated() {
            let snapshot = self.transport.read_state_266()?;
            self.state
                .apply_afex_state_266(&snapshot)
                .map_err(TransportError::Other)?;
            for (base, expected) in [
                (param_id::MIXER_CH_GAIN_L, gl),
                (param_id::MIXER_CH_GAIN_R, gr),
            ] {
                let offset = (base as usize + idx) * 2;
                if snapshot[offset..offset + 2] != expected.to_le_bytes() {
                    return Err(TransportError::HardwareRejected(format!(
                        "Channel {} gain pair differs on readback",
                        idx + 1
                    )));
                }
            }
        }
        let (decoded_db, decoded_pan) = decode_mixer_channel_gains_q88(gl, gr);
        self.state.mixer_channels[idx].level_db = decoded_db;
        // Preserve chosen pan when fader is pulled all the way to -inf
        if clamped_db < MIXER_FADER_NEG_INF_THRESHOLD_DB {
            self.state.mixer_channels[idx].pan = clamped_pan;
        } else {
            self.state.mixer_channels[idx].pan = decoded_pan;
        }
        Ok(())
    }

    fn write_channel_mute(&mut self, idx: usize, mute: bool) -> Result<(), TransportError> {
        if idx < 16 {
            self.transport.set_parameter(
                param_id::MIXER_MUTE_ANALOG_ADAT,
                idx as u8,
                u16::from(mute),
            )?;
        } else {
            self.transport.set_parameter(
                param_id::MIXER_MUTE_USB,
                (idx - 16) as u8,
                u16::from(mute),
            )?;
        }
        if self.is_simulated() {
            self.state.mixer_channels[idx].mute = mute;
        }
        self.confirm_field("channel mute", |s| s.mixer_channels[idx].mute == mute)
    }

    fn write_channel_solo(&mut self, idx: usize, solo: bool) -> Result<(), TransportError> {
        if idx < 16 {
            self.transport.set_parameter(
                param_id::MIXER_SOLO_ANALOG_ADAT,
                idx as u8,
                u16::from(solo),
            )?;
        } else {
            self.transport.set_parameter(
                param_id::MIXER_SOLO_USB,
                (idx - 16) as u8,
                u16::from(solo),
            )?;
        }
        if self.is_simulated() {
            self.state.mixer_channels[idx].solo = solo;
        }
        self.confirm_field("channel solo", |s| s.mixer_channels[idx].solo == solo)
    }

    /// Set fader level in dB (`-61.0..=+12.0` dB) for a mixer channel (`0..23`), propagating
    /// to stereo-linked partner and any members of its active channel group (`1..4`).
    pub fn set_mixer_channel_level_db(
        &mut self,
        mixer_idx: usize,
        level_db: f32,
    ) -> Result<(), TransportError> {
        if mixer_idx >= 24 {
            return Err(TransportError::InvalidArgument(format!(
                "mixer_idx {mixer_idx} is outside 0..24"
            )));
        }
        if !(MIXER_FADER_MIN_DB..=MIXER_FADER_MAX_DB).contains(&level_db) {
            return Err(TransportError::InvalidArgument(
                "Mixer level must be -61..=12 dB".into(),
            ));
        }
        let delta = level_db - self.state.mixer_channels[mixer_idx].level_db;
        let partner = self.stereo_partner(mixer_idx);

        let targets = self.linked_and_grouped_channels(mixer_idx);
        for (completed, &idx) in targets.iter().enumerate() {
            let level = if idx == mixer_idx || Some(idx) == partner {
                level_db
            } else {
                self.state.mixer_channels[idx].level_db + delta
            };
            let pan = self.state.mixer_channels[idx].pan;
            self.write_channel_gain_pan(idx, level, pan).map_err(|e| {
                TransportError::Other(format!(
                    "Mixer channels confirmed {:?}; failed on index {idx}: {e}",
                    &targets[..completed]
                ))
            })?;
        }
        Ok(())
    }

    /// Set pan (`-64..=+64`) for a mixer channel (`0..23`).
    pub fn set_mixer_channel_pan(
        &mut self,
        mixer_idx: usize,
        pan: i8,
    ) -> Result<(), TransportError> {
        if mixer_idx >= 24 {
            return Err(TransportError::InvalidArgument(format!(
                "mixer_idx {mixer_idx} is outside 0..24"
            )));
        }
        if !(-64..=64).contains(&pan) {
            return Err(TransportError::InvalidArgument(
                "Pan must be -64..=64".into(),
            ));
        }
        let level_db = self.state.mixer_channels[mixer_idx].level_db;
        self.write_channel_gain_pan(mixer_idx, level_db, pan)
    }

    /// Set Mute for a mixer channel (`0..23`), propagating to stereo partner and channel group (`1..4`).
    pub fn set_mixer_channel_mute(
        &mut self,
        mixer_idx: usize,
        mute: bool,
    ) -> Result<(), TransportError> {
        if mixer_idx >= 24 {
            return Err(TransportError::InvalidArgument(format!(
                "mixer_idx {mixer_idx} is outside 0..24"
            )));
        }
        let targets = self.linked_and_grouped_channels(mixer_idx);
        for (completed, &idx) in targets.iter().enumerate() {
            self.write_channel_mute(idx, mute).map_err(|e| {
                TransportError::Other(format!(
                    "Mixer channels confirmed {:?}; failed on index {idx}: {e}",
                    &targets[..completed]
                ))
            })?;
        }
        Ok(())
    }

    /// Set Solo for a mixer channel (`0..23`), propagating to stereo partner and channel group (`1..4`).
    pub fn set_mixer_channel_solo(
        &mut self,
        mixer_idx: usize,
        solo: bool,
    ) -> Result<(), TransportError> {
        if mixer_idx >= 24 {
            return Err(TransportError::InvalidArgument(format!(
                "mixer_idx {mixer_idx} is outside 0..24"
            )));
        }
        let targets = self.linked_and_grouped_channels(mixer_idx);
        for (completed, &idx) in targets.iter().enumerate() {
            self.write_channel_solo(idx, solo).map_err(|e| {
                TransportError::Other(format!(
                    "Mixer channels confirmed {:?}; failed on index {idx}: {e}",
                    &targets[..completed]
                ))
            })?;
        }
        Ok(())
    }

    /// Set custom 16-character channel name (host-side only on real hardware; persisted in simulator).
    pub fn set_mixer_channel_name(
        &mut self,
        mixer_idx: usize,
        name: &str,
    ) -> Result<(), TransportError> {
        if mixer_idx >= 24 {
            return Err(TransportError::InvalidArgument(format!(
                "mixer_idx {mixer_idx} is outside 0..24"
            )));
        }
        self.transport
            .set_string(STRING_TYPE_CHANNEL_NAME, mixer_idx as u16, name)?;
        let encoded = encode_device_string_16(name);
        self.state.mixer_channels[mixer_idx].name = decode_device_string_16(&encoded);
        Ok(())
    }

    /// Assign a mixer channel (`0..23`) to a host-side channel group (`0` = None, `1..=4` = Group 1..4).
    pub fn set_mixer_channel_group(
        &mut self,
        mixer_idx: usize,
        group: u8,
    ) -> Result<(), TransportError> {
        if mixer_idx >= 24 || group > 4 {
            return Err(TransportError::InvalidArgument(
                "Mixer index must be 0..24 and group 0..=4".into(),
            ));
        }
        let grp = group;
        self.state.mixer_channels[mixer_idx].group = grp;
        if let Some(partner) = self.stereo_partner(mixer_idx) {
            self.state.mixer_channels[partner].group = grp;
        }
        Ok(())
    }

    /// Toggle stereo linking for a pair (`0` = `1-2`, `1` = `3-4`, `2` = `5-6`, `3` = `7-8`) in a bank.
    pub fn set_stereo_link(
        &mut self,
        bank: MixerBank,
        pair_idx: usize,
        linked: bool,
    ) -> Result<(), TransportError> {
        if pair_idx >= 4 {
            return Err(TransportError::InvalidArgument(format!(
                "pair_idx {pair_idx} is outside 0..4"
            )));
        }
        let bank_idx = bank as usize;
        let bit_idx = (bank_idx * 4 + pair_idx) as u8;
        let left_idx = bank.base_index() + pair_idx * 2;
        let right_idx = left_idx + 1;

        self.transport
            .set_parameter(param_id::MIXER_STEREO_SHOW, bit_idx, u16::from(linked))?;

        if self.is_simulated() {
            self.state.stereo_links[bank_idx].set_pair(pair_idx, linked);
        }
        self.confirm_field("Stereo link", |s| {
            s.stereo_links[bank_idx].get_pair(pair_idx) == linked
        })?;
        if linked {
            let left_lvl = self.state.mixer_channels[left_idx].level_db;
            let left_mute = self.state.mixer_channels[left_idx].mute;
            let left_solo = self.state.mixer_channels[left_idx].solo;
            self.write_channel_gain_pan(left_idx, left_lvl, -64)
                .map_err(|e| {
                    TransportError::Other(format!(
                        "Stereo link submitted; channel {} gain_pan failed: {e}",
                        left_idx + 1
                    ))
                })?;
            self.write_channel_gain_pan(right_idx, left_lvl, 64)
                .map_err(|e| {
                    TransportError::Other(format!(
                        "Stereo link submitted; channel {} gain_pan failed: {e}",
                        right_idx + 1
                    ))
                })?;
            self.write_channel_mute(right_idx, left_mute).map_err(|e| {
                TransportError::Other(format!(
                    "Stereo link submitted; channel {} mute failed: {e}",
                    right_idx + 1
                ))
            })?;
            self.write_channel_solo(right_idx, left_solo).map_err(|e| {
                TransportError::Other(format!(
                    "Stereo link submitted; channel {} solo failed: {e}",
                    right_idx + 1
                ))
            })?;
            self.state.mixer_channels[right_idx].group = self.state.mixer_channels[left_idx].group;
        } else {
            let left_lvl = self.state.mixer_channels[left_idx].level_db;
            let right_lvl = self.state.mixer_channels[right_idx].level_db;
            self.write_channel_gain_pan(left_idx, left_lvl, 0)
                .map_err(|e| {
                    TransportError::Other(format!(
                        "Stereo link submitted; channel {} gain_pan failed: {e}",
                        left_idx + 1
                    ))
                })?;
            self.write_channel_gain_pan(right_idx, right_lvl, 0)
                .map_err(|e| {
                    TransportError::Other(format!(
                        "Stereo link submitted; channel {} gain_pan failed: {e}",
                        right_idx + 1
                    ))
                })?;
        }

        Ok(())
    }

    /// Clear all Solo states across all 24 mixer channels (and Master Output Solo in simulator).
    pub fn clear_all_solos(&mut self) -> Result<(), TransportError> {
        for idx in 0..24 {
            if self.state.mixer_channels[idx].solo {
                self.write_channel_solo(idx, false)?;
            }
        }
        if self.transport.is_simulated() && self.state.monitor.mixer_output_solo {
            self.set_mixer_output_solo(false)?;
        }
        Ok(())
    }

    // --- Master Cue & Monitor Section ---

    pub fn set_direct_computer_mix(&mut self, mix: f32) -> Result<(), TransportError> {
        self.transport.set_simulated_direct_computer_mix(mix)?;
        let clamped = mix.clamp(0.0, 1.0);
        self.state.monitor.direct_computer_mix = (clamped * 100.0).round() / 100.0;
        Ok(())
    }

    pub fn set_mixer_output_level_db(&mut self, level_db: f32) -> Result<(), TransportError> {
        if !(MIXER_FADER_MIN_DB..=0.0).contains(&level_db) {
            return Err(TransportError::InvalidArgument(
                "Master level must be -61..=0 dB".into(),
            ));
        }
        let raw_q88 = encode_master_level_q88(level_db);
        self.transport.set_parameter(
            param_id::MIXER_OUT_LEVEL,
            AFEX_GLOBAL_CHANNEL,
            raw_q88 as u16,
        )?;
        if self.is_simulated() {
            self.state.monitor.mixer_output_level_db = decode_master_level_q88(raw_q88);
        }
        self.confirm_field("mixer_output_level_db", |s| {
            s.monitor.mixer_output_level_db == decode_master_level_q88(raw_q88)
        })
    }

    pub fn set_mixer_output_mute(&mut self, mute: bool) -> Result<(), TransportError> {
        self.transport.set_simulated_mixer_output_mute(mute)?;
        self.state.monitor.mixer_output_mute = mute;
        Ok(())
    }

    pub fn set_mixer_output_solo(&mut self, solo: bool) -> Result<(), TransportError> {
        self.transport.set_simulated_mixer_output_solo(solo)?;
        self.state.monitor.mixer_output_solo = solo;
        Ok(())
    }

    pub fn set_speakers_source(&mut self, source: SpeakersSource) -> Result<(), TransportError> {
        self.transport.set_parameter(
            param_id::SPEAKERS_SOURCE,
            AFEX_GLOBAL_CHANNEL,
            source as u16,
        )?;
        if self.is_simulated() {
            self.state.monitor.speakers_source = source;
        }
        self.confirm_field("speakers_source", |s| s.monitor.speakers_source == source)
    }

    pub fn set_speakers_mono(&mut self, mono: bool) -> Result<(), TransportError> {
        self.transport.set_parameter(
            param_id::SPEAKERS_MONO,
            AFEX_GLOBAL_CHANNEL,
            u16::from(mono),
        )?;
        if self.is_simulated() {
            self.state.monitor.speakers_mono = mono;
        }
        self.confirm_field("speakers_mono", |s| s.monitor.speakers_mono == mono)
    }

    pub fn set_phones_source(&mut self, source: PhonesSource) -> Result<(), TransportError> {
        self.transport.set_parameter(
            param_id::PHONES_SOURCE,
            AFEX_GLOBAL_CHANNEL,
            source as u16,
        )?;
        if self.is_simulated() {
            self.state.monitor.phones_source = source;
        }
        self.confirm_field("phones_source", |s| s.monitor.phones_source == source)
    }

    pub fn set_phones_mono(&mut self, mono: bool) -> Result<(), TransportError> {
        self.transport.set_parameter(
            param_id::PHONES_MONO,
            AFEX_GLOBAL_CHANNEL,
            u16::from(mono),
        )?;
        if self.is_simulated() {
            self.state.monitor.phones_mono = mono;
        }
        self.confirm_field("phones_mono", |s| s.monitor.phones_mono == mono)
    }

    // --- Output & Loopback Routing Matrix ---

    pub fn set_analog_out_source(
        &mut self,
        pair_idx: usize,
        source: AnalogOutSource,
    ) -> Result<(), TransportError> {
        if pair_idx >= 4 {
            return Err(TransportError::InvalidArgument(format!(
                "pair_idx {pair_idx} is outside 0..4"
            )));
        }
        self.transport.set_parameter(
            param_id::ANALOG_OUT_SOURCE,
            pair_idx as u8,
            source.to_wire_mask(),
        )?;
        if self.is_simulated() {
            self.state.routing.analog_out_sources[pair_idx] = source;
        }
        self.confirm_field("analog_out_sources", |s| {
            s.routing.analog_out_sources[pair_idx] == source
        })
    }

    pub fn set_adat_out_source(
        &mut self,
        pair_idx: usize,
        source: AdatOutSource,
    ) -> Result<(), TransportError> {
        if pair_idx >= 4 {
            return Err(TransportError::InvalidArgument(format!(
                "pair_idx {pair_idx} is outside 0..4"
            )));
        }
        self.transport.set_parameter(
            param_id::ADAT_OUT_SOURCE,
            pair_idx as u8,
            source.to_wire_mask(),
        )?;
        if self.is_simulated() {
            self.state.routing.adat_out_sources[pair_idx] = source;
        }
        self.confirm_field("adat_out_sources", |s| {
            s.routing.adat_out_sources[pair_idx] == source
        })
    }

    pub fn set_loopback_source(&mut self, source: LoopbackSource) -> Result<(), TransportError> {
        self.transport.set_parameter(
            param_id::LOOPBACK_SOURCE,
            AFEX_GLOBAL_CHANNEL,
            source.to_wire_mask(),
        )?;
        if self.is_simulated() {
            self.state.routing.loopback_source = source;
        }
        self.confirm_field("loopback_source", |s| s.routing.loopback_source == source)
    }

    // --- Presets & Persistence ---

    /// Save the current state to a preset slot (`0..=7`, simulator-only; real hardware uses JSON export).
    pub fn save_hardware_preset(&mut self, slot: u8) -> Result<(), TransportError> {
        let payload = AudioFusePresetFile::encode_device_slot_bytes(&self.state);
        self.transport.preset_save(slot, &payload)?;
        self.state.active_preset_slot = slot;
        Ok(())
    }

    /// Load a preset slot (`0..=7`, simulator-only; real hardware uses JSON import).
    pub fn load_hardware_preset(&mut self, slot: u8) -> Result<(), TransportError> {
        let bytes = self
            .transport
            .preset_load(slot, DEVICE_PRESET_PAYLOAD_SIZE)?;
        let mut loaded_state = self.state.clone();
        if AudioFusePresetFile::decode_device_slot_bytes(&bytes, &mut loaded_state) {
            self.apply_full_state(loaded_state)?;
        } else {
            return Err(TransportError::Other("Malformed simulator preset".into()));
        }
        self.state.active_preset_slot = slot;
        Ok(())
    }

    /// Export the current state to a local JSON preset file.
    pub fn export_preset_file(
        &self,
        path: impl AsRef<Path>,
        preset_name: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<AudioFusePresetFile, PresetError> {
        let preset = AudioFusePresetFile::from_state(preset_name, description, self.state.clone());
        preset.save_to_path(path)?;
        Ok(preset)
    }

    /// Import a local JSON preset file and apply all supported parameters to the transport.
    pub fn import_preset_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<(AudioFusePresetFile, ApplyReport), PresetError> {
        let preset = AudioFusePresetFile::load_from_path(path)?;
        let report = self.apply_full_state(preset.state.clone())?;
        Ok((preset, report))
    }

    /// Push an entire `AudioFuseState` to the hardware/simulator transport.
    pub fn apply_full_state(
        &mut self,
        new_state: AudioFuseState,
    ) -> Result<ApplyReport, TransportError> {
        AudioFusePresetFile::validate_state(&new_state)?;
        let mut report = ApplyReport::default();
        if !self.is_simulated() {
            self.refresh_all()?;
            if new_state.config.sample_rate != self.state.config.sample_rate {
                if let Some(reason) = self.sample_rate_read_only_reason() {
                    return Err(TransportError::ReadOnly(format!(
                        "Preset preflight: {reason}; nothing applied"
                    )));
                }
                self.transport.check_rate_change()?;
            }
            for (i, (desired, current)) in
                new_state.inputs.iter().zip(&self.state.inputs).enumerate()
            {
                if ((desired.inst_hi_z != current.inst_hi_z
                    || desired.pad_line != current.pad_line)
                    && !current.jack_active)
                    || (desired.pad_mic != current.pad_mic && !current.mic_active)
                    || (i >= 2 && desired.inst_hi_z)
                {
                    return Err(TransportError::HardwareRejected(format!(
                        "Preset preflight: incompatible connector on input {}; nothing applied",
                        i + 1
                    )));
                }
            }
            report.skipped.extend(
                [
                    "physical input modes",
                    "hardware channel names",
                    "master balance/mute/solo",
                    "hardware preset slot",
                ]
                .map(String::from),
            );
        }
        report.record(
            "device_name",
            self.set_device_name(&new_state.config.device_name),
        )?;
        report.record(
            "device_mode",
            self.set_device_mode(new_state.config.device_mode),
        )?;
        if self.transport.is_simulated()
            || new_state.config.clock_source != self.state.config.clock_source
        {
            report.record(
                "clock_source",
                self.set_clock_source(new_state.config.clock_source),
            )?;
        }
        if self.transport.is_simulated()
            || new_state.config.sample_rate != self.state.config.sample_rate
        {
            report.record(
                "sample_rate",
                self.set_sample_rate(new_state.config.sample_rate),
            )?;
        }
        report.record(
            "wordclock_impedance",
            self.set_wordclock_impedance(new_state.config.wordclock_impedance),
        )?;
        report.record(
            "wordclock_thru",
            self.set_wordclock_thru(new_state.config.wordclock_thru),
        )?;
        report.record("adat_on", self.set_adat_on(new_state.config.adat_on))?;
        report.record("smux_mode", self.set_smux_mode(new_state.config.smux_mode))?;

        for i in 0..8 {
            let inp = &new_state.inputs[i];
            if self.transport.is_simulated() {
                report.record(
                    format!("input_type[{i}]"),
                    self.set_input_type(i, inp.input_type),
                )?;
                if i < 2 {
                    report.record(
                        format!("input_inst[{i}]"),
                        self.set_input_inst(i, inp.inst_hi_z),
                    )?;
                }
                report.record(
                    format!("input_line_pad[{i}]"),
                    self.set_input_line_pad(i, inp.pad_line),
                )?;
                report.record(
                    format!("input_mic_pad_boost[{i}]"),
                    self.set_input_mic_pad_boost(i, inp.pad_mic),
                )?;
            } else {
                // Capability gates were checked against the initial hardware snapshot.
                if i < 2 && inp.inst_hi_z != self.state.inputs[i].inst_hi_z {
                    report.record(
                        format!("input_inst[{i}]"),
                        self.set_input_inst(i, inp.inst_hi_z),
                    )?;
                }
                if inp.pad_line != self.state.inputs[i].pad_line {
                    report.record(
                        format!("input_line_pad[{i}]"),
                        self.set_input_line_pad(i, inp.pad_line),
                    )?;
                }
                if inp.pad_mic != self.state.inputs[i].pad_mic {
                    report.record(
                        format!("input_mic_pad_boost[{i}]"),
                        self.set_input_mic_pad_boost(i, inp.pad_mic),
                    )?;
                }
            }
            report.record(
                format!("input_phantom[{i}]"),
                self.set_input_phantom(i, inp.phantom_48v),
            )?;
            report.record(
                format!("input_phase_invert[{i}]"),
                self.set_input_phase_invert(i, inp.phase_invert),
            )?;
        }

        for bank in 0..3 {
            for pair in 0..4 {
                let linked = new_state.stereo_links[bank].get_pair(pair);
                report.record(
                    format!("stereo link[{bank},{pair}]"),
                    self.transport.set_parameter(
                        param_id::MIXER_STEREO_SHOW,
                        (bank * 4 + pair) as u8,
                        u16::from(linked),
                    ),
                )?;
                if !self.is_simulated() {
                    report.record(
                        format!("stereo link[{bank},{pair}] readback"),
                        self.confirm_field("Stereo link", |s| {
                            s.stereo_links[bank].get_pair(pair) == linked
                        }),
                    )?;
                }
            }
        }
        self.state.stereo_links = new_state.stereo_links;

        for i in 0..24 {
            let ch = &new_state.mixer_channels[i];
            self.state.mixer_channels[i].group = ch.group;
            self.state.mixer_channels[i].show = ch.show;
            if self.transport.is_simulated() {
                report.record(
                    format!("mixer_channel_name[{i}]"),
                    self.set_mixer_channel_name(i, &ch.name),
                )?;
            }
            report.record(
                format!("channel_gain_pan[{i}]"),
                self.write_channel_gain_pan(i, ch.level_db, ch.pan),
            )?;
            report.record(
                format!("channel_mute[{i}]"),
                self.write_channel_mute(i, ch.mute),
            )?;
            report.record(
                format!("channel_solo[{i}]"),
                self.write_channel_solo(i, ch.solo),
            )?;
        }

        if self.transport.is_simulated() {
            report.record(
                "direct_computer_mix",
                self.set_direct_computer_mix(new_state.monitor.direct_computer_mix),
            )?;
            report.record(
                "mixer_output_mute",
                self.set_mixer_output_mute(new_state.monitor.mixer_output_mute),
            )?;
            report.record(
                "mixer_output_solo",
                self.set_mixer_output_solo(new_state.monitor.mixer_output_solo),
            )?;
        }
        report.record(
            "mixer_output_level_db",
            self.set_mixer_output_level_db(new_state.monitor.mixer_output_level_db),
        )?;
        report.record(
            "speakers_source",
            self.set_speakers_source(new_state.monitor.speakers_source),
        )?;
        report.record(
            "speakers_mono",
            self.set_speakers_mono(new_state.monitor.speakers_mono),
        )?;
        report.record(
            "phones_source",
            self.set_phones_source(new_state.monitor.phones_source),
        )?;
        report.record(
            "phones_mono",
            self.set_phones_mono(new_state.monitor.phones_mono),
        )?;

        for pair in 0..4 {
            report.record(
                format!("analog_out_source[{pair}]"),
                self.set_analog_out_source(pair, new_state.routing.analog_out_sources[pair]),
            )?;
            report.record(
                format!("adat_out_source[{pair}]"),
                self.set_adat_out_source(pair, new_state.routing.adat_out_sources[pair]),
            )?;
        }
        report.record(
            "loopback_source",
            self.set_loopback_source(new_state.routing.loopback_source),
        )?;

        if self.is_simulated() {
            self.state.active_preset_slot = new_state.active_preset_slot;
        }
        Ok(report)
    }

    /// Query USB DFU Interface 3 status (blocked on live hardware).
    pub fn dfu_status(&self) -> Result<DfuStatusReport, TransportError> {
        self.transport.get_dfu_status()
    }
}
