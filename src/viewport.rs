use crate::model::{WorldBounds, WorldPoint};

#[derive(Debug, Clone, Copy)]
pub struct ViewportTransform {
    pub world: WorldBounds,
    pub width: f32,
    pub height: f32,
    pub padding: f32,
    pub zoom: f32,
}

impl ViewportTransform {
    pub fn new(world: WorldBounds, width: u32, height: u32, padding: f32, zoom: f32) -> Self {
        Self {
            world,
            width: width as f32,
            height: height as f32,
            padding,
            zoom,
        }
    }

    pub fn with_zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }

    pub fn scale(&self) -> f32 {
        let usable_width = (self.width - self.padding * 2.0).max(1.0);
        let usable_height = (self.height - self.padding * 2.0).max(1.0);
        let scale_x = usable_width / self.world.width();
        let scale_y = usable_height / self.world.height();
        scale_x.min(scale_y) * self.zoom.max(0.001)
    }

    pub fn fit_margin(&self) -> (f32, f32, f32) {
        let scale = self.scale();
        let draw_width = self.world.width() * scale;
        let draw_height = self.world.height() * scale;
        let offset_x = (self.width - draw_width) * 0.5;
        let offset_y = (self.height - draw_height) * 0.5;
        (scale, offset_x, offset_y)
    }

    pub fn map(&self, point: WorldPoint) -> (f32, f32) {
        let (scale, offset_x, offset_y) = self.fit_margin();
        let x = offset_x + (point.x - self.world.min_x) * scale;
        let y = offset_y + (self.world.max_y - point.y) * scale;
        (x, y)
    }

    pub fn map_size(&self, value: f32) -> f32 {
        value * self.scale()
    }

    pub fn screen_rect(&self) -> String {
        format!("0 0 {} {}", self.width, self.height)
    }
}

pub fn polyline_path(points: &[(f32, f32)]) -> String {
    let mut output = String::new();
    for (index, (x, y)) in points.iter().enumerate() {
        if index == 0 {
            output.push_str(&format!("M{:.2} {:.2}", x, y));
        } else {
            output.push_str(&format!(" L{:.2} {:.2}", x, y));
        }
    }
    output
}

pub fn polygon_points(points: &[(f32, f32)]) -> String {
    points
        .iter()
        .map(|(x, y)| format!("{:.2},{:.2}", x, y))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn distance_point_to_segment(point: WorldPoint, start: WorldPoint, end: WorldPoint) -> f32 {
    let segment_x = end.x - start.x;
    let segment_y = end.y - start.y;
    let length_squared = segment_x * segment_x + segment_y * segment_y;

    if length_squared <= f32::EPSILON {
        return point.distance(start);
    }

    let projection = ((point.x - start.x) * segment_x + (point.y - start.y) * segment_y) / length_squared;
    let projection = projection.clamp(0.0, 1.0);
    let closest = WorldPoint::new(start.x + projection * segment_x, start.y + projection * segment_y);
    point.distance(closest)
}

pub fn distance_point_to_polyline(point: WorldPoint, polyline: &[WorldPoint]) -> f32 {
    if polyline.len() < 2 {
        return polyline.first().copied().map(|start| point.distance(start)).unwrap_or(f32::INFINITY);
    }

    let mut best = f32::INFINITY;
    for pair in polyline.windows(2) {
        best = best.min(distance_point_to_segment(point, pair[0], pair[1]));
    }
    best
}

pub fn polygon_centroid(points: &[WorldPoint]) -> Option<WorldPoint> {
    if points.is_empty() {
        return None;
    }

    let mut area = 0.0;
    let mut centroid_x = 0.0;
    let mut centroid_y = 0.0;

    for index in 0..points.len() {
        let p1 = points[index];
        let p2 = points[(index + 1) % points.len()];
        let cross = p1.x * p2.y - p2.x * p1.y;
        area += cross;
        centroid_x += (p1.x + p2.x) * cross;
        centroid_y += (p1.y + p2.y) * cross;
    }

    if area.abs() < f32::EPSILON {
        let sum_x = points.iter().map(|point| point.x).sum::<f32>() / points.len() as f32;
        let sum_y = points.iter().map(|point| point.y).sum::<f32>() / points.len() as f32;
        return Some(WorldPoint::new(sum_x, sum_y));
    }

    let area = area * 0.5;
    Some(WorldPoint::new(centroid_x / (6.0 * area), centroid_y / (6.0 * area)))
}
