use crate::video_player::VideoFrame;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::Path;
use std::time::{Duration, Instant};
use sysinfo::{System, Pid, ProcessRefreshKind, RefreshKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameMetrics {
    pub frame_number: u64,
    pub timestamp: f64,
    pub processing_time_ms: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetrics {
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub total_frames: u64,
    pub total_duration_seconds: f64,
    pub average_fps: f64,
    pub max_fps: f64,
    pub min_fps: f64,
    pub peak_memory_mb: f64,
    pub average_memory_mb: f64,
    pub average_cpu_percent: f64,
    pub peak_cpu_percent: f64,
    pub dropped_frames: u64,
    pub frame_metrics: Vec<FrameMetrics>,
}

pub struct MetricsCollector {
    session_start: Instant,
    session_start_utc: DateTime<Utc>,
    frame_times: VecDeque<(Instant, u64)>, // (timestamp, frame_number)
    frame_metrics: Vec<FrameMetrics>,
    
    // System monitoring
    system: System,
    current_pid: Pid,
    
    // Running statistics
    total_frames: u64,
    peak_memory_mb: f64,
    peak_cpu_percent: f64,
    dropped_frames: u64,
    
    // FPS calculation window (last N frames)
    fps_window_size: usize,
    last_frame_time: Option<Instant>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::everything())
        );
        system.refresh_all();
        
        let current_pid = sysinfo::get_current_pid().unwrap();
        
        Self {
            session_start: Instant::now(),
            session_start_utc: Utc::now(),
            frame_times: VecDeque::new(),
            frame_metrics: Vec::new(),
            system,
            current_pid,
            total_frames: 0,
            peak_memory_mb: 0.0,
            peak_cpu_percent: 0.0,
            dropped_frames: 0,
            fps_window_size: 60, // Calculate FPS over last 60 frames
            last_frame_time: None,
        }
    }
    
    pub fn record_frame(&mut self, frame_number: u64, frame: &VideoFrame) {
        let now = Instant::now();
        
        // Calculate processing time (for now, just the time since last frame)
        let processing_time = if let Some(last_time) = self.last_frame_time {
            now.duration_since(last_time)
        } else {
            Duration::from_millis(0)
        };
        
        // Update system info
        self.system.refresh_processes_specifics(ProcessRefreshKind::new().with_memory().with_cpu());
        
        let memory_usage_mb = if let Some(process) = self.system.process(self.current_pid) {
            process.memory() as f64 / 1024.0 / 1024.0 // Convert from KB to MB
        } else {
            0.0
        };
        
        let cpu_usage_percent = if let Some(process) = self.system.process(self.current_pid) {
            process.cpu_usage() as f64
        } else {
            0.0
        };
        
        // Update peak values
        self.peak_memory_mb = self.peak_memory_mb.max(memory_usage_mb);
        self.peak_cpu_percent = self.peak_cpu_percent.max(cpu_usage_percent);
        
        // Record frame metrics
        let frame_metrics = FrameMetrics {
            frame_number,
            timestamp: frame.timestamp.as_secs_f64(),
            processing_time_ms: processing_time.as_secs_f64() * 1000.0,
            memory_usage_mb,
            cpu_usage_percent,
        };
        
        self.frame_metrics.push(frame_metrics);
        
        // Update FPS calculation window
        self.frame_times.push_back((now, frame_number));
        if self.frame_times.len() > self.fps_window_size {
            self.frame_times.pop_front();
        }
        
        self.total_frames += 1;
        self.last_frame_time = Some(now);
    }
    
    pub fn get_current_fps(&self) -> f64 {
        if self.frame_times.len() < 2 {
            return 0.0;
        }
        
        let (first_time, first_frame) = self.frame_times.front().unwrap();
        let (last_time, last_frame) = self.frame_times.back().unwrap();
        
        let time_diff = last_time.duration_since(*first_time).as_secs_f64();
        let frame_diff = last_frame - first_frame;
        
        if time_diff > 0.0 {
            frame_diff as f64 / time_diff
        } else {
            0.0
        }
    }
    
    pub fn get_average_fps(&self) -> f64 {
        let elapsed = self.session_start.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.total_frames as f64 / elapsed
        } else {
            0.0
        }
    }
    
    pub fn get_max_fps(&self) -> f64 {
        self.frame_metrics
            .windows(2)
            .map(|window| {
                let time_diff = window[1].timestamp - window[0].timestamp;
                if time_diff > 0.0 {
                    1.0 / time_diff
                } else {
                    0.0
                }
            })
            .fold(0.0, f64::max)
    }
    
    pub fn get_min_fps(&self) -> f64 {
        self.frame_metrics
            .windows(2)
            .map(|window| {
                let time_diff = window[1].timestamp - window[0].timestamp;
                if time_diff > 0.0 {
                    1.0 / time_diff
                } else {
                    f64::INFINITY
                }
            })
            .fold(f64::INFINITY, f64::min)
    }
    
    pub fn get_peak_memory_mb(&self) -> f64 {
        self.peak_memory_mb
    }
    
    pub fn get_average_memory_mb(&self) -> f64 {
        if self.frame_metrics.is_empty() {
            0.0
        } else {
            self.frame_metrics.iter()
                .map(|m| m.memory_usage_mb)
                .sum::<f64>() / self.frame_metrics.len() as f64
        }
    }
    
    pub fn get_peak_cpu_percent(&self) -> f64 {
        self.peak_cpu_percent
    }
    
    pub fn get_average_cpu_percent(&self) -> f64 {
        if self.frame_metrics.is_empty() {
            0.0
        } else {
            self.frame_metrics.iter()
                .map(|m| m.cpu_usage_percent)
                .sum::<f64>() / self.frame_metrics.len() as f64
        }
    }
    
    pub fn get_dropped_frames(&self) -> u64 {
        self.dropped_frames
    }
    
    pub fn increment_dropped_frames(&mut self) {
        self.dropped_frames += 1;
    }
    
    pub fn get_session_duration(&self) -> Duration {
        self.session_start.elapsed()
    }
    
    pub fn get_total_frames(&self) -> u64 {
        self.total_frames
    }
    
    pub fn finalize_session(&mut self) -> SessionMetrics {
        SessionMetrics {
            start_time: self.session_start_utc,
            end_time: Some(Utc::now()),
            total_frames: self.total_frames,
            total_duration_seconds: self.session_start.elapsed().as_secs_f64(),
            average_fps: self.get_average_fps(),
            max_fps: self.get_max_fps(),
            min_fps: self.get_min_fps(),
            peak_memory_mb: self.peak_memory_mb,
            average_memory_mb: self.get_average_memory_mb(),
            average_cpu_percent: self.get_average_cpu_percent(),
            peak_cpu_percent: self.peak_cpu_percent,
            dropped_frames: self.dropped_frames,
            frame_metrics: self.frame_metrics.clone(),
        }
    }
    
    pub fn export_to_file(&mut self, path: &Path) -> Result<()> {
        let session_metrics = self.finalize_session();
        let json = serde_json::to_string_pretty(&session_metrics)?;
        std::fs::write(path, json)?;
        Ok(())
    }
    
    pub fn print_summary(&self) {
        println!("\n=== Performance Metrics Summary ===");
        println!("Session Duration: {:.2}s", self.session_start.elapsed().as_secs_f64());
        println!("Total Frames: {}", self.total_frames);
        println!("Average FPS: {:.2}", self.get_average_fps());
        println!("Current FPS: {:.2}", self.get_current_fps());
        println!("Max FPS: {:.2}", self.get_max_fps());
        println!("Min FPS: {:.2}", self.get_min_fps());
        println!("Peak Memory: {:.2} MB", self.peak_memory_mb);
        println!("Average Memory: {:.2} MB", self.get_average_memory_mb());
        println!("Peak CPU: {:.1}%", self.peak_cpu_percent);
        println!("Average CPU: {:.1}%", self.get_average_cpu_percent());
        println!("Dropped Frames: {}", self.dropped_frames);
    }
    
    // Real-time monitoring getters for GUI
    pub fn get_current_memory_mb(&mut self) -> f64 {
        self.system.refresh_processes_specifics(ProcessRefreshKind::new().with_memory());
        if let Some(process) = self.system.process(self.current_pid) {
            process.memory() as f64 / 1024.0 / 1024.0
        } else {
            0.0
        }
    }
    
    pub fn get_current_cpu_percent(&mut self) -> f64 {
        self.system.refresh_processes_specifics(ProcessRefreshKind::new().with_cpu());
        if let Some(process) = self.system.process(self.current_pid) {
            process.cpu_usage() as f64
        } else {
            0.0
        }
    }
}