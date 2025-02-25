use clap::Parser;
use derive_more::{Deref, DerefMut};
use image::{ImageBuffer, Rgb, RgbImage};
use imageproc::filter::gaussian_blur_f32;
use indicatif::{ProgressBar, ProgressStyle};
use noise::{NoiseFn, OpenSimplex, Seedable};
use rayon::prelude::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output video filename
    #[arg(short, long, default_value = "balatro_background.mp4")]
    output: String,

    /// List of colors in hex format (e.g. #FF0000)
    #[arg(
        short,
        long,
        default_values_t = vec![
            String::from("#0f0f3c"),
            String::from("#4b1d52"),
            String::from("#802c52"),
            String::from("#c2414b")
        ]
    )]
    colors: Vec<String>,

    /// Duration of video in seconds
    #[arg(short, long, default_value_t = 5.0)]
    duration: f64,

    /// Frames per second
    #[arg(short, long, default_value_t = 30)]
    fps: u32,

    /// Video width
    #[arg(short, long, default_value_t = 800)]
    width: u32,

    /// Video height
    #[arg(short = 'H', long, default_value_t = 600)]
    height: u32,

    /// Animation speed
    #[arg(short, long, default_value_t = 0.002)]
    speed: f64,

    /// Turbulence/violence of the swirls
    #[arg(short, long, default_value_t = 1.0)]
    turbulence: f64,

    /// Pixelation level (1 = no pixelation)
    #[arg(short, long, default_value_t = 4)]
    pixelation: u32,

    /// Scale of the noise pattern
    #[arg(short = 'S', long, default_value_t = 0.003)]
    scale: f64,
}

/// Wrapper around Vec<u8> to represent an RGB color
#[derive(Debug, Clone, Copy, Deref, DerefMut)]
struct Color([u8; 3]);

impl Color {
    fn from_hex(hex: &str) -> Self {
        let hex = hex.trim_start_matches('#');
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        Color([r, g, b])
    }
}

/// Interpolate between two colors
fn interpolate_colors(colors: &[Color], t: f64) -> [u8; 3] {
    let num_colors = colors.len();
    let sharpness = 2.0; // Increase for more contrast
    let t = t.powf(sharpness) * (1.0 - t.powf(sharpness)) * 4.0; // Creates sharper boundaries

    let index = t * (num_colors - 1) as f64;
    let lower_idx = index.floor() as usize;
    let upper_idx = (lower_idx + 1).min(num_colors - 1);
    let weight = index - lower_idx as f64;

    let color1 = colors[lower_idx];
    let color2 = colors[upper_idx];

    [
        ((1.0 - weight) * color1[0] as f64 + weight * color2[0] as f64) as u8,
        ((1.0 - weight) * color1[1] as f64 + weight * color2[1] as f64) as u8,
        ((1.0 - weight) * color1[2] as f64 + weight * color2[2] as f64) as u8,
    ]
}
/// Generate a single frame of the animation
/// Generate a single frame of the animation
fn generate_frame(
    frame_idx: usize,
    width: u32,
    height: u32,
    pixelation: u32,
    speed: f64,
    turbulence: f64,
    scale: f64,
    colors: &[Color],
    total_frames: usize, // Add total_frames as a parameter
) -> RgbImage {
    // Calculate buffer dimensions
    let buffer_width = width / pixelation;
    let buffer_height = height / pixelation;

    // IMPORTANT: Use constant seeds for all frames to ensure continuity
    // We only want the time to change, not the noise pattern itself
    let noise1 = OpenSimplex::new(1234);
    let noise2 = OpenSimplex::new(5678);
    let noise_turb = OpenSimplex::new(9012);

    // Calculate time value for looping animation
    let cycle_length = total_frames as f64;
    let normalized_frame = frame_idx as f64 / cycle_length;
    let time_val = normalized_frame * 2.0 * std::f64::consts::PI * speed;

    // Create the small pixelated buffer
    let mut buffer: RgbImage = ImageBuffer::new(buffer_width, buffer_height);

    // Generate each pixel in the buffer
    for y in 0..buffer_height {
        for x in 0..buffer_width {
            let nx = x as f64 * scale;
            let ny = y as f64 * scale;

            // Use layered noise with different frequencies to create more stable patterns
            // Primary flow field - very slow movement
            let flow_x = noise1.get([nx * 0.5, ny * 0.5, time_val.sin() * 0.5]) * 0.7 + time_val.cos() * 0.2;
            let flow_y = noise2.get([nx * 0.5, ny * 0.5, time_val.cos() * 0.5]) * 0.7 + time_val.sin() * 0.2;

            // Sample main noise using the flow field for displacement
            // This creates the swirling, meandering effect
            let main_val = noise1.get([nx + flow_x, ny + flow_y, time_val.sin() * 0.3]);

            // Add detail with a second noise layer
            let detail_val = noise2.get([
                nx * 1.5 + flow_x * 0.5,
                ny * 1.5 + flow_y * 0.5,
                time_val.cos() * 0.2,
            ]) * 0.3;

            // Add subtle turbulence
            let turb_val =
                noise_turb.get([nx * 2.0, ny * 2.0, time_val.sin() * 0.15]) * (turbulence * 0.3);

            // Combine all components
            let mut value = (main_val + detail_val + turb_val + 1.0) * 0.5;
            value = value.clamp(0.0, 1.0);

            // Get color by interpolation
            let rgb = interpolate_colors(colors, value);
            buffer.put_pixel(x, y, Rgb(rgb));
        }
    }
    
    // Apply a slight blur
    let blurred = gaussian_blur_f32(&buffer, 0.5);
    // Now resize the buffer to full resolution
    let resized = image::imageops::resize(
        &blurred,
        width,
        height,
        image::imageops::FilterType::Triangle,
    );
    resized
}

