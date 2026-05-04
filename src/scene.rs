use std::collections::HashMap;

use anyhow::Result;
use eframe::egui::ColorImage;
use resvg::tiny_skia::Pixmap;

use crate::model::{MapDocument, RasterKind, WorldBounds};
use crate::theme::theme;

#[derive(Debug, Clone)]
pub struct RasterLayer {
    pub kind: RasterKind,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub world_bounds: WorldBounds,
    pub opacity: f32,
}

impl RasterLayer {
    pub fn to_color_image(&self) -> ColorImage {
        ColorImage::from_rgba_unmultiplied([self.width as usize, self.height as usize], &self.pixels)
    }

    pub fn to_png_bytes(&self) -> Result<Vec<u8>> {
        let mut pixmap = Pixmap::new(self.width, self.height).ok_or_else(|| anyhow::anyhow!("failed to allocate raster pixmap"))?;
        let data = pixmap.data_mut();
        for (index, chunk) in self.pixels.chunks_exact(4).enumerate() {
            let alpha = chunk[3] as u16;
            let base = index * 4;
            data[base] = ((chunk[0] as u16 * alpha + 127) / 255) as u8;
            data[base + 1] = ((chunk[1] as u16 * alpha + 127) / 255) as u8;
            data[base + 2] = ((chunk[2] as u16 * alpha + 127) / 255) as u8;
            data[base + 3] = alpha as u8;
        }
        Ok(pixmap.encode_png()?)
    }
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub document: MapDocument,
    pub bounds: WorldBounds,
    pub segment_index: HashMap<u64, usize>,
    pub node_index: HashMap<u64, usize>,
    pub terrain: Option<RasterLayer>,
    pub forests: Option<RasterLayer>,
}

pub fn build_scene(document: MapDocument) -> Scene {
    let mut bounds = if document.bounds.is_empty() {
        let mut bounds = WorldBounds::default();
        for node in &document.nodes {
            bounds.include_point(node.position);
        }
        for segment in &document.segments {
            bounds.include_bounds(&segment.bounds);
        }
        for area in &document.buildings {
            bounds.include_bounds(&area.bounds);
        }
        for area in &document.parks {
            bounds.include_bounds(&area.bounds);
        }
        for district in &document.districts {
            bounds.include_point(district.anchor);
        }
        bounds
    } else {
        document.bounds
    };

    let terrain = document
        .terrain
        .as_ref()
        .and_then(|source| decode_terrain_layer(source, bounds, document.metadata.sea_level));
    let forests = document
        .forests
        .as_ref()
        .and_then(|source| decode_forest_layer(source, bounds));

    if bounds.is_empty() {
        if let Some(terrain_surface) = terrain.as_ref() {
            bounds = terrain_surface.world_bounds;
        } else if let Some(forest_surface) = forests.as_ref() {
            bounds = forest_surface.world_bounds;
        }
    }
    if let Some(terrain_surface) = terrain.as_ref() {
        bounds.include_bounds(&terrain_surface.world_bounds);
    }
    if let Some(forest_layer) = forests.as_ref() {
        bounds.include_bounds(&forest_layer.world_bounds);
    }

    let segment_index = document
        .segments
        .iter()
        .enumerate()
        .map(|(index, segment)| (segment.id, index))
        .collect::<HashMap<_, _>>();
    let node_index = document
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id, index))
        .collect::<HashMap<_, _>>();

    Scene {
        document,
        bounds,
        segment_index,
        node_index,
        terrain,
        forests,
    }
}

fn decode_terrain_layer(source: &crate::model::RasterSource, world_bounds: WorldBounds, sea_level: f32) -> Option<RasterLayer> {
    let tokens = source
        .packed
        .split(|ch: char| matches!(ch, ',' | ':' | '\n' | '\r' | '\t' | ' '))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    if tokens.len() < 2 {
        return None;
    }

    let pair_count = tokens.len() / 2;
    let side = (pair_count as f64).sqrt().round() as usize;
    if side == 0 || side.saturating_mul(side) > pair_count {
        return None;
    }

    let world_bounds = if world_bounds.is_empty() {
        WorldBounds::from_corners(0.0, 0.0, side as f32, side as f32)
    } else {
        world_bounds
    };

    let mut pixels = vec![0u8; side * side * 4];
    for index in 0..(side * side) {
        let height = tokens[index * 2].parse::<f32>().unwrap_or(sea_level) * 0.01;
        let color = terrain_color(height, sea_level);
        let base = index * 4;
        pixels[base] = color.r;
        pixels[base + 1] = color.g;
        pixels[base + 2] = color.b;
        pixels[base + 3] = color.a;
    }

    Some(RasterLayer {
        kind: source.kind,
        width: side as u32,
        height: side as u32,
        pixels,
        world_bounds,
        opacity: 1.0,
    })
}

