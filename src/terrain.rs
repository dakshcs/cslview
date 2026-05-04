use std::fmt::Write;

use eframe::egui::{self, Color32, Pos2};

use crate::model::{RasterSource, RgbaColor, WorldBounds, WorldPoint};
use crate::viewport::{polygon_points, ViewportTransform};

const TERRAIN_VALUE_SCALE: f32 = 0.01;

#[derive(Debug, Clone)]
pub struct TerrainSurface {
    pub world_bounds: WorldBounds,
    pub sea_level: f32,
    width: usize,
    height: usize,
    heights: Vec<f32>,
    compares: Vec<f32>,
    height_max: f32,
}

#[derive(Debug, Clone)]
pub struct TerrainRenderGrid {
    pub columns: usize,
    pub rows: usize,
    pub vertices: Vec<TerrainVertex>,
}

#[derive(Debug, Clone, Copy)]
pub struct TerrainVertex {
    pub position: WorldPoint,
    pub height: f32,
    pub compare: f32,
    pub color: RgbaColor,
}

impl TerrainSurface {
    pub fn from_source(source: &RasterSource, world_bounds: WorldBounds, sea_level: f32) -> Option<Self> {
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

        let mut heights = Vec::with_capacity(side * side);
        let mut compares = Vec::with_capacity(side * side);
        let mut height_max = f32::NEG_INFINITY;

        for index in 0..(side * side) {
            let height = tokens[index * 2].parse::<f32>().unwrap_or(sea_level) * TERRAIN_VALUE_SCALE;
            let compare = tokens[index * 2 + 1].parse::<f32>().unwrap_or(0.0) * TERRAIN_VALUE_SCALE;
            height_max = height_max.max(height);
            heights.push(height);
            compares.push(compare);
        }

        Some(Self {
            world_bounds,
            sea_level,
            width: side,
            height: side,
            heights,
            compares,
            height_max,
        })
    }

    pub fn suggested_interactive_resolution(zoom: f32) -> usize {
        (128.0 * zoom.sqrt().clamp(0.85, 2.0)).round().clamp(96.0, 256.0) as usize
    }

    pub fn suggested_export_resolution(width: u32, height: u32) -> usize {
        width.max(height).max(2) as usize
    }

    pub fn sample_grid(&self, region: WorldBounds, columns: usize) -> TerrainRenderGrid {
        let columns = columns.max(2);
        let region_width = region.width().max(1.0);
        let region_height = region.height().max(1.0);
        let rows = ((columns as f32) * (region_height / region_width))
            .round()
            .clamp(2.0, (columns * 2) as f32) as usize;

        let mut vertices = Vec::with_capacity((columns + 1) * (rows + 1));

        for row in 0..=rows {
            let row_fraction = row as f32 / rows as f32;
            let world_y = region.max_y - row_fraction * region_height;
            for column in 0..=columns {
                let column_fraction = column as f32 / columns as f32;
                let world_x = region.min_x + column_fraction * region_width;
                let position = WorldPoint::new(world_x, world_y);
                let (height, compare) = self.sample_world(position);
                let color = self.color_for(height, compare);
                vertices.push(TerrainVertex {
                    position,
                    height,
                    compare,
                    color,
                });
            }
        }

        TerrainRenderGrid {
            columns,
            rows,
            vertices,
        }
    }

    fn sample_world(&self, point: WorldPoint) -> (f32, f32) {
        let width = self.world_bounds.width().max(1.0);
        let height = self.world_bounds.height().max(1.0);
        let u = ((point.x - self.world_bounds.min_x) / width).clamp(0.0, 1.0);
        let v = ((self.world_bounds.max_y - point.y) / height).clamp(0.0, 1.0);

        let sample_x = u * (self.width.saturating_sub(1)) as f32;
        let sample_y = v * (self.height.saturating_sub(1)) as f32;
        let x0 = sample_x.floor() as usize;
        let y0 = sample_y.floor() as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = sample_x - x0 as f32;
        let ty = sample_y - y0 as f32;

        let h00 = self.height_at(x0, y0);
        let h10 = self.height_at(x1, y0);
        let h01 = self.height_at(x0, y1);
        let h11 = self.height_at(x1, y1);
        let c00 = self.compare_at(x0, y0);
        let c10 = self.compare_at(x1, y0);
        let c01 = self.compare_at(x0, y1);
        let c11 = self.compare_at(x1, y1);

        let height = lerp(lerp(h00, h10, tx), lerp(h01, h11, tx), ty);
        let compare = lerp(lerp(c00, c10, tx), lerp(c01, c11, tx), ty);
        (height, compare)
    }

