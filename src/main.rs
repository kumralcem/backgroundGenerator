mod balatro;
mod grainient;
mod noise_style;

use clap::{Parser, ValueEnum};
use image::RgbImage;
use indicatif::ProgressBar;
use rayon::prelude::*;
use std::{
    error::Error,
    path::PathBuf,
    process::{Command, ExitCode},
};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
enum Style {
    Noise,
    Balatro,
    Grainient,
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Generate noise, Balatro-inspired spirals, or Grainient-inspired gradients as PNG images or videos"
)]
struct Args {
    /// Output .png still image, or .mp4/.mov/.mkv video
    #[arg(short, long, default_value = "balatro_background.mp4")]
    output: PathBuf,
    /// Background algorithm (noise, spiral paint, or grainy gradient)
    #[arg(long, value_enum)]
    style: Style,
    /// Repeat for each #RRGGBB color; Balatro and Grainient require exactly three
    #[arg(short, long)]
    colors: Vec<Color>,
    #[arg(short, long, default_value_t = 5.0)]
    duration: f64,
    #[arg(short, long, default_value_t = 30)]
    fps: u32,
    #[arg(short, long, default_value_t = 800)]
    width: u32,
    #[arg(short = 'H', long, default_value_t = 600)]
    height: u32,
    /// Original noise animation speed
    #[arg(short, long, default_value_t = 0.002)]
    speed: f64,
    /// Domain distortion strength (both styles)
    #[arg(short, long, default_value_t = 1.0)]
    turbulence: f64,
    /// Pixel block size; 1 gives full resolution
    #[arg(short, long, default_value_t = 4)]
    pixelation: u32,
    /// Original noise spatial frequency
    #[arg(short = 'S', long, default_value_t = 0.003)]
    scale: f64,
    /// Time in seconds to sample for a PNG, or starting time for Balatro video
    #[arg(long, default_value_t = 0.0)]
    time: f64,
    /// Balatro spiral radius relative to the shorter image dimension
    #[arg(long, default_value_t = 0.65)]
    swirl_radius: f64,
    /// Balatro spiral winding strength; negative reverses the spiral
    #[arg(long, default_value_t = 4.0, allow_hyphen_values = true)]
    swirl_amount: f64,
    /// Balatro rotation in radians per second; negative reverses direction
    #[arg(long, default_value_t = 0.25, allow_hyphen_values = true)]
    spin_speed: f64,
    /// Balatro paint flow speed, independent of rotation
    #[arg(long, default_value_t = 1.0, allow_hyphen_values = true)]
    flow_speed: f64,
    /// Balatro initial orientation in degrees
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    rotation: f64,
    /// Horizontal center (Balatro/Grainient) (0 = left, 1 = right)
    #[arg(long, default_value_t = 0.5)]
    center_x: f64,
    /// Vertical center (Balatro/Grainient) (0 = top, 1 = bottom)
    #[arg(long, default_value_t = 0.5)]
    center_y: f64,
    /// Magnification (Balatro/Grainient)
    #[arg(long, default_value_t = 1.0)]
    zoom: f64,
    /// Color contrast (0.1..20); default 3.5 for Balatro, 1.5 for Grainient
    #[arg(long)]
    contrast: Option<f64>,
    /// Balatro highlight intensity (0..1)
    #[arg(long, default_value_t = 0.15)]
    lighting: f64,
    /// Use periodic motion for a seamless loop; Balatro rotation oscillates
    #[arg(long = "loop")]
    seamless_loop: bool,
    /// Grainient animation speed
    #[arg(long, default_value_t = 0.25, allow_hyphen_values = true)]
    time_speed: f64,
    /// Grainient wave distortion strength
    #[arg(long, default_value_t = 0.3)]
    warp_strength: f64,
    /// Grainient wave spatial frequency
    #[arg(long, default_value_t = 5.0)]
    warp_frequency: f64,
    /// Grainient wave motion speed
    #[arg(long, default_value_t = 2.0, allow_hyphen_values = true)]
    warp_speed: f64,
    /// Grainient gradient angle in degrees
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    blend_angle: f64,
    /// Grainient transition width (0.01..2)
    #[arg(long, default_value_t = 0.6)]
    blend_softness: f64,
    /// Grainient palette balance (-1..1)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    color_balance: f64,
    /// Grainient grain intensity (0..1); zero disables grain
    #[arg(long, default_value_t = 0.1)]
    grain_amount: f64,
    /// Grainient grain size in output pixels (0.1..100)
    #[arg(long, default_value_t = 1.0)]
    grain_scale: f64,
    /// Animate Grainient grain; otherwise it remains fixed
    #[arg(long)]
    grain_animated: bool,
    /// Grainient repeatable grain seed
    #[arg(long, default_value_t = 42)]
    seed: u32,
    /// Grainient gamma (0.1..5)
    #[arg(long, default_value_t = 1.0)]
    gamma: f64,
    /// Grainient saturation (0..3)
    #[arg(long, default_value_t = 1.0)]
    saturation: f64,
    /// Grainient pale paper-like palette
    #[arg(long)]
    light_mode: bool,
}

