//! High-level `AudioFuseController` (`AudiofuseExStateController` equivalent)
//! coordinating USB parameter get/set transfers, stereo linking, channel groups 1-4,
//! +48V phantom power groups (`1-4` and `5-8`), 78-byte real-time VU polling,
//! device presets, and local JSON import/export.

use crate::firmware::DfuStatusReport;
use crate::preset::{AudioFusePresetFile, DEVICE_PRESET_PAYLOAD_SIZE, PresetError};
use crate::protocol::*;
use crate::state::AudioFuseState;
use crate::transport::{AudioFuseTransport, HardwareDiscoveryInfo, TransportError};
use std::path::Path;
use std::sync::Arc;

/// High-level state controller for the Arturia AudioFuse 8Pre (`1c75:af0e`).
pub struct AudioFuseController {
    transport: Arc<dyn AudioFuseTransport>,
    state: AudioFuseState,
}

impl AudioFuseController {
    /// Create a new `AudioFuseController` over the given transport and synchronize state.
    pub fn new(transport: Arc<dyn AudioFuseTransport>) -> Result<Self, TransportError> {
        let mut state = AudioFuseState::default();
        let disc = transport.discovery_info();
        state.config.serial_number = disc.serial_number.clone();
        state.config.firmware_version = disc.bcd_device.clone();

        let mut controller = Self { transport, state };
        controller.refresh_all()?;
        Ok(controller)
    }

    /// Return a reference to the current `AudioFuseState`.
    pub fn state(&self) -> &AudioFuseState {
        &self.state
    }

    /// Return a mutable reference to the current `AudioFuseState` (for testing/offline inspection).
    pub fn state_mut(&mut self) -> &mut AudioFuseState {
        &mut self.state
    }

    /// Return the active transport backend name.
    pub fn backend_name(&self) -> &str {
        self.transport.backend_name()
    }

    /// Return whether the physical USB device `1c75:af0e` is detected on the host.
    pub fn is_hardware_detected(&self) -> bool {
        self.transport.is_hardware_detected()
    }

    /// Return host hardware discovery info.
    pub fn discovery_info(&self) -> HardwareDiscoveryInfo {
        self.transport.discovery_info()
    }

    /// Poll the 78-byte (`0x4e`) hardware bulk state and VU meters and update `self.state`.
    pub fn poll_bulk_state_and_meters(
        &mut self,
    ) -> Result<[u8; BULK_STATE_VU_SIZE], TransportError> {
        let bulk = self.transport.read_bulk_state_vu()?;
        self.state.apply_bulk_state_78(&bulk);
        Ok(bulk)
    }