    fn height_at(&self, x: usize, y: usize) -> f32 {
        self.heights[y * self.width + x]
    }

    fn compare_at(&self, x: usize, y: usize) -> f32 {
        self.compares[y * self.width + x]
    }

    fn color_for(&self, height: f32, compare: f32) -> RgbaColor {
        if height < self.sea_level {
            let depth = ((self.sea_level - height) / 70.0).clamp(0.0, 1.0);
            return mix_color(
                RgbaColor::rgb(56, 132, 210),
                RgbaColor::rgb(14, 72, 160),
                depth,
            );
        }

        let land_span = (self.height_max - self.sea_level).max(220.0);
        let relative = ((height - self.sea_level) / land_span).clamp(0.0, 1.0).powf(1.22);
        let mut color = if relative < 0.18 {
            mix_color(RgbaColor::rgb(64, 138, 72), RgbaColor::rgb(92, 162, 80), relative / 0.18)
        } else if relative < 0.36 {
            mix_color(
                RgbaColor::rgb(92, 162, 80),
                RgbaColor::rgb(126, 170, 84),
                (relative - 0.18) / 0.18,
            )
        } else if relative < 0.58 {
            mix_color(
                RgbaColor::rgb(126, 170, 84),
                RgbaColor::rgb(166, 154, 88),
                (relative - 0.36) / 0.22,
            )
        } else if relative < 0.78 {
            mix_color(
                RgbaColor::rgb(166, 154, 88),
                RgbaColor::rgb(170, 138, 92),
                (relative - 0.58) / 0.20,
            )
        } else if relative < 0.92 {
            mix_color(
                RgbaColor::rgb(170, 138, 92),
                RgbaColor::rgb(186, 186, 188),
                (relative - 0.78) / 0.14,
            )
        } else {
            mix_color(
                RgbaColor::rgb(186, 186, 188),
                RgbaColor::rgb(244, 244, 248),
                ((relative - 0.92) / 0.08).clamp(0.0, 1.0),
            )
        };

        if compare > 0.0 {
            let shading = ((height - compare).abs() / 120.0).clamp(0.0, 1.0);
            color = mix_color(color, RgbaColor::rgb(34, 34, 38), shading * 0.12);
        }

        color
    }
}

impl TerrainRenderGrid {
    pub fn to_mesh<F>(&self, map_point: F) -> egui::Mesh
    where
        F: Fn(WorldPoint) -> Pos2,
    {
        let mut mesh = egui::Mesh::default();
        mesh.reserve_vertices(self.vertices.len());
        mesh.reserve_triangles(self.columns * self.rows * 2);

        for vertex in &self.vertices {
            mesh.colored_vertex(map_point(vertex.position), to_color32(vertex.color));
        }

        let stride = self.columns + 1;
        for row in 0..self.rows {
            for column in 0..self.columns {
                let top_left = (row * stride + column) as u32;
                let top_right = top_left + 1;
                let bottom_left = top_left + stride as u32;
                let bottom_right = bottom_left + 1;
                mesh.add_triangle(top_left, top_right, bottom_right);
                mesh.add_triangle(top_left, bottom_right, bottom_left);
            }
        }

        mesh
    }

