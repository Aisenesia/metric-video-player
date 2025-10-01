use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;
use std::path::Path;
use std::time::{Duration, Instant};

pub struct VideoFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp: Duration,
    pub frame_number: u64,
}

pub struct VideoPlayer {
    format_context: ffmpeg::format::context::Input,
    video_stream_index: usize,
    decoder: ffmpeg::decoder::Video,
    scaler: ffmpeg::software::scaling::Context,
    
    target_fps: u32,
    frame_duration: Option<Duration>,
    last_frame_time: Option<Instant>,
    
    current_frame: u64,
    total_frames: u64,
    duration: Duration,
}

impl VideoPlayer {
    pub fn new(video_path: &Path, target_fps: u32) -> Result<Self> {
        // Initialize FFmpeg
        ffmpeg::init().context("Failed to initialize FFmpeg")?;
        
        log::info!("Loading video file: {:?}", video_path);
        
        // Open input file
        let input = ffmpeg::format::input(video_path)
            .context("Failed to open video file")?;
        
        // Find video stream
        let video_stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .context("No video stream found")?;
        
        let video_stream_index = video_stream.index();
        
        // Get decoder with hardware acceleration if available
        let context_decoder = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
            .context("Failed to create decoder context")?;
        
        let mut decoder = context_decoder
            .decoder()
            .video()
            .context("Failed to create video decoder")?;
        
        // Try to enable hardware acceleration
        // Note: This may not work on all systems, but will gracefully fall back to software decoding
        unsafe {
            // Enable multi-threading for faster decoding
            (*decoder.as_mut_ptr()).thread_count = num_cpus::get() as i32;
            (*decoder.as_mut_ptr()).thread_type = ffmpeg_sys_next::FF_THREAD_FRAME | ffmpeg_sys_next::FF_THREAD_SLICE;
            
            log::debug!("Decoder configured with {} threads", (*decoder.as_mut_ptr()).thread_count);
        }
        
        if let Some(codec) = decoder.codec() {
            log::info!("Codec: {}", codec.name());
        }
        
        // Create scaler for RGB conversion (use FAST_BILINEAR for speed)
        let scaler = ffmpeg::software::scaling::Context::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            ffmpeg::format::Pixel::RGB24,
            decoder.width(),
            decoder.height(),
            ffmpeg::software::scaling::Flags::FAST_BILINEAR,
        ).context("Failed to create scaler")?;
        
        // Calculate frame duration for target FPS
        let frame_duration = if target_fps > 0 {
            Some(Duration::from_nanos(1_000_000_000 / target_fps as u64))
        } else {
            None
        };
        
        // Get video metadata
        let total_frames = video_stream.frames() as u64;
        let duration_secs = video_stream.duration() as f64 * f64::from(video_stream.time_base());
        let duration = if duration_secs > 0.0 {
            Duration::from_secs_f64(duration_secs)
        } else {
            // Fallback: estimate from frame rate if duration is invalid
            let fps = video_stream.avg_frame_rate();
            if fps.numerator() > 0 && fps.denominator() > 0 {
                let native_fps = fps.numerator() as f64 / fps.denominator() as f64;
                Duration::from_secs_f64(total_frames as f64 / native_fps)
            } else {
                Duration::from_secs(1) // Fallback to 1 second if we can't determine
            }
        };
        
        log::info!("Video loaded:");
        log::info!("  Resolution: {}x{}", decoder.width(), decoder.height());
        log::info!("  Total frames: {}", total_frames);
        log::info!("  Duration: {:.2}s", duration.as_secs_f64());
        log::info!("  Native FPS: {:.2}", total_frames as f64 / duration.as_secs_f64());
        