#[derive(Debug, Clone, Copy)]
struct Color([u8; 3]);
impl std::ops::Index<usize> for Color {
    type Output = u8;
    fn index(&self, index: usize) -> &u8 {
        &self.0[index]
    }
}
impl std::str::FromStr for Color {
    type Err = String;
    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let hex = value.strip_prefix('#').unwrap_or(value);
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("invalid color {value:?}: expected #RRGGBB"));
        }
        let mut rgb = [0; 3];
        for (i, component) in rgb.iter_mut().enumerate() {
            *component =
                u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
        }
        Ok(Self(rgb))
    }
}

impl Args {
    fn contrast(&self) -> f64 {
        self.contrast.unwrap_or(if self.style == Style::Grainient {
            1.5
        } else {
            3.5
        })
    }

    fn validate(&self) -> Result<usize> {
        for (name, value) in [
            ("duration", self.duration),
            ("speed", self.speed),
            ("turbulence", self.turbulence),
            ("scale", self.scale),
            ("time", self.time),
            ("swirl-radius", self.swirl_radius),
            ("swirl-amount", self.swirl_amount),
            ("spin-speed", self.spin_speed),
            ("flow-speed", self.flow_speed),
            ("rotation", self.rotation),
            ("center-x", self.center_x),
            ("center-y", self.center_y),
            ("zoom", self.zoom),
            ("contrast", self.contrast()),
            ("lighting", self.lighting),
            ("time-speed", self.time_speed),
            ("warp-strength", self.warp_strength),
            ("warp-frequency", self.warp_frequency),
            ("warp-speed", self.warp_speed),
            ("blend-angle", self.blend_angle),
            ("blend-softness", self.blend_softness),
            ("color-balance", self.color_balance),
            ("grain-amount", self.grain_amount),
            ("grain-scale", self.grain_scale),
            ("gamma", self.gamma),
            ("saturation", self.saturation),
        ] {
            if !value.is_finite() || value.abs() > 1e6 {
                return Err(format!("--{name} must be finite and within ±1000000").into());
            }
        }
        if self.width == 0 || self.height == 0 || self.width > 16384 || self.height > 16384 {
            return Err("dimensions must be between 1 and 16384".into());
        }
        if self.pixelation == 0 || self.fps == 0 || self.fps > 240 {
            return Err("pixelation must be positive and fps must be between 1 and 240".into());
        }
        if self.duration <= 0.0
            || self.swirl_radius <= 0.0
            || self.zoom <= 0.0
            || self.scale <= 0.0
            || self.turbulence < 0.0
            || self.time < 0.0
        {
            return Err("duration, radius, zoom and scale must be positive; turbulence and time must be nonnegative".into());
        }
        if !(0.1..=20.0).contains(&self.contrast())
            || !(0.0..=1.0).contains(&self.lighting)
            || !(0.0..=1.0).contains(&self.center_x)
            || !(0.0..=1.0).contains(&self.center_y)
        {
            return Err(
                "contrast must be 0.1..20; lighting and center coordinates must be 0..1".into(),
            );
        }
        if self.style != Style::Noise && !self.colors.is_empty() && self.colors.len() != 3 {
            return Err(
                "Balatro and Grainient use exactly three --colors: primary, secondary, shadow"
                    .into(),
            );
        }
        if self.warp_strength < 0.0
            || self.warp_frequency <= 0.0
            || !(0.01..=2.0).contains(&self.blend_softness)
            || !(-1.0..=1.0).contains(&self.color_balance)
            || !(0.0..=1.0).contains(&self.grain_amount)
            || !(0.1..=100.0).contains(&self.grain_scale)
            || !(0.1..=5.0).contains(&self.gamma)
            || !(0.0..=3.0).contains(&self.saturation)
        {
            return Err("invalid Grainient controls: check --help for valid ranges; warp strength must be nonnegative and frequency positive".into());
        }
        let frames = (self.duration * self.fps as f64).round();
        if !(1.0..=1_000_000.0).contains(&frames) {
            return Err("duration × fps must produce between 1 and 1000000 frames".into());
        }
        Ok(frames as usize)
    }
    fn palette(&self) -> Vec<Color> {
        if !self.colors.is_empty() {
            return self.colors.clone();
        }
        match self.style {
            Style::Noise => vec![
                Color([15, 15, 60]),
                Color([75, 29, 82]),
                Color([128, 44, 82]),
                Color([194, 65, 75]),
            ],
            Style::Grainient => vec![
                Color([255, 159, 252]),
                Color([82, 39, 255]),
                Color([180, 151, 207]),
            ],
            Style::Balatro => vec![
                Color([222, 68, 59]),
                Color([0, 107, 180]),
                Color([22, 35, 37]),
            ],
        }
    }
}