fn main() {
    let args = Args::parse();
    
    // Convert hex color strings to Color objects
    let colors: Vec<Color> = args.colors.iter().map(|c| Color::from_hex(c)).collect();
    
    // Calculate total frames
    let total_frames = (args.duration * args.fps as f64) as usize;
    
    println!("Balatro Background Generator");
    println!("----------------------------");
    println!("Output: {}", args.output);
    println!("Resolution: {}x{}", args.width, args.height);
    println!("Duration: {}s at {} FPS ({} frames)", args.duration, args.fps, total_frames);
    println!("Pixelation: {}", args.pixelation);
    println!("Colors: {:?}", args.colors);
    println!("Generating frames...");

    // Create temp directory for frames
    let temp_dir = PathBuf::from("./temp_frames");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).expect("Failed to clean up existing temp directory");
    }
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");

    let temp_dir = fs::canonicalize(&temp_dir).expect("Failed to get absolute path to temp directory");

    // Progress bar
    let start_time = Instant::now();
    let pb = ProgressBar::new(total_frames as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} frames ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Generate frames in parallel
    (0..total_frames).into_par_iter().for_each(|frame_idx| {
        let frame = generate_frame(
            frame_idx,
            args.width,
            args.height,
            args.pixelation,
            args.speed,
            args.turbulence,
            args.scale,
            &colors,
            total_frames,
        );

        // Save frame to temp directory
        let frame_path = temp_dir.join(format!("frame_{:05}.png", frame_idx));
        frame.save(&frame_path).expect("Failed to save frame");

        pb.inc(1);
    });

    pb.finish_with_message("All frames generated!");
    
    // Calculate frame generation speed
    let frames_per_second = total_frames as f64 / start_time.elapsed().as_secs_f64();
    println!(
        "Generated {} frames in {:.2} seconds ({:.2} frames/sec)",
        total_frames,
        start_time.elapsed().as_secs_f64(),
        frames_per_second
    );

    // Use ffmpeg to combine frames into video
    println!("Combining frames into video...");
    
    // Debug: Print the exact command being executed
    let input_pattern = format!("{}/frame_%05d.png", temp_dir.to_str().unwrap());
    println!("FFmpeg input pattern: {}", input_pattern);

    let output_status = Command::new("ffmpeg")
        .args([
            "-y",                                      // Overwrite output file
            "-framerate", &args.fps.to_string(),       // Input framerate
            "-i", &input_pattern,                      // Input pattern
            "-c:v", "libx264",                         // Video codec
            "-preset", "medium",                       // Encoding preset
            "-crf", "18",                              // Quality (lower is better)
            "-pix_fmt", "yuv420p",                     // Pixel format
            &args.output,                              // Output file
        ])
        .status();

    match output_status {
        Ok(status) => {
            if status.success() {
                println!("Video successfully created: {}", args.output);
                
                // Clean up temp files
                fs::remove_dir_all(&temp_dir).expect("Failed to clean up temp directory");
            } else {
                eprintln!("Error: ffmpeg command failed");
                eprintln!("Make sure ffmpeg is installed and in your PATH");
            }
        }
        Err(e) => {
            eprintln!("Error executing ffmpeg: {}", e);
            eprintln!("Make sure ffmpeg is installed and in your PATH");
        }
    }
}
