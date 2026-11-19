//! Hardware preset load/save (`PresetLoad` `0x48` / `PresetSave` `0x49`) and
//! local JSON preset file import/export for the Arturia AudioFuse 8Pre.

use crate::state::AudioFuseState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Preset format version for local JSON files.
pub const PRESET_SCHEMA_VERSION: u32 = 1;

/// Size of the simulator slot record including 25 fixed-width names.
pub const DEVICE_PRESET_PAYLOAD_SIZE: usize = 181 + 25 * 16;

#[derive(Debug, Error)]
pub enum PresetError {
    #[error(transparent)]
    Apply(#[from] crate::transport::TransportError),
    #[error("I/O error accessing preset file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid preset JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Unsupported preset product ID '{0}' (expected '0xAF0E' / 'AudioFuse 8Pre')")]
    UnsupportedProduct(String),
}

/// Complete portable AudioFuse 8Pre Preset File (`.af8preset.json` / `.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioFusePresetFile {
    pub schema_version: u32,
    pub product_name: String,
    pub vendor_id: String,
    pub product_id: String,
    pub preset_name: String,
    pub description: String,
    pub state: AudioFuseState,
}

impl AudioFusePresetFile {
    /// Construct a new preset file wrapper around a given `AudioFuseState`.
    pub fn from_state(
        preset_name: impl Into<String>,
        description: impl Into<String>,
        state: AudioFuseState,
    ) -> Self {
        Self {
            schema_version: PRESET_SCHEMA_VERSION,
            product_name: "Arturia AudioFuse 8Pre".to_string(),
            vendor_id: "0x1C75".to_string(),
            product_id: "0xAF0E".to_string(),
            preset_name: preset_name.into(),
            description: description.into(),
            state,
        }
    }

    /// Export this preset to a pretty-printed JSON file on disk.
    pub fn save_to_path(&self, path: impl AsRef<Path>) -> Result<(), PresetError> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Load and validate a preset from a JSON file on disk.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, PresetError> {
        let content = fs::read_to_string(path)?;
        Self::from_json_str(&content)
    }

    /// Parse and validate a preset from a JSON string.
    pub fn from_json_str(json: &str) -> Result<Self, PresetError> {
        let preset: Self = serde_json::from_str(json)?;
        if !preset.product_id.eq_ignore_ascii_case("0xaf0e")
            || !preset.vendor_id.eq_ignore_ascii_case("0x1c75")
            || preset.schema_version != PRESET_SCHEMA_VERSION
        {
            return Err(PresetError::UnsupportedProduct(preset.product_id));
        }
        Self::validate_state(&preset.state)?;
        Ok(preset)
    }

    pub fn validate_state(s: &AudioFuseState) -> Result<(), crate::transport::TransportError> {
        use crate::protocol::*;
        let valid_name = |v: &str| v.len() <= 16 && v.bytes().all(|c| (32..127).contains(&c));
        if s.mixer_channels.len() != 24
            || s.active_preset_slot > 7
            || !valid_name(&s.config.device_name)
            || !(0.0..=1.0).contains(&s.monitor.direct_computer_mix)
            || !(-61.0..=0.0).contains(&s.monitor.mixer_output_level_db)
            || s.mixer_channels.iter().enumerate().any(|(i, ch)| {
                ch.index as usize != i
                    || ch.bank.base_index() != (i / 8) * 8
                    || ch.bank_channel as usize != i % 8 + 1
                    || !valid_name(&ch.name)
                    || ch.group > 4
                    || !(-64..=64).contains(&ch.pan)
                    || !(-61.0..=12.0).contains(&ch.level_db)
            })
            || s.inputs.iter().enumerate().any(|(i, ch)| {
                ch.channel_number as usize != i + 1
                    || (i >= 2 && ch.input_type == InputType::Inst)
                    || ch.inst_hi_z != (ch.input_type == InputType::Inst)
                    || (ch.pad_mic != MicPadBoost::Off
                        && !matches!(ch.input_type, InputType::ComboAuto | InputType::Mic))
                    || (ch.pad_line != LinePad::Off
                        && !matches!(ch.input_type, InputType::ComboAuto | InputType::Line))
            })
        {
            return Err(crate::transport::TransportError::Other(
                "Invalid preset channel, name, level, or input configuration".into(),
            ));
        }
        Ok(())
    }