fn render(args: &Args, colors: &[Color], frame: usize, frames: usize) -> RgbImage {
    match args.style {
        Style::Grainient => grainient::generate_frame(
            args,
            colors,
            args.time + frame as f64 / args.fps as f64,
            frames as f64 / args.fps as f64,
        ),
        Style::Balatro => balatro::generate_frame(
            args,
            colors,
            args.time + frame as f64 / args.fps as f64,
            frames as f64 / args.fps as f64,
        ),
        Style::Noise => noise_style::generate_frame(args, colors, frame, frames),
    }
}

fn run(args: Args) -> Result<()> {
    let frames = args.validate()?;
    let colors = args.palette();
    let extension = args
        .output
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "png" {
        let frame = match args.style {
            Style::Grainient => grainient::generate_frame(&args, &colors, args.time, args.duration),
            Style::Balatro => balatro::generate_frame(&args, &colors, args.time, args.duration),
            Style::Noise => render(
                &args,
                &colors,
                (args.time * args.fps as f64).round() as usize,
                frames,
            ),
        };
        frame.save(&args.output)?;
    } else {
        if !["mp4", "mov", "mkv"].contains(&extension.as_str()) {
            return Err("output extension must be .png, .mp4, .mov or .mkv".into());
        }
        if !args.width.is_multiple_of(2) || !args.height.is_multiple_of(2) {
            return Err(
                "video dimensions must be even for H.264; PNG accepts odd dimensions".into(),
            );
        }
        if !Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map_err(|e| format!("FFmpeg is required for video export: {e}"))?
            .status
            .success()
        {
            return Err("FFmpeg is unavailable".into());
        }
        let directory = tempfile::Builder::new()
            .prefix("background-generator-")
            .tempdir()?;
        let progress = ProgressBar::new(frames as u64);
        (0..frames)
            .into_par_iter()
            .try_for_each(|frame| -> Result<()> {
                render(&args, &colors, frame, frames)
                    .save(directory.path().join(format!("frame_{frame:07}.png")))?;
                progress.inc(1);
                Ok(())
            })?;
        progress.finish();
        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-framerate",
                &args.fps.to_string(),
                "-i",
            ])
            .arg(directory.path().join("frame_%07d.png"))
            .args([
                "-c:v", "libx264", "-preset", "medium", "-crf", "18", "-pix_fmt", "yuv420p",
            ])
            .arg(&args.output)
            .status()?;
        if !status.success() {
            return Err(format!("FFmpeg failed with {status}").into());
        }
    }
    println!("Created {}", args.output.display());
    Ok(())
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(style: &str) -> Args {
        Args::try_parse_from([
            "test",
            "--style",
            style,
            "--width",
            "64",
            "--height",
            "48",
            "--pixelation",
            "2",
        ])
        .unwrap()
    }
    #[test]
    fn style_must_be_selected() {
        assert!(Args::try_parse_from(["test"]).is_err());
        for style in ["noise", "balatro", "grainient"] {
            assert!(args(style).validate().is_ok());
        }
    }
    #[test]
    fn rejects_malformed_colors_without_panics() {
        for color in ["", "#fff", "#GG1234", "ééé", "#12345678"] {
            assert!(color.parse::<Color>().is_err());
        }
        assert_eq!("#abCD09".parse::<Color>().unwrap().0, [171, 205, 9]);
    }
    #[test]
    fn rejects_invalid_render_parameters() {
        let mut a = args("balatro");
        a.pixelation = 0;
        assert!(a.validate().is_err());
        a.pixelation = 1;
        a.swirl_radius = 0.0;
        assert!(a.validate().is_err());
        a.swirl_radius = 1.0;
        a.spin_speed = f64::NAN;
        assert!(a.validate().is_err());
        a.spin_speed = 1.0;
        a.colors = vec![Color([0, 0, 0])];
        assert!(a.validate().is_err());
        a.style = Style::Grainient;
        assert!(a.validate().is_err());
    }
    #[test]
    fn frames_are_repeatable_and_animated() {
        for style in ["noise", "balatro", "grainient"] {
            let a = args(style);
            let colors = a.palette();
            let first = render(&a, &colors, 0, 150);
            assert_eq!(first, render(&a, &colors, 0, 150));
            assert_ne!(
                first,
                render(&a, &colors, 50, 150),
                "{style} should animate"
            );
            assert!(first.pixels().any(|p| p != first.get_pixel(0, 0)));
        }
    }
    #[test]
    fn seamless_modes_close_at_the_endpoint() {
        for style in ["noise", "balatro", "grainient"] {
            let mut a = args(style);
            a.seamless_loop = true;
            a.grain_animated = true;
            let colors = a.palette();
            let first = render(&a, &colors, 0, 150);
            let end = render(&a, &colors, 150, 150);
            let difference = first
                .as_raw()
                .iter()
                .zip(end.as_raw())
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            assert!(difference <= 1, "{style} loop has a seam: {difference}");
            assert_ne!(first, render(&a, &colors, 40, 150));
        }
    }
    #[test]
    fn tiny_and_partial_pixel_blocks_are_supported() {
        for style in ["noise", "balatro", "grainient"] {
            let mut a = args(style);
            a.width = 3;
            a.height = 1;
            a.pixelation = 8;
            assert_eq!(render(&a, &a.palette(), 0, 150).dimensions(), (3, 1));
        }
    }
    #[test]
    fn zero_speeds_freeze_all_styles_even_in_loop_mode() {
        for style in ["noise", "balatro", "grainient"] {
            let mut a = args(style);
            a.seamless_loop = true;
            a.speed = 0.0;
            a.spin_speed = 0.0;
            a.flow_speed = 0.0;
            a.time_speed = 0.0;
            let colors = a.palette();
            assert_eq!(
                render(&a, &colors, 0, 150),
                render(&a, &colors, 50, 150),
                "{style}"
            );
        }
    }
    #[test]
    fn grain_controls_change_texture_and_preserve_determinism() {
        let mut a = args("grainient");
        let colors = a.palette();
        let original = render(&a, &colors, 0, 150);
        a.seed += 1;
        assert_ne!(original, render(&a, &colors, 0, 150));
        a.grain_amount = 0.0;
        let no_grain = render(&a, &colors, 0, 150);
        a.seed += 1;
        assert_eq!(no_grain, render(&a, &colors, 0, 150));
    }
    #[test]
    fn spiral_radius_and_palette_affect_output() {
        let mut a = args("balatro");
        let colors = a.palette();
        let first = render(&a, &colors, 0, 150);
        a.swirl_radius *= 2.0;
        assert_ne!(first, render(&a, &colors, 0, 150));
        let black = vec![Color([0, 0, 0]); 3];
        a.lighting = 0.0;
        assert!(render(&a, &black, 0, 150).as_raw().iter().all(|v| *v == 0));
    }
}
