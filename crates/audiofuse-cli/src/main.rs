//! `audiofuse-cli`: Command-line control utility for the Arturia AudioFuse 8Pre (`1c75:af0e`).
//!
//! Supports subcommands:
//! - `status`
//! - `inputs`
//! - `mixer`
//! - `routing`
//! - `clock`
//! - `prefs`
//! - `meters`
//! - `preset {save,load,export,import}`
//! - `firmware inspect`

use anyhow::{Context, Result, bail};
use audiofuse_core::{
    AdatOutSource, AnalogOutSource, AudioFuseController, ClockSource, DeviceMode, InputType,
    LinePad, LoopbackSource, MicPadBoost, MixerBank, PhonesSource, SampleRate, SmuxMode,
    SpeakersSource, WordclockImpedance, format_db, format_pan, inspect_af8pre_file,
    open_best_available_transport, vu_to_dbfs,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "audiofuse-cli",
    version,
    about = "Native Linux CLI control utility for the Arturia AudioFuse 8Pre (1c75:af0e)"
)]
struct Cli {
    /// Emit machine-readable JSON output
    #[arg(long, global = true)]
    json: bool,

    /// Force offline deterministic simulator backend
    #[arg(long, global = true)]
    simulator: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Display full hardware discovery, clock, inputs, mixer, monitor, and routing status
    Status,

    /// Inspect or modify the 8 analog input channel strips
    Inputs {
        /// Select input channel (1..=8) to modify
        #[arg(short, long)]
        channel: Option<u8>,

        /// Set input source type: input, mic, line, inst (inst valid on ch 1-2)
        #[arg(long)]
        mode: Option<String>,

        /// Enable or disable +48V phantom power (on/off)
        #[arg(long)]
        phantom: Option<String>,

        /// Enable or disable +48V phantom power for group 1-4 or 5-8 (e.g. --phantom-bank 1-4:on)
        #[arg(long)]
        phantom_bank: Option<String>,

        /// Enable or disable phase invert (on/off)
        #[arg(long)]
        phase: Option<String>,

        /// Set -20dB line pad (off / -20db)
        #[arg(long)]
        line_pad: Option<String>,

        /// Set mic pad/boost (off / pad / boost)
        #[arg(long)]
        mic_pad: Option<String>,
    },

    /// Inspect or modify the 24-channel low-latency hardware cue mixer
    Mixer {
        /// Select mixer channel index (1..=24: 1-8 Analog, 9-16 ADAT, 17-24 USB)
        #[arg(short, long)]
        channel: Option<u8>,

        /// Set channel fader level in dB (-61.0 to +12.0)
        #[arg(long)]
        level: Option<f32>,

        /// Set channel pan (-64 to +64, 0 = Center)
        #[arg(long)]
        pan: Option<i8>,

        /// Set channel mute (on/off)
        #[arg(long)]
        mute: Option<String>,

        /// Set channel solo (on/off)
        #[arg(long)]
        solo: Option<String>,

        /// Set custom channel name (max 16 ASCII characters)
        #[arg(long)]
        name: Option<String>,

        /// Assign channel group (0 = None, 1..=4 = Group 1-4)
        #[arg(long)]
        group: Option<u8>,

        /// Toggle stereo link for a bank pair (e.g. analog:1-2:on, adat:3-4:off)
        #[arg(long)]
        stereo_link: Option<String>,
    },

    /// Inspect or modify the master monitor section and output/loopback routing matrix
    Routing {
        /// Set Speakers source (main / cue)
        #[arg(long)]
        speakers: Option<String>,

        /// Set Speakers mono sum (on/off)
        #[arg(long)]
        speakers_mono: Option<String>,

        /// Set Phones source (main / cue / follow)
        #[arg(long)]
        phones: Option<String>,

        /// Set Phones mono sum (on/off)
        #[arg(long)]
        phones_mono: Option<String>,

        /// Set Direct/Computer Mix balance (0.0 = Direct Cue .. 1.0 = Computer USB)
        #[arg(long)]
        mix_balance: Option<f32>,

        /// Set Master Cue Output level in dB (-61.0 to 0.0)
        #[arg(long)]
        master_level: Option<f32>,
        #[arg(long)]
        master_mute: Option<String>,
        #[arg(long)]
        master_solo: Option<String>,

        /// Set Analog Out stereo pair source (e.g. 1-2:usb, 3-4:cue, 5-6:analog, 7-8:adat)
        #[arg(long)]
        analog_out: Option<String>,

        /// Set ADAT Out stereo pair source (e.g. 1-2:usb, 3-4:cue, 5-6:analog)
        #[arg(long)]
        adat_out: Option<String>,

        /// Set Loopback recording source (disabled / cue / usb)
        #[arg(long)]
        loopback: Option<String>,
    },

