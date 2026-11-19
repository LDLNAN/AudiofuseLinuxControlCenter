//! Native Linux Desktop Studio Control Center UI (`eframe`/`egui`) for the
//! Arturia AudioFuse 8Pre (`1c75:af0e`).
//!
//! Console sections:
//! - Top Hardware Rack Bar: Device Name, Serial, Firmware/DFU inspector, USB/ADAT Mode,
//!   Clock Source (`Internal` / `ADAT` / `Word Clock`), Lock Status, Sample Rate (`44.1..96 kHz`),
//!   Wordclock `75 Ohms` / `Thru`, ADAT & `S/MUX`, Hardware Preset Slots `0..7`, JSON Export/Import,
//!   and Dark (`#141413`) / Light (`#f6f5f2`) studio theme toggle.
//! - Upper Console: 8 Analog Input Channel Strips (`Mic`/`Line`/`Inst` Hi-Z on Ch 1-2,
//!   `+48V` Phantom Power per-channel & banks `1-4`/`5-8`, Phase Invert `Ø`, `-20dB` Line Pad,
//!   `OFF`/`PAD`/`BOOST` Mic Pad/Boost, and live dBFS VU meters).
//! - Lower Console (Left): 24-Channel Low-Latency Hardware Cue Mixer (`Ctrl+1` Analog 1-8,
//!   `Ctrl+2` ADAT 1-8, `Ctrl+3` USB 1-8, or All-24 Compact View) with `PiecewiseLinear(0.842105263)`
//!   faders (`-61..+12 dB`, `< -60.5` as `-inf`), Pan (`-64..+64`), Mute, Solo, Peak indicator,
//!   Stereo Linking (`1-2`, `3-4`, `5-6`, `7-8`), Channel Groups `1-4`, and 16-char custom channel names.
//! - Lower Console (Right): Master Cue & Monitor Section (`Direct/Computer Mix`, Master Output Fader/Mute/Solo,
//!   Speakers Source/Mono, Phones Source/Mono) + Output & Loopback Routing Matrix (`Analog Out 1-8`,
//!   `ADAT Out 1-8`, `Loopback`).

use crate::theme::{StudioPalette, StudioThemeMode};
use audiofuse_core::{
    AdatOutSource, AnalogOutSource, AudioFuseController, ClockSource, DeviceMode,
    FirmwareInspection, InputType, LinePad, LoopbackSource, MicPadBoost, MixerBank, PhonesSource,
    SampleRate, SmuxMode, SpeakersSource, TelemetryFrame, WordclockImpedance,
    fader_db_to_normalized, fader_normalized_to_db, format_db, format_pan, inspect_af8pre_file,
    vu_to_dbfs,
};
use egui::{
    Align, Color32, CornerRadius, FontId, Frame, Key, Layout, Margin, RichText, Sense, Stroke, Ui,
    Vec2,
};
use std::path::PathBuf;
use std::time::Duration;

pub struct ControlCenterApp {
    pub controller: AudioFuseController,
    pub theme_mode: StudioThemeMode,
    pub active_bank: MixerBank,
    pub show_all_24_channels: bool,
    pub preset_file_path: String,
    pub firmware_archive_path: String,
    pub inspected_firmware: Option<FirmwareInspection>,
    pub show_firmware_modal: bool,
    pub status_banner: String,
    meters: std::sync::mpsc::Receiver<Result<TelemetryFrame, audiofuse_core::TransportError>>,
    pub screenshot: Option<PathBuf>,
    page: usize,
    device_name: String,
    dfu: String,
    frames: u32,
}

