use resvg::{tiny_skia, usvg};
use std::path::Path;

const SAMPLES: u32 = 4;
const BADGE_CENTER: f32 = 0.74;
const BADGE_RADIUS: f32 = 0.22;
const BADGE_GAP: f32 = 1.28;

fn coverage(x: u32, y: u32, cx: f32, cy: f32, radius: f32) -> f32 {
    let mut inside = 0u32;
    for sy in 0..SAMPLES {
        for sx in 0..SAMPLES {
            let px = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32 - cx;
            let py = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32 - cy;
            if px * px + py * py <= radius * radius {
                inside += 1;
            }
        }
    }
    inside as f32 / (SAMPLES * SAMPLES) as f32
}

fn disc(size: u32, rgb: [u8; 3]) -> Vec<u8> {
    let center = size as f32 / 2.0;
    let radius = size as f32 * 0.40;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let alpha = (coverage(x, y, center, center, radius) * 255.0) as u8;
            rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
        }
    }
    rgba
}

fn to_argb(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4).flat_map(|p| [p[3], p[0], p[1], p[2]]).collect()
}

fn encode(size: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let Ok(mut writer) = encoder.write_header() else {
            return Vec::new();
        };
        if writer.write_image_data(rgba).is_err() {
            return Vec::new();
        }
    }
    out
}

pub fn argb(size: u32, rgb: [u8; 3]) -> Vec<u8> {
    to_argb(&disc(size, rgb))
}

pub fn png(size: u32, rgb: [u8; 3]) -> Vec<u8> {
    encode(size, &disc(size, rgb))
}

pub struct Logo {
    layers: Vec<(u32, Vec<u8>)>,
}

impl Logo {
    pub fn load(path: &Path, sizes: &[u32]) -> Result<Logo, String> {
        let data = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let tree = usvg::Tree::from_data(&data, &usvg::Options::default())
            .map_err(|e| format!("{} is not an SVG livedot can draw: {e}", path.display()))?;
        let extent = tree.size().width().max(tree.size().height());
        let mut layers = Vec::with_capacity(sizes.len());
        for size in sizes {
            let mut pixmap = tiny_skia::Pixmap::new(*size, *size)
                .ok_or_else(|| format!("cannot allocate a {size}px icon"))?;
            let scale = *size as f32 / extent;
            let dx = (*size as f32 - tree.size().width() * scale) / 2.0;
            let dy = (*size as f32 - tree.size().height() * scale) / 2.0;
            let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy);
            resvg::render(&tree, transform, &mut pixmap.as_mut());
            layers.push((*size, pixmap.take()));
        }
        Ok(Logo { layers })
    }

    fn badged(&self, size: u32, rgb: [u8; 3]) -> Option<Vec<u8>> {
        let (_, layer) = self.layers.iter().find(|(s, _)| *s == size)?;
        let center = size as f32 * BADGE_CENTER;
        let radius = size as f32 * BADGE_RADIUS;
        let mut rgba = Vec::with_capacity(layer.len());
        for y in 0..size {
            for x in 0..size {
                let at = ((y * size + x) * 4) as usize;
                let keep = 1.0 - coverage(x, y, center, center, radius * BADGE_GAP);
                let badge = coverage(x, y, center, center, radius);
                let mut pixel = [0f32; 4];
                for channel in 0..4 {
                    pixel[channel] = layer[at + channel] as f32 * keep * (1.0 - badge);
                }
                for channel in 0..3 {
                    pixel[channel] += rgb[channel] as f32 * badge;
                }
                pixel[3] += 255.0 * badge;
                let alpha = pixel[3];
                for value in pixel.iter().take(3) {
                    let straight = if alpha > 0.0 { value * 255.0 / alpha } else { 0.0 };
                    rgba.push(straight.round().min(255.0) as u8);
                }
                rgba.push(alpha.round().min(255.0) as u8);
            }
        }
        Some(rgba)
    }

    pub fn argb(&self, size: u32, rgb: [u8; 3]) -> Vec<u8> {
        self.badged(size, rgb).map(|rgba| to_argb(&rgba)).unwrap_or_else(|| argb(size, rgb))
    }

    pub fn png(&self, size: u32, rgb: [u8; 3]) -> Vec<u8> {
        self.badged(size, rgb).map(|rgba| encode(size, &rgba)).unwrap_or_else(|| png(size, rgb))
    }

    pub fn plain_argb(&self, size: u32) -> Option<Vec<u8>> {
        let (_, layer) = self.layers.iter().find(|(s, _)| *s == size)?;
        Some(to_argb(layer))
    }
}