    /// Encode a simulator slot. Hardware encoding requires a captured preset transfer.
    pub fn encode_device_slot_bytes(state: &AudioFuseState) -> [u8; DEVICE_PRESET_PAYLOAD_SIZE] {
        let mut buf = [0u8; DEVICE_PRESET_PAYLOAD_SIZE];
        // Magic header: "AF8P"
        buf[0..4].copy_from_slice(b"AF8P");
        // Bytes 4..82: 78-byte bulk state
        let bulk = state.encode_bulk_state_78();
        buf[4..82].copy_from_slice(&bulk);
        // Bytes 82..130: 24 mixer channel fader levels (u16 LE each = 48 bytes)
        for (i, ch) in state.mixer_channels.iter().take(24).enumerate() {
            let raw = crate::protocol::encode_mixer_fader_db(ch.level_db);
            let offset = 82 + i * 2;
            buf[offset..offset + 2].copy_from_slice(&raw.to_le_bytes());
        }
        // Bytes 130..154: 24 mixer channel pan values (i8 each = 24 bytes)
        for (i, ch) in state.mixer_channels.iter().take(24).enumerate() {
            buf[130 + i] = ch.pan as u8;
        }
        // Bytes 154..178: 24 mixer channel group values (`0..4`) + show bit (`bit 7`)
        for (i, ch) in state.mixer_channels.iter().take(24).enumerate() {
            let mut b = ch.group & 0x07;
            if ch.show {
                b |= 0x80;
            }
            buf[154 + i] = b;
        }
        // Bytes 178..181: Stereo link masks per bank (3 bytes)
        for bank_idx in 0..3 {
            let links = &state.stereo_links[bank_idx];
            let mut mask = 0u8;
            if links.pair_1_2 {
                mask |= 1 << 0;
            }
            if links.pair_3_4 {
                mask |= 1 << 1;
            }
            if links.pair_5_6 {
                mask |= 1 << 2;
            }
            if links.pair_7_8 {
                mask |= 1 << 3;
            }
            buf[178 + bank_idx] = mask;
        }
        for (slot, name) in buf[181..].chunks_exact_mut(16).zip(
            std::iter::once(&state.config.device_name)
                .chain(state.mixer_channels.iter().map(|ch| &ch.name)),
        ) {
            slot.copy_from_slice(&crate::protocol::encode_device_string_16(name));
        }
        buf
    }

    /// Decode a simulator slot into a state snapshot.
    pub fn decode_device_slot_bytes(buf: &[u8], target: &mut AudioFuseState) -> bool {
        if buf.len() < DEVICE_PRESET_PAYLOAD_SIZE || &buf[0..4] != b"AF8P" {
            return false;
        }
        let mut bulk = [0u8; crate::protocol::BULK_STATE_VU_SIZE];
        bulk.copy_from_slice(&buf[4..82]);
        target.apply_bulk_state_78(&bulk);

        for (i, ch) in target.mixer_channels.iter_mut().take(24).enumerate() {
            let offset = 82 + i * 2;
            let raw = u16::from_le_bytes([buf[offset], buf[offset + 1]]);
            ch.level_db = crate::protocol::decode_mixer_fader_db(raw);
            ch.pan = (buf[130 + i] as i8).clamp(-64, 64);
            let grp_byte = buf[154 + i];
            ch.group = (grp_byte & 0x07).min(4);
            ch.show = (grp_byte & 0x80) != 0;
        }

        for bank_idx in 0..3 {
            let mask = buf[178 + bank_idx];
            target.stereo_links[bank_idx].pair_1_2 = (mask & (1 << 0)) != 0;
            target.stereo_links[bank_idx].pair_3_4 = (mask & (1 << 1)) != 0;
            target.stereo_links[bank_idx].pair_5_6 = (mask & (1 << 2)) != 0;
            target.stereo_links[bank_idx].pair_7_8 = (mask & (1 << 3)) != 0;
        }
        let mut names = buf[181..]
            .chunks_exact(16)
            .map(crate::protocol::decode_device_string_16);
        target.config.device_name = names.next().unwrap();
        for (ch, name) in target.mixer_channels.iter_mut().zip(names) {
            ch.name = name;
        }
        true
    }
}