impl ControlCenterApp {
    pub fn new(controller: AudioFuseController, theme_mode: StudioThemeMode) -> Self {
        let device_name = controller.state().config.device_name.clone();
        let meters = controller.subscribe_telemetry();
        let dfu = match controller.dfu_status() {
            Ok(report) => format!("{} • {}", controller.backend_name(), report.state.as_str()),
            Err(error) => format!("DFU status unavailable: {error}"),
        };
        let status_banner = controller.backend_name().to_string();
        Self {
            controller,
            theme_mode,
            active_bank: MixerBank::Analog,
            show_all_24_channels: false,
            preset_file_path: "audiofuse-8pre-session.json".to_string(),
            firmware_archive_path: String::new(),
            inspected_firmware: None,
            show_firmware_modal: false,
            status_banner,
            meters,
            frames: 0,
            screenshot: None,
            page: 0,
            device_name,
            dfu,
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.modifiers.ctrl || i.modifiers.command {
                if i.key_pressed(Key::Num1) {
                    self.page = 0;
                    self.active_bank = MixerBank::Analog;
                    self.show_all_24_channels = false;
                    self.status_banner = "BANK SELECT: ANALOG IN 1-8 (Ctrl+1)".to_string();
                } else if i.key_pressed(Key::Num2) {
                    self.page = 0;
                    self.active_bank = MixerBank::Adat;
                    self.show_all_24_channels = false;
                    self.status_banner = "BANK SELECT: ADAT IN 1-8 (Ctrl+2)".to_string();
                } else if i.key_pressed(Key::Num3) {
                    self.page = 0;
                    self.active_bank = MixerBank::Usb;
                    self.show_all_24_channels = false;
                    self.status_banner = "BANK SELECT: USB PLAY 1-8 (Ctrl+3)".to_string();
                } else if i.key_pressed(Key::Num4) {
                    self.page = 0;
                    self.show_all_24_channels = !self.show_all_24_channels;
                    self.status_banner = if self.show_all_24_channels {
                        "MIXER VIEW: ALL 24 CHANNELS".to_string()
                    } else {
                        format!("MIXER VIEW: {}", self.active_bank.as_str())
                    };
                }
            }
        });
    }

    fn draw_top_rack_bar(&mut self, ui: &mut Ui, pal: &StudioPalette) {
        Frame::NONE
            .fill(pal.surface)
            .stroke(Stroke::new(1.0_f32, pal.border))
            .inner_margin(Margin::symmetric(8, 6))
            .corner_radius(CornerRadius::same(3))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // Brand & Device Identity Block
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("ARTURIA // AUDIOFUSE 8PRE CONTROL CENTER")
                                .font(FontId::proportional(12.0))
                                .strong()
                                .color(pal.text_primary),
                        );
                        let disc = self.controller.discovery_info();
                        let cfg = &self.controller.state().config;
                        ui.label(
                            RichText::new(format!(
                                "VID:PID {:04X}:{:04X} | SN {} | FW v{} | UNIT 0x0A",
                                disc.vendor_id,
                                disc.product_id,
                                cfg.serial_number,
                                cfg.firmware_version
                            ))
                            .font(FontId::monospace(10.0))
                            .color(pal.text_muted),
                        );
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Device name");
                        ui.add(egui::TextEdit::singleline(&mut self.device_name).char_limit(16));
                        if ui.button("Apply").clicked() {
                            if let Err(e) = self.controller.set_device_name(&self.device_name) {
                                self.status_banner = e.to_string();
                            }
                        }
                    });
                    // Device Mode (USB vs ADAT)
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("DEVICE MODE")
                                .font(FontId::proportional(10.0))
                                .color(pal.text_muted),
                        );
                        ui.horizontal(|ui| {
                            let cur_mode = self.controller.state().config.device_mode;
                            for mode in [DeviceMode::Usb, DeviceMode::Adat] {
                                let active = cur_mode == mode;
                                if studio_toggle_btn(
                                    ui,
                                    mode.as_str(),
                                    active,
                                    pal.accent_orange,
                                    pal,
                                )
                                .clicked()
                                {
                                    if let Err(error) = self.controller.set_device_mode(mode) {
                                        self.status_banner = error.to_string();
                                        return;
                                    }
                                    self.status_banner =
                                        format!("SET DEVICE MODE -> {}", mode.as_str());
                                }
                            }
                        });
                    });

                    ui.separator();

                    // Sample Rate (44.1 / 48 / 88.2 / 96 kHz)
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("SAMPLE RATE (ENTITY 41)")
                                .font(FontId::proportional(10.0))
                                .color(pal.text_muted),
                        );
                        ui.horizontal(|ui| {
                            let cur_sr = self.controller.state().config.sample_rate;
                            for sr in SampleRate::ALL {
                                let active = cur_sr == sr;
                                if studio_toggle_btn(
                                    ui,
                                    sr.khz_label(),
                                    active,
                                    pal.accent_orange,
                                    pal,
                                )
                                .clicked()
                                {
                                    if let Err(error) = self.controller.set_sample_rate(sr) {
                                        self.status_banner = error.to_string();
                                        return;
                                    }
                                    self.status_banner =
                                        format!("SET SAMPLE RATE -> {}", sr.khz_label());
                                }
                            }
                        });
                    });

                    ui.separator();

                    // Clock Source (Entity 40) & Lock Indicator
                    ui.vertical(|ui| {
                        let locked = self.controller.state().config.clock_locked;
                        let lock_txt = if locked { "SYNC LOCKED" } else { "UNLOCKED" };
                        let lock_col = if locked { pal.sync_teal } else { pal.alert_red };
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("CLOCK SOURCE (E40)")
                                    .font(FontId::proportional(10.0))
                                    .color(pal.text_muted),
                            );
                            ui.label(
                                RichText::new(lock_txt)
                                    .font(FontId::monospace(10.0))
                                    .strong()
                                    .color(lock_col),
                            );
                        });
                        ui.horizontal(|ui| {
                            let cur_clk = self.controller.state().config.clock_source;
                            for cs in [
                                ClockSource::Internal,
                                ClockSource::Adat,
                                ClockSource::WordClock,
                            ] {
                                let active = cur_clk == cs;
                                if studio_toggle_btn(
                                    ui,
                                    cs.short_label(),
                                    active,
                                    pal.sync_teal,
                                    pal,
                                )
                                .clicked()
                                {
                                    if let Err(error) = self.controller.set_clock_source(cs) {
                                        self.status_banner = error.to_string();
                                        return;
                                    }
                                    self.status_banner =
                                        format!("SET CLOCK SOURCE -> {}", cs.as_str());
                                }
                            }
                        });
                    });

                    ui.separator();

                    // Wordclock & ADAT / S-MUX Preferences
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("WORDCLOCK / ADAT PREFS")
                                .font(FontId::proportional(10.0))
                                .color(pal.text_muted),
                        );
                        ui.horizontal(|ui| {
                            let wc_75 = self.controller.state().config.wordclock_impedance
                                == WordclockImpedance::Ohms75;
                            if studio_toggle_btn(ui, "WC 75 OHM", wc_75, pal.accent_orange, pal)
                                .clicked()
                            {
                                let next = if wc_75 {
                                    WordclockImpedance::HiZ
                                } else {
                                    WordclockImpedance::Ohms75
                                };
                                if let Err(error) = self.controller.set_wordclock_impedance(next) {
                                    self.status_banner = error.to_string();
                                    return;
                                }
                                self.status_banner = format!("WORDCLOCK TERMINATION -> {}", next);
                            }

                            let wc_thru = self.controller.state().config.wordclock_thru;
                            if studio_toggle_btn(ui, "WC THRU", wc_thru, pal.accent_orange, pal)
                                .clicked()
                            {
                                if let Err(error) = self.controller.set_wordclock_thru(!wc_thru) {
                                    self.status_banner = error.to_string();
                                    return;
                                }
                                self.status_banner = format!("WORDCLOCK THRU -> {}", !wc_thru);
                            }

                            let adat_on = self.controller.state().config.adat_on;
                            if studio_toggle_btn(ui, "ADAT ON", adat_on, pal.sync_teal, pal)
                                .clicked()
                            {
                                if let Err(error) = self.controller.set_adat_on(!adat_on) {
                                    self.status_banner = error.to_string();
                                    return;
                                }
                                self.status_banner = format!("ADAT OPTICAL I/O -> {}", !adat_on);
                            }

                            let smux_dbl =
                                self.controller.state().config.smux_mode == SmuxMode::DoubleSpeed;
                            if studio_toggle_btn(ui, "S/MUX 2X", smux_dbl, pal.warn_amber, pal)
                                .clicked()
                            {
                                let next = if smux_dbl {
                                    SmuxMode::Normal
                                } else {
                                    SmuxMode::DoubleSpeed
                                };
                                if let Err(error) = self.controller.set_smux_mode(next) {
                                    self.status_banner = error.to_string();
                                    return;
                                }
                                self.status_banner = format!("ADAT S/MUX MODE -> {}", next);
                            }
                        });
                    });

                    ui.separator();

                    // Presets & Firmware / Theme
                    ui.vertical(|ui| {
                        let simulated = self.controller.is_simulated();
                        ui.label(
                            RichText::new(if simulated {
                                "PRESETS (SIMULATOR SLOTS & JSON) & TOOLS"
                            } else {
                                "PRESETS (JSON SESSION) & TOOLS"
                            })
                            .font(FontId::proportional(10.0))
                            .color(pal.text_muted),
                        );
                        ui.horizontal(|ui| {
                            ui.label("Preset file");
                            ui.text_edit_singleline(&mut self.preset_file_path);
                        });
                        ui.horizontal_wrapped(|ui| {
                            let active_slot = self.controller.state().active_preset_slot;
                            if simulated {
                                for slot in 0..8u8 {
                                    let label = format!("P{}", slot + 1);
                                    if studio_toggle_btn(
                                        ui,
                                        &label,
                                        active_slot == slot,
                                        pal.accent_orange,
                                        pal,
                                    )
                                    .clicked()
                                    {
                                        if let Err(error) = self.controller.load_hardware_preset(slot) {
                                            self.status_banner = error.to_string();
                                            return;
                                        }
                                        self.status_banner =
                                            format!("LOADED SIMULATOR PRESET SLOT {}", slot + 1);
                                    }
                                }
                                if studio_action_btn(ui, "SAVE SLOT", pal).clicked() {
                                    let slot = self.controller.state().active_preset_slot;
                                    if let Err(error) = self.controller.save_hardware_preset(slot) {
                                        self.status_banner = error.to_string();
                                        return;
                                    }
                                    self.status_banner =
                                        format!("SAVED SIMULATOR PRESET SLOT {}", slot + 1);
                                }
                            }
                            if studio_action_btn(ui, "EXPORT JSON", pal).clicked() {
                                match self.controller.export_preset_file(
                                    &self.preset_file_path,
                                    "Studio Session",
                                    "Exported from AudioFuse Linux Control Center",
                                ) {
                                    Ok(_) => {
                                        self.status_banner =
                                            format!("EXPORTED PRESET -> {}", self.preset_file_path);
                                    }
                                    Err(e) => {
                                        self.status_banner = format!("EXPORT ERROR: {}", e);
                                    }
                                }
                            }
                            if studio_action_btn(ui, "IMPORT JSON", pal).clicked() {
                                match self.controller.import_preset_file(&self.preset_file_path) {
                                    Ok(p) => {
                                        self.status_banner =
                                            format!("IMPORTED PRESET '{}'", p.preset_name);
                                    }
                                    Err(e) => {
                                        self.status_banner = format!("IMPORT ERROR: {}", e);
                                    }
                                }
                            }
                            if studio_toggle_btn(
                                ui,
                                "FW / DFU",
                                self.show_firmware_modal,
                                pal.sync_teal,
                                pal,
                            )
                            .clicked()
                            {
                                self.show_firmware_modal = !self.show_firmware_modal;
                            }
                            if studio_action_btn(ui, self.theme_mode.label(), pal).clicked() {
                                self.theme_mode.toggle();
                            }
                        });
                    });
                });
            });
    }

    fn draw_analog_inputs_rack(&mut self, ui: &mut Ui, pal: &StudioPalette) {
        Frame::NONE
            .fill(pal.surface)
            .stroke(Stroke::new(1.0_f32, pal.border))
            .inner_margin(Margin::symmetric(8, 6))
            .corner_radius(CornerRadius::same(3))
            .show(ui, |ui| {
                // Section Header Bar with Group +48V Phantom controls (1-4 & 5-8)
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("ANALOG INPUTS")
                            .font(FontId::proportional(11.0))
                            .strong()
                            .color(pal.text_primary),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let bank2_48v =
                            (4..8).all(|i| self.controller.state().inputs[i].phantom_48v);
                        if studio_toggle_btn(ui, "+48V BANK 5-8", bank2_48v, pal.accent_orange, pal)
                            .clicked()
                        {
                            if let Err(error) = self.controller.set_phantom_bank_4(1, !bank2_48v) {
                                self.status_banner = error.to_string();
                                return;
                            }
                            self.status_banner = format!("+48V PHANTOM BANK 5-8 -> {}", !bank2_48v);
                        }

                        let bank1_48v =
                            (0..4).all(|i| self.controller.state().inputs[i].phantom_48v);
                        if studio_toggle_btn(ui, "+48V BANK 1-4", bank1_48v, pal.accent_orange, pal)
                            .clicked()
                        {
                            if let Err(error) = self.controller.set_phantom_bank_4(0, !bank1_48v) {
                                self.status_banner = error.to_string();
                                return;
                            }
                            self.status_banner = format!("+48V PHANTOM BANK 1-4 -> {}", !bank1_48v);
                        }
                    });
                });

                ui.add_space(4.0);

                let avail_w = ui.available_width();
                let strip_w = ((avail_w - 7.0 * 4.0) / 8.0).max(110.0);

                ui.horizontal(|ui| {
                    for ch_idx in 0..8 {
                        let inp = self.controller.state().inputs[ch_idx].clone();
                        Frame::NONE
                            .fill(pal.surface_alt)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::symmetric(6, 5))
                            .corner_radius(CornerRadius::same(3))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(strip_w - 16.0);
                                    // Strip Header + Live dBFS Readout
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!("IN {:02}", ch_idx + 1))
                                                .font(FontId::monospace(11.0))
                                                .strong()
                                                .color(pal.text_primary),
                                        );
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                let dbfs = vu_to_dbfs(inp.vu);
                                                ui.label(
                                                    RichText::new(format!("{:5.1} dBFS", dbfs))
                                                        .font(FontId::monospace(10.0))
                                                        .color(pal.text_secondary),
                                                );
                                            },
                                        );
                                    });

                                    // Horizontal Segmented VU Bar
                                    draw_horizontal_vu_bar(ui, inp.vu, pal);

                                    // Source Selector Row: INPUT / MIC / LINE / INST (Ch 1-2 only)
                                    ui.horizontal_wrapped(|ui| {
                                        for mode in
                                            [InputType::ComboAuto, InputType::Mic, InputType::Line]
                                        {
                                            let active = inp.input_type == mode;
                                            let lbl = match mode {
                                                InputType::ComboAuto => "AUTO",
                                                InputType::Mic => "MIC",
                                                InputType::Line => "LINE",
                                                InputType::Inst => "INST",
                                            };
                                            if studio_toggle_btn(
                                                ui,
                                                lbl,
                                                active,
                                                pal.sync_teal,
                                                pal,
                                            )
                                            .clicked()
                                            {
                                                if let Err(error) =
                                                    self.controller.set_input_type(ch_idx, mode)
                                                {
                                                    self.status_banner = error.to_string();
                                                    return;
                                                }
                                            }
                                        }
                                        if inp.supports_inst() {
                                            let active =
                                                inp.inst_hi_z || inp.input_type == InputType::Inst;
                                            if studio_toggle_btn(
                                                ui,
                                                "INST",
                                                active,
                                                pal.accent_orange,
                                                pal,
                                            )
                                            .clicked()
                                            {
                                                if let Err(error) =
                                                    self.controller.set_input_inst(ch_idx, !active)
                                                {
                                                    self.status_banner = error.to_string();
                                                    return;
                                                }
                                            }
                                        } else {
                                            ui.label(
                                                RichText::new("TRS")
                                                    .font(FontId::monospace(11.0))
                                                    .color(pal.text_muted),
                                            );
                                        }
                                    });

                                    // Hardware Preamp Controls: +48V, Phase Invert (Ø), Line -20dB
                                    ui.horizontal_wrapped(|ui| {
                                        if studio_toggle_btn(
                                            ui,
                                            "+48V",
                                            inp.phantom_48v,
                                            pal.accent_orange,
                                            pal,
                                        )
                                        .clicked()
                                        {
                                            if let Err(error) = self
                                                .controller
                                                .set_input_phantom(ch_idx, !inp.phantom_48v)
                                            {
                                                self.status_banner = error.to_string();
                                                return;
                                            }
                                        }
                                        if studio_toggle_btn(
                                            ui,
                                            "Ø INV",
                                            inp.phase_invert,
                                            pal.warn_amber,
                                            pal,
                                        )
                                        .clicked()
                                        {
                                            if let Err(error) = self
                                                .controller
                                                .set_input_phase_invert(ch_idx, !inp.phase_invert)
                                            {
                                                self.status_banner = error.to_string();
                                                return;
                                            }
                                        }
                                        if !matches!(
                                            inp.input_type,
                                            InputType::Line | InputType::ComboAuto
                                        ) {
                                            ui.disable();
                                        }
                                        let line_pad_on = inp.pad_line == LinePad::Pad20dB;
                                        if studio_toggle_btn(
                                            ui,
                                            "LINE -20dB",
                                            line_pad_on,
                                            pal.warn_amber,
                                            pal,
                                        )
                                        .clicked()
                                        {
                                            let next = if line_pad_on {
                                                LinePad::Off
                                            } else {
                                                LinePad::Pad20dB
                                            };
                                            if let Err(error) =
                                                self.controller.set_input_line_pad(ch_idx, next)
                                            {
                                                self.status_banner = error.to_string();
                                                return;
                                            }
                                        }
                                    });

                                    // Mic Pad / Boost Selector: OFF / -20dB PAD / BOOST
                                    ui.horizontal_wrapped(|ui| {
                                        if !inp.mic_active {
                                            ui.disable();
                                        }
                                        ui.label(
                                            RichText::new("MIC:")
                                                .font(FontId::monospace(11.0))
                                                .color(pal.text_muted),
                                        );
                                        for mpad in [
                                            MicPadBoost::Off,
                                            MicPadBoost::Pad20dB,
                                            MicPadBoost::Boost,
                                        ] {
                                            let active = inp.pad_mic == mpad;
                                            let col = match mpad {
                                                MicPadBoost::Off => pal.sync_teal,
                                                MicPadBoost::Pad20dB => pal.warn_amber,
                                                MicPadBoost::Boost => pal.accent_orange,
                                            };
                                            if studio_toggle_btn(
                                                ui,
                                                mpad.short_label(),
                                                active,
                                                col,
                                                pal,
                                            )
                                            .clicked()
                                            {
                                                if let Err(error) = self
                                                    .controller
                                                    .set_input_mic_pad_boost(ch_idx, mpad)
                                                {
                                                    self.status_banner = error.to_string();
                                                    return;
                                                }
                                            }
                                        }
                                    });
                                });
                            });
                    }
                });
            });
    }

    fn draw_cue_mixer_section(&mut self, ui: &mut Ui, pal: &StudioPalette) {
        Frame::NONE
            .fill(pal.surface)
            .stroke(Stroke::new(1.0_f32, pal.border))
            .inner_margin(Margin::symmetric(8, 6))
            .corner_radius(CornerRadius::same(3))
            .show(ui, |ui| {
                // Mixer Header Bar: Bank Selectors (Ctrl+1..3), Stereo Link Pair Toggles, Clear Solos
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("CUE MIXER")
                            .font(FontId::proportional(11.0))
                            .strong()
                            .color(pal.text_primary),
                    );
                    ui.separator();

                    for (idx, bank) in MixerBank::ALL.iter().enumerate() {
                        let label = format!("{} (Ctrl+{})", bank.as_str(), idx + 1);
                        let active = !self.show_all_24_channels && self.active_bank == *bank;
                        if studio_toggle_btn(ui, &label, active, pal.accent_orange, pal).clicked() {
                            self.active_bank = *bank;
                            self.show_all_24_channels = false;
                        }
                    }

                    if studio_toggle_btn(
                        ui,
                        "ALL 24 CH (Ctrl+4)",
                        self.show_all_24_channels,
                        pal.sync_teal,
                        pal,
                    )
                    .clicked()
                    {
                        self.show_all_24_channels = !self.show_all_24_channels;
                    }

                    ui.separator();

                    // Stereo Link Toggles for active bank
                    let bank_idx = self.active_bank as usize;
                    let links = self.controller.state().stereo_links[bank_idx];
                    ui.label(
                        RichText::new("STEREO LINK:")
                            .font(FontId::monospace(10.0))
                            .color(pal.text_muted),
                    );
                    for (pair_idx, pair_lbl) in ["1-2", "3-4", "5-6", "7-8"].iter().enumerate() {
                        let linked = links.get_pair(pair_idx);
                        if studio_toggle_btn(ui, pair_lbl, linked, pal.sync_teal, pal).clicked() {
                            if let Err(error) =
                                self.controller
                                    .set_stereo_link(self.active_bank, pair_idx, !linked)
                            {
                                self.status_banner = error.to_string();
                                return;
                            }
                        }
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let any_solo = self
                            .controller
                            .state()
                            .mixer_channels
                            .iter()
                            .any(|c| c.solo)
                            || self.controller.state().monitor.mixer_output_solo;
                        if studio_toggle_btn(ui, "CLEAR SOLOS", any_solo, pal.warn_amber, pal)
                            .clicked()
                        {
                            if let Err(error) = self.controller.clear_all_solos() {
                                self.status_banner = error.to_string();
                                return;
                            }
                        }
                    });
                });

                ui.add_space(4.0);

                if self.show_all_24_channels {
                    self.draw_all_24_mixer_grid(ui, pal);
                } else {
                    self.draw_bank_8_mixer_strips(ui, pal);
                }
            });
    }

    fn draw_bank_8_mixer_strips(&mut self, ui: &mut Ui, pal: &StudioPalette) {
        let base_idx = self.active_bank.base_index();
        let avail_w = ui.available_width();
        let strip_w = ((avail_w - 7.0 * 4.0) / 8.0).max(92.0);
        let avail_h = 320.0;

        ui.horizontal(|ui| {
            for offset in 0..8 {
                let mixer_idx = base_idx + offset;
                let ch = self.controller.state().mixer_channels[mixer_idx].clone();
                let is_linked = self.controller.stereo_partner(mixer_idx).is_some();

                Frame::NONE
                    .fill(pal.surface_alt)
                    .stroke(Stroke::new(
                        1.0_f32,
                        if is_linked { pal.sync_teal } else { pal.border },
                    ))
                    .inner_margin(Margin::symmetric(5, 5))
                    .corner_radius(CornerRadius::same(3))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.set_width(strip_w - 14.0);
                            ui.set_min_height(avail_h - 14.0);
                            // Channel Index & Link/Peak Badge
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "CH {:02} ({})",
                                        mixer_idx + 1,
                                        if is_linked { "ST" } else { "MO" }
                                    ))
                                    .font(FontId::monospace(10.5))
                                    .strong()
                                    .color(pal.text_primary),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let peak_col =
                                        if ch.peak { pal.alert_red } else { pal.recessed };
                                    Frame::NONE
                                        .fill(peak_col)
                                        .stroke(Stroke::new(1.0_f32, pal.border))
                                        .inner_margin(Margin::symmetric(3, 1))
                                        .corner_radius(CornerRadius::same(2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new("PK")
                                                    .font(FontId::monospace(9.0))
                                                    .color(if ch.peak {
                                                        Color32::WHITE
                                                    } else {
                                                        pal.text_muted
                                                    }),
                                            );
                                        });
                                });
                            });

                            // Persistent 16-char Channel Name Editor
                            let mut name_buf = ch.name.clone();
                            let name_resp = ui.add(
                                egui::TextEdit::singleline(&mut name_buf)
                                    .font(FontId::monospace(10.0))
                                    .char_limit(16)
                                    .desired_width(f32::INFINITY),
                            );
                            if name_resp.changed() {
                                if let Err(error) =
                                    self.controller.set_mixer_channel_name(mixer_idx, &name_buf)
                                {
                                    self.status_banner = error.to_string();
                                    return;
                                }
                            }

                            ui.checkbox(
                                &mut self.controller.state_mut().mixer_channels[mixer_idx].show,
                                "Show in overview",
                            );
                            // Pan Readout & Slider (-64..+64)
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("PAN")
                                        .font(FontId::monospace(11.0))
                                        .color(pal.text_muted),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format_pan(ch.pan))
                                            .font(FontId::monospace(10.0))
                                            .strong()
                                            .color(pal.text_secondary),
                                    );
                                });
                            });
                            let mut pan_f32 = ch.pan as f32;
                            let pan_slider = ui.add(
                                egui::Slider::new(&mut pan_f32, -64.0..=64.0)
                                    .show_value(false)
                                    .trailing_fill(false),
                            );
                            if pan_slider.changed() {
                                if let Err(error) = self
                                    .controller
                                    .set_mixer_channel_pan(mixer_idx, pan_f32.round() as i8)
                                {
                                    self.status_banner = error.to_string();
                                    return;
                                }
                            }
                            if pan_slider.double_clicked() {
                                if let Err(error) =
                                    self.controller.set_mixer_channel_pan(mixer_idx, 0)
                                {
                                    self.status_banner = error.to_string();
                                    return;
                                }
                            }

                            // Mute / Solo / Group Row
                            ui.horizontal(|ui| {
                                if studio_toggle_btn(ui, "MUTE", ch.mute, pal.alert_red, pal)
                                    .clicked()
                                {
                                    if let Err(error) =
                                        self.controller.set_mixer_channel_mute(mixer_idx, !ch.mute)
                                    {
                                        self.status_banner = error.to_string();
                                        return;
                                    }
                                }
                                if studio_toggle_btn(ui, "SOLO", ch.solo, pal.warn_amber, pal)
                                    .clicked()
                                {
                                    if let Err(error) =
                                        self.controller.set_mixer_channel_solo(mixer_idx, !ch.solo)
                                    {
                                        self.status_banner = error.to_string();
                                        return;
                                    }
                                }
                                let grp_lbl = if ch.group == 0 {
                                    "GRP -".to_string()
                                } else {
                                    format!("GRP {}", ch.group)
                                };
                                if studio_toggle_btn(ui, &grp_lbl, ch.group > 0, pal.sync_teal, pal)
                                    .clicked()
                                {
                                    let next_grp = (ch.group + 1) % 5;
                                    self.controller.set_mixer_channel_group(mixer_idx, next_grp);
                                }
                            });

                            ui.add_space(2.0);

                            // Fader + Vertical VU Meter Area (`PiecewiseLinear(0.842105263)`)
                            let fader_h = 142.0;
                            ui.horizontal(|ui| {
                                let mut norm = fader_db_to_normalized(ch.level_db);
                                if draw_vertical_fader(ui, &mut norm, fader_h, pal) {
                                    let new_db = fader_normalized_to_db(norm);
                                    if let Err(error) = self
                                        .controller
                                        .set_mixer_channel_level_db(mixer_idx, new_db)
                                    {
                                        self.status_banner = error.to_string();
                                        return;
                                    }
                                }
                                draw_vertical_vu_bar(ui, ch.vu, fader_h, pal);
                            });

                            // Tabular Monospace dB Readout + 0.0 dB Unity Reset
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format_db(ch.level_db))
                                        .font(FontId::monospace(10.5))
                                        .strong()
                                        .color(pal.text_primary),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if studio_action_btn(ui, "0dB", pal).clicked() {
                                        if let Err(error) = self
                                            .controller
                                            .set_mixer_channel_level_db(mixer_idx, 0.0)
                                        {
                                            self.status_banner = error.to_string();
                                            return;
                                        }
                                    }
                                });
                            });
                        });
                    });
            }
        });
    }

    fn draw_all_24_mixer_grid(&mut self, ui: &mut Ui, pal: &StudioPalette) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for bank in MixerBank::ALL {
                ui.label(
                    RichText::new(format!("BANK // {}", bank.as_str()))
                        .font(FontId::monospace(10.5))
                        .strong()
                        .color(pal.accent_orange),
                );
                let base = bank.base_index();
                ui.horizontal_wrapped(|ui| {
                    for offset in 0..8 {
                        let idx = base + offset;
                        let ch = self.controller.state().mixer_channels[idx].clone();
                        if !ch.show {
                            continue;
                        }
                        Frame::NONE
                            .fill(pal.surface_alt)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::symmetric(5, 4))
                            .corner_radius(CornerRadius::same(3))
                            .show(ui, |ui| {
                                ui.set_width(112.0);
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(format!("{:02}: {}", idx + 1, ch.name))
                                            .font(FontId::monospace(10.0))
                                            .strong()
                                            .color(pal.text_primary),
                                    );
                                    draw_horizontal_vu_bar(ui, ch.vu, pal);
                                    ui.horizontal_wrapped(|ui| {
                                        if studio_toggle_btn(ui, "M", ch.mute, pal.alert_red, pal)
                                            .clicked()
                                        {
                                            if let Err(error) = self
                                                .controller
                                                .set_mixer_channel_mute(idx, !ch.mute)
                                            {
                                                self.status_banner = error.to_string();
                                                return;
                                            }
                                        }
                                        if studio_toggle_btn(ui, "S", ch.solo, pal.warn_amber, pal)
                                            .clicked()
                                        {
                                            if let Err(error) = self
                                                .controller
                                                .set_mixer_channel_solo(idx, !ch.solo)
                                            {
                                                self.status_banner = error.to_string();
                                                return;
                                            }
                                        }
                                        ui.label(
                                            RichText::new(format_db(ch.level_db))
                                                .font(FontId::monospace(11.0))
                                                .color(pal.text_secondary),
                                        );
                                    });
                                });
                            });
                    }
                });
                ui.add_space(4.0);
            }
        });
    }

    fn draw_master_and_routing_column(&mut self, ui: &mut Ui, pal: &StudioPalette) {
        ui.vertical(|ui| {
            // Section 03: Master Cue & Monitor Section
            Frame::NONE
                .fill(pal.surface)
                .stroke(Stroke::new(1.0_f32, pal.border))
                .inner_margin(Margin::symmetric(8, 6))
                .corner_radius(CornerRadius::same(3))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("SECTION 03 // MASTER CUE & MONITOR SECTION")
                            .font(FontId::proportional(11.0))
                            .strong()
                            .color(pal.text_primary),
                    );
                    ui.add_space(3.0);

                    let mon = self.controller.state().monitor.clone();
                    let simulated = self.controller.is_simulated();

                    // Direct / Computer USB Balance (simulator-only; not a hardware register on 8Pre)
                    if simulated {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("DIRECT / USB MIX")
                                    .font(FontId::monospace(10.0))
                                    .color(pal.text_muted),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{:.0}% CUE / {:.0}% USB",
                                        (1.0 - mon.direct_computer_mix) * 100.0,
                                        mon.direct_computer_mix * 100.0
                                    ))
                                    .font(FontId::monospace(10.0))
                                    .color(pal.text_secondary),
                                );
                            });
                        });
                        let mut mix_val = mon.direct_computer_mix;
                        if ui
                            .add(egui::Slider::new(&mut mix_val, 0.0..=1.0).show_value(false))
                            .changed()
                        {
                            if let Err(error) = self.controller.set_direct_computer_mix(mix_val) {
                                self.status_banner = error.to_string();
                                return;
                            }
                        }
                    }

                    // Master Cue Level (-61.0 .. 0.0 dB, param_id 0x55) + simulator-only Mute / Solo
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("CUE MASTER LEVEL (0x55)")
                                .font(FontId::monospace(10.0))
                                .color(pal.text_muted),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if simulated {
                                if studio_toggle_btn(
                                    ui,
                                    "SOLO",
                                    mon.mixer_output_solo,
                                    pal.warn_amber,
                                    pal,
                                )
                                .clicked()
                                {
                                    if let Err(error) = self
                                        .controller
                                        .set_mixer_output_solo(!mon.mixer_output_solo)
                                    {
                                        self.status_banner = error.to_string();
                                        return;
                                    }
                                }
                                if studio_toggle_btn(
                                    ui,
                                    "MUTE",
                                    mon.mixer_output_mute,
                                    pal.alert_red,
                                    pal,
                                )
                                .clicked()
                                {
                                    if let Err(error) = self
                                        .controller
                                        .set_mixer_output_mute(!mon.mixer_output_mute)
                                    {
                                        self.status_banner = error.to_string();
                                        return;
                                    }
                                }
                            }
                            ui.label(
                                RichText::new(format_db(mon.mixer_output_level_db))
                                    .font(FontId::monospace(10.5))
                                    .strong()
                                    .color(pal.text_primary),
                            );
                        });
                    });
                    let mut master_lvl = mon.mixer_output_level_db;
                    if ui
                        .add(egui::Slider::new(&mut master_lvl, -61.0..=0.0).show_value(false))
                        .changed()
                    {
                        if let Err(error) = self.controller.set_mixer_output_level_db(master_lvl) {
                            self.status_banner = error.to_string();
                            return;
                        }
                    }
                    draw_stereo_vu_pair(
                        ui,
                        "CUE BUS L/R",
                        mon.mixer_main_vu_l,
                        mon.mixer_main_vu_r,
                        pal,
                    );

                    ui.separator();

                    // Speakers Source (Main 1-2 / Cue Mix) + Mono
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("SPEAKERS OUT:")
                                .font(FontId::monospace(10.0))
                                .color(pal.text_muted),
                        );
                        for src in [SpeakersSource::Main1_2, SpeakersSource::CueMix] {
                            if studio_toggle_btn(
                                ui,
                                src.as_str(),
                                mon.speakers_source == src,
                                pal.accent_orange,
                                pal,
                            )
                            .clicked()
                            {
                                if let Err(error) = self.controller.set_speakers_source(src) {
                                    self.status_banner = error.to_string();
                                    return;
                                }
                            }
                        }
                        if studio_toggle_btn(ui, "MONO", mon.speakers_mono, pal.warn_amber, pal)
                            .clicked()
                        {
                            if let Err(error) =
                                self.controller.set_speakers_mono(!mon.speakers_mono)
                            {
                                self.status_banner = error.to_string();
                                return;
                            }
                        }
                    });
                    draw_stereo_vu_pair(
                        ui,
                        "SPKRS L/R  ",
                        mon.speakers_vu_l,
                        mon.speakers_vu_r,
                        pal,
                    );

                    // Phones Source (Main 1-2 / Cue Mix / Follow Spkrs) + Mono
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("PHONES OUT  :")
                                .font(FontId::monospace(10.0))
                                .color(pal.text_muted),
                        );
                        for src in [
                            PhonesSource::Main1_2,
                            PhonesSource::CueMix,
                            PhonesSource::FollowSpeakers,
                        ] {
                            if studio_toggle_btn(
                                ui,
                                src.as_str(),
                                mon.phones_source == src,
                                pal.accent_orange,
                                pal,
                            )
                            .clicked()
                            {
                                if let Err(error) = self.controller.set_phones_source(src) {
                                    self.status_banner = error.to_string();
                                    return;
                                }
                            }
                        }
                        if studio_toggle_btn(ui, "MONO", mon.phones_mono, pal.warn_amber, pal)
                            .clicked()
                        {
                            if let Err(error) = self.controller.set_phones_mono(!mon.phones_mono) {
                                self.status_banner = error.to_string();
                                return;
                            }
                        }
                    });
                    draw_stereo_vu_pair(ui, "PHONES L/R ", mon.phones_vu_l, mon.phones_vu_r, pal);
                });

            ui.add_space(4.0);

            // Section 04: Output & Loopback Routing Matrix
            Frame::NONE
                .fill(pal.surface)
                .stroke(Stroke::new(1.0_f32, pal.border))
                .inner_margin(Margin::symmetric(8, 6))
                .corner_radius(CornerRadius::same(3))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("SECTION 04 // OUTPUT & LOOPBACK ROUTING MATRIX")
                            .font(FontId::proportional(11.0))
                            .strong()
                            .color(pal.text_primary),
                    );
                    ui.add_space(3.0);

                    let rt = self.controller.state().routing.clone();

                    // Analog Out 1-2 .. 7-8
                    ui.label(
                        RichText::new("ANALOG LINE OUTPUTS 1-8 (0x7a)")
                            .font(FontId::monospace(10.0))
                            .color(pal.text_muted),
                    );
                    for pair in 0..4 {
                        let ch_a = pair * 2 + 1;
                        let ch_b = ch_a + 1;
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("OUT {}-{}:", ch_a, ch_b))
                                    .font(FontId::monospace(10.0))
                                    .color(pal.text_secondary),
                            );
                            let cur = rt.analog_out_sources[pair];
                            for src in AnalogOutSource::ALL {
                                let short = match src {
                                    AnalogOutSource::CueMix => "CUE".to_string(),
                                    AnalogOutSource::UsbPair => format!("USB {}-{}", ch_a, ch_b),
                                    AnalogOutSource::AnalogInPair => {
                                        format!("ANLG {}-{}", ch_a, ch_b)
                                    }
                                    AnalogOutSource::AdatInPair => {
                                        format!("ADAT {}-{}", ch_a, ch_b)
                                    }
                                };
                                if studio_toggle_btn(ui, &short, cur == src, pal.accent_orange, pal)
                                    .clicked()
                                {
                                    if let Err(error) =
                                        self.controller.set_analog_out_source(pair, src)
                                    {
                                        self.status_banner = error.to_string();
                                        return;
                                    }
                                }
                            }
                        });
                    }

                    ui.separator();

                    // ADAT Out 1-2 .. 7-8
                    ui.label(
                        RichText::new("ADAT OPTICAL OUTPUTS 1-8 (0x7e)")
                            .font(FontId::monospace(10.0))
                            .color(pal.text_muted),
                    );
                    for pair in 0..4 {
                        let ch_a = pair * 2 + 1;
                        let ch_b = ch_a + 1;
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("ADAT {}-{}:", ch_a, ch_b))
                                    .font(FontId::monospace(10.0))
                                    .color(pal.text_secondary),
                            );
                            let cur = rt.adat_out_sources[pair];
                            for src in AdatOutSource::ALL {
                                let short = match src {
                                    AdatOutSource::CueMix => "CUE".to_string(),
                                    AdatOutSource::UsbPair => {
                                        format!("USB {}-{}", 8 + ch_a, 8 + ch_b)
                                    }
                                    AdatOutSource::AnalogInPair => {
                                        format!("ANLG {}-{}", ch_a, ch_b)
                                    }
                                };
                                if studio_toggle_btn(ui, &short, cur == src, pal.sync_teal, pal)
                                    .clicked()
                                {
                                    if let Err(error) =
                                        self.controller.set_adat_out_source(pair, src)
                                    {
                                        self.status_banner = error.to_string();
                                        return;
                                    }
                                }
                            }
                        });
                    }

                    ui.separator();

                    // Loopback Source (Disabled / Cue Mix / USB 17-18)
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("LOOPBACK REC (0x4A):")
                                .font(FontId::monospace(10.0))
                                .color(pal.text_muted),
                        );
                        for src in LoopbackSource::ALL {
                            if studio_toggle_btn(
                                ui,
                                src.as_str(),
                                rt.loopback_source == src,
                                pal.accent_orange,
                                pal,
                            )
                            .clicked()
                            {
                                if let Err(error) = self.controller.set_loopback_source(src) {
                                    self.status_banner = error.to_string();
                                    return;
                                }
                            }
                        }
                    });
                    draw_stereo_vu_pair(ui, "LOOPBK L/R ", rt.loopback_vu_l, rt.loopback_vu_r, pal);
                });
        });
    }

    fn draw_firmware_dfu_modal(&mut self, ctx: &egui::Context, pal: &StudioPalette) {
        if !self.show_firmware_modal {
            return;
        }
        let mut open = self.show_firmware_modal;
        egui::Window::new("FIRMWARE ARCHIVE INSPECTOR & USB DFU (INTERFACE 3)")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(&self.dfu);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(".af8pre Path:").font(FontId::monospace(10.5)));
                    ui.text_edit_singleline(&mut self.firmware_archive_path);
                    if studio_action_btn(ui, "INSPECT .AF8PRE", pal).clicked() {
                        match inspect_af8pre_file(&self.firmware_archive_path) {
                            Ok(info) => {
                                self.status_banner = format!(
                                    "INSPECTED .AF8PRE v{} ({} blocks)",
                                    info.manifest.version_number,
                                    info.images
                                        .first()
                                        .map(|i| i.dfu_block_count_64b)
                                        .unwrap_or(0)
                                );
                                self.inspected_firmware = Some(info);
                            }
                            Err(e) => {
                                self.status_banner = format!("FIRMWARE INSPECT ERROR: {}", e);
                            }
                        }
                    }
                });
                if let Some(info) = &self.inspected_firmware {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!(
                            "Archive Version: v{} ({}) | VID:PID {}:{} | Archive targets 8Pre: {}",
                            info.manifest.version_number,
                            info.manifest.date,
                            info.manifest.vendorid,
                            info.manifest.productid,
                            if info.targets_audiofuse_8pre {
                                "YES"
                            } else {
                                "NO"
                            }
                        ))
                        .font(FontId::monospace(10.5))
                        .color(pal.text_secondary),
                    );
                    for img in &info.images {
                        ui.label(
                            RichText::new(format!(
                                "Image #{}: {} | {} bytes | {} x 64B DFU blocks | FNV1a {}",
                                img.image_num,
                                img.file_name,
                                img.size_bytes,
                                img.dfu_block_count_64b,
                                img.fnv1a_64_hex
                            ))
                            .font(FontId::monospace(10.0))
                            .color(pal.text_muted),
                        );
                    }
                }
            });
        self.show_firmware_modal = open;
    }
}

