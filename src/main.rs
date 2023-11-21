#![windows_subsystem = "windows"]
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use wav2mono::Wav;

use eframe::egui;

#[derive(Debug, Clone, Copy)]
enum AppState {
    Idle,
    Converting,
}

impl Default for AppState {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Default, Debug)]
struct MyApp {
    dropped_files: Arc<Mutex<Vec<egui::DroppedFile>>>,
    app_state: Arc<Mutex<AppState>>,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("developed by ");
                ui.hyperlink_to("Atano", "https://twitter.com/AtanoOkakura");
                egui::warn_if_debug_build(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Drag-and-drop files onto the window!");

            let dropped_files = self.dropped_files.lock().unwrap();
            // Show dropped files (if any):
            if !dropped_files.is_empty() {
                ui.group(|ui| {
                    ui.label("Converting to mono:");

                    for file in dropped_files.iter() {
                        let info = if let Some(path) = &file.path {
                            path.display().to_string()
                        } else if !file.name.is_empty() {
                            file.name.clone()
                        } else {
                            "???".to_owned()
                        };

                        ui.label(info);
                    }
                });
            }
        });

        if !self.dropped_files.lock().unwrap().is_empty() {
            let app_state = *self.app_state.lock().unwrap();
            match app_state {
                AppState::Idle => {
                    let state_store = Arc::clone(&self.app_state);

                    *self.app_state.lock().unwrap() = AppState::Converting;
                    let ctx_store = ctx.clone();
                    let file = Arc::clone(&self.dropped_files);

                    thread::spawn(move || {
                        if let Err(e) = convert_to_mono(file, &ctx_store) {
                            eprintln!("{}", e);
                        }
                        *state_store.lock().unwrap() = AppState::Idle;
                    });
                }
                AppState::Converting => {
                    println!("app state converting");
                }
            }
        }

        preview_files_being_dropped(ctx);

        // Collect dropped files:
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                let mut dropped_files = self.dropped_files.lock().unwrap();
                for f in i.raw.dropped_files.iter() {
                    dropped_files.push(f.clone());
                }
            }
        });
    }
}

fn convert_to_mono(
    files: Arc<Mutex<Vec<egui::DroppedFile>>>,
    ctx: &egui::Context,
) -> io::Result<()> {
    loop {
        if files.lock().unwrap().is_empty() {
            break;
        }

        let file = files.lock().unwrap().remove(0);
        let Some(input) = file.path else {
            continue;
        };

        if input.extension().unwrap_or_default() != "wav" {
            continue;
        }

        let output = input
            .parent()
            .unwrap()
            .join("mono")
            .join(input.file_name().unwrap());
        Wav::open(&input).to_mono().write(&output)?;
        ctx.request_repaint();
    }
    Ok(())
}

/// Preview hovering files:
fn preview_files_being_dropped(ctx: &egui::Context) {
    use egui::*;
    use std::fmt::Write as _;

    if !ctx.input(|i| i.raw.hovered_files.is_empty()) {
        let text = ctx.input(|i| {
            let mut text = "Dropping files:\n".to_owned();
            for file in &i.raw.hovered_files {
                if let Some(path) = &file.path {
                    write!(text, "\n{}", path.display()).ok();
                } else if !file.mime.is_empty() {
                    write!(text, "\n{}", file.mime).ok();
                } else {
                    text += "\n???";
                }
            }
            text
        });

        let painter =
            ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("file_drop_target")));

        let screen_rect = ctx.screen_rect();
        painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(192));
        painter.text(
            screen_rect.center(),
            Align2::CENTER_CENTER,
            text,
            TextStyle::Heading.resolve(&ctx.style()),
            Color32::WHITE,
        );
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        drag_and_drop_support: true,
        always_on_top: true,
        initial_window_size: Some(egui::vec2(320.0, 240.0)),

        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        concat!("wav2mono ver", env!("CARGO_PKG_VERSION")),
        native_options,
        Box::new(|_cc| Box::<MyApp>::default()),
    )
}
