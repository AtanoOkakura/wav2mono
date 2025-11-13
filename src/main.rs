#![windows_subsystem = "windows"]
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use wav2mono::process_wav_file;

use eframe::egui::ViewportBuilder;

use eframe::egui;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum AppState {
    #[default]
    Idle,
    Converting,
}

#[derive(Default, Debug)]
struct MyApp {
    dropped_files: Arc<Mutex<Vec<egui::DroppedFile>>>,
    app_state: Arc<Mutex<AppState>>,
}

impl MyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // set theme to system theme
        cc.egui_ctx.set_visuals(egui::Visuals::light());

        Self::default()
    }
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

        process_wav_file(input.as_ref()).map_err(|e| {
            io::Error::other(format!("Failed to process file {}: {}", input.display(), e))
        })?;

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

        let screen_rect = ctx.content_rect();
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

// const ICON: &[u8] = include_bytes!("../assets/wav2mono_icon.png");

fn main() -> eframe::Result<()> {
    let view_port = ViewportBuilder::default()
        .with_always_on_top()
        .with_title(concat!("wav2mono ver", env!("CARGO_PKG_VERSION")))
        // .with_icon(viewport::IconData {
        //     rgba: ICON.to_vec(),
        //     width: 58,
        //     height: 58,
        // })
        .with_drag_and_drop(true)
        .with_inner_size(egui::vec2(320.0, 240.0));

    let native_options = eframe::NativeOptions {
        viewport: view_port,
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        concat!("wav2mono ver", env!("CARGO_PKG_VERSION")),
        native_options,
        Box::new(|cc| Ok(Box::new(MyApp::new(cc)))),
    )
}
