use crate::{Args, Color};
use image::{ImageBuffer, Rgb, RgbImage};
use imageproc::filter::gaussian_blur_f32;
use noise::{NoiseFn, OpenSimplex};

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
pub(crate) fn generate_frame(
    args: &Args,
    colors: &[Color],
    frame_idx: usize,
    total_frames: usize,
) -> RgbImage {
    let Args {
        width,
        height,
        pixelation,
        speed,
        turbulence,
        scale,
        ..
    } = *args;
    let buffer_width = width.div_ceil(pixelation);
    let buffer_height = height.div_ceil(pixelation);

    // IMPORTANT: Use constant seeds for all frames to ensure continuity
    // We only want the time to change, not the noise pattern itself
    let noise1 = OpenSimplex::new(1234);
    let noise2 = OpenSimplex::new(5678);
    let noise_turb = OpenSimplex::new(9012);

    // Calculate time value for looping animation
    let cycle_length = total_frames as f64;
    let normalized_frame = frame_idx as f64 / cycle_length;
    let time_val = normalized_frame
        * 2.0
        * std::f64::consts::PI
        * if args.seamless_loop && speed != 0.0 {
            speed.signum() * speed.abs().round().max(1.0)
        } else {
            speed
        };

    // Create the small pixelated buffer
    let mut buffer: RgbImage = ImageBuffer::new(buffer_width, buffer_height);

    // Generate each pixel in the buffer
    for y in 0..buffer_height {
        for x in 0..buffer_width {
            let nx = x as f64 * scale;
            let ny = y as f64 * scale;

            // Use layered noise with different frequencies to create more stable patterns
            // Primary flow field - very slow movement
            let flow_x =
                noise1.get([nx * 0.5, ny * 0.5, time_val.sin() * 0.5]) * 0.7 + time_val.cos() * 0.2;
            let flow_y =
                noise2.get([nx * 0.5, ny * 0.5, time_val.cos() * 0.5]) * 0.7 + time_val.sin() * 0.2;

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
    image::imageops::resize(
        &blurred,
        width,
        height,
        image::imageops::FilterType::Triangle,
    )
}