impl eframe::App for ControlCenterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let pal = StudioPalette::for_mode(self.theme_mode);
        pal.apply_to_ctx(ctx, self.theme_mode);

        self.handle_keyboard_shortcuts(ctx);

        while let Ok(update) = self.meters.try_recv() {
            match update {
                Ok(frame) => {
                    if let Some(state_266) = frame.state_266 {
                        if let Err(e) = self.controller.state_mut().apply_afex_state_266(&state_266)
                        {
                            self.status_banner = format!("State decode error: {e}");
                        }
                    }
                    self.controller
                        .state_mut()
                        .apply_afex_vu_152(&frame.vu_152);
                }
                Err(e) => self.status_banner = format!("Meters stale: {e}"),
            }
        }
        ctx.request_repaint_after(Duration::from_millis(30));
        // Bottom Studio Telemetry Status Bar
        egui::TopBottomPanel::bottom("studio_status_bar")
            .frame(
                Frame::NONE
                    .fill(pal.surface)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .inner_margin(Margin::symmetric(8, 4)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&self.status_banner)
                            .font(FontId::monospace(10.5))
                            .color(pal.text_secondary),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(
                                "SHORTCUTS: Ctrl+1 ANALOG | Ctrl+2 ADAT | Ctrl+3 USB | Ctrl+4 ALL-24",
                            )
                            .font(FontId::monospace(10.0))
                            .color(pal.text_muted),
                        );
                    });
                });
            });

        // Main Studio Console Viewport
        egui::CentralPanel::default()
            .frame(
                Frame::NONE
                    .fill(pal.bg)
                    .inner_margin(Margin::symmetric(8, 6)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("AudioFuse 8Pre");
                    for (i, label) in ["Console", "Monitor & routing", "Device & presets"]
                        .into_iter()
                        .enumerate()
                    {
                        if studio_toggle_btn(ui, label, self.page == i, pal.sync_teal, &pal)
                            .clicked()
                        {
                            self.page = i;
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if studio_action_btn(ui, self.theme_mode.label(), &pal).clicked() {
                            self.theme_mode.toggle();
                        }
                        ui.label(self.controller.backend_name());
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt(self.page)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        match self.page {
                            0 => {
                                self.draw_analog_inputs_rack(ui, &pal);
                                ui.add_space(6.0);
                                self.draw_cue_mixer_section(ui, &pal);
                            }
                            1 => {
                                ui.set_max_width(900.0);
                                self.draw_master_and_routing_column(ui, &pal);
                            }
                            _ => self.draw_top_rack_bar(ui, &pal),
                        }
                    });
            });

        self.frames = self.frames.saturating_add(1);
        if let Some(path) = &self.screenshot {
            if self.frames == 4 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            let events = ctx.input(|i| i.events.clone());
            for event in &events {
                if let egui::Event::Screenshot { image, .. } = event {
                    let pixels: Vec<_> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                    if let Err(e) = image::save_buffer(
                        path,
                        &pixels,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    ) {
                        eprintln!("Screenshot failed: {e}");
                        std::process::exit(1);
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        self.draw_firmware_dfu_modal(ctx, &pal);
    }
}

fn studio_toggle_btn(
    ui: &mut Ui,
    label: &str,
    active: bool,
    active_col: Color32,
    pal: &StudioPalette,
) -> egui::Response {
    let fill = if active { active_col } else { pal.recessed };
    let text_col = if active {
        if active_col.to_normalized_gamma_f32()[1] < 0.49
            && pal.bg == Color32::from_rgb(246, 245, 242)
        {
            Color32::WHITE
        } else {
            Color32::BLACK
        }
    } else {
        pal.text_secondary
    };
    let stroke_col = if active { active_col } else { pal.border };
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .font(FontId::monospace(11.0))
                .strong()
                .color(text_col),
        )
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, stroke_col))
        .corner_radius(CornerRadius::same(2)),
    )
}

