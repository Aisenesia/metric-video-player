use crate::{video_player::VideoPlayer, metrics::MetricsCollector, Args};
use eframe::egui;
use std::time::Instant;

pub struct MetricVideoPlayerApp {
    player: VideoPlayer,
    metrics: MetricsCollector,
    args: Args,
    
    // GUI state
    is_playing: bool,
    frame_texture: Option<egui::TextureHandle>,
    last_frame_time: Option<Instant>,
    
    // Control state
    target_fps_input: String,
    show_metrics_window: bool,
    show_advanced_metrics: bool,
}

impl MetricVideoPlayerApp {
    pub fn new(player: VideoPlayer, metrics: MetricsCollector, args: Args) -> Self {
        Self {
            target_fps_input: args.target_fps.to_string(),
            player,
            metrics,
            args,
            is_playing: true, // Start playing automatically
            frame_texture: None,
            last_frame_time: None,
            show_metrics_window: true,
            show_advanced_metrics: false,
        }
    }
    
    fn update_frame(&mut self, ctx: &egui::Context) {
        if !self.is_playing {
            log::debug!("Playback is paused");
            return;
        }
        
        log::debug!("update_frame called, is_playing: {}", self.is_playing);
        
        // Check if it's time for the next frame
        let should_advance = if let Some(last_time) = self.last_frame_time {
            let target_interval = if self.args.target_fps > 0 {
                std::time::Duration::from_nanos(1_000_000_000 / self.args.target_fps as u64)
            } else {
                std::time::Duration::from_millis(33) // ~30 FPS default
            };
            
            last_time.elapsed() >= target_interval
        } else {
            true // Always advance the first frame
        };
        
        if should_advance {
            log::debug!("Advancing to next frame...");
            if let Ok(Some(frame)) = self.player.next_frame() {
                log::debug!("Got frame {}: {}x{}", frame.frame_number, frame.width, frame.height);
                self.metrics.record_frame(frame.frame_number, &frame);
                
                // Save first frame to disk for debugging
                if frame.frame_number == 1 {
                    if let Err(e) = image::save_buffer(
                        "debug_frame_1.png",
                        &frame.data,
                        frame.width,
                        frame.height,
                        image::ColorType::Rgb8,
                    ) {
                        log::error!("Failed to save debug frame: {}", e);
                    } else {
                        log::info!("Saved debug frame to debug_frame_1.png");
                    }
                }
                
                // Convert frame data to texture
                let color_image = egui::ColorImage::from_rgb(
                    [frame.width as usize, frame.height as usize],
                    &frame.data,
                );
                
                log::debug!("Creating texture from {}x{} image with {} bytes", 
                    frame.width, frame.height, frame.data.len());
                log::debug!("ColorImage size: {:?}", color_image.size);
                
                // Create texture with explicit options
                let texture_options = egui::TextureOptions {
                    magnification: egui::TextureFilter::Linear,
                    minification: egui::TextureFilter::Linear,
                    wrap_mode: egui::TextureWrapMode::ClampToEdge,
                };
                
                // Always use the same texture name so it gets updated, not recreated
                self.frame_texture = Some(ctx.load_texture(
                    "video_frame",
                    color_image,
                    texture_options,
                ));
                
                log::debug!("Texture created successfully");
                log::debug!("Texture handle ID: {:?}", self.frame_texture.as_ref().unwrap().id());
                
                self.last_frame_time = Some(Instant::now());
            } else {
                // End of video
                self.is_playing = false;
                log::info!("Video playback completed");
            }
        }
    }
}