    /// Inspect or configure UAC2 clock source, lock validity, and sample rate
    Clock {
        /// Set clock source (internal / adat / wordclock)
        #[arg(long)]
        source: Option<String>,

        /// Set sample rate in Hz (44100 / 48000 / 88200 / 96000)
        #[arg(long)]
        rate: Option<u32>,
    },

    /// Inspect or configure global hardware preferences (USB/ADAT mode, Wordclock, ADAT, S/MUX)
    Prefs {
        /// Set Device Name (16 ASCII chars max)
        #[arg(long)]
        device_name: Option<String>,

        /// Set Device Mode (usb / adat)
        #[arg(long)]
        mode: Option<String>,

        /// Set Wordclock 75-ohm termination (75ohm / hiz)
        #[arg(long)]
        wc_impedance: Option<String>,

        /// Set Wordclock Thru (on / off)
        #[arg(long)]
        wc_thru: Option<String>,

        /// Set ADAT optical I/O enable (on / off)
        #[arg(long)]
        adat_on: Option<String>,

        /// Set S/MUX framing mode (normal / double)
        #[arg(long)]
        smux: Option<String>,
    },

    /// Poll and display the 78-byte (0x4e) hardware bulk state and real-time VU meters
    Meters {
        /// Also include raw 78-byte hex dump
        #[arg(long)]
        hex: bool,
    },

    /// Hardware preset slot management and local JSON preset import/export
    Preset {
        #[command(subcommand)]
        action: PresetCommand,
    },

    /// Inspect .af8pre firmware archives and query USB DFU Interface 3 status
    Firmware {
        #[command(subcommand)]
        action: FirmwareCommand,
    },
}

