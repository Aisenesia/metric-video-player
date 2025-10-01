use anyhow::Result;
use clap::Parser;
use log::info;
use std::path::PathBuf;

mod video_player;
mod metrics;
mod gui;
mod sdl_gui;

use video_player::VideoPlayer;
use metrics::MetricsCollector;

#[derive(Parser, Debug)]
#[command(name = "metric-video-player")]
#[command(about = "High-performance video player with FPS and performance metrics")]
pub struct Args {
    /// Path to the video file to play
    #[arg(short = 'i', long)]
    pub video_path: PathBuf,
    
    /// Target FPS (0 = maximum possible)
    #[arg(short, long, default_value = "0")]
    pub target_fps: u32,
    
    /// Enable GUI mode (default: true)
    #[arg(short, long, default_value = "true")]
    pub gui: bool,
    
    /// Use SDL2 for GUI instead of egui
    #[arg(long)]
    pub sdl: bool,
    
    /// Export metrics to JSON file
    #[arg(short, long)]
    pub export_metrics: Option<PathBuf>,
    
    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,
    
    /// Run in benchmark mode (no GUI, just metrics)
    #[arg(short, long)]
    pub benchmark: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize logging
    if args.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    }
    
    info!("Starting Metric Video Player");
    info!("Video file: {:?}", args.video_path);
    info!("Target FPS: {}", if args.target_fps == 0 { "Maximum".to_string() } else { args.target_fps.to_string() });
    
    // Validate video file exists
    if !args.video_path.exists() {
        anyhow::bail!("Video file does not exist: {:?}", args.video_path);
    }
    
    // Initialize metrics collector
    let mut metrics = MetricsCollector::new();
    
    // Create video player
    let mut player = VideoPlayer::new(&args.video_path, args.target_fps)?;
    
    if args.benchmark {
        // Run in benchmark mode
        info!("Running in benchmark mode...");
        run_benchmark(&mut player, &mut metrics).await?;
        
        // Export metrics if requested
        if let Some(export_path) = &args.export_metrics {
            info!("Exporting metrics to: {:?}", export_path);
            metrics.export_to_file(export_path)?;
        }
    } else if args.gui {
        // Run with GUI
        info!("Starting GUI mode...");
        if args.sdl {
            info!("Using SDL2 for video display...");
            sdl_gui::run_sdl_gui(player, metrics, args)?;
        } else {
            info!("Using egui for video display...");
            run_gui(player, metrics, args).await?;
        }
    } else {
        // Run in CLI mode
        info!("Running in CLI mode...");
        run_cli(&mut player, &mut metrics).await?;
        
        // Export metrics if requested
        if let Some(export_path) = &args.export_metrics {
            info!("Exporting metrics to: {:?}", export_path);
            metrics.export_to_file(export_path)?;
        }
    }
    
    info!("Metric Video Player finished");
    Ok(())
}

async fn run_benchmark(player: &mut VideoPlayer, metrics: &mut MetricsCollector) -> Result<()> {
    info!("Starting benchmark...");
    
    let start_time = std::time::Instant::now();
    let mut frame_count = 0;
    
    while let Some(frame) = player.next_frame()? {
        frame_count += 1;
        metrics.record_frame(frame_count, &frame);
        
        // Update metrics every 100 frames
        if frame_count % 100 == 0 {
            let elapsed = start_time.elapsed();
            let current_fps = frame_count as f64 / elapsed.as_secs_f64();
            println!("Processed {} frames, Current FPS: {:.2}", frame_count, current_fps);
        }
    }
    
    let total_time = start_time.elapsed();
    let average_fps = frame_count as f64 / total_time.as_secs_f64();
    
    println!("\n=== Benchmark Results ===");
    println!("Total frames: {}", frame_count);
    println!("Total time: {:.2}s", total_time.as_secs_f64());
    println!("Average FPS: {:.2}", average_fps);
    println!("Maximum FPS achieved: {:.2}", metrics.get_max_fps());
    println!("Memory usage: {:.2} MB", metrics.get_peak_memory_mb());
    
    Ok(())
}

async fn run_cli(player: &mut VideoPlayer, metrics: &mut MetricsCollector) -> Result<()> {
    info!("Starting CLI playback...");
    
    let start_time = std::time::Instant::now();
    let mut frame_count = 0;
    
    println!("Playing video... Press Ctrl+C to stop");
    
    while let Some(frame) = player.next_frame()? {
        frame_count += 1;
        metrics.record_frame(frame_count, &frame);
        
        // Display progress every second
        let elapsed = start_time.elapsed();
        if elapsed.as_secs() > 0 && frame_count % (metrics.get_average_fps() as u64).max(1) == 0 {
            let current_fps = frame_count as f64 / elapsed.as_secs_f64();
            println!("Frame: {}, FPS: {:.2}, Time: {:.1}s", 
                frame_count, current_fps, elapsed.as_secs_f64());
        }
        
        // Sleep to maintain target FPS if specified
        player.maintain_target_fps();
    }
    
    let total_time = start_time.elapsed();
    println!("\nPlayback completed in {:.2}s", total_time.as_secs_f64());
    
    Ok(())
}

async fn run_gui(player: VideoPlayer, metrics: MetricsCollector, args: Args) -> Result<()> {
    log::info!("Setting up eframe options...");
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Metric Video Player"),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    
    log::info!("Running eframe...");
    let app = gui::MetricVideoPlayerApp::new(player, metrics, args);
    
    eframe::run_native(
        "Metric Video Player",
        options,
        Box::new(move |cc| {
            log::info!("eframe creation callback called, GL available: {:?}", cc.gl.is_some());
            Ok(Box::new(app))
        }),
    ).map_err(|e| anyhow::anyhow!("Failed to run GUI: {}", e))?;
    
    Ok(())
}