fn decode_forest_layer(source: &crate::model::RasterSource, world_bounds: WorldBounds) -> Option<RasterLayer> {
    let rows = source
        .rows
        .iter()
        .map(|row| {
            row.split(',')
                .map(|cell| cell.trim().parse::<f32>().unwrap_or(0.0).max(0.0))
                .collect::<Vec<_>>()
        })
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();

    let height = rows.len();
    let width = rows.iter().map(|row| row.len()).max().unwrap_or(0);
    if width == 0 || height == 0 {
        return None;
    }

    let world_bounds = if world_bounds.is_empty() {
        WorldBounds::from_corners(0.0, 0.0, width as f32, height as f32)
    } else {
        world_bounds
    };

    let mut pixels = vec![0u8; width * height * 4];
    for (y, row) in rows.iter().enumerate() {
        for (x, density) in row.iter().enumerate() {
            let color = forest_overlay_color(*density);
            let base = (y * width + x) * 4;
            pixels[base] = color.r;
            pixels[base + 1] = color.g;
            pixels[base + 2] = color.b;
            pixels[base + 3] = color.a;
        }
    }

    let forest_theme = &theme().forest;
    if forest_theme.blur_radius > 0 && forest_theme.blur_passes > 0 {
        blur_rgba(&mut pixels, width, height, forest_theme.blur_radius, forest_theme.blur_passes);
    }

    Some(RasterLayer {
        kind: source.kind,
        width: width as u32,
        height: height as u32,
        pixels,
        world_bounds,
        opacity: 1.0,
    })
}

fn terrain_color(height: f32, sea_level: f32) -> crate::model::RgbaColor {
    let terrain = &theme().terrain;
    if height < sea_level {
        let depth = ((sea_level - height) / 80.0).clamp(0.0, 1.0);
        return mix_color(
            terrain.water_light.to_color(),
            terrain.water_dark.to_color(),
            depth,
        );
    }

    let relative = ((height - sea_level) / 260.0).clamp(0.0, 1.0).powf(0.9);
    let low = terrain.land_low.to_color();
    let mid = terrain.land_mid.to_color();
    let high = terrain.land_high.to_color();
    let snow = terrain.snow.to_color();

    if relative < 0.45 {
        mix_color(low, mid, relative / 0.45)
    } else if relative < 0.82 {
        mix_color(mid, high, (relative - 0.45) / 0.37)
    } else {
        mix_color(high, snow, ((relative - 0.82) / 0.18).clamp(0.0, 1.0))
    }
}

fn forest_overlay_color(density: f32) -> crate::model::RgbaColor {
    let forest = &theme().forest;
    if density <= f32::EPSILON {
        return crate::model::RgbaColor::rgba(0, 0, 0, 0);
    }

    let alpha = (density * forest.opacity).round().clamp(0.0, 255.0) as u8;
    let color = forest.color.to_color();
    crate::model::RgbaColor::rgba(color.r, color.g, color.b, alpha)
}

fn blur_rgba(pixels: &mut [u8], width: usize, height: usize, radius: usize, passes: usize) {
    if width == 0 || height == 0 || radius == 0 || passes == 0 {
        return;
    }

    let mut source = pixels.to_vec();
    let mut target = vec![0u8; pixels.len()];
    for _ in 0..passes {
        box_blur_horizontal(&source, &mut target, width, height, radius);
        box_blur_vertical(&target, &mut source, width, height, radius);
    }

    pixels.copy_from_slice(&source);
}

fn box_blur_horizontal(source: &[u8], target: &mut [u8], width: usize, height: usize, radius: usize) {
    let kernel = (radius * 2 + 1) as u32;
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0u32; 4];
            for offset in x.saturating_sub(radius)..=(x + radius).min(width - 1) {
                let base = (y * width + offset) * 4;
                sum[0] += source[base] as u32;
                sum[1] += source[base + 1] as u32;
                sum[2] += source[base + 2] as u32;
                sum[3] += source[base + 3] as u32;
            }
            let base = (y * width + x) * 4;
            target[base] = (sum[0] / kernel) as u8;
            target[base + 1] = (sum[1] / kernel) as u8;
            target[base + 2] = (sum[2] / kernel) as u8;
            target[base + 3] = (sum[3] / kernel) as u8;
        }
    }
}

fn box_blur_vertical(source: &[u8], target: &mut [u8], width: usize, height: usize, radius: usize) {
    let kernel = (radius * 2 + 1) as u32;
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0u32; 4];
            for offset in y.saturating_sub(radius)..=(y + radius).min(height - 1) {
                let base = (offset * width + x) * 4;
                sum[0] += source[base] as u32;
                sum[1] += source[base + 1] as u32;
                sum[2] += source[base + 2] as u32;
                sum[3] += source[base + 3] as u32;
            }
            let base = (y * width + x) * 4;
            target[base] = (sum[0] / kernel) as u8;
            target[base + 1] = (sum[1] / kernel) as u8;
            target[base + 2] = (sum[2] / kernel) as u8;
            target[base + 3] = (sum[3] / kernel) as u8;
        }
    }
}

fn mix_color(start: crate::model::RgbaColor, end: crate::model::RgbaColor, t: f32) -> crate::model::RgbaColor {
    let t = t.clamp(0.0, 1.0);
    let lerp_channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8;

    crate::model::RgbaColor::rgba(
        lerp_channel(start.r, end.r),
        lerp_channel(start.g, end.g),
        lerp_channel(start.b, end.b),
        lerp_channel(start.a, end.a),
    )
}