#[derive(Subcommand, Debug)]
enum PresetCommand {
    /// Save current state to a hardware preset slot (0..=7) via PresetSave (0x49)
    Save {
        /// Hardware slot index (0..=7)
        slot: u8,
    },
    /// Load state from a hardware preset slot (0..=7) via PresetLoad (0x48)
    Load {
        /// Hardware slot index (0..=7)
        slot: u8,
    },
    /// Export current state to a local JSON preset file
    Export {
        /// Output file path (.json)
        path: PathBuf,
        /// Optional preset name
        #[arg(long, default_value = "Studio Session")]
        name: String,
        /// Optional description
        #[arg(long, default_value = "Exported from audiofuse-cli")]
        description: String,
    },
    /// Import and apply state from a local JSON preset file
    Import {
        /// Input file path (.json)
        path: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum FirmwareCommand {
    /// Inspect an Arturia .af8pre firmware archive and USB DFU Interface 3 status
    Inspect {
        /// Optional path to a .af8pre firmware file
        path: Option<PathBuf>,
    },
}

fn parse_bool_flag(s: &str) -> Result<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" | "enable" | "enabled" => Ok(true),
        "0" | "false" | "off" | "no" | "disable" | "disabled" => Ok(false),
        other => bail!(
            "Invalid boolean value '{}' (expected on/off or true/false)",
            other
        ),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if !cli.simulator
        && matches!(
            cli.command,
            None | Some(Commands::Status)
                | Some(Commands::Clock {
                    source: None,
                    rate: None
                })
        )
    {
        let discovery = audiofuse_core::probe_linux_host_hardware();
        if discovery.usb_present {
            let payload = serde_json::json!({"backend":"Linux sysfs / ALSA (read-only)", "hardware_detected":true,"discovery":discovery});
            println!("{}", serde_json::to_string_pretty(&payload)?);
            if let Some(e) = discovery.alsa_error {
                bail!("ALSA discovery failed: {e}");
            }
            return Ok(());
        }
    }
    if let Some(Commands::Firmware {
        action: FirmwareCommand::Inspect { path: Some(path) },
    }) = &cli.command
    {
        let info = inspect_af8pre_file(path)?;
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }
    if !cli.simulator
        && matches!(
            cli.command,
            Some(Commands::Firmware {
                action: FirmwareCommand::Inspect { path: None }
            })
        )
    {
        bail!(
            "Live DFU status is unavailable. Pass a .af8pre path to inspect archive metadata offline."
        );
    }
    let transport = open_best_available_transport(cli.simulator)?;
    let mut ctrl = AudioFuseController::new(transport)?;

    match cli.command.unwrap_or(Commands::Status) {
        Commands::Status => {
            if cli.json {
                let payload = serde_json::json!({
                    "backend": ctrl.backend_name(),
                    "hardware_detected": ctrl.is_hardware_detected(),
                    "discovery": ctrl.discovery_info(),
                    "state": ctrl.state(),
                });
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print_status(&ctrl);
            }
        }
        Commands::Inputs {
            channel,
            mode,
            phantom,
            phantom_bank,
            phase,
            line_pad,
            mic_pad,
        } => {
            if let Some(bank_spec) = phantom_bank {
                let parts: Vec<&str> = bank_spec.split(':').collect();
                if parts.len() != 2 {
                    bail!("Expected --phantom-bank <1-4|5-8>:<on|off>");
                }
                let bank_idx = match parts[0] {
                    "1-4" | "1" => 0,
                    "5-8" | "2" => 1,
                    other => bail!("Invalid phantom bank '{}' (expected 1-4 or 5-8)", other),
                };
                let en = parse_bool_flag(parts[1])?;
                ctrl.set_phantom_bank_4(bank_idx, en)?;
            }
            if let Some(ch) = channel {
                if !(1..=8).contains(&ch) {
                    bail!("Input channel must be 1..=8");
                }
                let idx = (ch - 1) as usize;
                if let Some(m) = mode {
                    let it = match m.to_ascii_lowercase().as_str() {
                        "input" | "auto" | "combo" => InputType::ComboAuto,
                        "mic" => InputType::Mic,
                        "line" => InputType::Line,
                        "inst" | "hiz" | "hi-z" => InputType::Inst,
                        other => bail!(
                            "Unknown input mode '{}' (expected input/mic/line/inst)",
                            other
                        ),
                    };
                    ctrl.set_input_type(idx, it)?;
                }
                if let Some(ph) = phantom {
                    ctrl.set_input_phantom(idx, parse_bool_flag(&ph)?)?;
                }
                if let Some(p) = phase {
                    ctrl.set_input_phase_invert(idx, parse_bool_flag(&p)?)?;
                }
                if let Some(lp) = line_pad {
                    let pad = match lp.to_ascii_lowercase().as_str() {
                        "off" | "0" => LinePad::Off,
                        "-20db" | "pad" | "on" | "1" => LinePad::Pad20dB,
                        other => bail!("Unknown line pad '{}' (expected off or -20db)", other),
                    };
                    ctrl.set_input_line_pad(idx, pad)?;
                }
                if let Some(mp) = mic_pad {
                    let mpad = match mp.to_ascii_lowercase().as_str() {
                        "off" | "0" => MicPadBoost::Off,
                        "pad" | "-20db" | "1" => MicPadBoost::Pad20dB,
                        "boost" | "2" => MicPadBoost::Boost,
                        other => bail!("Unknown mic pad '{}' (expected off, pad, or boost)", other),
                    };
                    ctrl.set_input_mic_pad_boost(idx, mpad)?;
                }
            }
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&ctrl.state().inputs)?);
            } else {
                print_inputs(&ctrl);
            }
        }
        Commands::Mixer {
            channel,
            level,
            pan,
            mute,
            solo,
            name,
            group,
            stereo_link,
        } => {
            if let Some(link_spec) = stereo_link {
                let parts: Vec<&str> = link_spec.split(':').collect();
                if parts.len() != 3 {
                    bail!("Expected --stereo-link <analog|adat|usb>:<1-2|3-4|5-6|7-8>:<on|off>");
                }
                let bank = match parts[0].to_ascii_lowercase().as_str() {
                    "analog" => MixerBank::Analog,
                    "adat" => MixerBank::Adat,
                    "usb" => MixerBank::Usb,
                    other => bail!(
                        "Invalid mixer bank '{}' (expected analog, adat, or usb)",
                        other
                    ),
                };
                let pair_idx = match parts[1] {
                    "1-2" => 0,
                    "3-4" => 1,
                    "5-6" => 2,
                    "7-8" => 3,
                    other => bail!(
                        "Invalid stereo pair '{}' (expected 1-2, 3-4, 5-6, or 7-8)",
                        other
                    ),
                };
                let linked = parse_bool_flag(parts[2])?;
                ctrl.set_stereo_link(bank, pair_idx, linked)?;
            }
            if let Some(ch) = channel {
                if !(1..=24).contains(&ch) {
                    bail!("Mixer channel must be 1..=24");
                }
                let idx = (ch - 1) as usize;
                if let Some(grp) = group {
                    ctrl.set_mixer_channel_group(idx, grp);
                }
                if let Some(lvl) = level {
                    ctrl.set_mixer_channel_level_db(idx, lvl)?;
                }
                if let Some(p) = pan {
                    ctrl.set_mixer_channel_pan(idx, p)?;
                }
                if let Some(m) = mute {
                    ctrl.set_mixer_channel_mute(idx, parse_bool_flag(&m)?)?;
                }
                if let Some(s) = solo {
                    ctrl.set_mixer_channel_solo(idx, parse_bool_flag(&s)?)?;
                }
                if let Some(n) = name {
                    ctrl.set_mixer_channel_name(idx, &n)?;
                }
            }
            if cli.json {
                let payload = serde_json::json!({
                    "stereo_links": ctrl.state().stereo_links,
                    "channels": ctrl.state().mixer_channels,
                });
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print_mixer(&ctrl);
            }
        }
        Commands::Routing {
            speakers,
            speakers_mono,
            phones,
            phones_mono,
            mix_balance,
            master_level,
            master_mute,
            master_solo,
            analog_out,
            adat_out,
            loopback,
        } => {
            if let Some(sp) = speakers {
                let src = match sp.to_ascii_lowercase().as_str() {
                    "main" | "main1-2" | "0" => SpeakersSource::Main1_2,
                    "cue" | "cuemix" | "1" => SpeakersSource::CueMix,
                    other => bail!("Invalid speakers source '{}' (expected main or cue)", other),
                };
                ctrl.set_speakers_source(src)?;
            }
            if let Some(sm) = speakers_mono {
                ctrl.set_speakers_mono(parse_bool_flag(&sm)?)?;
            }
            if let Some(ph) = phones {
                let src = match ph.to_ascii_lowercase().as_str() {
                    "main" | "main1-2" | "0" => PhonesSource::Main1_2,
                    "cue" | "cuemix" | "1" => PhonesSource::CueMix,
                    "follow" | "speakers" | "2" => PhonesSource::FollowSpeakers,
                    other => bail!(
                        "Invalid phones source '{}' (expected main, cue, or follow)",
                        other
                    ),
                };
                ctrl.set_phones_source(src)?;
            }
            if let Some(pm) = phones_mono {
                ctrl.set_phones_mono(parse_bool_flag(&pm)?)?;
            }
            if let Some(mb) = mix_balance {
                ctrl.set_direct_computer_mix(mb)?;
            }
            if let Some(v) = master_mute {
                ctrl.set_mixer_output_mute(parse_bool_flag(&v)?)?;
            }
            if let Some(v) = master_solo {
                ctrl.set_mixer_output_solo(parse_bool_flag(&v)?)?;
            }
            if let Some(ml) = master_level {
                ctrl.set_mixer_output_level_db(ml)?;
            }
            if let Some(ao) = analog_out {
                let parts: Vec<&str> = ao.split(':').collect();
                if parts.len() != 2 {
                    bail!("Expected --analog-out <1-2|3-4|5-6|7-8>:<cue|usb|analog|adat>");
                }
                let pair_idx = match parts[0] {
                    "1-2" => 0,
                    "3-4" => 1,
                    "5-6" => 2,
                    "7-8" => 3,
                    other => bail!("Invalid pair '{}' (expected 1-2, 3-4, 5-6, or 7-8)", other),
                };
                let src = match parts[1].to_ascii_lowercase().as_str() {
                    "cue" | "cuemix" => AnalogOutSource::CueMix,
                    "usb" => AnalogOutSource::UsbPair,
                    "analog" => AnalogOutSource::AnalogInPair,
                    "adat" => AnalogOutSource::AdatInPair,
                    other => bail!(
                        "Invalid analog out source '{}' (expected cue, usb, analog, or adat)",
                        other
                    ),
                };
                ctrl.set_analog_out_source(pair_idx, src)?;
            }
            if let Some(ado) = adat_out {
                let parts: Vec<&str> = ado.split(':').collect();
                if parts.len() != 2 {
                    bail!("Expected --adat-out <1-2|3-4|5-6|7-8>:<cue|usb|analog>");
                }
                let pair_idx = match parts[0] {
                    "1-2" => 0,
                    "3-4" => 1,
                    "5-6" => 2,
                    "7-8" => 3,
                    other => bail!("Invalid pair '{}' (expected 1-2, 3-4, 5-6, or 7-8)", other),
                };
                let src = match parts[1].to_ascii_lowercase().as_str() {
                    "cue" | "cuemix" => AdatOutSource::CueMix,
                    "usb" => AdatOutSource::UsbPair,
                    "analog" => AdatOutSource::AnalogInPair,
                    other => bail!(
                        "Invalid ADAT out source '{}' (expected cue, usb, or analog)",
                        other
                    ),
                };
                ctrl.set_adat_out_source(pair_idx, src)?;
            }
            if let Some(lb) = loopback {
                let src = match lb.to_ascii_lowercase().as_str() {
                    "disabled" | "off" | "0" => LoopbackSource::Disabled,
                    "cue" | "cuemix" | "1" => LoopbackSource::CueMix,
                    "usb" | "usb17-18" | "2" => LoopbackSource::Usb17_18,
                    other => bail!(
                        "Invalid loopback source '{}' (expected disabled, cue, or usb)",
                        other
                    ),
                };
                ctrl.set_loopback_source(src)?;
            }
            if cli.json {
                let payload = serde_json::json!({
                    "monitor": ctrl.state().monitor,
                    "routing": ctrl.state().routing,
                });
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print_routing(&ctrl);
            }
        }
        Commands::Clock { source, rate } => {
            if let Some(src_str) = source {
                let cs = match src_str.to_ascii_lowercase().as_str() {
                    "internal" | "int" | "0" => ClockSource::Internal,
                    "adat" | "1" => ClockSource::Adat,
                    "wordclock" | "word" | "wc" | "2" => ClockSource::WordClock,
                    other => bail!(
                        "Unknown clock source '{}' (expected internal, adat, or wordclock)",
                        other
                    ),
                };
                ctrl.set_clock_source(cs)?;
            }
            if let Some(hz) = rate {
                let sr = SampleRate::from_hz(hz)
                    .ok_or_else(|| anyhow::anyhow!("Unsupported sample rate {} Hz", hz))?;
                ctrl.set_sample_rate(sr)?;
            }
            if cli.json {
                let cfg = &ctrl.state().config;
                let payload = serde_json::json!({
                    "clock_source": cfg.clock_source,
                    "uac2_selector_entity": 40,
                    "uac2_clock_entity": cfg.clock_source.uac2_clock_entity(),
                    "clock_locked": cfg.clock_locked,
                    "clock_validity": {
                        "internal_41": cfg.clock_validity[0],
                        "adat_43": cfg.clock_validity[1],
                        "wordclock_44": cfg.clock_validity[2],
                    },
                    "sample_rate_hz": cfg.sample_rate.to_hz(),
                    "sample_rate_label": cfg.sample_rate.khz_label(),
                });
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print_clock(&ctrl);
            }
        }
        Commands::Prefs {
            device_name,
            mode,
            wc_impedance,
            wc_thru,
            adat_on,
            smux,
        } => {
            if let Some(name) = device_name {
                ctrl.set_device_name(&name)?;
            }
            if let Some(m) = mode {
                let dm = match m.to_ascii_lowercase().as_str() {
                    "usb" | "0" => DeviceMode::Usb,
                    "adat" | "1" => DeviceMode::Adat,
                    other => bail!("Invalid device mode '{}' (expected usb or adat)", other),
                };
                ctrl.set_device_mode(dm)?;
            }
            if let Some(imp) = wc_impedance {
                let wi = match imp.to_ascii_lowercase().as_str() {
                    "75" | "75ohm" | "75ohms" | "on" | "0" => WordclockImpedance::Ohms75,
                    "hiz" | "hi-z" | "off" | "1" => WordclockImpedance::HiZ,
                    other => bail!(
                        "Invalid wordclock impedance '{}' (expected 75ohm or hiz)",
                        other
                    ),
                };
                ctrl.set_wordclock_impedance(wi)?;
            }
            if let Some(thru) = wc_thru {
                ctrl.set_wordclock_thru(parse_bool_flag(&thru)?)?;
            }
            if let Some(ad) = adat_on {
                ctrl.set_adat_on(parse_bool_flag(&ad)?)?;
            }
            if let Some(sm) = smux {
                let mode = match sm.to_ascii_lowercase().as_str() {
                    "normal" | "single" | "0" => SmuxMode::Normal,
                    "double" | "doublespeed" | "smux" | "1" => SmuxMode::DoubleSpeed,
                    other => bail!("Invalid SMUX mode '{}' (expected normal or double)", other),
                };
                ctrl.set_smux_mode(mode)?;
            }
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&ctrl.state().config)?);
            } else {
                print_prefs(&ctrl);
            }
        }
        Commands::Meters { hex } => {
            let bulk = ctrl.poll_bulk_state_and_meters()?;
            if cli.json {
                let hex_str: String = bulk.iter().map(|b| format!("{:02x}", b)).collect();
                let payload = serde_json::json!({
                    "payload_size_bytes": bulk.len(),
                    "bulk_hex_78b": hex_str,
                    "analog_in_vu": ctrl.state().inputs.iter().map(|c| c.vu).collect::<Vec<_>>(),
                    "adat_in_vu": ctrl.state().adat_in_vu,
                    "usb_in_vu": ctrl.state().usb_in_vu,
                    "analog_out_vu": ctrl.state().routing.analog_out_vu,
                    "adat_out_vu": ctrl.state().routing.adat_out_vu,
                    "mixer_main_vu": [ctrl.state().monitor.mixer_main_vu_l, ctrl.state().monitor.mixer_main_vu_r],
                    "speakers_vu": [ctrl.state().monitor.speakers_vu_l, ctrl.state().monitor.speakers_vu_r],
                    "phones_vu": [ctrl.state().monitor.phones_vu_l, ctrl.state().monitor.phones_vu_r],
                    "loopback_vu": [ctrl.state().routing.loopback_vu_l, ctrl.state().routing.loopback_vu_r],
                });
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                print_meters(&ctrl, &bulk, hex);
            }
        }
        Commands::Preset { action } => match action {
            PresetCommand::Save { slot } => {
                ctrl.save_hardware_preset(slot)?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "action": "preset_save",
                            "slot": slot,
                            "request_code": "0x49",
                            "status": "ok"
                        }))?
                    );
                } else {
                    println!("Saved hardware preset to slot {} (PresetSave 0x49).", slot);
                }
            }
            PresetCommand::Load { slot } => {
                ctrl.load_hardware_preset(slot)?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "action": "preset_load",
                            "slot": slot,
                            "request_code": "0x48",
                            "status": "ok"
                        }))?
                    );
                } else {
                    println!(
                        "Loaded hardware preset from slot {} (PresetLoad 0x48).",
                        slot
                    );
                }
            }
            PresetCommand::Export {
                path,
                name,
                description,
            } => {
                let preset = ctrl
                    .export_preset_file(&path, name, description)
                    .with_context(|| format!("Failed to export preset to {}", path.display()))?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&preset)?);
                } else {
                    println!(
                        "Exported preset '{}' to {}",
                        preset.preset_name,
                        path.display()
                    );
                }
            }
            PresetCommand::Import { path } => {
                let preset = ctrl
                    .import_preset_file(&path)
                    .with_context(|| format!("Failed to import preset from {}", path.display()))?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&preset)?);
                } else {
                    println!(
                        "Imported and applied preset '{}' from {}",
                        preset.preset_name,
                        path.display()
                    );
                }
            }
        },
        Commands::Firmware { action } => match action {
            FirmwareCommand::Inspect { path } => {
                let dfu = ctrl.dfu_status()?;
                let archive_info = if let Some(p) = path {
                    Some(inspect_af8pre_file(&p).with_context(|| {
                        format!("Failed to inspect .af8pre archive at {}", p.display())
                    })?)
                } else {
                    None
                };

                if cli.json {
                    let payload = serde_json::json!({
                        "device_bcd_firmware": ctrl.state().config.firmware_version,
                        "dfu_status": dfu,
                        "archive_inspection": archive_info,
                    });
                    println!("{}", serde_json::to_string_pretty(&payload)?);
                } else {
                    println!("=== ARTURIA AUDIOFUSE 8PRE FIRMWARE & DFU STATUS ===");
                    println!(
                        "Device Firmware (bcdDevice): v{}",
                        ctrl.state().config.firmware_version
                    );
                    println!(
                        "USB DFU Interface          : Interface {} (0xfe/0x01, wTransferSize={}B, bmAttributes=0x{:02x})",
                        dfu.interface_number, dfu.transfer_size, dfu.attributes
                    );
                    println!(
                        "DFU State                  : {} (status={}, pollTimeout={}ms)",
                        dfu.state.as_str(),
                        dfu.status_code,
                        dfu.poll_timeout_ms
                    );
                    if let Some(info) = archive_info {
                        println!("\n--- ARCHIVE METADATA ({}) ---", info.archive_path);
                        println!(
                            "Vendor / Product ID        : {} : {}",
                            info.manifest.vendorid, info.manifest.productid
                        );
                        println!(
                            "Package Version            : v{} (date {})",
                            info.manifest.version_number, info.manifest.date
                        );
                        println!(
                            "Archive targets AudioFuse 8Pre : {}",
                            if info.targets_audiofuse_8pre {
                                "YES"
                            } else {
                                "NO"
                            }
                        );
                        for img in &info.images {
                            println!(
                                "Image #{}                   : {} ({} bytes, {} x 64B DFU blocks, FNV1a={})",
                                img.image_num,
                                img.file_name,
                                img.size_bytes,
                                img.dfu_block_count_64b,
                                img.fnv1a_64_hex
                            );
                        }
                    }
                }
            }
        },
    }

    Ok(())
}

