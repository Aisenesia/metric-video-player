use crate::{video_player::VideoPlayer, metrics::MetricsCollector, Args};
use anyhow::Result;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use std::time::Instant;

pub fn run_sdl_gui(mut player: VideoPlayer, mut metrics: MetricsCollector, args: Args) -> Result<()> {
    let sdl_context = sdl2::init().map_err(|e| anyhow::anyhow!("SDL init failed: {}", e))?;
    let video_subsystem = sdl_context.video().map_err(|e| anyhow::anyhow!("Video subsystem failed: {}", e))?;

    let width = player.get_width();
    let height = player.get_height();

    let window = video_subsystem
        .window("Metric Video Player (SDL2)", width, height)
        .position_centered()
        .resizable()
        .build()?;

    let mut canvas = window.into_canvas().accelerated().present_vsync().build()?;
    let texture_creator = canvas.texture_creator();

    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, width, height)
        .map_err(|e| anyhow::anyhow!("Texture creation failed: {}", e))?;

    let mut event_pump = sdl_context.event_pump().map_err(|e| anyhow::anyhow!("Event pump failed: {}", e))?;
    let mut is_playing = true;
    let mut last_frame_time = Instant::now();

    log::info!("SDL2 GUI started. Press SPACE to pause/play, ESC to quit.");

    'running: loop {
        // Handle events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::Space),
                    ..
                } => {
                    is_playing = !is_playing;
                    log::info!("Playback {}", if is_playing { "resumed" } else { "paused" });
                }
                _ => {}
            }
        }

        if is_playing {
            // Check if it's time for the next frame
            let should_advance = if args.target_fps > 0 {
                let target_interval = std::time::Duration::from_nanos(1_000_000_000 / args.target_fps as u64);
                last_frame_time.elapsed() >= target_interval
            } else {
                true // Maximum FPS
            };

            if should_advance {
                if let Ok(Some(frame)) = player.next_frame() {
                    metrics.record_frame(frame.frame_number, &frame);

                    // Update texture with frame data
                    texture
                        .update(None, &frame.data, (frame.width * 3) as usize)
                        .map_err(|e| anyhow::anyhow!("Texture update failed: {}", e))?;

                    // Clear and render
                    canvas.clear();
                    
                    // Calculate aspect ratio preserving size
                    let (window_width, window_height) = canvas.output_size().map_err(|e| anyhow::anyhow!("{}", e))?;
                    let aspect_ratio = width as f32 / height as f32;
                    let window_aspect = window_width as f32 / window_height as f32;
                    
                    let (dst_width, dst_height) = if window_aspect > aspect_ratio {
                        let h = window_height;
                        let w = (h as f32 * aspect_ratio) as u32;
                        (w, h)
                    } else {
                        let w = window_width;
                        let h = (w as f32 / aspect_ratio) as u32;
                        (w, h)
                    };
                    
                    let x = (window_width - dst_width) / 2;
                    let y = (window_height - dst_height) / 2;
                    
                    canvas.copy(&texture, None, Rect::new(x as i32, y as i32, dst_width, dst_height)).map_err(|e| anyhow::anyhow!("{}", e))?;
                    canvas.present();

                    last_frame_time = Instant::now();

                    if frame.frame_number % 100 == 0 {
                        log::info!(
                            "Frame {}: {:.2} FPS (avg: {:.2})",
                            frame.frame_number,
                            metrics.get_current_fps(),
                            metrics.get_average_fps()
                        );
                    }
                } else {
                    // End of video
                    is_playing = false;
                    log::info!("Video playback completed");
                    
                    // Show final metrics
                    let session = metrics.finalize_session();
                    log::info!("\n=== Final Metrics ===");
                    log::info!("Total frames: {}", session.total_frames);
                    log::info!("Average FPS: {:.2}", session.average_fps);
                    log::info!("Max FPS: {:.2}", session.max_fps);
                    log::info!("Peak Memory: {:.1} MB", session.peak_memory_mb);
                    log::info!("Session Duration: {:.2}s", session.total_duration_seconds);
                }
            }
        }

        // Small delay to prevent maxing out CPU when paused
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    Ok(())
}
