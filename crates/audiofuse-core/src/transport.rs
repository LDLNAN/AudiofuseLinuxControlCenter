//! Native Linux USB / UAC2 hardware transport (via non-intrusive `/dev/audiofuse8pre-ctl`
//! EP0 coexistence bridge or unclaimed `rusb`), sysfs + ALSA discovery, and deterministic
//! offline simulator backend for the Arturia AudioFuse 8Pre (`1c75:af0e`).

use crate::firmware::DfuStatusReport;
use crate::preset::{AudioFusePresetFile, DEVICE_PRESET_PAYLOAD_SIZE};
use crate::protocol::*;
use crate::state::AudioFuseState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

/// Default USB control transfer timeout (`500 ms`).
pub const USB_CONTROL_TIMEOUT: Duration = Duration::from_millis(500);

/// Path to the non-intrusive Linux kernel EP0 coexistence bridge (`kmod/audiofuse8pre_ctl.c`).
pub const KERNEL_CTL_DEVICE_PATH: &str = "/dev/audiofuse8pre-ctl";

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("USB error: {0}")]
    Usb(#[from] rusb::Error),
    #[error("Arturia AudioFuse 8Pre (1c75:af0e) not found on USB bus")]
    DeviceNotFound,
    #[error(
        "Permission denied accessing 1c75:af0e (install udev/70-arturia-audiofuse.rules or check /dev/audiofuse8pre-ctl permissions)"
    )]
    PermissionDenied,
    #[error("Invalid response length (expected {expected} bytes, got {actual} bytes)")]
    InvalidLength { expected: usize, actual: usize },
    #[error("Unsupported sample rate: {0} Hz")]
    UnsupportedSampleRate(u32),
    #[error("Preset slot {0} out of range (expected 0..=7)")]
    InvalidPresetSlot(u8),
    #[error("Unsupported on AudioFuse 8Pre hardware: {0}")]
    Unsupported(String),
    #[error("Hardware rejected or gated change: {0}")]
    HardwareRejected(String),
    #[error("Transport error: {0}")]
    Other(String),
}

/// Discovered USB & ALSA status of the Arturia AudioFuse 8Pre (`1c75:af0e`) on the Linux host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareDiscoveryInfo {
    pub usb_present: bool,
    pub control_driver: Option<String>,
    pub kernel_ctl_available: bool,
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
    /// `[Internal (41), ADAT (44), WordClock (43)]`
    pub alsa_clock_validity: [bool; 3],
    pub direct_usb_writable: bool,
    pub alsa_error: Option<String>,
}