fn print_status(ctrl: &AudioFuseController) {
    let disc = ctrl.discovery_info();
    let cfg = &ctrl.state().config;
    println!("================================================================================");
    println!(
        " ARTURIA AUDIOFUSE 8PRE CONTROL CENTER | VID:PID {:04x}:{:04x} | SN {}",
        disc.vendor_id, disc.product_id, cfg.serial_number
    );
    println!("================================================================================");
    println!("Backend         : {}", ctrl.backend_name());
    println!(
        "USB Bus/Address : Bus {:03} Device {:03} (ALSA Card: {})",
        disc.bus_number,
        disc.device_address,
        disc.alsa_card_id.as_deref().unwrap_or("offline")
    );
    println!(
        "Device Name     : {} (FW v{}, Mode: {})",
        cfg.device_name, cfg.firmware_version, cfg.device_mode
    );
    print_clock(ctrl);
    println!();
    print_prefs(ctrl);
    println!();
    print_inputs(ctrl);
    println!();
    print_routing(ctrl);
}

fn print_clock(ctrl: &AudioFuseController) {
    let cfg = &ctrl.state().config;
    println!("--- CLOCK & SAMPLE RATE (UAC2 Entities 40, 41, 43, 44) ---");
    println!(
        "Clock Source    : {} (Entity 0x{:02x}) | Lock: {}",
        cfg.clock_source,
        cfg.clock_source.uac2_clock_entity(),
        if cfg.clock_locked {
            "LOCKED"
        } else {
            "UNLOCKED"
        }
    );
    println!(
        "Clock Validity  : Internal(41)={} | ADAT(43)={} | WordClock(44)={}",
        if cfg.clock_validity[0] { "VALID" } else { "NO" },
        if cfg.clock_validity[1] { "VALID" } else { "NO" },
        if cfg.clock_validity[2] { "VALID" } else { "NO" }
    );
    println!("Sample Rate     : {}", cfg.sample_rate);
}