fn studio_action_btn(ui: &mut Ui, label: &str, pal: &StudioPalette) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .font(FontId::monospace(11.0))
                .color(pal.text_primary),
        )
        .fill(pal.recessed)
        .stroke(Stroke::new(1.0_f32, pal.border_strong))
        .corner_radius(CornerRadius::same(2)),
    )
}

fn draw_horizontal_vu_bar(ui: &mut Ui, vu: u8, pal: &StudioPalette) {
    let desired = Vec2::new(ui.available_width().max(60.0), 8.0);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(1), pal.recessed);
    painter.rect_stroke(
        rect,
        CornerRadius::same(1),
        Stroke::new(1.0_f32, pal.border),
        egui::StrokeKind::Inside,
    );

    let frac = (vu.min(127) as f32) / 127.0;
    if frac > 0.0 {
        let fill_w = (rect.width() - 2.0) * frac;
        let fill_rect =
            egui::Rect::from_min_size(rect.min + Vec2::new(1.0, 1.0), Vec2::new(fill_w, 6.0));
        let col = if vu > 115 {
            pal.meter_red
        } else if vu > 92 {
            pal.meter_amber
        } else {
            pal.meter_green
        };
        painter.rect_filled(fill_rect, CornerRadius::ZERO, col);
    }
}