    pub fn to_svg(&self, transform: &ViewportTransform, group_id: &str) -> String {
        let mut output = String::new();
        let _ = write!(output, "<g id=\"{}\">", group_id);

        let stride = self.columns + 1;
        for row in 0..self.rows {
            for column in 0..self.columns {
                let top_left = &self.vertices[row * stride + column];
                let top_right = &self.vertices[row * stride + column + 1];
                let bottom_left = &self.vertices[(row + 1) * stride + column];
                let bottom_right = &self.vertices[(row + 1) * stride + column + 1];

                let fill = average_color([
                    top_left.color,
                    top_right.color,
                    bottom_right.color,
                    bottom_left.color,
                ]);
                let opacity = (fill.a as f32 / 255.0).clamp(0.0, 1.0);
                let points = [
                    transform.map(top_left.position),
                    transform.map(top_right.position),
                    transform.map(bottom_right.position),
                    transform.map(bottom_left.position),
                ];

                let _ = writeln!(
                    output,
                    "<polygon points=\"{}\" fill=\"{}\" fill-opacity=\"{:.3}\"/>",
                    polygon_points(&points),
                    fill.to_hex(),
                    opacity,
                );
            }
        }

        output.push_str("</g>");
        output
    }
}

        #[derive(Debug, Clone)]
        pub struct ForestSurface {
            pub world_bounds: WorldBounds,
            width: usize,
            height: usize,
            densities: Vec<f32>,
            density_max: f32,
        }

        impl ForestSurface {
            pub fn from_source(source: &RasterSource, world_bounds: WorldBounds) -> Option<Self> {
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

                let mut densities = vec![0.0; width * height];
                let mut density_max: f32 = 0.0;
                for (y, row) in rows.iter().enumerate() {
                    for (x, density) in row.iter().enumerate() {
                        density_max = density_max.max(*density);
                        densities[y * width + x] = *density;
                    }
                }

                Some(Self {
                    world_bounds,
                    width,
                    height,
                    densities,
                    density_max,
                })
            }

            pub fn sample_grid(&self, region: WorldBounds, columns: usize) -> TerrainRenderGrid {
                let columns = columns.max(2);
                let region_width = region.width().max(1.0);
                let region_height = region.height().max(1.0);
                let rows = ((columns as f32) * (region_height / region_width))
                    .round()
                    .clamp(2.0, (columns * 2) as f32) as usize;

                let mut vertices = Vec::with_capacity((columns + 1) * (rows + 1));

                for row in 0..=rows {
                    let row_fraction = row as f32 / rows as f32;
                    let world_y = region.max_y - row_fraction * region_height;
                    for column in 0..=columns {
                        let column_fraction = column as f32 / columns as f32;
                        let world_x = region.min_x + column_fraction * region_width;
                        let position = WorldPoint::new(world_x, world_y);
                        let density = self.sample_world(position);
                        let color = self.color_for(density);
                        vertices.push(TerrainVertex {
                            position,
                            height: density,
                            compare: 0.0,
                            color,
                        });
                    }
                }

                TerrainRenderGrid {
                    columns,
                    rows,
                    vertices,
                }
            }

            fn sample_world(&self, point: WorldPoint) -> f32 {
                let width = self.world_bounds.width().max(1.0);
                let height = self.world_bounds.height().max(1.0);
                let u = ((point.x - self.world_bounds.min_x) / width).clamp(0.0, 1.0);
                let v = ((self.world_bounds.max_y - point.y) / height).clamp(0.0, 1.0);

                let sample_x = u * (self.width.saturating_sub(1)) as f32;
                let sample_y = v * (self.height.saturating_sub(1)) as f32;
                let x0 = sample_x.floor() as usize;
                let y0 = sample_y.floor() as usize;
                let x1 = (x0 + 1).min(self.width - 1);
                let y1 = (y0 + 1).min(self.height - 1);
                let tx = sample_x - x0 as f32;
                let ty = sample_y - y0 as f32;

                let d00 = self.density_at(x0, y0);
                let d10 = self.density_at(x1, y0);
                let d01 = self.density_at(x0, y1);
                let d11 = self.density_at(x1, y1);

                lerp(lerp(d00, d10, tx), lerp(d01, d11, tx), ty)
            }

            fn density_at(&self, x: usize, y: usize) -> f32 {
                self.densities[y * self.width + x]
            }

            fn color_for(&self, density: f32) -> RgbaColor {
                if self.density_max <= f32::EPSILON || density <= f32::EPSILON {
                    return RgbaColor::rgba(0, 0, 0, 0);
                }

                let intensity = (density / self.density_max).clamp(0.0, 1.0).powf(0.72);
                mix_color(
                    RgbaColor::rgba(36, 90, 38, 0),
                    RgbaColor::rgba(62, 134, 54, 210),
                    intensity,
                )
            }
        }

fn lerp(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t.clamp(0.0, 1.0)
}

fn mix_color(start: RgbaColor, end: RgbaColor, t: f32) -> RgbaColor {
    let t = t.clamp(0.0, 1.0);
    let lerp_channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8;

    RgbaColor::rgba(
        lerp_channel(start.r, end.r),
        lerp_channel(start.g, end.g),
        lerp_channel(start.b, end.b),
        lerp_channel(start.a, end.a),
    )
}

fn average_color(colors: [RgbaColor; 4]) -> RgbaColor {
    let mut total_r = 0u32;
    let mut total_g = 0u32;
    let mut total_b = 0u32;
    let mut total_a = 0u32;

    for color in colors {
        total_r += color.r as u32;
        total_g += color.g as u32;
        total_b += color.b as u32;
        total_a += color.a as u32;
    }

    RgbaColor::rgba(
        (total_r / 4) as u8,
        (total_g / 4) as u8,
        (total_b / 4) as u8,
        (total_a / 4) as u8,
    )
}

fn to_color32(color: RgbaColor) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}