impl Default for HardwareDiscoveryInfo {
    fn default() -> Self {
        Self {
            usb_present: false,
            control_driver: None,
            kernel_ctl_available: false,
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
/// `/dev/audiofuse8pre-ctl`, and `amixer` for an attached Arturia AudioFuse 8Pre (`1c75:af0e`).
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

    // 4. Check `/dev/audiofuse8pre-ctl` coexistence bridge availability
    info.kernel_ctl_available = OpenOptions::new()
        .read(true)
        .write(true)
        .open(KERNEL_CTL_DEVICE_PATH)
        .is_ok();

    // 5. Check if `/dev/bus/usb/<bus>/<dev>` is writable by current user
    if info.usb_present && info.bus_number > 0 && info.device_address > 0 {
        let dev_node = format!(
            "/dev/bus/usb/{:03}/{:03}",
            info.bus_number, info.device_address
        );
        info.direct_usb_writable = OpenOptions::new()
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
                    // Internal Clock Validity (Entity 41 / 0x29)
                    info.alsa_clock_validity[0] = val_str.trim() == "on";
                }
                Some(2) => {
                    // WORD Clock Validity (Entity 43 / 0x2B)
                    info.alsa_clock_validity[2] = val_str.trim() == "on";
                }
                Some(3) => {
                    // ADAT Clock Validity (Entity 44 / 0x2C)
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
    /// Human-readable backend name.
    fn backend_name(&self) -> &str;

    /// Returns whether the physical USB device `1c75:af0e` is detected/attached.
    fn is_hardware_detected(&self) -> bool;

    /// Returns `true` when running against the offline deterministic simulator.
    fn is_simulated(&self) -> bool {
        false
    }

    /// Read the verified 266-byte (`0x010a`) hardware state snapshot from AFEX Unit `0x0d` (`wValue = 0x0002`, `bRequest = 0x03`).
    fn read_state_266(&self) -> Result<[u8; AFEX_STATE_SIZE], TransportError>;

    /// Read the verified 152-byte (`0x98`, `38 × i32` LE) live VU meter payload from AFEX Unit `0x11` (`wValue = 0x0000`, `bRequest = 0x03`).
    fn read_vu_152(&self) -> Result<[u8; AFEX_VU_SIZE], TransportError>;

    /// Read a 2-byte little-endian parameter on AFEX Unit `0x0a` (`bRequest = 0x03`).
    fn get_parameter(&self, param_id: u8, channel_index: u8) -> Result<u16, TransportError>;

    /// Write a 2-byte little-endian parameter on AFEX Unit `0x0a` (`bRequest = 0x03`).
    fn set_parameter(
        &self,
        param_id: u8,
        channel_index: u8,
        value: u16,
    ) -> Result<(), TransportError>;

    /// Write a variable-length payload on AFEX Unit `0x0a` (`bRequest = 0x03`, e.g., 16-byte `Device Name` `0x68`).
    fn set_parameter_bytes(
        &self,
        param_id: u8,
        channel_index: u8,
        data: &[u8],
    ) -> Result<(), TransportError>;

    /// Read a 16-byte ASCII string (`STRING_TYPE_DEVICE_NAME` backed by `0x68`; channel names host-only on hardware).
    fn get_string(&self, str_type: u8, index: u16) -> Result<String, TransportError>;

    /// Write a 16-byte ASCII string (`STRING_TYPE_DEVICE_NAME` backed by `0x68`; channel names host-only on hardware).
    fn set_string(&self, str_type: u8, index: u16, value: &str) -> Result<(), TransportError>;

    /// Load a preset buffer (simulator-only; AudioFuse 8Pre has no on-device preset slots).
    fn preset_load(&self, _slot: u8, _len: usize) -> Result<Vec<u8>, TransportError> {
        Err(TransportError::Unsupported(
            "AudioFuse 8Pre has no on-device preset slots; use JSON preset export/import".into(),
        ))
    }

    /// Save a preset buffer (simulator-only; AudioFuse 8Pre has no on-device preset slots).
    fn preset_save(&self, _slot: u8, _data: &[u8]) -> Result<(), TransportError> {
        Err(TransportError::Unsupported(
            "AudioFuse 8Pre has no on-device preset slots; use JSON preset export/import".into(),
        ))
    }

    /// Set simulated-only `InputType` (on real hardware, Mic/Line is physically sensed by XLR/TRS jack `0x15`).
    fn set_simulated_input_type(
        &self,
        _channel_idx: usize,
        _input_type: InputType,
    ) -> Result<(), TransportError> {
        Err(TransportError::Unsupported(
            "Input Mic/Line mode is physically sensed by the XLR/TRS combo jack (register 0x15); only Ch 1-2 Inst (Hi-Z) is software-switchable".into(),
        ))
    }

    /// Set simulated-only `Mixer Output Mute` (not a hardware register on AudioFuse 8Pre).
    fn set_simulated_mixer_output_mute(&self, _mute: bool) -> Result<(), TransportError> {
        Err(TransportError::Unsupported(
            "Mixer Output Mute is not a hardware control on AudioFuse 8Pre (use Mixer Output Level)".into(),
        ))
    }

    /// Set simulated-only `Mixer Output Solo` (not a hardware register on AudioFuse 8Pre).
    fn set_simulated_mixer_output_solo(&self, _solo: bool) -> Result<(), TransportError> {
        Err(TransportError::Unsupported(
            "Mixer Output Solo is not a hardware control on AudioFuse 8Pre".into(),
        ))
    }

    /// Set simulated-only `Direct/Computer Mix` (not a hardware register on AudioFuse 8Pre).
    fn set_simulated_direct_computer_mix(&self, _mix: f32) -> Result<(), TransportError> {
        Err(TransportError::Unsupported(
            "Direct/Computer Mix is not a hardware control on AudioFuse 8Pre".into(),
        ))
    }

    /// Read the UAC2 Clock Selector (`Entity 40` / `0x28`, `CS_CLOCK_SELECTOR_CONTROL = 0x01`).
    fn get_clock_source(&self) -> Result<ClockSource, TransportError>;

    /// Set the UAC2 Clock Selector (`Entity 40` / `0x28`, `CS_CLOCK_SELECTOR_CONTROL = 0x01`).
    fn set_clock_source(&self, source: ClockSource) -> Result<(), TransportError>;

    /// Read UAC2 Clock Validity (`CS_CLOCK_VALID_CONTROL = 0x02`) as `[Internal (41), ADAT (44), WordClock (43)]`.
    fn get_clock_validity(&self) -> Result<[bool; 3], TransportError>;

    /// Read UAC2 Sample Rate (`CS_SAM_FREQ_CONTROL = 0x01`) on Entity `41` (`0x29`).
    fn get_sample_rate(&self) -> Result<SampleRate, TransportError>;

    /// Set UAC2 Sample Rate (`CS_SAM_FREQ_CONTROL = 0x01`) on Entity `41` (`0x29`).
    fn set_sample_rate(&self, rate: SampleRate) -> Result<(), TransportError>;

    /// Read USB DFU status (blocked on live hardware to protect audio streaming).
    fn get_dfu_status(&self) -> Result<DfuStatusReport, TransportError>;

    /// Return host hardware discovery details.
    fn discovery_info(&self) -> HardwareDiscoveryInfo;
}

// =============================================================================
// Deterministic Offline Simulator Backend (speaks the exact 266B/152B wire protocol)
// =============================================================================

#[derive(Debug, Clone)]
pub struct SimulatedTransport {
    inner: Arc<Mutex<SimInner>>,
    backend_label: String,
}

#[derive(Debug, Clone)]
struct SimInner {
    state: AudioFuseState,
    gains_l: [i16; 24],
    gains_r: [i16; 24],
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
        if discovery.serial_number != "unknown" {
            state.config.serial_number = discovery.serial_number.clone();
        }
        if discovery.bcd_device != "unknown" {
            state.config.firmware_version = discovery.bcd_device.clone();
        }
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

        // Seed realistic studio metering values for offline simulator inspection
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
        state.monitor.speakers_vu_l = 96;
        state.monitor.speakers_vu_r = 95;
        state.monitor.phones_vu_l = 96;
        state.monitor.phones_vu_r = 95;
        state.routing.loopback_vu_l = 92;
        state.routing.loopback_vu_r = 90;
        state.sync_mixer_vu_from_inputs();

        let mut gains_l = [0i16; 24];
        let mut gains_r = [0i16; 24];
        for i in 0..24 {
            let (gl, gr) = encode_mixer_channel_gains_q88(
                state.mixer_channels[i].level_db,
                state.mixer_channels[i].pan,
            );
            gains_l[i] = gl;
            gains_r[i] = gr;
        }

        let mut preset_slots = HashMap::new();
        let slot0 = AudioFusePresetFile::encode_device_slot_bytes(&state).to_vec();
        preset_slots.insert(0, slot0);

        let backend_label = "Deterministic Studio Simulator (--simulator)".to_string();

        Self {
            inner: Arc::new(Mutex::new(SimInner {
                state,
                gains_l,
                gains_r,
                preset_slots,
                discovery,
                tick_counter: 0,
                animate_meters,
            })),
            backend_label,
        }
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

    fn is_simulated(&self) -> bool {
        true
    }

    fn read_state_266(&self) -> Result<[u8; AFEX_STATE_SIZE], TransportError> {
        let guard = self.inner.lock().unwrap();
        Ok(guard.state.encode_afex_state_266())
    }

    fn read_vu_152(&self) -> Result<[u8; AFEX_VU_SIZE], TransportError> {
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
        Ok(guard.state.encode_afex_vu_152())
    }

    fn get_parameter(&self, param_id: u8, channel_index: u8) -> Result<u16, TransportError> {
        let guard = self.inner.lock().unwrap();
        let snap = guard.state.encode_afex_state_266();
        let idx = channel_index as usize;
        let val = match param_id {
            param_id::INPUT_PAD_MIC if idx < 8 => guard.state.inputs[idx].pad_mic as u16,
            param_id::INPUT_PAD_LINE if idx < 8 => {
                u16::from(guard.state.inputs[idx].pad_line == LinePad::Pad20dB)
            }
            param_id::INPUT_INST if idx < 2 => u16::from(guard.state.inputs[idx].inst_hi_z),
            param_id::INPUT_PHASE_INVERT if idx < 8 => {
                u16::from(guard.state.inputs[idx].phase_invert)
            }
            param_id::INPUT_PHANTOM if idx < 8 => u16::from(guard.state.inputs[idx].phantom_48v),
            param_id::MIXER_CH_GAIN_L if idx < 24 => guard.gains_l[idx] as u16,
            param_id::MIXER_CH_GAIN_R if idx < 24 => guard.gains_r[idx] as u16,
            param_id::MIXER_MUTE_ANALOG_ADAT if idx < 16 => {
                u16::from(guard.state.mixer_channels[idx].mute)
            }
            param_id::MIXER_MUTE_USB if idx < 8 => {
                u16::from(guard.state.mixer_channels[16 + idx].mute)
            }
            param_id::MIXER_SOLO_ANALOG_ADAT if idx < 16 => {
                u16::from(guard.state.mixer_channels[idx].solo)
            }
            param_id::MIXER_SOLO_USB if idx < 8 => {
                u16::from(guard.state.mixer_channels[16 + idx].solo)
            }
            param_id::MIXER_STEREO_SHOW if idx < 12 => {
                u16::from(guard.state.stereo_links[idx / 4].get_pair(idx % 4))
            }
            param_id::ANALOG_OUT_SOURCE if idx < 4 => {
                guard.state.routing.analog_out_sources[idx].to_wire_mask()
            }
            param_id::ADAT_OUT_SOURCE if idx < 4 => {
                guard.state.routing.adat_out_sources[idx].to_wire_mask()
            }
            _ => {
                let off = (param_id as usize) * 2;
                if off + 2 <= snap.len() {
                    u16::from_le_bytes([snap[off], snap[off + 1]])
                } else {
                    0
                }
            }
        };
        Ok(val)
    }

    fn set_parameter(
        &self,
        param_id: u8,
        channel_index: u8,
        value: u16,
    ) -> Result<(), TransportError> {
        self.set_parameter_bytes(param_id, channel_index, &value.to_le_bytes())
    }

    fn set_parameter_bytes(
        &self,
        param_id: u8,
        channel_index: u8,
        data: &[u8],
    ) -> Result<(), TransportError> {
        let mut guard = self.inner.lock().unwrap();
        let idx = channel_index as usize;
        if param_id == param_id::DEVICE_NAME {
            let clean = decode_device_string_16(data);
            guard.state.config.device_name = clean;
            return Ok(());
        }
        if data.len() < 2 {
            return Err(TransportError::InvalidLength {
                expected: 2,
                actual: data.len(),
            });
        }
        let value = u16::from_le_bytes([data[0], data[1]]);

        match param_id {
            param_id::INPUT_PAD_MIC if idx < 8 => {
                guard.state.inputs[idx].pad_mic = MicPadBoost::from_raw(value);
            }
            param_id::INPUT_PAD_LINE if idx < 8 => {
                guard.state.inputs[idx].pad_line = LinePad::from_raw(value);
            }
            param_id::INPUT_INST if idx < 2 => {
                let hi_z = value != 0;
                guard.state.inputs[idx].inst_hi_z = hi_z;
                if hi_z {
                    guard.state.inputs[idx].input_type = InputType::Inst;
                    guard.state.inputs[idx].mic_active = false;
                    guard.state.inputs[idx].jack_active = true;
                } else if guard.state.inputs[idx].input_type == InputType::Inst {
                    guard.state.inputs[idx].input_type = InputType::Line;
                    guard.state.inputs[idx].mic_active = false;
                    guard.state.inputs[idx].jack_active = true;
                }
            }
            param_id::INPUT_PHASE_INVERT if idx < 8 => {
                guard.state.inputs[idx].phase_invert = value != 0;
            }
            param_id::SPEAKERS_SOURCE => {
                guard.state.monitor.speakers_source = SpeakersSource::from_raw(value);
            }
            param_id::PHONES_SOURCE => {
                guard.state.monitor.phones_source = PhonesSource::from_raw(value);
            }
            param_id::SPEAKERS_MONO => {
                guard.state.monitor.speakers_mono = value != 0;
            }
            param_id::PHONES_MONO => {
                guard.state.monitor.phones_mono = value != 0;
            }
            param_id::INPUT_PHANTOM if idx < 8 => {
                guard.state.inputs[idx].phantom_48v = value != 0;
            }
            param_id::MIXER_CH_GAIN_L if idx < 24 => {
                guard.gains_l[idx] = value as i16;
                let (lvl, pan) =
                    decode_mixer_channel_gains_q88(guard.gains_l[idx], guard.gains_r[idx]);
                guard.state.mixer_channels[idx].level_db = lvl;
                guard.state.mixer_channels[idx].pan = pan;
            }
            param_id::MIXER_CH_GAIN_R if idx < 24 => {
                guard.gains_r[idx] = value as i16;
                let (lvl, pan) =
                    decode_mixer_channel_gains_q88(guard.gains_l[idx], guard.gains_r[idx]);
                guard.state.mixer_channels[idx].level_db = lvl;
                guard.state.mixer_channels[idx].pan = pan;
            }
            param_id::MIXER_MUTE_ANALOG_ADAT if idx < 16 => {
                guard.state.mixer_channels[idx].mute = value != 0;
            }
            param_id::MIXER_MUTE_USB if idx < 8 => {
                guard.state.mixer_channels[16 + idx].mute = value != 0;
            }
            param_id::MIXER_SOLO_ANALOG_ADAT if idx < 16 => {
                guard.state.mixer_channels[idx].solo = value != 0;
            }
            param_id::MIXER_SOLO_USB if idx < 8 => {
                guard.state.mixer_channels[16 + idx].solo = value != 0;
            }
            param_id::MIXER_OUT_LEVEL => {
                guard.state.monitor.mixer_output_level_db = decode_master_level_q88(value as i16);
            }
            param_id::MIXER_SHOW_USB if idx < 8 => {
                guard.state.mixer_channels[16 + idx].show = value != 0;
            }
            param_id::MIXER_SHOW_ANALOG_ADAT if idx < 16 => {
                guard.state.mixer_channels[idx].show = value != 0;
            }
            param_id::MIXER_STEREO_SHOW if idx < 12 => {
                guard.state.stereo_links[idx / 4].set_pair(idx % 4, value != 0);
            }
            param_id::DEVICE_MODE => {
                guard.state.config.device_mode = DeviceMode::from_raw(value);
            }
            param_id::WORDCLOCK_IMPEDANCE => {
                guard.state.config.wordclock_impedance = WordclockImpedance::from_raw(value);
            }
            param_id::WORDCLOCK_THRU => {
                guard.state.config.wordclock_thru = value != 0;
            }
            param_id::ANALOG_OUT_SOURCE if idx < 4 => {
                guard.state.routing.analog_out_sources[idx] = AnalogOutSource::from_wire_mask(value);
            }
            param_id::ADAT_OUT_SOURCE if idx < 4 => {
                guard.state.routing.adat_out_sources[idx] = AdatOutSource::from_wire_mask(value);
            }
            param_id::LOOPBACK_SOURCE => {
                guard.state.routing.loopback_source = LoopbackSource::from_wire_mask(value);
            }
            param_id::SMUX_MODE => {
                guard.state.config.smux_mode = SmuxMode::from_raw(value);
            }
            param_id::ADAT_ON => {
                guard.state.config.adat_on = value != 0;
            }
            _ => {}
        }
        Ok(())
    }

    fn get_string(&self, str_type: u8, index: u16) -> Result<String, TransportError> {
        let guard = self.inner.lock().unwrap();
        match str_type {
            STRING_TYPE_DEVICE_NAME => Ok(guard.state.config.device_name.clone()),
            STRING_TYPE_CHANNEL_NAME => {
                let idx = index as usize;
                Ok(guard
                    .state
                    .mixer_channels
                    .get(idx)
                    .map(|c| c.name.clone())
                    .unwrap_or_default())
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

    fn set_simulated_input_type(
        &self,
        channel_idx: usize,
        input_type: InputType,
    ) -> Result<(), TransportError> {
        if channel_idx >= 8 {
            return Ok(());
        }
        let mut guard = self.inner.lock().unwrap();
        let it = if channel_idx >= 2 && input_type == InputType::Inst {
            InputType::Line
        } else {
            input_type
        };
        let inp = &mut guard.state.inputs[channel_idx];
        inp.input_type = it;
        inp.mic_active = matches!(it, InputType::ComboAuto | InputType::Mic);
        inp.jack_active = matches!(it, InputType::ComboAuto | InputType::Line | InputType::Inst);
        if channel_idx < 2 {
            inp.inst_hi_z = it == InputType::Inst;
        }
        Ok(())
    }

    fn set_simulated_mixer_output_mute(&self, mute: bool) -> Result<(), TransportError> {
        self.inner.lock().unwrap().state.monitor.mixer_output_mute = mute;
        Ok(())
    }

    fn set_simulated_mixer_output_solo(&self, solo: bool) -> Result<(), TransportError> {
        self.inner.lock().unwrap().state.monitor.mixer_output_solo = solo;
        Ok(())
    }

    fn set_simulated_direct_computer_mix(&self, mix: f32) -> Result<(), TransportError> {
        let clamped = mix.clamp(0.0, 1.0);
        self.inner.lock().unwrap().state.monitor.direct_computer_mix =
            (clamped * 100.0).round() / 100.0;
        Ok(())
    }

    fn get_clock_source(&self) -> Result<ClockSource, TransportError> {
        Ok(self.inner.lock().unwrap().state.config.clock_source)
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
        Ok(self.inner.lock().unwrap().state.config.clock_validity)
    }

    fn get_sample_rate(&self) -> Result<SampleRate, TransportError> {
        Ok(self.inner.lock().unwrap().state.config.sample_rate)
    }

    fn set_sample_rate(&self, rate: SampleRate) -> Result<(), TransportError> {
        self.inner.lock().unwrap().state.config.sample_rate = rate;
        Ok(())
    }

    fn get_dfu_status(&self) -> Result<DfuStatusReport, TransportError> {
        Ok(DfuStatusReport::default())
    }

    fn discovery_info(&self) -> HardwareDiscoveryInfo {
        self.inner.lock().unwrap().discovery.clone()
    }
}

// =============================================================================
// Linux `/dev/audiofuse8pre-ctl` EP0 Coexistence Bridge (kmod/audiofuse8pre_ctl.c)
// =============================================================================

#[cfg(target_os = "linux")]
mod kernel_ctl {
    use super::*;
    use std::ffi::{c_int, c_ulong};
    use std::os::fd::AsRawFd;

    const AF8PRE_MAX_DATA_LEN: usize = 512;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct Af8PreDeviceInfo {
        pub vid: u16,
        pub pid: u16,
        pub bcd_device: u16,
        pub bus_num: u8,
        pub dev_num: u8,
    }

    #[repr(C)]
    pub struct Af8PreCtrlTransfer {
        pub bm_request_type: u8,
        pub b_request: u8,
        pub w_value: u16,
        pub w_index: u16,
        pub w_length: u16,
        pub timeout_ms: u32,
        pub transferred: i32,
        pub bus_num: u8,
        pub dev_num: u8,
        pub reserved: u16,
        pub data: [u8; AF8PRE_MAX_DATA_LEN],
    }

    // Linux `_IOWR('A', nr, struct)` encoding: dir=3 (READ|WRITE) << 30 | size << 16 | type << 8 | nr
    const fn iowr(ty: u8, nr: u8, size: usize) -> c_ulong {
        (3u64 << 30 | ((size as u64) << 16) | ((ty as u64) << 8) | (nr as u64)) as c_ulong
    }

    const AF8PRE_IOCTL_DEVICE_INFO: c_ulong =
        iowr(b'A', 1, std::mem::size_of::<Af8PreDeviceInfo>());
    const AF8PRE_IOCTL_CTRL_TRANSFER: c_ulong =
        iowr(b'A', 2, std::mem::size_of::<Af8PreCtrlTransfer>());

    unsafe extern "C" {
        fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    }

    pub struct KernelCtlBridge {
        file: File,
        pub info: Af8PreDeviceInfo,
    }

    impl KernelCtlBridge {
        pub fn open() -> Result<Option<Self>, TransportError> {
            let file = match OpenOptions::new()
                .read(true)
                .write(true)
                .open(KERNEL_CTL_DEVICE_PATH)
            {
                Ok(f) => f,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Err(TransportError::PermissionDenied);
                }
                Err(e) => {
                    return Err(TransportError::Other(format!(
                        "Failed to open {KERNEL_CTL_DEVICE_PATH}: {e}"
                    )));
                }
            };

            let mut info = Af8PreDeviceInfo::default();
            let ret = unsafe { ioctl(file.as_raw_fd(), AF8PRE_IOCTL_DEVICE_INFO, &mut info) };
            if ret < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(19) {
                    // ENODEV
                    return Err(TransportError::DeviceNotFound);
                }
                return Err(TransportError::Other(format!(
                    "AF8PRE_IOCTL_DEVICE_INFO failed: {err}"
                )));
            }
            Ok(Some(Self { file, info }))
        }

        pub fn ctrl_in(
            &self,
            b_request: u8,
            w_value: u16,
            w_index: u16,
            out: &mut [u8],
            timeout_ms: u32,
        ) -> Result<usize, TransportError> {
            if out.len() > AF8PRE_MAX_DATA_LEN {
                return Err(TransportError::Other(format!(
                    "Control transfer length {} exceeds {}",
                    out.len(),
                    AF8PRE_MAX_DATA_LEN
                )));
            }
            let mut req = Af8PreCtrlTransfer {
                bm_request_type: USB_REQ_CLASS_INTERFACE_IN,
                b_request,
                w_value,
                w_index,
                w_length: out.len() as u16,
                timeout_ms,
                transferred: 0,
                bus_num: self.info.bus_num,
                dev_num: self.info.dev_num,
                reserved: 0,
                data: [0u8; AF8PRE_MAX_DATA_LEN],
            };
            let ret = unsafe { ioctl(self.file.as_raw_fd(), AF8PRE_IOCTL_CTRL_TRANSFER, &mut req) };
            if ret < 0 {
                let err = std::io::Error::last_os_error();
                return Err(TransportError::Other(format!(
                    "EP0 IN failed (req=0x{b_request:02x} val=0x{w_value:04x} idx=0x{w_index:04x}): {err}"
                )));
            }
            let n = req.transferred.max(0) as usize;
            let copy_len = n.min(out.len());
            out[..copy_len].copy_from_slice(&req.data[..copy_len]);
            Ok(n)
        }

        pub fn ctrl_out(
            &self,
            b_request: u8,
            w_value: u16,
            w_index: u16,
            data: &[u8],
            timeout_ms: u32,
        ) -> Result<usize, TransportError> {
            if data.len() > AF8PRE_MAX_DATA_LEN {
                return Err(TransportError::Other(format!(
                    "Control transfer length {} exceeds {}",
                    data.len(),
                    AF8PRE_MAX_DATA_LEN
                )));
            }
            let mut req = Af8PreCtrlTransfer {
                bm_request_type: USB_REQ_CLASS_INTERFACE_OUT,
                b_request,
                w_value,
                w_index,
                w_length: data.len() as u16,
                timeout_ms,
                transferred: 0,
                bus_num: self.info.bus_num,
                dev_num: self.info.dev_num,
                reserved: 0,
                data: [0u8; AF8PRE_MAX_DATA_LEN],
            };
            req.data[..data.len()].copy_from_slice(data);
            let ret = unsafe { ioctl(self.file.as_raw_fd(), AF8PRE_IOCTL_CTRL_TRANSFER, &mut req) };
            if ret < 0 {
                let err = std::io::Error::last_os_error();
                return Err(TransportError::Other(format!(
                    "EP0 OUT failed (req=0x{b_request:02x} val=0x{w_value:04x} idx=0x{w_index:04x}): {err}"
                )));
            }
            Ok(req.transferred.max(0) as usize)
        }
    }
}

// =============================================================================
// Native Hardware Transport (Coexists Safely with snd-usb-audio / PipeWire)
// =============================================================================

enum UsbBackend {
    #[cfg(target_os = "linux")]
    KernelBridge(Mutex<kernel_ctl::KernelCtlBridge>),
    Rusb(Mutex<rusb::DeviceHandle<rusb::GlobalContext>>),
}

/// Native Hardware USB / UAC2 Control Backend for the Arturia AudioFuse 8Pre (`1c75:af0e`).
///
/// Safety rules enforced:
/// - Never detaches `snd-usb-audio` and never touches Interface 1, Interface 2 (audio streaming), or Interface 3 (DFU).
/// - On Linux, uses the non-intrusive `/dev/audiofuse8pre-ctl` EP0 coexistence bridge when loaded so
///   ALSA and PipeWire audio stay 100% uninterrupted while full hardware control and 152-byte live VU metering run.
/// - Falls back to direct `rusb` only when Interface 0 has no kernel driver attached.
pub struct UsbHardwareTransport {
    backend: UsbBackend,
    backend_label: String,
    discovery: HardwareDiscoveryInfo,
}

impl UsbHardwareTransport {
    /// Open the physically connected Arturia AudioFuse 8Pre (`1c75:af0e`).
    pub fn open() -> Result<Self, TransportError> {
        let mut discovery = probe_linux_host_hardware();

        // 1. Prefer the non-intrusive Linux EP0 coexistence bridge (`/dev/audiofuse8pre-ctl`)
        #[cfg(target_os = "linux")]
        {
            if let Some(bridge) = kernel_ctl::KernelCtlBridge::open()? {
                discovery.usb_present = true;
                discovery.kernel_ctl_available = true;
                discovery.bus_number = bridge.info.bus_num;
                discovery.device_address = bridge.info.dev_num;
                let maj = (bridge.info.bcd_device >> 8) & 0xff;
                let min = bridge.info.bcd_device & 0xff;
                if discovery.bcd_device == "unknown" {
                    discovery.bcd_device = format!("{maj:x}.{min:02x}");
                }
                return Ok(Self {
                    backend: UsbBackend::KernelBridge(Mutex::new(bridge)),
                    backend_label: format!(
                        "Native Linux Hardware ({KERNEL_CTL_DEVICE_PATH}, Bus {:03} Dev {:03}, ALSA {})",
                        discovery.bus_number,
                        discovery.device_address,
                        discovery.alsa_card_id.as_deref().unwrap_or("unbound")
                    ),
                    discovery,
                });
            }
        }

        // 2. If `snd-usb-audio` owns Interface 0 and `/dev/audiofuse8pre-ctl` is not loaded,
        //    NEVER detach `snd-usb-audio` — return an actionable error!
        if let Some(driver) = &discovery.control_driver {
            return Err(TransportError::Other(format!(
                "AudioControl interface 0 is owned by {driver} and {KERNEL_CTL_DEVICE_PATH} is not loaded. \
                 Build and load the non-intrusive EP0 helper (`make -C kmod && sudo insmod kmod/audiofuse8pre_ctl.ko`) \
                 for live hardware control alongside ALSA/PipeWire, or pass --simulator for offline mode."
            )));
        }

        // 3. Interface 0 is unclaimed: direct `rusb` fallback (Interface 0 only)
        let handle = Self::open_rusb_handle()?;
        handle
            .claim_interface(UAC2_CONTROL_INTERFACE)
            .map_err(|e| {
                TransportError::Other(format!(
                    "Cannot claim AudioControl interface 0: {e}. Leave the ALSA driver attached and load kmod/audiofuse8pre_ctl.ko, or use --simulator."
                ))
            })?;
        Ok(Self {
            backend: UsbBackend::Rusb(Mutex::new(handle)),
            backend_label: "Native USB/UAC2 Hardware (rusb IF0, 1c75:af0e)".to_string(),
            discovery,
        })
    }

    fn open_rusb_handle() -> Result<rusb::DeviceHandle<rusb::GlobalContext>, TransportError> {
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

    fn ctrl_in_exact<const N: usize>(
        &self,
        b_request: u8,
        w_value: u16,
        w_index: u16,
    ) -> Result<[u8; N], TransportError> {
        // Defense-in-depth: strictly enforce Interface 0 only!
        if (w_index & 0xff) != UAC2_CONTROL_INTERFACE as u16 {
            return Err(TransportError::Other(format!(
                "Refusing control transfer targeting non-zero interface in wIndex=0x{w_index:04x}"
            )));
        }
        let mut buf = [0u8; N];
        let count = match &self.backend {
            #[cfg(target_os = "linux")]
            UsbBackend::KernelBridge(bridge) => bridge.lock().unwrap().ctrl_in(
                b_request,
                w_value,
                w_index,
                &mut buf,
                USB_CONTROL_TIMEOUT.as_millis() as u32,
            )?,
            UsbBackend::Rusb(handle) => handle.lock().unwrap().read_control(
                USB_REQ_CLASS_INTERFACE_IN,
                b_request,
                w_value,
                w_index,
                &mut buf,
                USB_CONTROL_TIMEOUT,
            )?,
        };
        exact(N, count)?;
        Ok(buf)
    }

    fn ctrl_out_exact(
        &self,
        b_request: u8,
        w_value: u16,
        w_index: u16,
        data: &[u8],
    ) -> Result<(), TransportError> {
        // Defense-in-depth: strictly enforce Interface 0 only!
        if (w_index & 0xff) != UAC2_CONTROL_INTERFACE as u16 {
            return Err(TransportError::Other(format!(
                "Refusing control transfer targeting non-zero interface in wIndex=0x{w_index:04x}"
            )));
        }
        let count = match &self.backend {
            #[cfg(target_os = "linux")]
            UsbBackend::KernelBridge(bridge) => bridge.lock().unwrap().ctrl_out(
                b_request,
                w_value,
                w_index,
                data,
                USB_CONTROL_TIMEOUT.as_millis() as u32,
            )?,
            UsbBackend::Rusb(handle) => handle.lock().unwrap().write_control(
                USB_REQ_CLASS_INTERFACE_OUT,
                b_request,
                w_value,
                w_index,
                data,
                USB_CONTROL_TIMEOUT,
            )?,
        };
        exact(data.len(), count)
    }
}

impl AudioFuseTransport for UsbHardwareTransport {
    fn backend_name(&self) -> &str {
        &self.backend_label
    }

    fn is_hardware_detected(&self) -> bool {
        true
    }

    fn read_state_266(&self) -> Result<[u8; AFEX_STATE_SIZE], TransportError> {
        self.ctrl_in_exact(UAC2_REQ_MEM, AFEX_STATE_WVALUE_FULL, AFEX_STATE_WINDEX)
    }

    fn read_vu_152(&self) -> Result<[u8; AFEX_VU_SIZE], TransportError> {
        self.ctrl_in_exact(UAC2_REQ_MEM, 0x0000, AFEX_VU_WINDEX)
    }

    fn get_parameter(&self, param: u8, channel: u8) -> Result<u16, TransportError> {
        let raw: [u8; 2] = self.ctrl_in_exact(
            UAC2_REQ_MEM,
            encode_param_wvalue(param, channel),
            AFEX_DEFAULT_WINDEX,
        )?;
        Ok(u16::from_le_bytes(raw))
    }

    fn set_parameter(&self, param: u8, channel: u8, value: u16) -> Result<(), TransportError> {
        self.set_parameter_bytes(param, channel, &value.to_le_bytes())
    }

    fn set_parameter_bytes(
        &self,
        param: u8,
        channel: u8,
        data: &[u8],
    ) -> Result<(), TransportError> {
        self.ctrl_out_exact(
            UAC2_REQ_MEM,
            encode_param_wvalue(param, channel),
            AFEX_DEFAULT_WINDEX,
            data,
        )
    }

    fn get_string(&self, str_type: u8, _index: u16) -> Result<String, TransportError> {
        match str_type {
            STRING_TYPE_DEVICE_NAME => {
                let state = self.read_state_266()?;
                let off = (param_id::DEVICE_NAME as usize) * 2;
                Ok(decode_device_string_16(
                    &state[off..off + MAX_DEVICE_STRING_LEN],
                ))
            }
            STRING_TYPE_CHANNEL_NAME => Err(TransportError::Unsupported(
                "Mixer channel names are host-side only on AudioFuse 8Pre (not stored on hardware)"
                    .into(),
            )),
            _ => Err(TransportError::Unsupported(format!(
                "Unknown string type {str_type}"
            ))),
        }
    }

    fn set_string(&self, str_type: u8, _index: u16, value: &str) -> Result<(), TransportError> {
        match str_type {
            STRING_TYPE_DEVICE_NAME => {
                let encoded = encode_device_name_16(value);
                self.set_parameter_bytes(param_id::DEVICE_NAME, AFEX_GLOBAL_CHANNEL, &encoded)
            }
            STRING_TYPE_CHANNEL_NAME => Err(TransportError::Unsupported(
                "Mixer channel names are host-side only on AudioFuse 8Pre (not stored on hardware)"
                    .into(),
            )),
            _ => Err(TransportError::Unsupported(format!(
                "Unknown string type {str_type}"
            ))),
        }
    }

    fn get_clock_source(&self) -> Result<ClockSource, TransportError> {
        let [pin] = self.ctrl_in_exact(
            UAC2_REQ_CUR,
            (UAC2_CS_CLOCK_SELECTOR_CONTROL as u16) << 8,
            encode_unit_windex(UAC2_ENTITY_CLOCK_SELECTOR, UAC2_CONTROL_INTERFACE),
        )?;
        if !(1..=3).contains(&pin) {
            return Err(TransportError::Other(format!(
                "Invalid UAC2 clock selector pin {pin}"
            )));
        }
        Ok(ClockSource::from_uac2_pin(pin))
    }

    fn set_clock_source(&self, source: ClockSource) -> Result<(), TransportError> {
        // When `snd-usb-audio` owns the card, update via ALSA mixer control `numid=4` so
        // ALSA's internal clock cache stays completely synchronized with the hardware.
        if let Some(card) = &self.discovery.alsa_card_id {
            let idx_str = source.to_alsa_enum_index().to_string();
            let output = Command::new("amixer")
                .args(["-c", card, "cset", "numid=4", &idx_str])
                .output()
                .map_err(|e| TransportError::Other(format!("amixer clock cset failed: {e}")))?;
            if !output.status.success() {
                return Err(TransportError::Other(format!(
                    "amixer -c {card} cset numid=4 {idx_str} failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                )));
            }
            return Ok(());
        }
        self.ctrl_out_exact(
            UAC2_REQ_CUR,
            (UAC2_CS_CLOCK_SELECTOR_CONTROL as u16) << 8,
            encode_unit_windex(UAC2_ENTITY_CLOCK_SELECTOR, UAC2_CONTROL_INTERFACE),
            &[source.to_uac2_pin()],
        )
    }

    fn get_clock_validity(&self) -> Result<[bool; 3], TransportError> {
        // Indexed as [Internal (41), ADAT (44), WordClock (43)]
        let mut valid = [false; 3];
        for (i, entity) in [
            UAC2_ENTITY_CLOCK_INTERNAL,
            UAC2_ENTITY_CLOCK_ADAT,
            UAC2_ENTITY_CLOCK_WORD,
        ]
        .into_iter()
        .enumerate()
        {
            let [value] = self.ctrl_in_exact(
                UAC2_REQ_CUR,
                (UAC2_CS_CLOCK_VALID_CONTROL as u16) << 8,
                encode_unit_windex(entity, UAC2_CONTROL_INTERFACE),
            )?;
            valid[i] = value != 0;
        }
        Ok(valid)
    }

    fn get_sample_rate(&self) -> Result<SampleRate, TransportError> {
        let hz = u32::from_le_bytes(self.ctrl_in_exact(
            UAC2_REQ_CUR,
            (UAC2_CS_SAM_FREQ_CONTROL as u16) << 8,
            encode_unit_windex(UAC2_ENTITY_CLOCK_INTERNAL, UAC2_CONTROL_INTERFACE),
        )?);
        SampleRate::from_hz(hz).ok_or(TransportError::UnsupportedSampleRate(hz))
    }

    fn set_sample_rate(&self, rate: SampleRate) -> Result<(), TransportError> {
        let target_hz = rate.to_hz();
        self.ctrl_out_exact(
            UAC2_REQ_CUR,
            (UAC2_CS_SAM_FREQ_CONTROL as u16) << 8,
            encode_unit_windex(UAC2_ENTITY_CLOCK_INTERNAL, UAC2_CONTROL_INTERFACE),
            &target_hz.to_le_bytes(),
        )?;
        // Verify read-back (PipeWire / active ALSA streams may hold or override sample rate)
        let actual = self.get_sample_rate()?;
        if actual != rate {
            return Err(TransportError::HardwareRejected(format!(
                "Requested {} but hardware remains at {} (likely held by an active ALSA/PipeWire stream)",
                rate.khz_label(),
                actual.khz_label()
            )));
        }
        Ok(())
    }

    fn get_dfu_status(&self) -> Result<DfuStatusReport, TransportError> {
        Err(TransportError::Unsupported(
            "Live runtime DFU queries on Interface 3 are blocked to protect active audio streaming; inspect .af8pre archives offline".into(),
        ))
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

/// Open the best available transport: real `1c75:af0e` hardware by default (`force_simulator = false`),
/// falling back to `SimulatedTransport` only when `force_simulator = true` or no hardware is attached.
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
