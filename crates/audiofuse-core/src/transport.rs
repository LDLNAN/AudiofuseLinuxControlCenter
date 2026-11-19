//! USB / UAC2 hardware transport, non-destructive sysfs + ALSA card discovery,
//! and deterministic offline simulator backend for the Arturia AudioFuse 8Pre (`1c75:af0e`).

use crate::firmware::DfuStatusReport;
use crate::preset::{AudioFusePresetFile, DEVICE_PRESET_PAYLOAD_SIZE};
use crate::protocol::*;
use crate::state::AudioFuseState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

/// Default USB control transfer timeout (`250 ms`).
pub const USB_CONTROL_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("USB error: {0}")]
    Usb(#[from] rusb::Error),
    #[error("Arturia AudioFuse 8Pre (1c75:af0e) not found on USB bus")]
    DeviceNotFound,
    #[error("USB permission denied on 1c75:af0e (install udev/70-arturia-audiofuse.rules)")]
    PermissionDenied,
    #[error("Invalid response length (expected {expected} bytes, got {actual} bytes)")]
    InvalidLength { expected: usize, actual: usize },
    #[error("Unsupported sample rate: {0} Hz")]
    UnsupportedSampleRate(u32),
    #[error("Preset slot {0} out of range (expected 0..=7)")]
    InvalidPresetSlot(u8),
    #[error("Transport error: {0}")]
    Other(String),
}

/// Discovered USB & ALSA status of the Arturia AudioFuse 8Pre (`1c75:af0e`) on the Linux host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareDiscoveryInfo {
    pub usb_present: bool,
    pub control_driver: Option<String>,
    pub vendor_id: u16,
    pub product_id: u16,
    pub bus_number: u8,
    pub device_address: u8,
    pub manufacturer: String,
    pub product_name: String,
    pub serial_number: String,
    pub bcd_device: String,
    pub alsa_card_id: Option<String>,
    pub alsa_momentary_freq_hz: Option<u32>,
    pub alsa_clock_source: Option<ClockSource>,
    pub alsa_clock_validity: [bool; 3],
    pub direct_usb_writable: bool,
    pub alsa_error: Option<String>,
}

impl Default for HardwareDiscoveryInfo {
    fn default() -> Self {
        Self {
            usb_present: false,
            control_driver: None,
            vendor_id: ARTURIA_VENDOR_ID,
            product_id: AUDIOFUSE_8PRE_PRODUCT_ID,
            bus_number: 0,
            device_address: 0,
            manufacturer: "ARTURIA".to_string(),
            product_name: "AudioFuse 8Pre".to_string(),
            serial_number: "unknown".to_string(),
            bcd_device: "unknown".to_string(),
            alsa_card_id: None,
            alsa_momentary_freq_hz: None,
            alsa_clock_source: None,
            alsa_clock_validity: [true, false, false],
            direct_usb_writable: false,
            alsa_error: None,
        }
    }
}