fn draw_stereo_vu_pair(ui: &mut Ui, label: &str, vu_l: u8, vu_r: u8, pal: &StudioPalette) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .font(FontId::monospace(11.0))
                .color(pal.text_muted),
        );
        ui.vertical(|ui| {
            draw_horizontal_vu_bar(ui, vu_l, pal);
            draw_horizontal_vu_bar(ui, vu_r, pal);
        });
    });
}

fn draw_vertical_fader(ui: &mut Ui, norm_val: &mut f32, height: f32, pal: &StudioPalette) -> bool {
    let desired = Vec2::new(34.0, height);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    let mut changed = false;

    if resp.dragged() || resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let t = 1.0 - ((pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
            *norm_val = t;
            changed = true;
        }
    }
    if resp.double_clicked() {
        *norm_val = audiofuse_core::MIXER_FADER_KNEE;
        changed = true;
    }

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(2), pal.recessed);
    painter.rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0_f32, pal.border),
        egui::StrokeKind::Inside,
    );

    // 0 dB Unity Knee Line at 0.842105263
    let knee_y = rect.bottom() - rect.height() * audiofuse_core::MIXER_FADER_KNEE;
    painter.line_segment(
        [
            egui::pos2(rect.left() + 2.0, knee_y),
            egui::pos2(rect.right() - 2.0, knee_y),
        ],
        Stroke::new(1.0_f32, pal.accent_orange),
    );

    // Center Fader Track
    let center_x = rect.center().x;
    painter.line_segment(
        [
            egui::pos2(center_x, rect.top() + 6.0),
            egui::pos2(center_x, rect.bottom() - 6.0),
        ],
        Stroke::new(2.0_f32, pal.border_strong),
    );

    // Fader Cap
    let cap_y = rect.bottom() - rect.height() * norm_val.clamp(0.0, 1.0);
    let cap_rect = egui::Rect::from_center_size(
        egui::pos2(center_x, cap_y.clamp(rect.top() + 7.0, rect.bottom() - 7.0)),
        Vec2::new(rect.width() - 6.0, 12.0),
    );
    painter.rect_filled(cap_rect, CornerRadius::same(2), pal.surface);
    painter.rect_stroke(
        cap_rect,
        CornerRadius::same(2),
        Stroke::new(1.0_f32, pal.text_primary),
        egui::StrokeKind::Inside,
    );
    painter.line_segment(
        [
            egui::pos2(cap_rect.left() + 3.0, cap_rect.center().y),
            egui::pos2(cap_rect.right() - 3.0, cap_rect.center().y),
        ],
        Stroke::new(1.5_f32, pal.accent_orange),
    );

    changed
}

fn draw_vertical_vu_bar(ui: &mut Ui, vu: u8, height: f32, pal: &StudioPalette) {
    let desired = Vec2::new(12.0, height);
    let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(1), pal.recessed);
    painter.rect_stroke(
        rect,
        CornerRadius::same(1),
        Stroke::new(1.0_f32, pal.border),
        egui::StrokeKind::Inside,
    );

    let frac = (vu.min(127) as f32) / 127.0;
    if frac > 0.0 {
        let fill_h = (rect.height() - 2.0) * frac;
        let fill_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 1.0, rect.bottom() - 1.0 - fill_h),
            egui::pos2(rect.right() - 1.0, rect.bottom() - 1.0),
        );
        let col = if vu > 115 {
            pal.meter_red
        } else if vu > 92 {
            pal.meter_amber
        } else {
            pal.meter_green
        };
        painter.rect_filled(fill_rect, CornerRadius::ZERO, col);
    }
}