    pub fn subscribe_meters(&self) -> std::sync::mpsc::Receiver<Result<[u8; 78], TransportError>> {
        let transport = self.transport.clone();
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            loop {
                if sender.send(transport.read_bulk_state_vu()).is_err() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        });
        receiver
    }

    /// Perform a full state refresh from the transport (bulk state + strings + clock validity).
    pub fn refresh_all(&mut self) -> Result<(), TransportError> {
        let _ = self.poll_bulk_state_and_meters()?;
        {
            let name = self.transport.get_string(STRING_TYPE_DEVICE_NAME, 0)?;
            if !name.is_empty() {
                self.state.config.device_name = name;
            }
        }
        {
            let validity = self.transport.get_clock_validity()?;
            self.state.config.clock_validity = validity;
        }
        {
            let src = self.transport.get_clock_source()?;
            self.state.config.clock_source = src;
            self.state.config.clock_locked = match src {
                ClockSource::Internal => self.state.config.clock_validity[0],
                ClockSource::Adat => self.state.config.clock_validity[1],
                ClockSource::WordClock => self.state.config.clock_validity[2],
            };
        }
        {
            let sr = self.transport.get_sample_rate()?;
            self.state.config.sample_rate = sr;
        }
        for i in 0..24 {
            let idx = i as u8;
            {
                let raw_lvl = self
                    .transport
                    .get_parameter(param_id::MIXER_CH_LEVEL, idx)?;
                self.state.mixer_channels[i].level_db = decode_mixer_fader_db(raw_lvl);
            }
            {
                let raw_pan = self.transport.get_parameter(param_id::MIXER_CH_PAN, idx)?;
                self.state.mixer_channels[i].pan = (raw_pan as i16).clamp(-64, 64) as i8;
            }
            {
                let ch_name = self
                    .transport
                    .get_string(STRING_TYPE_CHANNEL_NAME, i as u16)?;
                if !ch_name.is_empty() {
                    self.state.mixer_channels[i].name = ch_name;
                }
            }
        }
        Ok(())
    }

    // --- Global & Clock Controls (AF-001, AF-006) ---

    pub fn set_device_name(&mut self, name: &str) -> Result<(), TransportError> {
        self.transport
            .set_string(STRING_TYPE_DEVICE_NAME, 0, name)?;
        let encoded = encode_device_string_16(name);
        self.state.config.device_name = decode_device_string_16(&encoded);
        Ok(())
    }

    pub fn set_device_mode(&mut self, mode: DeviceMode) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::DEVICE_MODE, 0, mode as u16)?;
        self.state.config.device_mode = mode;
        Ok(())
    }

    pub fn set_clock_source(&mut self, source: ClockSource) -> Result<(), TransportError> {
        self.transport.set_clock_source(source)?;
        self.state.config.clock_source = source;
        self.state.config.clock_locked = match source {
            ClockSource::Internal => self.state.config.clock_validity[0],
            ClockSource::Adat => self.state.config.clock_validity[1],
            ClockSource::WordClock => self.state.config.clock_validity[2],
        };
        Ok(())
    }

    pub fn set_sample_rate(&mut self, rate: SampleRate) -> Result<(), TransportError> {
        self.transport.set_sample_rate(rate)?;
        self.state.config.sample_rate = rate;
        Ok(())
    }

    pub fn set_wordclock_impedance(
        &mut self,
        imp: WordclockImpedance,
    ) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::WORDCLOCK_IMPEDANCE, 0, imp as u16)?;
        self.state.config.wordclock_impedance = imp;
        Ok(())
    }

    pub fn set_wordclock_thru(&mut self, enabled: bool) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::WORDCLOCK_THRU, 0, u16::from(enabled))?;
        self.state.config.wordclock_thru = enabled;
        Ok(())
    }

    pub fn set_adat_on(&mut self, enabled: bool) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::ADAT_ON, 0, u16::from(enabled))?;
        self.state.config.adat_on = enabled;
        Ok(())
    }

    pub fn set_smux_mode(&mut self, mode: SmuxMode) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::SMUX_MODE, 0, mode as u16)?;
        self.state.config.smux_mode = mode;
        Ok(())
    }

    // --- 8 Analog Input Channel Strip Controls (AF-002) ---

    /// Set input connector / impedance mode (`Input`, `Mic`, `Line`, or `Inst` on channels 1 & 2).
    pub fn set_input_type(
        &mut self,
        channel_idx: usize,
        input_type: InputType,
    ) -> Result<(), TransportError> {
        if channel_idx >= 8 {
            return Ok(());
        }
        let validated = if channel_idx >= 2 && input_type == InputType::Inst {
            InputType::Line
        } else {
            input_type
        };
        if !matches!(validated, InputType::ComboAuto | InputType::Mic)
            && self.state.inputs[channel_idx].pad_mic != MicPadBoost::Off
        {
            self.set_input_mic_pad_boost(channel_idx, MicPadBoost::Off)?;
        }
        if !matches!(validated, InputType::ComboAuto | InputType::Line)
            && self.state.inputs[channel_idx].pad_line != LinePad::Off
        {
            self.set_input_line_pad(channel_idx, LinePad::Off)?;
        }
        self.transport
            .set_parameter(param_id::INPUT_TYPE, channel_idx as u8, validated as u16)?;
        let inp = &mut self.state.inputs[channel_idx];
        inp.input_type = validated;
        inp.mic_active = matches!(validated, InputType::ComboAuto | InputType::Mic);
        inp.jack_active = matches!(
            validated,
            InputType::ComboAuto | InputType::Line | InputType::Inst
        );
        if channel_idx < 2 {
            let inst_flag = validated == InputType::Inst;
            self.transport.set_parameter(
                param_id::INPUT_INST,
                channel_idx as u8,
                u16::from(inst_flag),
            )?;
            inp.inst_hi_z = inst_flag;
        }
        Ok(())
    }

    /// Toggle Hi-Z Instrument input mode on Channels 1 & 2 (`param_id = 0x18`, `idx = 0..1`).
    pub fn set_input_inst(&mut self, channel_idx: usize, hi_z: bool) -> Result<(), TransportError> {
        if channel_idx >= 2 {
            return Ok(());
        }
        let new_type = if hi_z {
            InputType::Inst
        } else if self.state.inputs[channel_idx].input_type == InputType::Inst {
            InputType::Line
        } else {
            self.state.inputs[channel_idx].input_type
        };
        self.set_input_type(channel_idx, new_type)
    }

    /// Set +48V Phantom Power for a single analog input (`0..7`).
    pub fn set_input_phantom(
        &mut self,
        channel_idx: usize,
        enabled: bool,
    ) -> Result<(), TransportError> {
        if channel_idx >= 8 {
            return Ok(());
        }
        self.transport.set_parameter(
            param_id::INPUT_PHANTOM,
            channel_idx as u8,
            u16::from(enabled),
        )?;
        self.state.inputs[channel_idx].phantom_48v = enabled;
        Ok(())
    }

    /// Toggle +48V Phantom Power for a bank of 4 channels (`bank = 0` for `Inputs 1-4`, `bank = 1` for `Inputs 5-8`).
    pub fn set_phantom_bank_4(&mut self, bank: usize, enabled: bool) -> Result<(), TransportError> {
        let start = if bank == 0 { 0 } else { 4 };
        for ch in start..start + 4 {
            self.set_input_phantom(ch, enabled)?;
        }
        Ok(())
    }

    /// Set Phase Invert (`Ø`) for an analog input (`0..7`).
    pub fn set_input_phase_invert(
        &mut self,
        channel_idx: usize,
        inverted: bool,
    ) -> Result<(), TransportError> {
        if channel_idx >= 8 {
            return Ok(());
        }
        self.transport.set_parameter(
            param_id::INPUT_PHASE_INVERT,
            channel_idx as u8,
            u16::from(inverted),
        )?;
        self.state.inputs[channel_idx].phase_invert = inverted;
        Ok(())
    }

    /// Set `-20dB` Line Pad for an analog input (`0..7`).
    pub fn set_input_line_pad(
        &mut self,
        channel_idx: usize,
        pad: LinePad,
    ) -> Result<(), TransportError> {
        if channel_idx >= 8 {
            return Ok(());
        }
        if pad != LinePad::Off
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
        self.state.inputs[channel_idx].pad_line = pad;
        Ok(())
    }

    /// Set Mic Pad / Boost (`OFF` / `PAD (-20dB)` / `BOOST`) for an analog input (`0..7`).
    pub fn set_input_mic_pad_boost(
        &mut self,
        channel_idx: usize,
        mode: MicPadBoost,
    ) -> Result<(), TransportError> {
        if channel_idx >= 8 {
            return Ok(());
        }
        if mode != MicPadBoost::Off && !self.state.inputs[channel_idx].mic_active {
            return Err(TransportError::Other(
                "Mic pad/boost requires Mic or Auto".into(),
            ));
        }
        self.transport
            .set_parameter(param_id::INPUT_PAD_BOOST, channel_idx as u8, mode as u16)?;
        self.state.inputs[channel_idx].pad_mic = mode;
        Ok(())
    }

    // --- 24-Channel Low-Latency Cue Mixer Controls (AF-003) ---

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

    /// Return all mixer channel indices (`0..23`) that should move together with `mixer_idx`
    /// due to stereo linking or channel grouping (`Groups 1..4`).
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

    /// Set fader level in dB (`-61.0..=+12.0` dB) for a mixer channel (`0..23`), propagating
    /// to stereo-linked partner and any members of its active channel group (`1..4`).
    pub fn set_mixer_channel_level_db(
        &mut self,
        mixer_idx: usize,
        level_db: f32,
    ) -> Result<(), TransportError> {
        if mixer_idx >= 24 {
            return Ok(());
        }
        let clamped_db = level_db.clamp(MIXER_FADER_MIN_DB, MIXER_FADER_MAX_DB);
        let delta = clamped_db - self.state.mixer_channels[mixer_idx].level_db;
        let partner = self.stereo_partner(mixer_idx);

        let targets = self.linked_and_grouped_channels(mixer_idx);
        for idx in targets {
            let level = if idx == mixer_idx || Some(idx) == partner {
                clamped_db
            } else {
                self.state.mixer_channels[idx].level_db + delta
            };
            let raw = encode_mixer_fader_db(level);
            let quantized_db = decode_mixer_fader_db(raw);
            self.transport
                .set_parameter(param_id::MIXER_CH_LEVEL, idx as u8, raw)?;
            self.state.mixer_channels[idx].level_db = quantized_db;
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
            return Ok(());
        }
        let clamped = pan.clamp(-64, 64);
        self.transport.set_parameter(
            param_id::MIXER_CH_PAN,
            mixer_idx as u8,
            (clamped as i16) as u16,
        )?;
        self.state.mixer_channels[mixer_idx].pan = clamped;
        Ok(())
    }

    /// Set Mute for a mixer channel (`0..23`), propagating to stereo partner and channel group (`1..4`).
    pub fn set_mixer_channel_mute(
        &mut self,
        mixer_idx: usize,
        mute: bool,
    ) -> Result<(), TransportError> {
        if mixer_idx >= 24 {
            return Ok(());
        }
        let targets = self.linked_and_grouped_channels(mixer_idx);
        for idx in targets {
            self.transport
                .set_parameter(param_id::MIXER_CH_MUTE, idx as u8, u16::from(mute))?;
            self.state.mixer_channels[idx].mute = mute;
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
            return Ok(());
        }
        let targets = self.linked_and_grouped_channels(mixer_idx);
        for idx in targets {
            self.transport
                .set_parameter(param_id::MIXER_CH_SOLO, idx as u8, u16::from(solo))?;
            self.state.mixer_channels[idx].solo = solo;
        }
        Ok(())
    }

    /// Set persistent 16-character custom channel name (`GetString`/`SetString` type `1`, `index = 0..23`).
    pub fn set_mixer_channel_name(
        &mut self,
        mixer_idx: usize,
        name: &str,
    ) -> Result<(), TransportError> {
        if mixer_idx >= 24 {
            return Ok(());
        }
        self.transport
            .set_string(STRING_TYPE_CHANNEL_NAME, mixer_idx as u16, name)?;
        let encoded = encode_device_string_16(name);
        self.state.mixer_channels[mixer_idx].name = decode_device_string_16(&encoded);
        Ok(())
    }

    /// Assign a mixer channel (`0..23`) to a channel group (`0` = None, `1..=4` = Group 1..4).
    pub fn set_mixer_channel_group(&mut self, mixer_idx: usize, group: u8) {
        if mixer_idx >= 24 {
            return;
        }
        let grp = group.min(4);
        self.state.mixer_channels[mixer_idx].group = grp;
        if let Some(partner) = self.stereo_partner(mixer_idx) {
            self.state.mixer_channels[partner].group = grp;
        }
    }

    /// Toggle stereo linking for a pair (`0` = `1-2`, `1` = `3-4`, `2` = `5-6`, `3` = `7-8`) in a bank.
    ///
    /// As specified in `docs/PROTOCOL.md`:
    /// When stereo linked, odd channel pan defaults to `-64` (`<64`), even channel pan defaults to `+64` (`64>`),
    /// and fader level/mute/solo synchronize to the left (odd) channel.
    /// When unlinked, both channels return to center (`0` = `C`).
    pub fn set_stereo_link(
        &mut self,
        bank: MixerBank,
        pair_idx: usize,
        linked: bool,
    ) -> Result<(), TransportError> {
        if pair_idx >= 4 {
            return Ok(());
        }
        let bank_idx = bank as usize;
        let left_idx = bank.base_index() + pair_idx * 2;
        let right_idx = left_idx + 1;

        if linked {
            self.set_mixer_channel_pan(left_idx, -64)?;
            self.set_mixer_channel_pan(right_idx, 64)?;
            let left_lvl = self.state.mixer_channels[left_idx].level_db;
            let left_mute = self.state.mixer_channels[left_idx].mute;
            let left_solo = self.state.mixer_channels[left_idx].solo;
            for (param, value) in [
                (param_id::MIXER_CH_LEVEL, encode_mixer_fader_db(left_lvl)),
                (param_id::MIXER_CH_MUTE, u16::from(left_mute)),
                (param_id::MIXER_CH_SOLO, u16::from(left_solo)),
            ] {
                self.transport
                    .set_parameter(param, right_idx as u8, value)?;
                let right = &mut self.state.mixer_channels[right_idx];
                match param {
                    param_id::MIXER_CH_LEVEL => right.level_db = decode_mixer_fader_db(value),
                    param_id::MIXER_CH_MUTE => right.mute = left_mute,
                    _ => right.solo = left_solo,
                }
            }
            self.state.mixer_channels[right_idx].group = self.state.mixer_channels[left_idx].group;
        } else {
            self.set_mixer_channel_pan(left_idx, 0)?;
            self.set_mixer_channel_pan(right_idx, 0)?;
        }
        self.state.stereo_links[bank_idx].set_pair(pair_idx, linked);
        Ok(())
    }

    /// Clear all Solo states across all 24 mixer channels and the Master Cue Output.
    pub fn clear_all_solos(&mut self) -> Result<(), TransportError> {
        for idx in 0..24 {
            if self.state.mixer_channels[idx].solo {
                self.transport
                    .set_parameter(param_id::MIXER_CH_SOLO, idx as u8, 0)?;
                self.state.mixer_channels[idx].solo = false;
            }
        }
        if self.state.monitor.mixer_output_solo {
            self.set_mixer_output_solo(false)?;
        }
        Ok(())
    }

    // --- Master Cue & Monitor Section (AF-004) ---

    pub fn set_direct_computer_mix(&mut self, mix: f32) -> Result<(), TransportError> {
        let clamped = mix.clamp(0.0, 1.0);
        let raw = (clamped * 1000.0).round() as u16;
        self.transport
            .set_parameter(param_id::DIRECT_COMPUTER_MIX, 0, raw)?;
        self.state.monitor.direct_computer_mix = (clamped * 100.0).round() / 100.0;
        Ok(())
    }

    pub fn set_mixer_output_level_db(&mut self, level_db: f32) -> Result<(), TransportError> {
        let clamped = level_db.clamp(MIXER_FADER_MIN_DB, 0.0);
        let raw = encode_master_level_db(clamped);
        let quantized = decode_master_level_db(raw);
        self.transport
            .set_parameter(param_id::MIXER_OUT_LEVEL, 0, raw)?;
        self.state.monitor.mixer_output_level_db = quantized;
        Ok(())
    }

    pub fn set_mixer_output_mute(&mut self, mute: bool) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::MIXER_OUT_MUTE, 0, u16::from(mute))?;
        self.state.monitor.mixer_output_mute = mute;
        Ok(())
    }

    pub fn set_mixer_output_solo(&mut self, solo: bool) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::MIXER_OUT_SOLO, 0, u16::from(solo))?;
        self.state.monitor.mixer_output_solo = solo;
        Ok(())
    }

    pub fn set_speakers_source(&mut self, source: SpeakersSource) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::SPEAKERS_SOURCE, 0, source as u16)?;
        self.state.monitor.speakers_source = source;
        Ok(())
    }

    pub fn set_speakers_mono(&mut self, mono: bool) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::SPEAKERS_MONO, 0, u16::from(mono))?;
        self.state.monitor.speakers_mono = mono;
        Ok(())
    }

    pub fn set_phones_source(&mut self, source: PhonesSource) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::PHONES_SOURCE, 0, source as u16)?;
        self.state.monitor.phones_source = source;
        Ok(())
    }

    pub fn set_phones_mono(&mut self, mono: bool) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::PHONES_MONO, 0, u16::from(mono))?;
        self.state.monitor.phones_mono = mono;
        Ok(())
    }

    // --- Output & Loopback Routing Matrix (AF-004) ---

    pub fn set_analog_out_source(
        &mut self,
        pair_idx: usize,
        source: AnalogOutSource,
    ) -> Result<(), TransportError> {
        if pair_idx >= 4 {
            return Ok(());
        }
        self.transport
            .set_parameter(param_id::ANALOG_OUT_SOURCE, pair_idx as u8, source as u16)?;
        self.state.routing.analog_out_sources[pair_idx] = source;
        Ok(())
    }

    pub fn set_adat_out_source(
        &mut self,
        pair_idx: usize,
        source: AdatOutSource,
    ) -> Result<(), TransportError> {
        if pair_idx >= 4 {
            return Ok(());
        }
        self.transport
            .set_parameter(param_id::ADAT_OUT_SOURCE, pair_idx as u8, source as u16)?;
        self.state.routing.adat_out_sources[pair_idx] = source;
        Ok(())
    }

    pub fn set_loopback_source(&mut self, source: LoopbackSource) -> Result<(), TransportError> {
        self.transport
            .set_parameter(param_id::LOOPBACK_SOURCE, 0, source as u16)?;
        self.state.routing.loopback_source = source;
        Ok(())
    }

    // --- Presets & Persistence (AF-006) ---

    /// Save the current state to a hardware preset slot (`0..=7`) via `PresetSave` (`0x49`).
    pub fn save_hardware_preset(&mut self, slot: u8) -> Result<(), TransportError> {
        let payload = AudioFusePresetFile::encode_device_slot_bytes(&self.state);
        self.transport.preset_save(slot, &payload)?;
        self.state.active_preset_slot = slot;
        Ok(())
    }

    /// Load a hardware preset slot (`0..=7`) via `PresetLoad` (`0x48`) and apply all parameters.
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

    /// Import a local JSON preset file and apply all parameters to the transport.
    pub fn import_preset_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<AudioFusePresetFile, PresetError> {
        let preset = AudioFusePresetFile::load_from_path(path)?;
        self.apply_full_state(preset.state.clone())?;
        Ok(preset)
    }

    /// Push an entire `AudioFuseState` to the hardware/simulator transport.
    pub fn apply_full_state(&mut self, new_state: AudioFuseState) -> Result<(), TransportError> {
        AudioFusePresetFile::validate_state(&new_state)?;
        self.set_device_name(&new_state.config.device_name)?;
        self.set_device_mode(new_state.config.device_mode)?;
        self.set_clock_source(new_state.config.clock_source)?;
        self.set_sample_rate(new_state.config.sample_rate)?;
        self.set_wordclock_impedance(new_state.config.wordclock_impedance)?;
        self.set_wordclock_thru(new_state.config.wordclock_thru)?;
        self.set_adat_on(new_state.config.adat_on)?;
        self.set_smux_mode(new_state.config.smux_mode)?;

        for i in 0..8 {
            let inp = &new_state.inputs[i];
            self.set_input_type(i, inp.input_type)?;
            if i < 2 {
                self.set_input_inst(i, inp.inst_hi_z)?;
            }
            self.set_input_phantom(i, inp.phantom_48v)?;
            self.set_input_phase_invert(i, inp.phase_invert)?;
            self.set_input_line_pad(i, inp.pad_line)?;
            self.set_input_mic_pad_boost(i, inp.pad_mic)?;
        }

        self.state.stereo_links = new_state.stereo_links;

        for i in 0..24 {
            let ch = &new_state.mixer_channels[i];
            self.state.mixer_channels[i].group = ch.group;
            self.state.mixer_channels[i].show = ch.show;
            self.set_mixer_channel_name(i, &ch.name)?;
            let raw_lvl = encode_mixer_fader_db(ch.level_db);
            self.transport
                .set_parameter(param_id::MIXER_CH_LEVEL, i as u8, raw_lvl)?;
            self.state.mixer_channels[i].level_db = decode_mixer_fader_db(raw_lvl);
            self.set_mixer_channel_pan(i, ch.pan)?;
            self.transport
                .set_parameter(param_id::MIXER_CH_MUTE, i as u8, u16::from(ch.mute))?;
            self.state.mixer_channels[i].mute = ch.mute;
            self.transport
                .set_parameter(param_id::MIXER_CH_SOLO, i as u8, u16::from(ch.solo))?;
            self.state.mixer_channels[i].solo = ch.solo;
        }

        self.set_direct_computer_mix(new_state.monitor.direct_computer_mix)?;
        self.set_mixer_output_level_db(new_state.monitor.mixer_output_level_db)?;
        self.set_mixer_output_mute(new_state.monitor.mixer_output_mute)?;
        self.set_mixer_output_solo(new_state.monitor.mixer_output_solo)?;
        self.set_speakers_source(new_state.monitor.speakers_source)?;
        self.set_speakers_mono(new_state.monitor.speakers_mono)?;
        self.set_phones_source(new_state.monitor.phones_source)?;
        self.set_phones_mono(new_state.monitor.phones_mono)?;

        for pair in 0..4 {
            self.set_analog_out_source(pair, new_state.routing.analog_out_sources[pair])?;
            self.set_adat_out_source(pair, new_state.routing.adat_out_sources[pair])?;
        }
        self.set_loopback_source(new_state.routing.loopback_source)?;

        self.state.active_preset_slot = new_state.active_preset_slot;
        Ok(())
    }

    /// Query USB DFU Interface 3 status.
    pub fn dfu_status(&self) -> Result<DfuStatusReport, TransportError> {
        self.transport.get_dfu_status()
    }
}