/// Non-destructively scan Linux `/sys/bus/usb/devices`, `/proc/asound/A8Pre/stream0`,
/// and `amixer` / `rusb` for an attached Arturia AudioFuse 8Pre (`1c75:af0e`).
pub fn probe_linux_host_hardware() -> HardwareDiscoveryInfo {
    let mut info = HardwareDiscoveryInfo::default();

    // 1. Check sysfs `/sys/bus/usb/devices/*`
    if let Ok(entries) = fs::read_dir("/sys/bus/usb/devices") {
        for entry in entries.flatten() {
            let path = entry.path();
            let vid = fs::read_to_string(path.join("idVendor"))
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            let pid = fs::read_to_string(path.join("idProduct"))
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            if vid == "1c75" && pid == "af0e" {
                info.usb_present = true;
                let interface =
                    path.with_file_name(format!("{}:1.0", entry.file_name().to_string_lossy()));
                info.control_driver =
                    fs::read_link(interface.join("driver"))
                        .ok()
                        .and_then(|driver| {
                            driver
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                        });
                if let Ok(bus) = fs::read_to_string(path.join("busnum")) {
                    info.bus_number = bus.trim().parse().unwrap_or(0);
                }
                if let Ok(dev) = fs::read_to_string(path.join("devnum")) {
                    info.device_address = dev.trim().parse().unwrap_or(0);
                }
                if let Ok(mfg) = fs::read_to_string(path.join("manufacturer")) {
                    let s = mfg.trim();
                    if !s.is_empty() {
                        info.manufacturer = s.to_string();
                    }
                }
                if let Ok(prod) = fs::read_to_string(path.join("product")) {
                    let s = prod.trim();
                    if !s.is_empty() {
                        info.product_name = s.to_string();
                    }
                }
                if let Ok(ser) = fs::read_to_string(path.join("serial")) {
                    let s = ser.trim();
                    if !s.is_empty() {
                        info.serial_number = s.to_string();
                    }
                }
                if let Ok(bcd) = fs::read_to_string(path.join("bcdDevice")) {
                    let s = bcd.trim();
                    if s.len() == 4 {
                        let maj = s[0..2].trim_start_matches('0');
                        let maj = if maj.is_empty() { "0" } else { maj };
                        info.bcd_device = format!("{}.{}", maj, &s[2..4]);
                    } else if !s.is_empty() {
                        info.bcd_device = s.to_string();
                    }
                }
                break;
            }
        }
    }

    // 2. Check `/proc/asound/A8Pre/stream0` for active ALSA card & sample rate
    let stream0_path = Path::new("/proc/asound/A8Pre/stream0");
    if stream0_path.exists() {
        info.alsa_card_id = Some("A8Pre".to_string());
        if let Ok(stream_txt) = fs::read_to_string(stream0_path) {
            for line in stream_txt.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("Momentary freq =") {
                    let hz_str = rest.split_whitespace().next().unwrap_or("");
                    if let Ok(hz) = hz_str.parse::<u32>() {
                        info.alsa_momentary_freq_hz = Some(hz);
                        break;
                    }
                }
            }
        }
    }

    // 3. Query ALSA UAC2 clock controls (`numid=1..4`) if card `A8Pre` is present
    if info.alsa_card_id.is_some() {
        match Command::new("amixer")
            .args(["-c", "A8Pre", "contents"])
            .output()
        {
            Ok(output) if output.status.success() && output.stderr.is_empty() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                parse_amixer_clock_contents(&stdout, &mut info);
            }
            Ok(output) => {
                info.alsa_error = Some(format!(
                    "{}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                ))
            }
            Err(e) => info.alsa_error = Some(e.to_string()),
        }
    }

    // 4. Check if `/dev/bus/usb/<bus>/<dev>` is writable by current user
    if info.usb_present && info.bus_number > 0 && info.device_address > 0 {
        let dev_node = format!(
            "/dev/bus/usb/{:03}/{:03}",
            info.bus_number, info.device_address
        );
        info.direct_usb_writable = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&dev_node)
            .is_ok();
    }

    info
}

