//! `audiofuse-control-center`: Native Linux desktop studio Control Center application
//! for the Arturia AudioFuse 8Pre (`1c75:af0e`).

mod app;
mod theme;

use anyhow::Result;
use app::ControlCenterApp;
use audiofuse_core::{AudioFuseController, MixerBank, open_best_available_transport};
use clap::Parser;
use std::path::PathBuf;
use theme::StudioThemeMode;

#[derive(Parser, Debug)]
#[command(
    name = "audiofuse-control-center",
    version,
    about = "Native Linux Studio Control Center for the Arturia AudioFuse 8Pre (1c75:af0e)"
)]
struct GuiArgs {
    /// Start in warm paper light studio theme (#f6f5f2) instead of carbon dark (#141413)
    #[arg(long)]
    light: bool,

    /// Force offline deterministic simulator backend
    #[arg(long)]
    simulator: bool,

    /// Select initial mixer bank (analog / adat / usb)
    #[arg(long, default_value = "analog")]
    bank: String,

    /// Render a 16:9 studio console screenshot directly to a PNG file and exit
    #[arg(long)]
    screenshot: Option<PathBuf>,

    /// Screenshot width in pixels (default 1600 for 16:9 studio viewport)
    #[arg(long, default_value_t = 1600)]
    width: u32,

    /// Screenshot height in pixels (default 900)
    #[arg(long, default_value_t = 900)]
    height: u32,
}

fn main() -> Result<()> {
    let args = GuiArgs::parse();
    let controller =
        open_best_available_transport(args.simulator).and_then(AudioFuseController::new);
    let theme_mode = if args.light {
        StudioThemeMode::LightPaper
    } else {
        StudioThemeMode::DarkCarbon
    };
    let active_bank = match args.bank.to_ascii_lowercase().as_str() {
        "adat" => MixerBank::Adat,
        "usb" => MixerBank::Usb,
        _ => MixerBank::Analog,
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Arturia AudioFuse 8Pre — Linux Studio Control Center")
            .with_inner_size([args.width as f32, args.height as f32])
            .with_min_inner_size([1150.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "AudioFuse Linux Control Center",
        native_options,
        Box::new(move |_cc| {
            let controller = match controller {
                Ok(controller) => controller,
                Err(error) => {
                    return Ok(Box::new(ConnectionError(
                        error.to_string(),
                        theme_mode,
                        None,
                    )));
                }
            };
            let mut app = ControlCenterApp::new(controller, theme_mode);
            app.active_bank = active_bank;
            app.screenshot = args.screenshot;
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Failed to launch eframe desktop window: {}", e))?;

    Ok(())
}

struct ConnectionError(String, StudioThemeMode, Option<ControlCenterApp>);

impl eframe::App for ConnectionError {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(app) = self.2.as_mut() {
            app.update(ctx, frame);
            return;
        }
        theme::StudioPalette::for_mode(self.1).apply_to_ctx(ctx, self.1);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("AudioFuse 8Pre");
            ui.label("Hardware controls unavailable");
            ui.label(&self.0);
            if ui.button("Open offline simulator").clicked() {
                match open_best_available_transport(true).and_then(AudioFuseController::new) {
                    Ok(controller) => self.2 = Some(ControlCenterApp::new(controller, self.1)),
                    Err(error) => self.0 = error.to_string(),
                }
            }
        });
    }
}