impl eframe::App for MetricVideoPlayerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ALWAYS request repaint for continuous updates
        ctx.request_repaint();
        
        // Update video frame
        self.update_frame(ctx);
        
        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Export Metrics").clicked() {
                        // TODO: Implement file dialog for export
                        if let Some(export_path) = &self.args.export_metrics {
                            if let Err(e) = self.metrics.export_to_file(export_path) {
                                log::error!("Failed to export metrics: {}", e);
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_metrics_window, "Show Metrics");
                    ui.checkbox(&mut self.show_advanced_metrics, "Advanced Metrics");
                });
            });
        });
        
        // Main video panel
        egui::CentralPanel::default().show(ctx, |ui| {
            // Fill the entire background with a color to test if panel is visible
            ui.painter().rect_filled(
                ui.available_rect_before_wrap(),
                0.0,
                egui::Color32::from_rgb(50, 50, 50)
            );
            
            ui.heading("Metric Video Player");
            ui.label("If you see this, text rendering works!");
            ui.colored_label(egui::Color32::YELLOW, "This should be YELLOW text");
            
            if ui.button("TEST BUTTON - Click me!").clicked() {
                log::info!("Button clicked!");
            }
            
            // Video display
            if let Some(texture) = &self.frame_texture {
                log::debug!("RENDER: Have texture, size: {:?}, ID: {:?}", texture.size_vec2(), texture.id());
                let available_size = ui.available_size();
                log::debug!("RENDER: Available UI size: {:?}", available_size);
                let texture_size = texture.size_vec2();
                
                // Test: Just draw a simple colored rectangle to see if rendering works
                ui.colored_label(egui::Color32::RED, "VIDEO AREA - If you see this text in red, UI rendering works!");
                
                // Draw a test rectangle
                let test_rect = egui::Rect::from_min_size(
                    egui::pos2(100.0, 100.0),
                    egui::vec2(200.0, 200.0)
                );
                ui.painter().rect_filled(test_rect, 0.0, egui::Color32::from_rgb(255, 0, 0));
                
                // Reserve space for controls at the bottom
                let video_area_height = available_size.y - 120.0; // Reserve 120px for controls
                let available_video_size = egui::vec2(available_size.x, video_area_height);
                
                // Calculate aspect ratio preserving size
                let aspect_ratio = texture_size.x / texture_size.y;
                log::debug!("RENDER: Aspect ratio: {}", aspect_ratio);
                let display_size = if available_video_size.x / available_video_size.y > aspect_ratio {
                    egui::vec2(available_video_size.y * aspect_ratio, available_video_size.y)
                } else {
                    egui::vec2(available_video_size.x, available_video_size.x / aspect_ratio)
                };
                log::debug!("RENDER: Display size: {:?}", display_size);
                
                // Center the video
                ui.allocate_ui_with_layout(
                    egui::vec2(available_size.x, video_area_height),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.add_space(10.0);
                        log::debug!("RENDER: About to add Image widget");
                        // Try simpler image rendering
                        let response = ui.add(egui::Image::new(texture).fit_to_exact_size(display_size));
                        log::debug!("RENDER: Image widget added, response rect: {:?}", response.rect);
                    },
                );
            } else {
                log::warn!("No texture available to display");
                // Show loading message or generate first frame
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 200.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.add_space(50.0);
                        if self.is_playing {
                            ui.label("Loading video...");
                            ui.spinner();
                        } else {
                            ui.label("Click Play to start video");
                        }
                    },
                );
            }
            
            // Control panel
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(if self.is_playing { "Pause" } else { "Play" }).clicked() {
                    self.is_playing = !self.is_playing;
                    if self.is_playing {
                        self.last_frame_time = Some(Instant::now());
                    }
                }
                
                ui.separator();
                
                ui.label("Target FPS:");
                if ui.text_edit_singleline(&mut self.target_fps_input).changed() {
                    if let Ok(fps) = self.target_fps_input.parse::<u32>() {
                        self.args.target_fps = fps;
                    }
                }
                
                ui.separator();
                
                // Progress bar
                let progress = self.player.get_progress();
                ui.label(format!("Progress: {:.1}%", progress * 100.0));
                ui.add(egui::ProgressBar::new(progress as f32).show_percentage());
            });
            
            // Quick metrics display
            ui.horizontal(|ui| {
                ui.label(format!("Frame: {}/{}", 
                    self.player.get_current_frame(),
                    self.player.get_total_frames()
                ));
                ui.separator();
                ui.label(format!("FPS: {:.1}", self.metrics.get_current_fps()));
                ui.separator();
                ui.label(format!("Avg FPS: {:.1}", self.metrics.get_average_fps()));
                ui.separator();
                ui.label(format!("Memory: {:.1} MB", self.metrics.get_current_memory_mb()));
            });
        });
        
        // Metrics window
        if self.show_metrics_window {
            egui::Window::new("Performance Metrics")
                .default_size([300.0, 400.0])
                .show(ctx, |ui| {
                    ui.heading("Real-time Metrics");
                    
                    egui::Grid::new("metrics_grid")
                        .num_columns(2)
                        .spacing([40.0, 4.0])
                        .show(ui, |ui| {
                            ui.label("Current FPS:");
                            ui.label(format!("{:.2}", self.metrics.get_current_fps()));
                            ui.end_row();
                            
                            ui.label("Average FPS:");
                            ui.label(format!("{:.2}", self.metrics.get_average_fps()));
                            ui.end_row();
                            
                            ui.label("Max FPS:");
                            ui.label(format!("{:.2}", self.metrics.get_max_fps()));
                            ui.end_row();
                            
                            ui.label("Current Memory:");
                            ui.label(format!("{:.1} MB", self.metrics.get_current_memory_mb()));
                            ui.end_row();
                            
                            ui.label("Peak Memory:");
                            ui.label(format!("{:.1} MB", self.metrics.get_peak_memory_mb()));
                            ui.end_row();
                            
                            ui.label("Current CPU:");
                            ui.label(format!("{:.1}%", self.metrics.get_current_cpu_percent()));
                            ui.end_row();
                            
                            ui.label("Peak CPU:");
                            ui.label(format!("{:.1}%", self.metrics.get_peak_cpu_percent()));
                            ui.end_row();
                            
                            ui.label("Dropped Frames:");
                            ui.label(format!("{}", self.metrics.get_dropped_frames()));
                            ui.end_row();
                            
                            ui.label("Session Time:");
                            ui.label(format!("{:.1}s", self.metrics.get_session_duration().as_secs_f64()));
                            ui.end_row();
                        });
                    
                    ui.separator();
                    
                    if self.show_advanced_metrics {
                        ui.heading("Video Information");
                        egui::Grid::new("video_info_grid")
                            .num_columns(2)
                            .spacing([40.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("Resolution:");
                                ui.label(format!("{}x{}", 
                                    self.player.get_width(),
                                    self.player.get_height()
                                ));
                                ui.end_row();
                                
                                ui.label("Duration:");
                                ui.label(format!("{:.1}s", self.player.get_duration().as_secs_f64()));
                                ui.end_row();
                                
                                ui.label("Native FPS:");
                                ui.label(format!("{:.2}", self.player.get_native_fps()));
                                ui.end_row();
                                
                                ui.label("Total Frames:");
                                ui.label(format!("{}", self.player.get_total_frames()));
                                ui.end_row();
                            });
                    }
                    
                    ui.separator();
                    
                    if ui.button("Export Metrics").clicked() {
                        // TODO: Implement proper file dialog
                        let export_path = std::path::PathBuf::from("metrics_export.json");
                        if let Err(e) = self.metrics.export_to_file(&export_path) {
                            log::error!("Failed to export metrics: {}", e);
                        } else {
                            log::info!("Metrics exported to: {:?}", export_path);
                        }
                    }
                    
                    if ui.button("Print Summary").clicked() {
                        self.metrics.print_summary();
                    }
                });
        }
    }
}