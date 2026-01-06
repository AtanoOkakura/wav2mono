#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use wav2mono::process_wav_file;

use eframe::egui;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum AppState {
    #[default]
    Idle,
    Converting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileStatus {
    Pending,
    Processing,
    Success(String),
    Error(String),
}

#[derive(Debug, Clone)]
struct ProcessTask {
    path: std::path::PathBuf,
    status: FileStatus,
}

#[derive(Default)]
struct MyApp {
    tasks: Arc<Mutex<Vec<ProcessTask>>>,
    app_state: Arc<Mutex<AppState>>,
}

impl MyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load Japanese font for Windows
        let mut fonts = egui::FontDefinitions::default();
        
        let font_path = "C:\\Windows\\Fonts\\msgothic.ttc";
        if std::path::Path::new(font_path).exists() {
            if let Ok(font_data) = std::fs::read(font_path) {
                fonts.font_data.insert(
                    "msgothic".to_owned(),
                    Arc::new(egui::FontData::from_owned(font_data)),
                );
                
                fonts.families
                    .get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "msgothic".to_owned());
                
                fonts.families
                    .get_mut(&egui::FontFamily::Monospace)
                    .unwrap()
                    .insert(0, "msgothic".to_owned());
                
                cc.egui_ctx.set_fonts(fonts);
            }
        }

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
            ui.label("Drag-and-drop WAV files onto the window!");

            let tasks = self.tasks.lock().unwrap();
            if !tasks.is_empty() {
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for task in tasks.iter() {
                        ui.horizontal(|ui| {
                            let (icon, color) = match &task.status {
                                FileStatus::Pending => ("⏳", egui::Color32::GRAY),
                                FileStatus::Processing => ("🔄", egui::Color32::BLUE),
                                FileStatus::Success(_) => ("✅", egui::Color32::GREEN),
                                FileStatus::Error(_) => ("❌", egui::Color32::RED),
                            };

                            ui.colored_label(color, icon);
                            
                            let name = task.path.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "???".to_string());
                            
                            let label = ui.label(&name);
                            
                            match &task.status {
                                FileStatus::Success(msg) => {
                                    label.on_hover_text(msg);
                                }
                                FileStatus::Error(err) => {
                                    label.on_hover_text(err);
                                }
                                _ => {}
                            }
                        });
                    }
                });
            }
        });

        let has_pending = {
            let tasks = self.tasks.lock().unwrap();
            tasks.iter().any(|t| matches!(t.status, FileStatus::Pending))
        };

        if has_pending {
            let app_state = *self.app_state.lock().unwrap();
            match app_state {
                AppState::Idle => {
                    let state_store = Arc::clone(&self.app_state);
                    *self.app_state.lock().unwrap() = AppState::Converting;
                    
                    let tasks = Arc::clone(&self.tasks);
                    let ctx_store = ctx.clone();

                    thread::spawn(move || {
                        run_conversion_loop(tasks, &ctx_store);
                        *state_store.lock().unwrap() = AppState::Idle;
                        ctx_store.request_repaint();
                    });
                }
                AppState::Converting => {}
            }
        }

        preview_files_being_dropped(ctx);

        // Collect dropped files:
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                let mut tasks = self.tasks.lock().unwrap();
                for f in i.raw.dropped_files.iter() {
                    if let Some(path) = &f.path {
                        if path.extension()
                            .and_then(|ext| ext.to_str())
                            .map(|s| s.to_ascii_lowercase()) == Some("wav".to_string()) 
                        {
                            tasks.push(ProcessTask {
                                path: path.clone(),
                                status: FileStatus::Pending,
                            });
                        }
                    }
                }
            }
        });
    }
}

fn run_conversion_loop(
    tasks: Arc<Mutex<Vec<ProcessTask>>>,
    ctx: &egui::Context,
) {
    loop {
        let mut index = None;
        {
            let mut tasks_lock = tasks.lock().unwrap();
            for (i, task) in tasks_lock.iter_mut().enumerate() {
                if matches!(task.status, FileStatus::Pending) {
                    task.status = FileStatus::Processing;
                    index = Some(i);
                    break;
                }
            }
        }
        ctx.request_repaint();

        if let Some(idx) = index {
            let path = {
                let tasks_lock = tasks.lock().unwrap();
                tasks_lock[idx].path.clone()
            };

            let result = process_wav_file(&path);

            {
                let mut tasks_lock = tasks.lock().unwrap();
                match result {
                    Ok(msg) => tasks_lock[idx].status = FileStatus::Success(msg),
                    Err(e) => tasks_lock[idx].status = FileStatus::Error(e.to_string()),
                }
            }
            ctx.request_repaint();
        } else {
            break;
        }
    }
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
        viewport: egui::ViewportBuilder::default()
            .with_drag_and_drop(true)
            .with_always_on_top()
            .with_inner_size(egui::vec2(320.0, 240.0)),

        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        concat!("wav2mono ver", env!("CARGO_PKG_VERSION")),
        native_options,
        Box::new(|cc| Ok(Box::new(MyApp::new(cc)))),
    )
}