        Ok(VideoPlayer {
            format_context: input,
            video_stream_index,
            decoder,
            scaler,
            target_fps,
            frame_duration,
            last_frame_time: None,
            current_frame: 0,
            total_frames,
            duration,
        })
    }
    
    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>> {
        let mut frame = ffmpeg::frame::Video::empty();
        let mut rgb_frame = ffmpeg::frame::Video::empty();
        
        // Read packets until we get a video frame
        for (stream, packet) in self.format_context.packets() {
            if stream.index() != self.video_stream_index {
                continue;
            }
            
            self.decoder.send_packet(&packet)?;
            
            while self.decoder.receive_frame(&mut frame).is_ok() {
                // Scale to RGB24
                self.scaler.run(&frame, &mut rgb_frame)?;
                
                self.current_frame += 1;
                
                // Convert frame data with proper stride handling
                let width = rgb_frame.width();
                let height = rgb_frame.height();
                let linesize = rgb_frame.stride(0);
                let data_ptr = rgb_frame.data(0);
                
                log::debug!("Frame {}: width={}, height={}, linesize={}, expected={}", 
                    self.current_frame, width, height, linesize, width as usize * 3);
                
                // If linesize equals width * 3, we can use the data directly
                // Otherwise, we need to copy row by row to remove padding
                let data = if linesize == width as usize * 3 {
                    log::debug!("Using direct copy (no padding)");
                    data_ptr.to_vec()
                } else {
                    log::debug!("Copying row by row (has padding)");
                    let mut data = Vec::with_capacity(width as usize * height as usize * 3);
                    for y in 0..height as usize {
                        let row_start = y * linesize;
                        let row_end = row_start + (width as usize * 3);
                        data.extend_from_slice(&data_ptr[row_start..row_end]);
                    }
                    data
                };
                
                // Debug: Check if we have actual pixel data (not all zeros) - only with verbose logging
                let non_zero_pixels = data.iter().take(100).filter(|&&b| b != 0).count();
                log::debug!("Frame {} data sample: first 100 bytes have {} non-zero values", 
                    self.current_frame, non_zero_pixels);
                
                let timestamp = if let Some(pts) = frame.timestamp() {
                    let time_secs = pts as f64 * f64::from(stream.time_base());
                    // Handle negative timestamps (can occur in some video formats)
                    if time_secs >= 0.0 {
                        Duration::from_secs_f64(time_secs)
                    } else {
                        Duration::from_secs_f64(self.current_frame as f64 / self.get_native_fps())
                    }
                } else {
                    Duration::from_secs_f64(self.current_frame as f64 / self.get_native_fps())
                };
                
                return Ok(Some(VideoFrame {
                    data,
                    width,
                    height,
                    timestamp,
                    frame_number: self.current_frame,
                }));
            }
        }
        
        // End of stream - flush decoder
        self.decoder.send_eof()?;
        while self.decoder.receive_frame(&mut frame).is_ok() {
            self.scaler.run(&frame, &mut rgb_frame)?;
            
            self.current_frame += 1;
            
            // Convert frame data with proper stride handling
            let width = rgb_frame.width();
            let height = rgb_frame.height();
            let linesize = rgb_frame.stride(0);
            let data_ptr = rgb_frame.data(0);
            
            // If linesize equals width * 3, we can use the data directly
            // Otherwise, we need to copy row by row to remove padding
            let data = if linesize == width as usize * 3 {
                data_ptr.to_vec()
            } else {
                let mut data = Vec::with_capacity(width as usize * height as usize * 3);
                for y in 0..height as usize {
                    let row_start = y * linesize;
                    let row_end = row_start + (width as usize * 3);
                    data.extend_from_slice(&data_ptr[row_start..row_end]);
                }
                data
            };
            
            let timestamp = Duration::from_secs_f64(
                self.current_frame as f64 / self.get_native_fps()
            );
            
            return Ok(Some(VideoFrame {
                data,
                width,
                height,
                timestamp,
                frame_number: self.current_frame,
            }));
        }
        
        Ok(None)
    }
    
    pub fn maintain_target_fps(&mut self) {
        if let Some(frame_duration) = self.frame_duration {
            if let Some(last_time) = self.last_frame_time {
                let elapsed = last_time.elapsed();
                if elapsed < frame_duration {
                    std::thread::sleep(frame_duration - elapsed);
                }
            }
            self.last_frame_time = Some(Instant::now());
        }
    }
    
    pub fn get_current_frame(&self) -> u64 {
        self.current_frame
    }
    
    pub fn get_total_frames(&self) -> u64 {
        self.total_frames
    }
    
    pub fn get_duration(&self) -> Duration {
        self.duration
    }
    
    pub fn get_progress(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            self.current_frame as f64 / self.total_frames as f64
        }
    }
    
    pub fn get_width(&self) -> u32 {
        self.decoder.width()
    }
    
    pub fn get_height(&self) -> u32 {
        self.decoder.height()
    }
    
    pub fn get_native_fps(&self) -> f64 {
        if self.duration.as_secs_f64() > 0.0 {
            self.total_frames as f64 / self.duration.as_secs_f64()
        } else {
            30.0 // Default fallback
        }
    }
    
    pub fn seek_to_frame(&mut self, _frame_number: u64) -> Result<()> {
        // Basic seek implementation - more advanced seeking would require
        // using ffmpeg's seek_frame functionality
        log::warn!("Seeking not fully implemented yet");
        Ok(())
    }
}