fn print_prefs(ctrl: &AudioFuseController) {
    let cfg = &ctrl.state().config;
    println!("--- DEVICE PREFERENCES (AFEXDevice Unit 0x0a) ---");
    println!(
        "Device Mode     : {} | ADAT Optical: {} | S/MUX: {}",
        cfg.device_mode,
        if cfg.adat_on { "ON" } else { "OFF" },
        cfg.smux_mode
    );
    println!(
        "Word Clock      : Termination={} | Thru={}",
        cfg.wordclock_impedance,
        if cfg.wordclock_thru { "ON" } else { "OFF" }
    );
}

fn print_inputs(ctrl: &AudioFuseController) {
    println!("--- ANALOG INPUT CHANNEL STRIPS (1-8) ---");
    println!("CH  SOURCE   HI-Z   +48V   PHASE  LINE-PAD  MIC-PAD/BOOST   VU (dBFS)");
    for inp in &ctrl.state().inputs {
        let hiz = if inp.supports_inst() {
            if inp.inst_hi_z { "INST" } else { "OFF " }
        } else {
            "N/A "
        };
        println!(
            "{:02}  {:6}   {:4}   {:4}   {:5}  {:8}  {:13}   {:3} ({:5.1} dB)",
            inp.channel_number,
            inp.input_type.as_str(),
            hiz,
            if inp.phantom_48v { "ON" } else { "OFF" },
            if inp.phase_invert { "INV" } else { "NORM" },
            inp.pad_line.as_str(),
            inp.pad_mic.as_str(),
            inp.vu,
            vu_to_dbfs(inp.vu)
        );
    }
}

