//! An original CPU spiral-paint renderer inspired by Balatro's visual style.
//! Coordinates are aspect-correct; pixelation changes sampling, not the pattern scale.
use crate::{Args, Color};
use image::{Rgb, RgbImage};
use std::f64::consts::TAU;

pub(crate) fn generate_frame(args: &Args, colors: &[Color], time: f64, duration: f64) -> RgbImage {
    let mut buffer = RgbImage::new(
        args.width.div_ceil(args.pixelation),
        args.height.div_ceil(args.pixelation),
    );
    let unit = args.width.min(args.height) as f64 * args.zoom;
    let (rotation, flow_x, flow_y) = if args.seamless_loop {
        let phase = TAU * time / duration;
        // A circular trajectory closes in both position and velocity at the seam.
        (
            args.spin_speed * duration / TAU * phase.sin(),
            args.flow_speed * duration / TAU * phase.sin(),
            args.flow_speed * duration / TAU * phase.cos(),
        )
    } else {
        (
            args.spin_speed * time,
            args.flow_speed * time * 0.7,
            args.flow_speed * time * 0.53,
        )
    };
    for (x, y, pixel) in buffer.enumerate_pixels_mut() {
        let px =
            (x as f64 * args.pixelation as f64 + 0.5 - args.width as f64 * args.center_x) / unit;
        let py =
            (y as f64 * args.pixelation as f64 + 0.5 - args.height as f64 * args.center_y) / unit;
        let radius = px.hypot(py) / args.swirl_radius;
        let angle =
            py.atan2(px) + args.rotation.to_radians() + rotation - args.swirl_amount * radius;
        let mut u = radius * angle.cos() * 5.0;
        let mut v = radius * angle.sin() * 5.0;
        // Successive smooth distortions curl the spiral into broad paint ribbons.
        for octave in 0..3 {
            let shift = octave as f64 * 1.7;
            let du = (v * 1.3 + flow_x + shift).sin();
            let dv = (u * 1.1 - flow_y + shift).cos();
            u += du * args.turbulence * 0.65;
            v += dv * args.turbulence * 0.65;
        }
        let field = ((u + 0.6 * v.sin()).sin() + (v * 0.8 - 0.4 * u.cos()).cos()) * 0.5;
        let primary = smoothstep(-0.25, 0.25, field * args.contrast());
        let shadow = 1.0 - smoothstep(0.08, 0.42, field.abs() * args.contrast());
        let highlight = args.lighting * (field.abs().clamp(0.0, 1.0)).powi(5);
        let mut rgb = [0; 3];
        for channel in 0..3 {
            let paint =
                colors[0][channel] as f64 * primary + colors[1][channel] as f64 * (1.0 - primary);
            rgb[channel] =
                (paint * (1.0 - shadow) + colors[2][channel] as f64 * shadow + 255.0 * highlight)
                    .clamp(0.0, 255.0)
                    .round() as u8;
        }
        *pixel = Rgb(rgb);
    }
    // Expand exact-size blocks, including partial blocks along the right/bottom edges.
    RgbImage::from_fn(args.width, args.height, |x, y| {
        *buffer.get_pixel(x / args.pixelation, y / args.pixelation)
    })
}

fn smoothstep(low: f64, high: f64, value: f64) -> f64 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
