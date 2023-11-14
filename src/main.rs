#![windows_subsystem = "windows"]
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use eframe::egui::DroppedFile;
use wav2mono::Wav;

use eframe::egui;

#[derive(Debug, Clone)]
enum AppState {
    Idle,
    Converting,
    Completed,
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
                    ui.label("Dropped files:");

                    for file in dropped_files.clone() {
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

                let state = self.app_state.lock().unwrap();
                match state.clone() {
                    AppState::Idle => {
                        println!("app state idle");
                        eprintln!("App state idle");
                        let state = Arc::clone(&self.app_state);
                        let file = dropped_files.last().unwrap().clone();
                        *state.lock().unwrap() = AppState::Converting;
                        thread::spawn(move || {
                            if let Err(e) = convert_to_mono(file) {
                                eprintln!("{}", e);
                            }
                            *state.lock().unwrap() = AppState::Completed;
                        });
                    }
                    AppState::Converting => {
                        println!("app state converting");
                        eprintln!("App state converting");
                    }
                    AppState::Completed => {
                        println!("app state completed");
                        self.dropped_files.lock().unwrap().pop();
                        *self.app_state.lock().unwrap() = AppState::Idle;
                    }
                }
            }
        });

        preview_files_being_dropped(ctx);

        // Collect dropped files:
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                let mut dropped_files = self.dropped_files.lock().unwrap();
                *dropped_files = i.raw.dropped_files.clone();
            }
        });
    }
}

fn convert_to_mono(file: DroppedFile) -> io::Result<()> {
    let input = file.path.unwrap();
    let output = input
        .parent()
        .unwrap()
        .join("mono")
        .join(input.file_name().unwrap());
    Wav::open(&input).to_mono().write(&output)?;
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
