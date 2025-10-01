# Metric Video Player

A high-performance video player built in Rust that plays videos at maximum FPS and provides detailed performance metrics.

## Features

- **Maximum FPS Playback**: Plays videos at the highest possible frame rate your hardware can achieve
- **Real-time Metrics**: Tracks and displays FPS, memory usage, CPU usage, and frame timing
- **Multiple Interfaces**: GUI mode with live video display, CLI mode, and benchmark mode
- **Performance Analysis**: Detailed metrics export with frame-by-frame analysis
- **Cross-platform**: Native executables for Windows, macOS, and Linux

## Performance Metrics Tracked

- Current/Average/Max/Min FPS
- Memory usage (current, peak, average)
- CPU usage (current, peak, average)
- Frame processing times
- Dropped frame count
- Session duration
- Video metadata (resolution, duration, native FPS)

## Installation

### Prerequisites

You'll need to install FFmpeg development libraries:

**Windows:**
```powershell
# Using vcpkg (recommended)
vcpkg install ffmpeg[core]:x64-windows

# Or download pre-built binaries from https://ffmpeg.org/download.html
```

### Build from Source

```bash
# Clone the repository
git clone https://github.com/Aisenesia/metric-video-player
cd metric-video-player

# Build in release mode for maximum performance
cargo build --release

# The executable will be in target/release/
```

## Usage

### GUI Mode (Default)
```bash
# Play video with GUI
./target/release/metric-video-player --video-path path/to/video.mp4

# Set target FPS
./target/release/metric-video-player --video-path video.mp4 --target-fps 120

# Export metrics to JSON
./target/release/metric-video-player --video-path video.mp4 --export-metrics metrics.json
```

### CLI Mode
```bash
# Play in terminal only
./target/release/metric-video-player --video-path video.mp4 --gui false
```

### Benchmark Mode
```bash
# Run performance benchmark (no GUI, maximum speed)
./target/release/metric-video-player --video-path video.mp4 --benchmark
```

### Command Line Options

```
Options:
  -v, --video-path <VIDEO_PATH>          Path to the video file to play
  -t, --target-fps <TARGET_FPS>          Target FPS (0 = maximum possible) [default: 0]
  -g, --gui <GUI>                        Enable GUI mode [default: true]
  -e, --export-metrics <EXPORT_METRICS>  Export metrics to JSON file
  -V, --verbose                          Enable verbose logging
  -b, --benchmark                        Run in benchmark mode (no GUI, just metrics)
  -h, --help                             Print help
  --sdl                                  Use sdl instead of egui (egui currently broken)
```

## Output Examples

### Benchmark Mode Output
```
=== Benchmark Results ===
Total frames: 7200
Total time: 45.32s
Average FPS: 158.84
Maximum FPS achieved: 240.15
Memory usage: 145.32 MB
```

### Exported Metrics (JSON)
```json
{
  "start_time": "2025-10-01T10:30:00Z",
  "end_time": "2025-10-01T10:31:30Z",
  "total_frames": 2700,
  "average_fps": 30.12,
  "max_fps": 240.5,
  "min_fps": 15.2,
  "peak_memory_mb": 128.5,
  "average_memory_mb": 95.3,
  "frame_metrics": [...]
}
```

## Use Cases

- **Performance Testing**: Measure your system's video playback capabilities
- **Hardware Benchmarking**: Compare different GPUs, CPUs, or storage systems
- **Codec Analysis**: Test performance of different video formats and codecs
- **Frame Rate Analysis**: Identify bottlenecks in video playback pipeline
- **Memory Profiling**: Monitor memory usage patterns during video playback

## Architecture

The application is built with performance in mind:

- **FFmpeg Integration**: Uses the battle-tested FFmpeg library for video decoding
- **Zero-copy Operations**: Minimizes memory allocations where possible
- **Immediate Mode GUI**: Uses egui for responsive real-time interface
- **Async Processing**: Leverages Tokio for efficient I/O operations
- **System Monitoring**: Uses platform-specific APIs for accurate resource monitoring

## Supported Formats

Supports all video formats that FFmpeg can decode, including:
- MP4, AVI, MKV, MOV, WebM
- H.264, H.265, VP8, VP9, AV1
- And many more...

## Performance Tips

1. **Use SSD storage** for video files to minimize I/O bottlenecks
2. **Close other applications** when benchmarking for accurate results
3. **Use release builds** (`--release`) for maximum performance
4. **Disable v-sync** in benchmark mode for true maximum FPS testing
5. **Use uncompressed or lightly compressed videos** for CPU-bound testing

## Troubleshooting

### FFmpeg Not Found
Make sure FFmpeg development libraries are installed and in your system PATH.

### Poor Performance
- Check if you're running in release mode
- Ensure video file is on fast storage (SSD)
- Close other resource-intensive applications
- Try different video codecs/formats

### High Memory Usage
- This is expected for high-resolution videos
- Use the `--benchmark` mode to minimize GUI overhead
- Monitor the exported metrics to identify memory patterns