fn print_mixer(ctrl: &AudioFuseController) {
    println!("--- 24-CHANNEL LOW-LATENCY HARDWARE CUE MIXER ---");
    println!(
        "IDX  BANK      NAME              FADER       PAN    MUTE  SOLO  PEAK  GRP  LINK   VU"
    );
    for ch in &ctrl.state().mixer_channels {
        let linked = ctrl.stereo_partner(ch.index as usize).is_some();
        println!(
            "{:02}   {:8}  {:16}  {:>9}   {:>4}   {:4}  {:4}  {:4}  {:3}  {:4}  {:3}",
            ch.index + 1,
            format!("{} {}", ch.bank.prefix(), ch.bank_channel),
            ch.name,
            format_db(ch.level_db),
            format_pan(ch.pan),
            if ch.mute { "MUTE" } else { "-" },
            if ch.solo { "SOLO" } else { "-" },
            if ch.peak { "PEAK" } else { "-" },
            if ch.group == 0 {
                "-".to_string()
            } else {
                format!("G{}", ch.group)
            },
            if linked { "ST" } else { "MO" },
            ch.vu
        );
    }
}

fn print_routing(ctrl: &AudioFuseController) {
    let mon = &ctrl.state().monitor;
    let rt = &ctrl.state().routing;
    println!("--- MASTER MONITOR & CUE SECTION ---");
    println!(
        "Cue Master Out  : {} | Mute={} | Solo={} | Direct/USB Balance={:.0}%/{:.0}% | VU L/R={}/{}",
        format_db(mon.mixer_output_level_db),
        if mon.mixer_output_mute { "ON" } else { "OFF" },
        if mon.mixer_output_solo { "ON" } else { "OFF" },
        (1.0 - mon.direct_computer_mix) * 100.0,
        mon.direct_computer_mix * 100.0,
        mon.mixer_main_vu_l,
        mon.mixer_main_vu_r
    );
    println!(
        "Speakers Out    : Source={:12} | Mono={:3} | VU L/R={}/{}",
        mon.speakers_source.as_str(),
        if mon.speakers_mono { "ON" } else { "OFF" },
        mon.speakers_vu_l,
        mon.speakers_vu_r
    );
    println!(
        "Phones Out      : Source={:12} | Mono={:3} | VU L/R={}/{}",
        mon.phones_source.as_str(),
        if mon.phones_mono { "ON" } else { "OFF" },
        mon.phones_vu_l,
        mon.phones_vu_r
    );
    println!("--- OUTPUT & LOOPBACK ROUTING MATRIX ---");
    for pair in 0..4 {
        let ch_start = pair * 2 + 1;
        let ch_end = ch_start + 1;
        println!(
            "Analog Out {}-{}  : {:16} | ADAT Out {}-{}: {:16}",
            ch_start,
            ch_end,
            rt.analog_out_sources[pair].label_for_pair(pair),
            ch_start,
            ch_end,
            rt.adat_out_sources[pair].label_for_pair(pair)
        );
    }
    println!(
        "Loopback Rec    : {:16} | VU L/R={}/{}",
        rt.loopback_source.as_str(),
        rt.loopback_vu_l,
        rt.loopback_vu_r
    );
}

