//! Soft, warped three-color gradients with independently sampled film grain.
//! Original CPU implementation inspired by the Grainient visual reference.
use crate::{Args, Color};
use image::{Rgb, RgbImage};
use noise::{NoiseFn, OpenSimplex};
use std::f64::consts::TAU;

pub(crate) fn generate_frame(args: &Args, colors: &[Color], time: f64, duration: f64) -> RgbImage {
    let phase = TAU * time / duration;
    let (motion_x, motion_y) = if args.seamless_loop {
        let amplitude = args.time_speed * args.warp_speed * duration / TAU;
        (amplitude * phase.sin(), amplitude * phase.cos())
    } else {
        (
            time * args.time_speed * args.warp_speed,
            time * args.time_speed * args.warp_speed * 0.73,
        )
    };
    let (sin_angle, cos_angle) = args.blend_angle.to_radians().sin_cos();
    let unit = args.width.min(args.height) as f64 * args.zoom;
    let grain = OpenSimplex::new(args.seed);
    let (grain_z, grain_w) = if !args.grain_animated {
        (0.0, 0.0)
    } else if args.seamless_loop {
        (phase.sin(), phase.cos())
    } else {
        (time * 2.0, 0.0)
    };
    RgbImage::from_fn(args.width, args.height, |x, y| {
        // Pixelation only affects the gradient; grain retains its own resolution.
        let px = ((x / args.pixelation * args.pixelation) as f64 + 0.5
            - args.width as f64 * args.center_x)
            / unit;
        let py = ((y / args.pixelation * args.pixelation) as f64 + 0.5
            - args.height as f64 * args.center_y)
            / unit;
        let mut u = px * cos_angle - py * sin_angle;
        let mut v = px * sin_angle + py * cos_angle;
        u += args.warp_strength * (v * args.warp_frequency + motion_x).sin();
        v += args.warp_strength * (u * args.warp_frequency * 0.8 - motion_y).sin();
        let coordinate = u * 0.65 + v * 0.75 + args.color_balance;
        let weights = [-0.55, 0.0, 0.55]
            .map(|center| (-((coordinate - center) / args.blend_softness).powi(2)).exp());
        let total = weights.iter().sum::<f64>();
        // Stable softmax fallback at extreme zoom/offset values.
        let weights = if total < 1e-100 {
            if coordinate < 0.0 {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 0.0, 1.0]
            }
        } else {
            weights.map(|w| w / total)
        };
        let grit = grain.get([
            x as f64 / args.grain_scale * 1.7,
            y as f64 / args.grain_scale * 1.7,
            grain_z,
            grain_w,
        ]) * args.grain_amount;
        let mut rgb = [0.0; 3];
        for channel in 0..3 {
            rgb[channel] = ((0..3)
                .map(|i| colors[i][channel] as f64 / 255.0 * weights[i])
                .sum::<f64>()
                - 0.5)
                * args.contrast()
                + 0.5;
        }
        let luminance = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
        Rgb(rgb.map(|c| {
            let c = (luminance + (c - luminance) * args.saturation)
                .max(0.0)
                .powf(1.0 / args.gamma);
            let c = if args.light_mode { 0.65 + c * 0.35 } else { c };
            ((c + grit).clamp(0.0, 1.0) * 255.0).round() as u8
        }))
    })
}