/// Parse `amixer -c A8Pre contents` output for clock validity (`numid=1..3`) and selector (`numid=4`).
pub fn parse_amixer_clock_contents(stdout: &str, info: &mut HardwareDiscoveryInfo) {
    let mut current_numid: Option<u32> = None;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("numid=") {
            current_numid = trimmed
                .split(',')
                .next()
                .and_then(|s| s.strip_prefix("numid="))
                .and_then(|n| n.parse::<u32>().ok());
        } else if let Some(val_str) = trimmed.strip_prefix(": values=") {
            match current_numid {
                Some(1) => {
                    // Internal Clock Validity
                    info.alsa_clock_validity[0] = val_str.trim() == "on";
                }
                Some(2) => {
                    // WORD Clock Validity
                    info.alsa_clock_validity[2] = val_str.trim() == "on";
                }
                Some(3) => {
                    // ADAT Clock Validity
                    info.alsa_clock_validity[1] = val_str.trim() == "on";
                }
                Some(4) => {
                    // ARTURIA Clock Selector Clock Source:
                    // Item #0 'Internal Clock', Item #1 'WORD Clock', Item #2 'ADAT Clock'
                    match val_str.trim() {
                        "0" => info.alsa_clock_source = Some(ClockSource::Internal),
                        "1" => info.alsa_clock_source = Some(ClockSource::WordClock),
                        "2" => info.alsa_clock_source = Some(ClockSource::Adat),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

/// Trait abstracting the low-level USB / UAC2 control backend (`AFEXDevice`) for
/// the Arturia AudioFuse 8Pre (`1c75:af0e`).
pub trait AudioFuseTransport: Send + Sync {
    /// Human-readable backend name (e.g. `"USB Hardware (1c75:af0e)"`, `"ALSA/Hybrid + Simulator"`, or `"Offline Simulator"`).
    fn backend_name(&self) -> &str;

    /// Returns whether the physical USB device `1c75:af0e` was detected on the host.
    fn is_hardware_detected(&self) -> bool;

    /// Read a 2-byte little-endian parameter (`AFEXDevice::GetParameter`) on Unit `0x0a`, Interface `0`.
    /// `wValue = ((param_id as u16) << 8) | (channel_index as u16)`, `wIndex = 0x0a00`, `wLength = 2`.
    fn get_parameter(&self, param_id: u8, channel_index: u8) -> Result<u16, TransportError>;

    /// Write a 2-byte little-endian parameter (`AFEXDevice::SetParameter`) on Unit `0x0a`, Interface `0`.
    /// `wValue = ((param_id as u16) << 8) | (channel_index as u16)`, `wIndex = 0x0a00`, `wLength = 2`.
    fn set_parameter(
        &self,
        param_id: u8,
        channel_index: u8,
        value: u16,
    ) -> Result<(), TransportError>;

    /// Read the 78-byte (`0x4e`) hardware bulk state and real-time VU meter payload.
    fn read_bulk_state_vu(&self) -> Result<[u8; BULK_STATE_VU_SIZE], TransportError>;

    /// Read a 16-byte ASCII string (`GenericUsbDevice::GetString`: `bRequest = 0x98 + str_type`).
    fn get_string(&self, str_type: u8, index: u16) -> Result<String, TransportError>;

    /// Write a 16-byte zero-padded ASCII string (`GenericUsbDevice::SetString`: `bRequest = 0x18 + str_type`).
    fn set_string(&self, str_type: u8, index: u16, value: &str) -> Result<(), TransportError>;

    /// Load a preset buffer from device memory (`PresetLoad`: `bRequest = 0x48`, `wIndex = slot`).
    fn preset_load(&self, slot: u8, len: usize) -> Result<Vec<u8>, TransportError>;

    /// Save a preset buffer to device memory (`PresetSave`: `bRequest = 0x49`, `wIndex = slot`).
    fn preset_save(&self, slot: u8, data: &[u8]) -> Result<(), TransportError>;

    /// Read the UAC2 Clock Selector (`Entity 40` / `0x28`, `CS_CLOCK_SELECTOR_CONTROL = 0x01`).
    fn get_clock_source(&self) -> Result<ClockSource, TransportError>;

    /// Set the UAC2 Clock Selector (`Entity 40` / `0x28`, `CS_CLOCK_SELECTOR_CONTROL = 0x01`).
    fn set_clock_source(&self, source: ClockSource) -> Result<(), TransportError>;

    /// Read UAC2 Clock Validity (`CS_CLOCK_VALID_CONTROL = 0x02`) for Entities `41`, `43`, `44`.
    fn get_clock_validity(&self) -> Result<[bool; 3], TransportError>;

    /// Read UAC2 Sample Rate (`CS_SAM_FREQ_CONTROL = 0x01`) on Entity `41` (`0x29`).
    fn get_sample_rate(&self) -> Result<SampleRate, TransportError>;

    /// Set UAC2 Sample Rate (`CS_SAM_FREQ_CONTROL = 0x01`) on Entity `41` (`0x29`).
    fn set_sample_rate(&self, rate: SampleRate) -> Result<(), TransportError>;

    /// Read USB DFU status (`Interface 3`, `bInterfaceClass = 0xfe`, `bInterfaceSubClass = 0x01`).
    fn get_dfu_status(&self) -> Result<DfuStatusReport, TransportError>;

    /// Return host hardware discovery details.
    fn discovery_info(&self) -> HardwareDiscoveryInfo;
}

/// Deterministic Simulator Backend that implements the exact `AFEXDevice` register map,
/// 78-byte bulk state & animated/deterministic VU meters, strings, presets, UAC2 entities,
/// and optionally mirrors live ALSA / sysfs hardware status when running without root USB permissions.
#[derive(Debug, Clone)]
pub struct SimulatedTransport {
    inner: Arc<Mutex<SimInner>>,
    backend_label: String,
}

#[derive(Debug, Clone)]
struct SimInner {
    state: AudioFuseState,
    raw_params: HashMap<(u8, u8), u16>,
    preset_slots: HashMap<u8, Vec<u8>>,
    discovery: HardwareDiscoveryInfo,
    tick_counter: u64,
    animate_meters: bool,
}

impl Default for SimulatedTransport {
    fn default() -> Self {
        Self::new_with_discovery(HardwareDiscoveryInfo::default(), true)
    }
}

impl SimulatedTransport {
    /// Create a deterministic simulator seeded from optional host discovery info.
    pub fn new_with_discovery(discovery: HardwareDiscoveryInfo, animate_meters: bool) -> Self {
        let mut state = AudioFuseState::default();
        state.config.serial_number = discovery.serial_number.clone();
        state.config.firmware_version = discovery.bcd_device.clone();
        if let Some(hz) = discovery.alsa_momentary_freq_hz {
            if let Some(sr) = SampleRate::from_hz(hz) {
                state.config.sample_rate = sr;
            }
        }
        if let Some(cs) = discovery.alsa_clock_source {
            state.config.clock_source = cs;
        }
        state.config.clock_validity = discovery.alsa_clock_validity;
        state.config.clock_locked = match state.config.clock_source {
            ClockSource::Internal => state.config.clock_validity[0],
            ClockSource::Adat => state.config.clock_validity[1],
            ClockSource::WordClock => state.config.clock_validity[2],
        };

        // Seed realistic studio metering values so UI and CLI meter inspection show live activity
        let default_analog_vu: [u8; 8] = [88, 84, 72, 65, 54, 48, 32, 20];
        let default_adat_vu: [u8; 8] = [60, 58, 44, 42, 30, 28, 12, 10];
        let default_usb_vu: [u8; 8] = [96, 95, 68, 66, 40, 40, 18, 18];
        for i in 0..8 {
            state.inputs[i].vu = default_analog_vu[i];
            state.adat_in_vu[i] = default_adat_vu[i];
            state.usb_in_vu[i] = default_usb_vu[i];
            state.routing.analog_out_vu[i] = default_usb_vu[i];
            state.routing.adat_out_vu[i] = default_adat_vu[i];
        }
        state.monitor.mixer_main_vu_l = 92;
        state.monitor.mixer_main_vu_r = 90;
        state.monitor.speakers_vu_l = 94;
        state.monitor.speakers_vu_r = 92;
        state.monitor.phones_vu_l = 94;
        state.monitor.phones_vu_r = 92;
        state.routing.loopback_vu_l = 92;
        state.routing.loopback_vu_r = 90;
        state.sync_mixer_vu_from_inputs();

        let mut preset_slots = HashMap::new();
        let slot0 = AudioFusePresetFile::encode_device_slot_bytes(&state).to_vec();
        preset_slots.insert(0, slot0);

        let backend_label = if discovery.usb_present {
            "ALSA/Sysfs Hardware Readout + Safe User-Space State (1c75:af0e detected)".to_string()
        } else {
            "Deterministic Studio Simulator (1c75:af0e offline)".to_string()
        };

        let mut sim = Self {
            inner: Arc::new(Mutex::new(SimInner {
                state,
                raw_params: HashMap::new(),
                preset_slots,
                discovery,
                tick_counter: 0,
                animate_meters,
            })),
            backend_label,
        };
        sim.sync_raw_params_from_state();
        sim
    }

    /// Synchronize internal raw parameter table `(param_id, channel_index)` from `AudioFuseState`.
    fn sync_raw_params_from_state(&mut self) {
        let mut guard = self.inner.lock().unwrap();
        let s = guard.state.clone();
        guard
            .raw_params
            .insert((param_id::DEVICE_MODE, 0), s.config.device_mode as u16);
        guard
            .raw_params
            .insert((param_id::ADAT_ON, 0), u16::from(s.config.adat_on));
        guard.raw_params.insert(
            (param_id::WORDCLOCK_IMPEDANCE, 0),
            s.config.wordclock_impedance as u16,
        );
        guard.raw_params.insert(
            (param_id::WORDCLOCK_THRU, 0),
            u16::from(s.config.wordclock_thru),
        );
        guard
            .raw_params
            .insert((param_id::SMUX_MODE, 0), s.config.smux_mode as u16);

        for i in 0..8 {
            let idx = i as u8;
            let inp = &s.inputs[i];
            guard
                .raw_params
                .insert((param_id::INPUT_TYPE, idx), inp.input_type as u16);
            guard
                .raw_params
                .insert((param_id::INPUT_PAD_BOOST, idx), inp.pad_mic as u16);
            guard
                .raw_params
                .insert((param_id::INPUT_PAD_LINE, idx), inp.pad_line as u16);
            guard
                .raw_params
                .insert((param_id::INPUT_PHANTOM, idx), u16::from(inp.phantom_48v));
            guard.raw_params.insert(
                (param_id::INPUT_PHASE_INVERT, idx),
                u16::from(inp.phase_invert),
            );
            if i < 2 {
                guard
                    .raw_params
                    .insert((param_id::INPUT_INST, idx), u16::from(inp.inst_hi_z));
            }
        }

        for i in 0..24 {
            let idx = i as u8;
            let ch = &s.mixer_channels[i];
            guard
                .raw_params
                .insert((param_id::MIXER_CH_MUTE, idx), u16::from(ch.mute));
            guard
                .raw_params
                .insert((param_id::MIXER_CH_SOLO, idx), u16::from(ch.solo));
            guard
                .raw_params
                .insert((param_id::MIXER_CH_PAN, idx), (ch.pan as i16) as u16);
            guard.raw_params.insert(
                (param_id::MIXER_CH_LEVEL, idx),
                encode_mixer_fader_db(ch.level_db),
            );
        }

        guard.raw_params.insert(
            (param_id::MIXER_OUT_MUTE, 0),
            u16::from(s.monitor.mixer_output_mute),
        );
        guard.raw_params.insert(
            (param_id::MIXER_OUT_SOLO, 0),
            u16::from(s.monitor.mixer_output_solo),
        );
        guard.raw_params.insert(
            (param_id::MIXER_OUT_LEVEL, 0),
            encode_master_level_db(s.monitor.mixer_output_level_db),
        );
        guard.raw_params.insert(
            (param_id::DIRECT_COMPUTER_MIX, 0),
            (s.monitor.direct_computer_mix.clamp(0.0, 1.0) * 1000.0).round() as u16,
        );
        guard.raw_params.insert(
            (param_id::SPEAKERS_SOURCE, 0),
            s.monitor.speakers_source as u16,
        );
        guard.raw_params.insert(
            (param_id::SPEAKERS_MONO, 0),
            u16::from(s.monitor.speakers_mono),
        );
        guard
            .raw_params
            .insert((param_id::PHONES_SOURCE, 0), s.monitor.phones_source as u16);
        guard
            .raw_params
            .insert((param_id::PHONES_MONO, 0), u16::from(s.monitor.phones_mono));

        for pair in 0..4 {
            guard.raw_params.insert(
                (param_id::ANALOG_OUT_SOURCE, pair as u8),
                s.routing.analog_out_sources[pair] as u16,
            );
            guard.raw_params.insert(
                (param_id::ADAT_OUT_SOURCE, pair as u8),
                s.routing.adat_out_sources[pair] as u16,
            );
        }
        guard.raw_params.insert(
            (param_id::LOOPBACK_SOURCE, 0),
            s.routing.loopback_source as u16,
        );
    }

    /// Return a snapshot of the simulator's internal `AudioFuseState`.
    pub fn snapshot_state(&self) -> AudioFuseState {
        self.inner.lock().unwrap().state.clone()
    }
}

impl AudioFuseTransport for SimulatedTransport {
    fn backend_name(&self) -> &str {
        &self.backend_label
    }

    fn is_hardware_detected(&self) -> bool {
        self.inner.lock().unwrap().discovery.usb_present
    }

    fn get_parameter(&self, param_id: u8, channel_index: u8) -> Result<u16, TransportError> {
        let guard = self.inner.lock().unwrap();
        Ok(*guard
            .raw_params
            .get(&(param_id, channel_index))
            .unwrap_or(&0))
    }

    fn set_parameter(
        &self,
        param_id: u8,
        channel_index: u8,
        value: u16,
    ) -> Result<(), TransportError> {
        let mut guard = self.inner.lock().unwrap();
        guard.raw_params.insert((param_id, channel_index), value);
        let idx = channel_index as usize;

        match param_id {
            param_id::DEVICE_MODE => {
                guard.state.config.device_mode = DeviceMode::from_raw(value);
            }
            param_id::ADAT_ON => {
                guard.state.config.adat_on = value != 0;
            }
            param_id::WORDCLOCK_IMPEDANCE => {
                guard.state.config.wordclock_impedance = WordclockImpedance::from_raw(value);
            }
            param_id::WORDCLOCK_THRU => {
                guard.state.config.wordclock_thru = value != 0;
            }
            param_id::SMUX_MODE => {
                guard.state.config.smux_mode = SmuxMode::from_raw(value);
            }
            param_id::INPUT_TYPE if idx < 8 => {
                let it = InputType::from_raw(value, idx);
                guard.state.inputs[idx].input_type = it;
                guard.state.inputs[idx].mic_active =
                    matches!(it, InputType::ComboAuto | InputType::Mic);
                guard.state.inputs[idx].jack_active =
                    matches!(it, InputType::ComboAuto | InputType::Line | InputType::Inst);
                if idx < 2 {
                    guard.state.inputs[idx].inst_hi_z = it == InputType::Inst;
                }
            }
            param_id::INPUT_PAD_BOOST if idx < 8 => {
                guard.state.inputs[idx].pad_mic = MicPadBoost::from_raw(value);
            }
            param_id::INPUT_PAD_LINE if idx < 8 => {
                guard.state.inputs[idx].pad_line = LinePad::from_raw(value);
            }
            param_id::INPUT_PHANTOM if idx < 8 => {
                guard.state.inputs[idx].phantom_48v = value != 0;
            }
            param_id::INPUT_PHASE_INVERT if idx < 8 => {
                guard.state.inputs[idx].phase_invert = value != 0;
            }
            param_id::INPUT_INST if idx < 2 => {
                let hi_z = value != 0;
                guard.state.inputs[idx].inst_hi_z = hi_z;
                if hi_z {
                    guard.state.inputs[idx].input_type = InputType::Inst;
                } else if guard.state.inputs[idx].input_type == InputType::Inst {
                    guard.state.inputs[idx].input_type = InputType::Line;
                }
            }
            param_id::MIXER_CH_MUTE if idx < 24 => {
                guard.state.mixer_channels[idx].mute = value != 0;
            }
            param_id::MIXER_CH_SOLO if idx < 24 => {
                guard.state.mixer_channels[idx].solo = value != 0;
            }
            param_id::MIXER_CH_PAN if idx < 24 => {
                let pan = (value as i16).clamp(-64, 64) as i8;
                guard.state.mixer_channels[idx].pan = pan;
            }
            param_id::MIXER_CH_LEVEL if idx < 24 => {
                guard.state.mixer_channels[idx].level_db = decode_mixer_fader_db(value);
            }
            param_id::MIXER_OUT_MUTE => {
                guard.state.monitor.mixer_output_mute = value != 0;
            }
            param_id::MIXER_OUT_SOLO => {
                guard.state.monitor.mixer_output_solo = value != 0;
            }
            param_id::MIXER_OUT_LEVEL => {
                guard.state.monitor.mixer_output_level_db = decode_master_level_db(value);
            }
            param_id::DIRECT_COMPUTER_MIX => {
                guard.state.monitor.direct_computer_mix =
                    ((value.min(1000) as f32) / 1000.0 * 100.0).round() / 100.0;
            }
            param_id::SPEAKERS_SOURCE => {
                guard.state.monitor.speakers_source = SpeakersSource::from_raw(value);
            }
            param_id::SPEAKERS_MONO => {
                guard.state.monitor.speakers_mono = value != 0;
            }
            param_id::PHONES_SOURCE => {
                guard.state.monitor.phones_source = PhonesSource::from_raw(value);
            }
            param_id::PHONES_MONO => {
                guard.state.monitor.phones_mono = value != 0;
            }
            param_id::ANALOG_OUT_SOURCE if idx < 4 => {
                guard.state.routing.analog_out_sources[idx] = AnalogOutSource::from_raw(value);
            }
            param_id::ADAT_OUT_SOURCE if idx < 4 => {
                guard.state.routing.adat_out_sources[idx] = AdatOutSource::from_raw(value);
            }
            param_id::LOOPBACK_SOURCE => {
                guard.state.routing.loopback_source = LoopbackSource::from_raw(value);
            }
            _ => {}
        }

        Ok(())
    }

    fn read_bulk_state_vu(&self) -> Result<[u8; BULK_STATE_VU_SIZE], TransportError> {
        let mut guard = self.inner.lock().unwrap();
        if guard.animate_meters {
            guard.tick_counter = guard.tick_counter.wrapping_add(1);
            let tick = guard.tick_counter;
            let base_analog: [u8; 8] = [88, 84, 72, 65, 54, 48, 32, 20];
            let base_adat: [u8; 8] = [60, 58, 44, 42, 30, 28, 12, 10];
            let base_usb: [u8; 8] = [96, 95, 68, 66, 40, 40, 18, 18];
            for i in 0..8 {
                let wave = (((tick.wrapping_add(i as u64 * 3)) % 9) as i16) - 4;
                guard.state.inputs[i].vu = (base_analog[i] as i16 + wave).clamp(0, 127) as u8;
                guard.state.adat_in_vu[i] = if guard.state.config.adat_on {
                    (base_adat[i] as i16 + wave).clamp(0, 127) as u8
                } else {
                    0
                };
                guard.state.usb_in_vu[i] = (base_usb[i] as i16 + wave).clamp(0, 127) as u8;
            }
            guard.state.sync_mixer_vu_from_inputs();
        }
        Ok(guard.state.encode_bulk_state_78())
    }

    fn get_string(&self, str_type: u8, index: u16) -> Result<String, TransportError> {
        let guard = self.inner.lock().unwrap();
        match str_type {
            STRING_TYPE_DEVICE_NAME => Ok(guard.state.config.device_name.clone()),
            STRING_TYPE_CHANNEL_NAME => {
                let idx = index as usize;
                if let Some(ch) = guard.state.mixer_channels.get(idx) {
                    Ok(ch.name.clone())
                } else {
                    Ok(String::new())
                }
            }
            _ => Ok(String::new()),
        }
    }

    fn set_string(&self, str_type: u8, index: u16, value: &str) -> Result<(), TransportError> {
        let mut guard = self.inner.lock().unwrap();
        let encoded = encode_device_string_16(value);
        let clean = decode_device_string_16(&encoded);
        match str_type {
            STRING_TYPE_DEVICE_NAME => {
                guard.state.config.device_name = clean;
            }
            STRING_TYPE_CHANNEL_NAME => {
                let idx = index as usize;
                if let Some(ch) = guard.state.mixer_channels.get_mut(idx) {
                    ch.name = clean;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn preset_load(&self, slot: u8, len: usize) -> Result<Vec<u8>, TransportError> {
        if slot > 7 {
            return Err(TransportError::InvalidPresetSlot(slot));
        }
        let guard = self.inner.lock().unwrap();
        if let Some(saved) = guard.preset_slots.get(&slot) {
            let mut out = saved.clone();
            out.resize(len.max(DEVICE_PRESET_PAYLOAD_SIZE), 0);
            Ok(out)
        } else {
            let mut out = AudioFusePresetFile::encode_device_slot_bytes(&guard.state).to_vec();
            out.resize(len.max(DEVICE_PRESET_PAYLOAD_SIZE), 0);
            Ok(out)
        }
    }

    fn preset_save(&self, slot: u8, data: &[u8]) -> Result<(), TransportError> {
        if slot > 7 {
            return Err(TransportError::InvalidPresetSlot(slot));
        }
        let mut guard = self.inner.lock().unwrap();
        guard.preset_slots.insert(slot, data.to_vec());
        guard.state.active_preset_slot = slot;
        Ok(())
    }

    fn get_clock_source(&self) -> Result<ClockSource, TransportError> {
        let guard = self.inner.lock().unwrap();
        Ok(guard.state.config.clock_source)
    }

    fn set_clock_source(&self, source: ClockSource) -> Result<(), TransportError> {
        let mut guard = self.inner.lock().unwrap();
        guard.state.config.clock_source = source;
        guard.state.config.clock_locked = match source {
            ClockSource::Internal => guard.state.config.clock_validity[0],
            ClockSource::Adat => guard.state.config.clock_validity[1],
            ClockSource::WordClock => guard.state.config.clock_validity[2],
        };
        Ok(())
    }

    fn get_clock_validity(&self) -> Result<[bool; 3], TransportError> {
        let guard = self.inner.lock().unwrap();
        Ok(guard.state.config.clock_validity)
    }

    fn get_sample_rate(&self) -> Result<SampleRate, TransportError> {
        let guard = self.inner.lock().unwrap();
        Ok(guard.state.config.sample_rate)
    }

    fn set_sample_rate(&self, rate: SampleRate) -> Result<(), TransportError> {
        let mut guard = self.inner.lock().unwrap();
        guard.state.config.sample_rate = rate;
        Ok(())
    }

    fn get_dfu_status(&self) -> Result<DfuStatusReport, TransportError> {
        Ok(DfuStatusReport::default())
    }

    fn discovery_info(&self) -> HardwareDiscoveryInfo {
        self.inner.lock().unwrap().discovery.clone()
    }
}

/// Native `rusb` (`libusb-1.0`) Hardware USB / UAC2 Control Backend for the Arturia AudioFuse 8Pre (`1c75:af0e`).
///
/// Safety rules enforced:
/// - Never detaches `Interface 1` or `Interface 2` (the active UAC2 18-channel audio streaming interfaces).
/// - Uses EP0 control transfers targeting Interface `0` (`wIndex = 0x0a00` for `AFEXDevice` unit `0x0a`).
/// - Claims only an available control interface and propagates transfer errors.
pub struct UsbHardwareTransport {
    handle: Mutex<rusb::DeviceHandle<rusb::GlobalContext>>,

    discovery: HardwareDiscoveryInfo,
}

impl UsbHardwareTransport {
    /// Attempt to open `1c75:af0e` via `rusb`.
    pub fn open() -> Result<Self, TransportError> {
        let discovery = probe_linux_host_hardware();
        if let Some(driver) = &discovery.control_driver {
            return Err(TransportError::Other(format!(
                "AudioControl interface 0 is owned by {driver}. Use --simulator for offline editing."
            )));
        }
        let handle = Self::open_handle()?;
        handle.claim_interface(UAC2_CONTROL_INTERFACE).map_err(|e| TransportError::Other(format!("Cannot claim AudioControl interface 0: {e}. Leave the ALSA driver attached. Use --simulator for offline editing.")))?;
        Ok(Self {
            handle: Mutex::new(handle),
            discovery,
        })
    }
    fn open_handle() -> Result<rusb::DeviceHandle<rusb::GlobalContext>, TransportError> {
        let devices = rusb::devices()?;
        for device in devices.iter() {
            let desc = device.device_descriptor()?;
            if desc.vendor_id() == ARTURIA_VENDOR_ID
                && desc.product_id() == AUDIOFUSE_8PRE_PRODUCT_ID
            {
                match device.open() {
                    Ok(handle) => return Ok(handle),
                    Err(rusb::Error::Access) => return Err(TransportError::PermissionDenied),
                    Err(e) => return Err(TransportError::Usb(e)),
                }
            }
        }
        Err(TransportError::DeviceNotFound)
    }
}

impl AudioFuseTransport for UsbHardwareTransport {
    fn backend_name(&self) -> &str {
        "Native USB/UAC2 Hardware (1c75:af0e)"
    }

    fn is_hardware_detected(&self) -> bool {
        true
    }

    fn get_parameter(&self, param: u8, channel: u8) -> Result<u16, TransportError> {
        verified_selector(param)?;
        Ok(u16::from_le_bytes(self.read_cur(
            encode_param_wvalue(param, channel),
            AFEX_DEFAULT_WINDEX,
        )?))
    }
    fn set_parameter(&self, param: u8, channel: u8, value: u16) -> Result<(), TransportError> {
        verified_selector(param)?;
        self.write_cur(
            encode_param_wvalue(param, channel),
            AFEX_DEFAULT_WINDEX,
            &value.to_le_bytes(),
        )
    }
    fn read_bulk_state_vu(&self) -> Result<[u8; BULK_STATE_VU_SIZE], TransportError> {
        unavailable("78-byte telemetry request and layout")
    }
    fn get_string(&self, _: u8, _: u16) -> Result<String, TransportError> {
        unavailable("string recipient and index")
    }
    fn set_string(&self, _: u8, _: u16, _: &str) -> Result<(), TransportError> {
        unavailable("string recipient and index")
    }
    fn preset_load(&self, _: u8, _: usize) -> Result<Vec<u8>, TransportError> {
        unavailable("preset slot addressing and payload")
    }
    fn preset_save(&self, _: u8, _: &[u8]) -> Result<(), TransportError> {
        unavailable("preset slot addressing and payload")
    }
    fn get_clock_source(&self) -> Result<ClockSource, TransportError> {
        let [pin] = self.read_cur(0x100, encode_unit_windex(UAC2_ENTITY_CLOCK_SELECTOR, 0))?;
        if !(1..=3).contains(&pin) {
            return Err(TransportError::Other(format!(
                "Invalid clock selector pin {pin}"
            )));
        }
        Ok(ClockSource::from_uac2_pin(pin))
    }
    fn set_clock_source(&self, source: ClockSource) -> Result<(), TransportError> {
        self.write_cur(
            0x100,
            encode_unit_windex(UAC2_ENTITY_CLOCK_SELECTOR, 0),
            &[source.to_uac2_pin()],
        )
    }
    fn get_clock_validity(&self) -> Result<[bool; 3], TransportError> {
        let mut valid = [false; 3];
        for (i, entity) in [
            UAC2_ENTITY_CLOCK_INTERNAL,
            UAC2_ENTITY_CLOCK_ADAT,
            UAC2_ENTITY_CLOCK_WORD,
        ]
        .into_iter()
        .enumerate()
        {
            let [value] = self.read_cur(0x200, encode_unit_windex(entity, 0))?;
            valid[i] = value != 0;
        }
        Ok(valid)
    }
    fn get_sample_rate(&self) -> Result<SampleRate, TransportError> {
        let hz = u32::from_le_bytes(
            self.read_cur(0x100, encode_unit_windex(UAC2_ENTITY_CLOCK_INTERNAL, 0))?,
        );
        SampleRate::from_hz(hz).ok_or(TransportError::UnsupportedSampleRate(hz))
    }
    fn set_sample_rate(&self, rate: SampleRate) -> Result<(), TransportError> {
        self.write_cur(
            0x100,
            encode_unit_windex(UAC2_ENTITY_CLOCK_INTERNAL, 0),
            &rate.to_hz().to_le_bytes(),
        )
    }
    fn get_dfu_status(&self) -> Result<DfuStatusReport, TransportError> {
        Err(TransportError::Other("Live DFU status disabled after a runtime query coincided with USB disconnect and ALSA control failure".into()))
    }
    fn discovery_info(&self) -> HardwareDiscoveryInfo {
        self.discovery.clone()
    }
}

fn exact(expected: usize, actual: usize) -> Result<(), TransportError> {
    if actual == expected {
        Ok(())
    } else {
        Err(TransportError::InvalidLength { expected, actual })
    }
}
fn unavailable<T>(operation: &str) -> Result<T, TransportError> {
    Err(TransportError::Other(format!(
        "Native {operation} lacks a confirmed wire mapping"
    )))
}
fn verified_selector(param: u8) -> Result<(), TransportError> {
    match param {
        param_id::INPUT_PAD_LINE | param_id::MIXER_OUT_SOLO | param_id::DIRECT_COMPUTER_MIX => {
            unavailable("parameter selector")
        }
        _ => Ok(()),
    }
}
impl UsbHardwareTransport {
    fn read_cur<const N: usize>(&self, value: u16, index: u16) -> Result<[u8; N], TransportError> {
        let mut buf = [0; N];
        let count = self.handle.lock().unwrap().read_control(
            USB_REQ_CLASS_INTERFACE_IN,
            UAC2_REQ_CUR,
            value,
            index,
            &mut buf,
            USB_CONTROL_TIMEOUT,
        )?;
        exact(N, count)?;
        Ok(buf)
    }
    fn write_cur(&self, value: u16, index: u16, data: &[u8]) -> Result<(), TransportError> {
        let count = self.handle.lock().unwrap().write_control(
            USB_REQ_CLASS_INTERFACE_OUT,
            UAC2_REQ_CUR,
            value,
            index,
            data,
            USB_CONTROL_TIMEOUT,
        )?;
        exact(data.len(), count)
    }
}
pub fn open_best_available_transport(
    force_simulator: bool,
) -> Result<Arc<dyn AudioFuseTransport>, TransportError> {
    if !force_simulator {
        match UsbHardwareTransport::open() {
            Ok(hw) => return Ok(Arc::new(hw)),
            Err(TransportError::DeviceNotFound) => (),
            Err(error) => return Err(error),
        }
    }
    Ok(Arc::new(SimulatedTransport::default()))
}