fn print_meters(ctrl: &AudioFuseController, bulk: &[u8], include_hex: bool) {
    let s = ctrl.state();
    println!("--- REAL-TIME HARDWARE VU METERS (0x4e / 78-Byte Bulk Payload) ---");
    print!("Analog In 1-8 : ");
    for i in 0..8 {
        print!("Ch{}:{:3} ", i + 1, s.inputs[i].vu);
    }
    println!();
    print!("ADAT In 1-8   : ");
    for i in 0..8 {
        print!("Ch{}:{:3} ", i + 1, s.adat_in_vu[i]);
    }
    println!();
    print!("USB Play 1-8  : ");
    for i in 0..8 {
        print!("Ch{}:{:3} ", i + 1, s.usb_in_vu[i]);
    }
    println!();
    print!("Analog Out 1-8: ");
    for i in 0..8 {
        print!("Ch{}:{:3} ", i + 1, s.routing.analog_out_vu[i]);
    }
    println!();
    print!("ADAT Out 1-8  : ");
    for i in 0..8 {
        print!("Ch{}:{:3} ", i + 1, s.routing.adat_out_vu[i]);
    }
    println!();
    println!(
        "Master / Mon  : CueMain={:3}/{:3} | Speakers={:3}/{:3} | Phones={:3}/{:3} | Loopback={:3}/{:3}",
        s.monitor.mixer_main_vu_l,
        s.monitor.mixer_main_vu_r,
        s.monitor.speakers_vu_l,
        s.monitor.speakers_vu_r,
        s.monitor.phones_vu_l,
        s.monitor.phones_vu_r,
        s.routing.loopback_vu_l,
        s.routing.loopback_vu_r
    );
    if include_hex {
        println!("\nRaw 78-byte payload (0x4e):");
        for (idx, chunk) in bulk.chunks(16).enumerate() {
            print!("{:04x}: ", idx * 16);
            for b in chunk {
                print!("{:02x} ", b);
            }
            println!();
        }
    }
}
