use std::collections::BTreeMap;
use std::f32::consts::TAU;
use std::path::{Path, PathBuf};

use anyhow::Result;
use eframe::egui::{self, Color32, FontId, PointerButton, Pos2, Rect, Sense, Stroke, Vec2};

use crate::export::{
    export_to_file, DistrictMode, DistrictSetting, ExportKind, ExportSettings, LayerSetting,
    LayerSettings, RoadLayers, TransitLayers,
};
use crate::model::{AreaRecord, SegmentRecord, WorldBounds, WorldPoint};
use crate::parser::parse_csl_file;
use crate::scene::{build_scene, RasterLayer, Scene};
use crate::theme::theme;
use crate::style::{elevation_state, node_style, park_path_style, park_style, segment_style};
use crate::viewport::distance_point_to_polyline;

const NODE_HIDE_SCREEN_RADIUS: f32 = 3.0;
const NODE_LABEL_SCREEN_RADIUS: f32 = 7.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selection {
    Node(u64),
    Segment(u64),
    Building(u64),
    Park(u64),
    District(u64),
}

#[derive(Debug, Clone)]
struct Camera {
    center: WorldPoint,
    zoom: f32,
    rotation: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: WorldPoint::new(0.0, 0.0),
            zoom: 1.0,
            rotation: 0.0,
        }
    }
}

impl Camera {
    fn fit_to(&mut self, bounds: WorldBounds) {
        self.center = bounds.center();
        self.zoom = 1.0;
        self.rotation = 0.0;
    }

    fn fit_scale(&self, bounds: WorldBounds, rect: Rect) -> f32 {
        let available_width = rect.width().max(1.0);
        let available_height = rect.height().max(1.0);
        let scale_x = available_width / bounds.width();
        let scale_y = available_height / bounds.height();
        scale_x.min(scale_y) * self.zoom.max(0.001)
    }

    fn world_to_screen(&self, world: WorldPoint, bounds: WorldBounds, rect: Rect) -> Pos2 {
        let scale = self.fit_scale(bounds, rect);
        let center = rect.center();
        let offset_x = world.x - self.center.x;
        let offset_y = world.y - self.center.y;
        let sin = self.rotation.sin();
        let cos = self.rotation.cos();
        let rotated_x = offset_x * cos - offset_y * sin;
        let rotated_y = offset_x * sin + offset_y * cos;
        Pos2::new(
            center.x + rotated_x * scale,
            center.y - rotated_y * scale,
        )
    }

    fn screen_to_world(&self, screen: Pos2, bounds: WorldBounds, rect: Rect) -> WorldPoint {
        let scale = self.fit_scale(bounds, rect);
        let center = rect.center();
        let offset_x = (screen.x - center.x) / scale;
        let offset_y = -(screen.y - center.y) / scale;
        let sin = self.rotation.sin();
        let cos = self.rotation.cos();
        let world_x = offset_x * cos + offset_y * sin;
        let world_y = -offset_x * sin + offset_y * cos;
        WorldPoint::new(self.center.x + world_x, self.center.y + world_y)
    }

    fn visible_world_bounds(&self, bounds: WorldBounds, rect: Rect) -> WorldBounds {
        let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
        let mut visible = WorldBounds::default();
        for corner in corners {
            visible.include_point(self.screen_to_world(corner, bounds, rect));
        }
        visible
    }

    fn pan(&mut self, delta: Vec2, bounds: WorldBounds, rect: Rect) {
        let scale = self.fit_scale(bounds, rect);
        let offset_x = delta.x / scale;
        let offset_y = -delta.y / scale;
        let sin = self.rotation.sin();
        let cos = self.rotation.cos();
        let world_x = offset_x * cos + offset_y * sin;
        let world_y = -offset_x * sin + offset_y * cos;
        self.center.x -= world_x;
        self.center.y -= world_y;
    }

    fn rotate_by(&mut self, delta: f32) {
        self.rotation = (self.rotation + delta).rem_euclid(TAU);
    }

    fn zoom_at(&mut self, factor: f32, cursor: Pos2, bounds: WorldBounds, rect: Rect) {
        let before = self.screen_to_world(cursor, bounds, rect);
        self.zoom = (self.zoom * factor).clamp(0.05, 50.0);
        let scale = self.fit_scale(bounds, rect);
        let center = rect.center();
        let offset_x = (cursor.x - center.x) / scale;
        let offset_y = -(cursor.y - center.y) / scale;
        let sin = self.rotation.sin();
        let cos = self.rotation.cos();
        let world_x = offset_x * cos + offset_y * sin;
        let world_y = -offset_x * sin + offset_y * cos;
        self.center = WorldPoint::new(before.x - world_x, before.y - world_y);
    }
}

pub struct CslViewApp {
    scene: Option<Scene>,
    camera: Camera,
    layers: LayerSettings,
    roads: RoadLayers,
    transit: TransitLayers,
    districts: BTreeMap<u64, DistrictSetting>,
    export_size: u32,
    current_file: Option<PathBuf>,
    last_error: Option<String>,
    status: String,
    hovered: Option<Selection>,
    selected: Option<Selection>,
    canvas_rect: Option<Rect>,
    terrain_texture: Option<egui::TextureHandle>,
    forest_texture: Option<egui::TextureHandle>,
    export_receiver: Option<std::sync::mpsc::Receiver<Result<(String, String), String>>>,
    //                                                        ^^^^^^  ^^^^^^
    //                                                        status  svg string (for cache)
    export_cache: Option<(u64, String)>,
}

impl Default for CslViewApp {
    fn default() -> Self {
        Self {
            scene: None,
            camera: Camera::default(),
            layers: LayerSettings::default(),
            roads: RoadLayers::default(),
            transit: TransitLayers::default(),
            districts: BTreeMap::new(),
            export_size: 4096,
            current_file: None,
            last_error: None,
            status: String::from("Open a CSL export XML to begin."),
            hovered: None,
            selected: None,
            canvas_rect: None,
            terrain_texture: None,
            forest_texture: None,
            export_receiver: None,
            export_cache: None, 
        }
    }
}

impl CslViewApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        let mut app = Self::default();
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        if let Some(path) = initial_file {
            if let Err(error) = app.load_file(&path) {
                app.last_error = Some(error.to_string());
            }
        }
        app
    }

    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let document = parse_csl_file(path)?;
        let scene = build_scene(document);

        self.camera.fit_to(scene.bounds);
        self.scene = Some(scene);
        self.current_file = Some(path.to_path_buf());
        self.hovered = None;
        self.selected = None;
        self.canvas_rect = None;
        self.rebuild_district_settings();
        self.terrain_texture = None;
        self.forest_texture = None;
        self.last_error = None;
        self.status = format!("Loaded {}", path.display());
        Ok(())
    }

    fn rebuild_district_settings(&mut self) {
        let Some(scene) = self.scene.as_ref() else {
            self.districts.clear();
            return;
        };

        self.districts.retain(|id, _| scene.document.districts.iter().any(|district| district.id == *id));
        for district in &scene.document.districts {
            self.districts.entry(district.id).or_default();
        }
    }

    fn load_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Cities: Skylines XML", &["xml"]).pick_file() else {
            return;
        };

        match self.load_file(&path) {
            Ok(()) => {}
            Err(error) => self.last_error = Some(error.to_string()),
        }
    }

    fn settings_hash(settings: &ExportSettings) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let mut h = DefaultHasher::new();
        // Hash the fields that actually affect SVG output
        settings.width.hash(&mut h);
        settings.height.hash(&mut h);
        settings.layers.terrain.enabled.hash(&mut h);
        settings.layers.roads.enabled.hash(&mut h);
        settings.layers.buildings.enabled.hash(&mut h);
        settings.layers.transit.enabled.hash(&mut h);
        settings.layers.labels.enabled.hash(&mut h);
        // Cast f32 bits to u64 so they're hashable
        (settings.zoom.to_bits() as u64).hash(&mut h);
        (settings.padding.to_bits() as u64).hash(&mut h);
        h.finish()
    }

    fn save_export(&mut self, kind: ExportKind) -> Result<()> {
        if self.export_receiver.is_some() {
            return Err(anyhow::anyhow!("export already in progress"));
        }
        let Some(scene) = self.scene.as_ref() else {
            return Err(anyhow::anyhow!("load a map before exporting"));
        };
        let Some(input_path) = self.current_file.as_ref() else {
            return Err(anyhow::anyhow!("load a file before exporting"));
        };

        let extension = match kind {
            ExportKind::Svg => "svg",
            ExportKind::Png => "png",
            ExportKind::Pdf => "pdf",
        };
        let export_path = input_path.with_extension(extension);

        let mut settings = ExportSettings::default();
        settings.width = self.export_size;
        settings.height = self.export_size;
        settings.padding = 80.0;
        settings.zoom = self.camera.zoom;
        settings.frame = match self.canvas_rect {
            Some(rect) => Some(self.camera.visible_world_bounds(scene.bounds, rect)),
            None => Some(scene.bounds),
        };
        settings.layers = self.layers;
        settings.roads = self.roads;
        settings.transit = self.transit;
        settings.districts = self.districts.clone();

        // Clone what the thread needs — Scene is Clone, so this is safe
        let scene_clone = scene.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        self.export_receiver = Some(receiver);
        self.status = format!("Exporting {}…", extension.to_uppercase());

        let hash = Self::settings_hash(&settings);
        let cached_svg = self.export_cache
            .as_ref()
            .filter(|(cached_hash, _)| *cached_hash == hash)
            .map(|(_, svg)| svg.clone());

        self.export_cache = self.export_cache.take().map(|(_, svg)| (hash, svg));

        std::thread::spawn(move || {
            let result = export_to_file(&scene_clone, kind, &settings, &export_path, cached_svg)
                .map(|svg| (format!("Saved {}", export_path.display()), svg))
                .map_err(|e| e.to_string());
            let _ = sender.send(result);
        });

        Ok(())
    }

    fn ensure_textures(&mut self, ctx: &egui::Context) {
        let Some(scene) = self.scene.as_ref() else {
            return;
        };

        if self.terrain_texture.is_none() {
            if let Some(layer) = scene.terrain.as_ref() {
                self.terrain_texture = Some(ctx.load_texture(
                    "cslview-terrain",
                    layer.to_color_image(),
                    egui::TextureOptions::LINEAR,
                ));
            }
        }

        if self.forest_texture.is_none() {
            if let Some(layer) = scene.forests.as_ref() {
                self.forest_texture = Some(ctx.load_texture(
                    "cslview-forests",
                    layer.to_color_image(),
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
    }

    fn draw_raster_layer(
        painter: &egui::Painter,
        layer: &RasterLayer,
        texture: &egui::TextureHandle,
        camera: &Camera,
        bounds: WorldBounds,
        rect: Rect,
        opacity: f32,
    ) {
        if opacity <= f32::EPSILON || layer.world_bounds.is_empty() {
            return;
        }

        let top_left = camera.world_to_screen(WorldPoint::new(layer.world_bounds.min_x, layer.world_bounds.max_y), bounds, rect);
        let top_right = camera.world_to_screen(WorldPoint::new(layer.world_bounds.max_x, layer.world_bounds.max_y), bounds, rect);
        let bottom_right = camera.world_to_screen(WorldPoint::new(layer.world_bounds.max_x, layer.world_bounds.min_y), bounds, rect);
        let bottom_left = camera.world_to_screen(WorldPoint::new(layer.world_bounds.min_x, layer.world_bounds.min_y), bounds, rect);
        let tint = Color32::from_white_alpha((opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
        let mut mesh = egui::Mesh::default();
        mesh.texture_id = texture.id();
        mesh.vertices.push(egui::epaint::Vertex { pos: top_left, uv: Pos2::new(0.0, 0.0), color: tint });
        mesh.vertices.push(egui::epaint::Vertex { pos: top_right, uv: Pos2::new(1.0, 0.0), color: tint });
        mesh.vertices.push(egui::epaint::Vertex { pos: bottom_right, uv: Pos2::new(1.0, 1.0), color: tint });
        mesh.vertices.push(egui::epaint::Vertex { pos: bottom_left, uv: Pos2::new(0.0, 1.0), color: tint });
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
    }

    fn draw_polyline(
        painter: &egui::Painter,
        points: Vec<Pos2>,
        color: Color32,
        width: f32,
        dash: &[f32],
    ) {
        if points.len() < 2 {
            return;
        }

        if dash.is_empty() {
            painter.add(egui::Shape::line(points, Stroke::new(width, color)));
            return;
        }

        let mut dashed_segments = Vec::new();
        let mut dash_index = 0usize;
        let mut remaining = dash[0];
        let mut drawing = true;

        for pair in points.windows(2) {
            let start = pair[0];
            let end = pair[1];
            let segment = end - start;
            let length = segment.length();
            if length <= f32::EPSILON {
                continue;
            }

            let direction = segment / length;
            let mut traveled = 0.0;
            let mut current = start;

            while traveled < length {
                let step = remaining.min(length - traveled);
                let next = current + direction * step;
                if drawing {
                    dashed_segments.push((current, next));
                }
                traveled += step;
                current = next;
                remaining -= step;

                if remaining <= f32::EPSILON {
                    dash_index = (dash_index + 1) % dash.len();
                    remaining = dash[dash_index].max(1.0);
                    drawing = !drawing;
                }
            }
        }

        for (start, end) in dashed_segments {
            painter.line_segment([start, end], Stroke::new(width, color));
        }
    }

    fn draw_segment(
        painter: &egui::Painter,
        segment: &SegmentRecord,
        camera: &Camera,
        bounds: WorldBounds,
        rect: Rect,
        sea_level: f32,
        opacity: f32,
        selected: bool,
    ) {
        let elevation = elevation_state(
            segment.underground,
            segment.overground,
            segment.elevated_hint,
            segment.points.iter().map(|point| point.elevation).fold(f32::NEG_INFINITY, f32::max),
            sea_level,
        );
        let style = segment_style(&segment.class, elevation);
        let opacity = (style.opacity * opacity).clamp(0.0, 1.0);
        let screen_points = segment
            .points
            .iter()
            .map(|point| camera.world_to_screen(point.position, bounds, rect))
            .collect::<Vec<_>>();

        let scale = camera.fit_scale(bounds, rect);
        let casing_width = (segment.width + style.casing_extra) * scale;
        let body_width = (segment.width * style.body_scale) * scale;
        let casing_color = color_with_opacity(rgba_to_color32(style.casing), opacity);
        let body_color = color_with_opacity(rgba_to_color32(style.body), opacity);
        let selected_boost = if selected { 1.45 } else { 1.0 };

        Self::draw_polyline(painter, screen_points.clone(), casing_color, casing_width * selected_boost, style.dash);
        Self::draw_polyline(painter, screen_points, body_color, body_width * selected_boost, style.dash);
    }

    fn draw_detailed_area(
        painter: &egui::Painter,
        area: &AreaRecord,
        camera: &Camera,
        bounds: WorldBounds,
        rect: Rect,
        fill: Color32,
        stroke: Color32,
        stroke_width: f32,
        opacity: f32,
        inset_factor: f32,
        shadow_offset: Vec2,
    ) {
        if area.points.len() < 3 {
            return;
        }

        let points = area
            .points
            .iter()
            .map(|point| camera.world_to_screen(*point, bounds, rect))
            .collect::<Vec<_>>();
        let shadow_points = points.iter().map(|point| *point + shadow_offset).collect::<Vec<_>>();
        let inset_points = inset_polygon(&points, inset_factor);

        let shadow_fill = color_with_opacity(fill.gamma_multiply(0.45), opacity * 0.55);
        painter.add(egui::Shape::convex_polygon(shadow_points, shadow_fill, Stroke::NONE));

        let fill_color = color_with_opacity(fill, opacity);
        let stroke_color = color_with_opacity(stroke, opacity);
        painter.add(egui::Shape::convex_polygon(points.clone(), fill_color, Stroke::new(stroke_width, stroke_color)));

        if inset_points.len() >= 3 {
            let inset_fill = color_with_opacity(fill.gamma_multiply(1.08), opacity * 0.65);
            painter.add(egui::Shape::convex_polygon(
                inset_points,
                inset_fill,
                Stroke::new((stroke_width * 0.6).max(0.4), color_with_opacity(stroke, opacity * 0.45)),
            ));
        }
    }

    fn draw_district_marker(
        painter: &egui::Painter,
        district: &crate::model::DistrictRecord,
        camera: &Camera,
        bounds: WorldBounds,
        rect: Rect,
        setting: DistrictSetting,
    ) {
        if !setting.enabled {
            return;
        }

        let district_theme = &theme().districts;
        let center = camera.world_to_screen(district.anchor, bounds, rect);
        let opacity = setting.opacity.clamp(0.0, 1.0);
        let fill = Color32::from_rgba_unmultiplied(
            district_theme.fill.r,
            district_theme.fill.g,
            district_theme.fill.b,
            multiply_alpha(district_theme.fill.a, opacity),
        );
        let stroke = Color32::from_rgba_unmultiplied(
            district_theme.stroke.r,
            district_theme.stroke.g,
            district_theme.stroke.b,
            multiply_alpha(district_theme.stroke.a, opacity),
        );
        let label = Color32::from_rgba_unmultiplied(
            district_theme.label.r,
            district_theme.label.g,
            district_theme.label.b,
            multiply_alpha(district_theme.label.a, opacity),
        );
        let halo = Color32::from_rgba_unmultiplied(
            district_theme.halo.r,
            district_theme.halo.g,
            district_theme.halo.b,
            multiply_alpha(district_theme.halo.a, opacity * 0.3),
        );
        let radius = district_theme.halo_radius.max(12.0);

        match setting.mode {
            DistrictMode::Badge => {
                painter.circle_filled(center, radius * 0.52, fill.gamma_multiply(0.92));
                painter.circle_stroke(center, radius * 0.7, Stroke::new(district_theme.stroke_width, stroke));
            }
            DistrictMode::Halo => {
                painter.circle_filled(center, radius * 1.15, halo);
                painter.circle_stroke(center, radius * 0.7, Stroke::new(district_theme.stroke_width, stroke));
            }
            DistrictMode::Label => {}
            DistrictMode::Outline => {
                painter.circle_stroke(center, radius * 0.8, Stroke::new(district_theme.stroke_width, stroke));
            }
        }

        let label_pos = Pos2::new(center.x, center.y + radius * 0.95);
        painter.text(
            label_pos,
            egui::Align2::CENTER_TOP,
            &district.name,
            FontId::proportional(district_theme.label_size.clamp(11.0, 28.0)),
            label,
        );
    }

    fn draw_park_paths(
        painter: &egui::Painter,
        scene: &Scene,
        area: &AreaRecord,
        camera: &Camera,
        bounds: WorldBounds,
        rect: Rect,
        opacity: f32,
    ) {
        let style = park_path_style();
        let path_opacity = (style.opacity * opacity).clamp(0.0, 1.0);
        let casing = color_with_opacity(rgba_to_color32(style.casing), path_opacity * 0.65);
        let body = color_with_opacity(rgba_to_color32(style.body), path_opacity);
        let scale = camera.fit_scale(bounds, rect);

        for segment in &scene.document.segments {
            if !matches!(segment.class, crate::model::SegmentClass::PedestrianStreet | crate::model::SegmentClass::PedestrianPath | crate::model::SegmentClass::PedestrianWay) {
                continue;
            }

            let segment_center = segment
                .points
                .iter()
                .fold(WorldPoint::new(0.0, 0.0), |accumulator, point| WorldPoint::new(accumulator.x + point.position.x, accumulator.y + point.position.y));
            let point_count = segment.points.len().max(1) as f32;
            let segment_center = WorldPoint::new(segment_center.x / point_count, segment_center.y / point_count);

            if !area.bounds.contains_point(segment_center) && !segment.points.iter().any(|point| point_in_polygon(point.position, &area.points)) {
                continue;
            }

            let screen_points = segment
                .points
                .iter()
                .map(|point| camera.world_to_screen(point.position, bounds, rect))
                .collect::<Vec<_>>();
            Self::draw_polyline(painter, screen_points.clone(), casing, (style.casing_extra + 2.0) * scale, style.dash);
            Self::draw_polyline(painter, screen_points, body, (style.body_scale.max(0.32) * 2.0) * scale, style.dash);
        }
    }

    fn draw_label(
        painter: &egui::Painter,
        text: &str,
        center: Pos2,
        font_size: f32,
        opacity: f32,
    ) {
        let label_theme = &theme().labels;
        let shadow = Color32::from_rgba_unmultiplied(
            label_theme.shadow.r,
            label_theme.shadow.g,
            label_theme.shadow.b,
            multiply_alpha(label_theme.shadow.a, opacity),
        );
        let fill = Color32::from_rgba_unmultiplied(
            label_theme.fill.r,
            label_theme.fill.g,
            label_theme.fill.b,
            multiply_alpha(label_theme.fill.a, opacity),
        );
        painter.text(
            center + Vec2::new(1.0, 1.5),
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(font_size),
            shadow,
        );
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(font_size),
            fill,
        );
    }

    fn draw_nodes(
        painter: &egui::Painter,
        scene: &Scene,
        camera: &Camera,
        bounds: WorldBounds,
        rect: Rect,
        opacity: f32,
        selected: Option<Selection>,
    ) {
        for node in &scene.document.nodes {
            let style = node_style(&node.kind);
            let center = camera.world_to_screen(node.position, bounds, rect);
            let scale = camera.fit_scale(bounds, rect);
            let radius = style.radius * scale;
            if radius < NODE_HIDE_SCREEN_RADIUS {
                continue;
            }
            let fill = color_with_opacity(rgba_to_color32(style.fill), opacity);
            let stroke = Stroke::new(1.6, color_with_opacity(rgba_to_color32(style.stroke), opacity));
            let highlight = selected == Some(Selection::Node(node.id));
            let show_detail = radius >= NODE_LABEL_SCREEN_RADIUS;

            if highlight {
                painter.circle_filled(center, radius * 1.15, color_with_opacity(Color32::from_rgba_unmultiplied(255, 255, 255, 30), opacity));
            }

            if show_detail {
                match style.letter {
                    "T" | "R" | "P" | "W" | "E" | "H" | "A" | "M" => {
                        painter.circle_filled(center, radius, fill);
                        painter.circle_stroke(center, radius, stroke);
                    }
                    _ => {
                        painter.circle_filled(center, radius, fill);
                        painter.circle_stroke(center, radius, stroke);
                    }
                }
            } else {
                painter.circle_filled(center, radius, fill.gamma_multiply(0.88));
                painter.circle_stroke(center, radius, stroke);
            }

            if show_detail {
                let text_color = color_with_opacity(rgba_to_color32(style.text), opacity);
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    style.letter,
                    FontId::proportional((radius * 1.1).clamp(10.0, 18.0)),
                    text_color,
                );
            }
        }
    }

    fn draw_labels(
        painter: &egui::Painter,
        scene: &Scene,
        camera: &Camera,
        bounds: WorldBounds,
        rect: Rect,
        opacity: f32,
    ) {
        let scale = camera.fit_scale(bounds, rect);
        let visible = camera.visible_world_bounds(bounds, rect);
        let label_opacity = (theme().labels.opacity * opacity).clamp(0.0, 1.0);

        for area in &scene.document.buildings {
            if !area.bounds.intersects(&visible) || area.points.len() < 3 {
                continue;
            }
            let Some(name) = area.name.as_deref() else {
                continue;
            };
            let points = area
                .points
                .iter()
                .map(|point| camera.world_to_screen(*point, bounds, rect))
                .collect::<Vec<_>>();
            let center = polygon_centroid(&points);
            let font_size = (11.0 * scale).clamp(8.0, 22.0);
            if font_size >= 8.0 {
                Self::draw_label(painter, name, center, font_size, label_opacity);
            }
        }

        for area in &scene.document.parks {
            if !area.bounds.intersects(&visible) || area.points.len() < 3 {
                continue;
            }
            let label = area.name.as_deref().unwrap_or("Park");
            let points = area
                .points
                .iter()
                .map(|point| camera.world_to_screen(*point, bounds, rect))
                .collect::<Vec<_>>();
            let center = polygon_centroid(&points);
            let font_size = (11.5 * scale).clamp(8.0, 22.0);
            if font_size >= 8.0 {
                Self::draw_label(painter, label, center, font_size, label_opacity);
            }
        }

        for route in &scene.document.transports {
            if route.stops.len() < 2 {
                continue;
            }

            let route_links = route_links_for_route(scene, route);
            let Some(link) = route_links.first() else {
                continue;
            };
            let Some(segment_id) = link.segment_ids.first() else {
                continue;
            };
            let Some(segment) = scene.document.segments.iter().find(|segment| segment.id == *segment_id) else {
                continue;
            };

            let points = segment
                .points
                .iter()
                .map(|point| camera.world_to_screen(point.position, bounds, rect))
                .collect::<Vec<_>>();
            if points.is_empty() {
                continue;
            }
            let center = polygon_centroid(&points);
            let font_size = (10.0 * scale).clamp(8.0, 18.0);
            Self::draw_label(painter, &route.name, center, font_size, label_opacity * 0.9);
        }
    }

    fn draw_grid(painter: &egui::Painter, bounds: WorldBounds, camera: &Camera, rect: Rect, opacity: f32) {
        let scale = camera.fit_scale(bounds, rect);
        let step = (200.0 / scale).clamp(50.0, 500.0);
        let visible = camera.visible_world_bounds(bounds, rect).expand(step * 2.0);

        let start_x = (visible.min_x / step).floor() * step;
        let end_x = (visible.max_x / step).ceil() * step;
        let start_y = (visible.min_y / step).floor() * step;
        let end_y = (visible.max_y / step).ceil() * step;

        let stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, multiply_alpha(18, opacity)));
        let mut x = start_x;
        while x <= end_x {
            let top = camera.world_to_screen(WorldPoint::new(x, visible.max_y), bounds, rect);
            let bottom = camera.world_to_screen(WorldPoint::new(x, visible.min_y), bounds, rect);
            painter.line_segment([top, bottom], stroke);
            x += step;
        }

        let mut y = start_y;
        while y <= end_y {
            let left = camera.world_to_screen(WorldPoint::new(visible.min_x, y), bounds, rect);
            let right = camera.world_to_screen(WorldPoint::new(visible.max_x, y), bounds, rect);
            painter.line_segment([left, right], stroke);
            y += step;
        }
    }

    fn hit_test(&self, scene: &Scene, rect: Rect, pointer: Pos2) -> Option<Selection> {
        let bounds = scene.bounds;
        let world = self.camera.screen_to_world(pointer, bounds, rect);
        let mut best: Option<(f32, Selection)> = None;
        let scale = self.camera.fit_scale(bounds, rect).max(0.001);

        for node in &scene.document.nodes {
            let style = node_style(&node.kind);
            if style.radius * scale < NODE_HIDE_SCREEN_RADIUS {
                continue;
            }
            let distance = world.distance(node.position);
            let threshold = 8.0 / scale;
            if distance <= threshold {
                let selection = Selection::Node(node.id);
                if best.map(|(best_distance, _)| distance < best_distance).unwrap_or(true) {
                    best = Some((distance, selection));
                }
            }
        }

        for segment in &scene.document.segments {
            if !segment.bounds.intersects(&scene.bounds.expand(200.0)) {
                continue;
            }

            let points = segment.points.iter().map(|point| point.position).collect::<Vec<_>>();
            let distance = distance_point_to_polyline(world, &points);
            let threshold = (segment.width.max(10.0) * 0.6).max(12.0) / scale;
            if distance <= threshold {
                let selection = Selection::Segment(segment.id);
                if best.map(|(best_distance, _)| distance < best_distance).unwrap_or(true) {
                    best = Some((distance, selection));
                }
            }
        }

        for area in &scene.document.buildings {
            if area.points.len() < 3 {
                continue;
            }
            if point_in_polygon(world, &area.points) {
                let selection = Selection::Building(area.id);
                return Some(selection);
            }
        }

        for area in &scene.document.parks {
            if area.points.len() < 3 {
                continue;
            }
            if point_in_polygon(world, &area.points) {
                let selection = Selection::Park(area.id);
                return Some(selection);
            }
        }

        for district in &scene.document.districts {
            if world.distance(district.anchor) <= 24.0 / scale {
                return Some(Selection::District(district.id));
            }
        }

        best.map(|(_, selection)| selection)
    }

    fn selection_summary(&self, selection: Selection) -> Option<String> {
        let scene = self.scene.as_ref()?;
        let text = match selection {
            Selection::Node(id) => scene
                .document
                .nodes
                .iter()
                .find(|node| node.id == id)
                .map(|node| {
                    format!(
                        "Node #{id}\n{} / {}\n{}\nElevation: {:.1}\nFlags: ug={} og={} dist={}",
                        node.service,
                        if node.subtype.is_empty() { "None" } else { &node.subtype },
                        node.kind.label(),
                        node.elevation,
                        node.underground,
                        node.overground,
                        node.dist.map(|value| value.to_string()).unwrap_or_else(|| String::from("n/a")),
                    )
                })?,
            Selection::Segment(id) => scene
                .document
                .segments
                .iter()
                .find(|segment| segment.id == id)
                .map(|segment| {
                    format!(
                        "Segment #{id}\n{}\n{}\nWidth: {:.1}\nNodes: {} -> {}\nPoints: {}",
                        segment.class.raw_label(),
                        segment.name.as_deref().unwrap_or("Unnamed"),
                        segment.width,
                        segment.start_node,
                        segment.end_node,
                        segment.points.len(),
                    )
                })?,
            Selection::Building(id) => scene
                .document
                .buildings
                .iter()
                .find(|area| area.id == id)
                .map(|area| area_summary("Building", area))?,
            Selection::Park(id) => scene
                .document
                .parks
                .iter()
                .find(|area| area.id == id)
                .map(|area| area_summary("Park", area))?,
            Selection::District(id) => scene
                .document
                .districts
                .iter()
                .find(|district| district.id == id)
                .map(|district| format!("District #{id}\n{}", district.name))?,
        };

        Some(text)
    }
}

impl eframe::App for CslViewApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let modifiers = ctx.input(|input| input.modifiers);
        // Poll background export thread
        if let Some(receiver) = self.export_receiver.as_ref() {
            match receiver.try_recv() {
                Ok(Ok((status, svg))) => {
                    self.status = status;
                    // Store the svg so the next export with same settings skips regeneration
                    if !svg.is_empty() {
                        let hash = self.export_cache.as_ref().map(|(h, _)| *h).unwrap_or(0);
                        self.export_cache = Some((hash, svg));
                    }
                    self.export_receiver = None;
                }
                Ok(Err(error)) => {
                    self.last_error = Some(error);
                    self.export_receiver = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint();
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.export_receiver = None;
                }
            }
        }
        if !ctx.input(|input| input.raw.dropped_files.is_empty()) {
            let dropped = ctx.input(|input| input.raw.dropped_files.clone());
            for file in dropped {
                if let Some(path) = file.path {
                    if let Err(error) = self.load_file(path) {
                        self.last_error = Some(error.to_string());
                    }
                    break;
                }
            }
        }

        egui::Panel::top("cslview-topbar").show_inside(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("CSLView");
                ui.separator();
                ui.label(self.scene.as_ref().map(|scene| scene.document.metadata.city.as_str()).unwrap_or("No map loaded"));
                if let Some(path) = &self.current_file {
                    ui.separator();
                    ui.label(path.display().to_string());
                }
                ui.separator();
                ui.label(&self.status);
            });
        });

        egui::Panel::left("cslview-sidebar")
            .resizable(false)
            .default_size(320.0)
            .show_inside(ui, |ui| {
                ui.vertical(|ui| {
                    if ui.button("Open XML").clicked() {
                        self.load_dialog();
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Fit view").clicked() {
                            if let Some(scene) = self.scene.as_ref() {
                                self.camera.fit_to(scene.bounds);
                            }
                        }
                        if ui.button("Clear selection").clicked() {
                            self.selected = None;
                        }
                    });

                    ui.separator();
                    ui.label("Layers");
                    layer_control(ui, "Terrain", &mut self.layers.terrain);
                    layer_control(ui, "Forests", &mut self.layers.forests);
                    layer_control(ui, "Districts", &mut self.layers.districts);
                    layer_control(ui, "Parks", &mut self.layers.parks);
                    layer_control(ui, "Roads", &mut self.layers.roads);
                    layer_control(ui, "Transit", &mut self.layers.transit);
                    layer_control(ui, "Buildings", &mut self.layers.buildings);
                    layer_control(ui, "Nodes", &mut self.layers.nodes);
                    layer_control(ui, "Labels", &mut self.layers.labels);
                    layer_control(ui, "Grid", &mut self.layers.grid);

                    ui.collapsing("Road groups", |ui| {
                        layer_control(ui, "Highways", &mut self.roads.highways);
                        layer_control(ui, "Surface roads", &mut self.roads.surface_roads);
                        layer_control(ui, "Pedestrian", &mut self.roads.pedestrian);
                        layer_control(ui, "Rail", &mut self.roads.rail);
                        layer_control(ui, "Metro", &mut self.roads.metro);
                        layer_control(ui, "Aviation", &mut self.roads.aviation);
                        layer_control(ui, "Utilities", &mut self.roads.utility);
                        layer_control(ui, "Beautification", &mut self.roads.beautification);
                        layer_control(ui, "Other", &mut self.roads.miscellaneous);
                    });

                    ui.collapsing("Transit groups", |ui| {
                        layer_control(ui, "Metro routes", &mut self.transit.metro);
                        layer_control(ui, "Pedestrian routes", &mut self.transit.pedestrian);
                        layer_control(ui, "Other routes", &mut self.transit.other);
                    });

                    ui.collapsing("District styles", |ui| {
                        if let Some(scene) = self.scene.as_ref() {
                            for district in &scene.document.districts {
                                let setting = self.districts.entry(district.id).or_default();
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut setting.enabled, &district.name);
                                    ui.add(egui::Slider::new(&mut setting.opacity, 0.0..=1.0).show_value(true));
                                    egui::ComboBox::from_id_salt(("district-mode", district.id))
                                        .selected_text(district_mode_label(setting.mode))
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut setting.mode, DistrictMode::Badge, "Badge");
                                            ui.selectable_value(&mut setting.mode, DistrictMode::Halo, "Halo");
                                            ui.selectable_value(&mut setting.mode, DistrictMode::Label, "Label");
                                            ui.selectable_value(&mut setting.mode, DistrictMode::Outline, "Outline");
                                        });
                                });
                            }
                        } else {
                            ui.label("Load a map to configure districts.");
                        }
                    });

                    ui.separator();
                    ui.label("Export size");
                    ui.add(egui::DragValue::new(&mut self.export_size).range(512..=16384).speed(128.0));
                    ui.horizontal(|ui| {
                        if ui.button("SVG").clicked() {
                            if let Err(error) = self.save_export(ExportKind::Svg) {
                                self.last_error = Some(error.to_string());
                            }
                        }
                        if ui.button("PNG").clicked() {
                            if let Err(error) = self.save_export(ExportKind::Png) {
                                self.last_error = Some(error.to_string());
                            }
                        }
                        if ui.button("PDF").clicked() {
                            if let Err(error) = self.save_export(ExportKind::Pdf) {
                                self.last_error = Some(error.to_string());
                            }
                        }
                    });

                    if let Some(error) = &self.last_error {
                        ui.separator();
                        ui.colored_label(Color32::from_rgb(240, 120, 120), error);
                    }

                    ui.separator();
                    ui.label("Summary");
                    if let Some(scene) = self.scene.as_ref() {
                        ui.label(format!("Nodes: {}", scene.document.nodes.len()));
                        ui.label(format!("Segments: {}", scene.document.segments.len()));
                        ui.label(format!("Buildings: {}", scene.document.buildings.len()));
                        ui.label(format!("Parks: {}", scene.document.parks.len()));
                        ui.label(format!("Districts: {}", scene.document.districts.len()));
                        ui.label(format!("Routes: {}", scene.document.transports.len()));
                    }

                    ui.separator();
                    if let Some(selection) = self.hovered.or(self.selected) {
                        ui.label("Inspection");
                        if let Some(summary) = self.selection_summary(selection) {
                            ui.monospace(summary);
                        }
                    } else {
                        ui.label("Hover a node, segment, building, park, or district to inspect it.");
                    }
                });
            });

        egui::Panel::bottom("cslview-statusbar")
        .resizable(false)
        .exact_size(26.0)
        .show_inside(ui, |ui| {
            Self::draw_statusbar(ui, &mut self.camera, modifiers, &self.status);
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.scene.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Drop a CSL XML export here");
                        ui.label("Cities: Skylines XML export, parsed directly from the sample schema.");
                        if ui.button("Open file").clicked() {
                            self.load_dialog();
                        }
                    });
                });
                return;
            }

            self.ensure_textures(&ctx);
            let Some(scene) = self.scene.as_ref() else {
                return;
            };

            let available = ui.available_size();
            let (rect, response) = ui.allocate_exact_size(available, Sense::drag());
            self.canvas_rect = Some(rect);
            let painter = ui.painter_at(rect);
            let bounds = scene.bounds;
            let visible = self.camera.visible_world_bounds(bounds, rect);

            //let modifiers = ctx.input(|input| input.modifiers);
            let pointer_delta = ctx.input(|input| input.pointer.delta());

            if response.dragged_by(PointerButton::Primary) && pointer_delta != Vec2::ZERO {
                if modifiers.alt {
                    self.camera.rotate_by((pointer_delta.x - pointer_delta.y) * 0.01);
                } else if modifiers.ctrl {
                    if let Some(cursor) = ctx.input(|input| input.pointer.hover_pos()) {
                        self.camera.zoom_at((-(pointer_delta.y) * 0.01).exp(), cursor, bounds, rect);
                    }
                } else {
                    self.camera.pan(pointer_delta, bounds, rect);
                }
                ctx.request_repaint();
            }

            if ctx.input(|input| input.pointer.button_down(PointerButton::Middle)) && pointer_delta != Vec2::ZERO {
                if modifiers.ctrl {
                    if let Some(cursor) = ctx.input(|input| input.pointer.hover_pos()) {
                        self.camera.zoom_at((-(pointer_delta.y) * 0.01).exp(), cursor, bounds, rect);
                    }
                } else {
                    self.camera.rotate_by((pointer_delta.x - pointer_delta.y) * 0.01);
                }
                ctx.request_repaint();
            }

            if response.hovered() {
                let scroll_delta = ctx.input(|input| input.smooth_scroll_delta);
                if scroll_delta != Vec2::ZERO {
                    if modifiers.ctrl {
                        if let Some(cursor) = ctx.input(|input| input.pointer.hover_pos()) {
                            self.camera.zoom_at((-(scroll_delta.y) * 0.01).exp(), cursor, bounds, rect);
                        }
                    } else if modifiers.alt {
                        self.camera.rotate_by((scroll_delta.x - scroll_delta.y) * 0.01);
                    } else {
                        self.camera.pan(scroll_delta, bounds, rect);
                    }
                    ctx.request_repaint();
                }

                let zoom = ctx.input(|input| input.zoom_delta());
                if (zoom - 1.0).abs() > f32::EPSILON {
                    if let Some(cursor) = ctx.input(|input| input.pointer.hover_pos()) {
                        self.camera.zoom_at(zoom, cursor, bounds, rect);
                        ctx.request_repaint();
                    }
                }
            }

            if self.layers.grid.enabled {
                Self::draw_grid(&painter, bounds, &self.camera, rect, self.layers.grid.opacity);
            }

            if self.layers.terrain.enabled {
                if let Some(layer) = scene.terrain.as_ref() {
                    if let Some(texture) = self.terrain_texture.as_ref() {
                        Self::draw_raster_layer(&painter, layer, texture, &self.camera, bounds, rect, self.layers.terrain.opacity);
                    }
                }
            }

            if self.layers.forests.enabled {
                if let Some(layer) = scene.forests.as_ref() {
                    if let Some(texture) = self.forest_texture.as_ref() {
                        Self::draw_raster_layer(&painter, layer, texture, &self.camera, bounds, rect, self.layers.forests.opacity);
                    }
                }
            }

            if self.layers.districts.enabled {
                for district in &scene.document.districts {
                    let setting = self.districts.get(&district.id).copied().unwrap_or_default();
                    Self::draw_district_marker(&painter, district, &self.camera, bounds, rect, setting);
                }
            }

            if self.layers.parks.enabled {
                for area in &scene.document.parks {
                    if !area.bounds.intersects(&visible) {
                        continue;
                    }
                    let style = park_style(area.area_type.as_deref());
                    Self::draw_detailed_area(
                        &painter,
                        area,
                        &self.camera,
                        bounds,
                        rect,
                        Color32::from_rgba_unmultiplied(style.fill.r, style.fill.g, style.fill.b, 255),
                        Color32::from_rgba_unmultiplied(style.stroke.r, style.stroke.g, style.stroke.b, 255),
                        style.stroke_width,
                        (style.opacity * self.layers.parks.opacity).clamp(0.0, 1.0),
                        0.08,
                        Vec2::new(1.0, -1.0),
                    );
                    Self::draw_park_paths(&painter, scene, area, &self.camera, bounds, rect, self.layers.parks.opacity);
                }
            }

            if self.layers.roads.enabled {
                for segment in &scene.document.segments {
                    if !segment.bounds.intersects(&visible) {
                        continue;
                    }
                    let setting = road_setting(&self.roads, &segment.class);
                    if !setting.enabled {
                        continue;
                    }
                    let selected = self.selected == Some(Selection::Segment(segment.id));
                    Self::draw_segment(
                        &painter,
                        segment,
                        &self.camera,
                        bounds,
                        rect,
                        scene.document.metadata.sea_level,
                        self.layers.roads.opacity * setting.opacity,
                        selected,
                    );
                }
            }

            if self.layers.transit.enabled {
                for route in &scene.document.transports {
                    let setting = transit_setting(&self.transit, route.kind);
                    if !setting.enabled || route.stops.len() < 2 {
                        continue;
                    }
                    for link in &scene.document.route_links {
                        if !route.stops.windows(2).any(|pair| pair[0] == link.start_node && pair[1] == link.end_node) {
                            continue;
                        }
                        for segment_id in &link.segment_ids {
                            if let Some(segment) = scene.document.segments.iter().find(|segment| segment.id == *segment_id) {
                                let points = segment
                                    .points
                                    .iter()
                                    .map(|point| self.camera.world_to_screen(point.position, bounds, rect))
                                    .collect::<Vec<_>>();
                                let route_color = Color32::from_rgba_unmultiplied(route.color.r, route.color.g, route.color.b, route.color.a);
                                Self::draw_polyline(
                                    &painter,
                                    points,
                                    color_with_opacity(route_color, self.layers.transit.opacity * setting.opacity),
                                    3.5,
                                    &[],
                                );
                            }
                        }
                    }
                }
            }

            if self.layers.buildings.enabled {
                for area in &scene.document.buildings {
                    if !area.bounds.intersects(&visible) {
                        continue;
                    }
                    let style = crate::style::building_style(&area.service, &area.subtype);
                    Self::draw_detailed_area(
                        &painter,
                        area,
                        &self.camera,
                        bounds,
                        rect,
                        Color32::from_rgba_unmultiplied(style.fill.r, style.fill.g, style.fill.b, 255),
                        Color32::from_rgba_unmultiplied(style.stroke.r, style.stroke.g, style.stroke.b, 255),
                        style.stroke_width,
                        (style.opacity * self.layers.buildings.opacity).clamp(0.0, 1.0),
                        0.16,
                        Vec2::new(1.5, -1.5),
                    );
                }
            }

            if self.layers.nodes.enabled {
                Self::draw_nodes(&painter, scene, &self.camera, bounds, rect, self.layers.nodes.opacity, self.selected);
            }

            if self.layers.labels.enabled {
                Self::draw_labels(&painter, scene, &self.camera, bounds, rect, self.layers.labels.opacity);
            }

            self.hovered = response
                .hover_pos()
                .and_then(|pointer| self.hit_test(scene, rect, pointer));

            if response.clicked() {
                self.selected = self.hovered;
            }
        });
    }
}

fn area_summary(prefix: &str, area: &AreaRecord) -> String {
    format!(
        "{} #{}\n{}\n{} / {}\nType: {}\nPoints: {}\nElevation: {:.1}",
        prefix,
        area.id,
        area.name.as_deref().unwrap_or("Unnamed"),
        area.service,
        if area.subtype.is_empty() { "None" } else { &area.subtype },
        area.area_type.as_deref().unwrap_or("n/a"),
        area.points.len(),
        area.elevation,
    )
}

fn draw_compass_needle(painter: &egui::Painter, center: Pos2, rotation: f32, radius: f32) {
    let north = Vec2::new(-rotation.sin(), -rotation.cos()) * radius;
    let tip = center + north;
    let base = center + north * 0.28;
    painter.line_segment([base, tip], Stroke::new(2.0, Color32::from_rgb(226, 238, 255)));

    let left = tip + Vec2::new(north.y, -north.x).normalized() * -4.0 + (center - tip).normalized() * 8.0;
    let right = tip + Vec2::new(-north.y, north.x).normalized() * -4.0 + (center - tip).normalized() * 8.0;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, left, right],
        Color32::from_rgb(248, 250, 255),
        Stroke::NONE,
    ));
}

fn statusbar_hint(ui: &mut egui::Ui, key: &str, action: &str, active: bool) {
    let key_color = if active {
        Color32::from_rgb(255, 210, 100)
    } else {
        Color32::from_rgb(180, 190, 210)
    };
    let key_bg = Color32::from_rgba_unmultiplied(key_color.r(), key_color.g(), key_color.b(), 38);
    ui.label(
        egui::RichText::new(format!(" {} ", key))
            .monospace()
            .strong()
            .color(key_color)
            .background_color(key_bg),
    );
    ui.label(
        egui::RichText::new(action)
            .color(if active {
                Color32::from_rgb(242, 244, 248)
            } else {
                Color32::from_rgba_unmultiplied(200, 208, 224, 180)
            })
            .small(),
    );
    ui.add_space(6.0);
}

impl CslViewApp {
    fn draw_statusbar(
        ui: &mut egui::Ui,
        camera: &mut Camera,
        modifiers: egui::Modifiers,
        status: &str,
    ) {
        // Determine the active action for each input given held modifiers
        let lmb_action = if modifiers.alt { "Rotate" }
            else if modifiers.ctrl { "Zoom" }
            else { "Pan" };
        let mmb_action = if modifiers.ctrl { "Zoom" } else { "Rotate" };
        let scroll_action = if modifiers.ctrl { "Zoom" }
            else if modifiers.alt { "Rotate" }
            else { "Pan" };

        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add_space(8.0);

            // Navigation hints
            statusbar_hint(ui, "LMB", lmb_action, modifiers.alt || modifiers.ctrl);
            ui.separator();
            statusbar_hint(ui, "MMB", mmb_action, modifiers.ctrl);
            ui.separator();
            statusbar_hint(ui, "Scroll", scroll_action, modifiers.alt || modifiers.ctrl);

            // Right-aligned section: status → compass → rotation → zoom
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                // Status message
                ui.label(
                    egui::RichText::new(status)
                        .small()
                        .color(Color32::from_rgba_unmultiplied(200, 208, 224, 200)),
                );
                ui.separator();

                // Zoom value + slider
                ui.label(
                    egui::RichText::new(format!("{:.2}×", camera.zoom))
                        .monospace()
                        .color(Color32::from_rgb(180, 220, 255)),
                );
                let slider = egui::Slider::new(&mut camera.zoom, 0.05..=50.0)
                    .logarithmic(true)
                    .show_value(false)
                    .clamping(egui::SliderClamping::Always);
                ui.add_sized([80.0, 16.0], slider);
                ui.separator();

                // Rotation angle text
                let degrees = camera.rotation.to_degrees();
                ui.label(
                    egui::RichText::new(format!("{:.0}°", degrees))
                        .monospace()
                        .small()
                        .color(Color32::from_rgb(220, 190, 140)),
                );

                // Tiny inline compass (24×24 painter widget)
                let (compass_rect, _) = ui.allocate_exact_size(
                    Vec2::splat(22.0),
                    egui::Sense::hover(),
                );
                let painter = ui.painter_at(compass_rect);
                let center = compass_rect.center();
                painter.circle_stroke(
                    center,
                    9.0,
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 50)),
                );
                draw_compass_needle(&painter, center, camera.rotation, 8.0);
                painter.text(
                    Pos2::new(center.x, compass_rect.top()),
                    egui::Align2::CENTER_TOP,
                    "N",
                    FontId::proportional(7.0),
                    Color32::from_rgb(200, 210, 240),
                );
                ui.separator();
            });
        });
    }
}

fn layer_control(ui: &mut egui::Ui, label: &str, setting: &mut LayerSetting) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut setting.enabled, label);
        ui.add(egui::Slider::new(&mut setting.opacity, 0.0..=1.0).show_value(true));
    });
}

fn rgba_to_color32(color: crate::model::RgbaColor) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

fn multiply_alpha(alpha: u8, opacity: f32) -> u8 {
    (alpha as f32 * opacity.clamp(0.0, 1.0)).round().clamp(0.0, 255.0) as u8
}

fn color_with_opacity(color: Color32, opacity: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), multiply_alpha(color.a(), opacity))
}

fn polygon_centroid(points: &[Pos2]) -> Pos2 {
    if points.is_empty() {
        return Pos2::new(0.0, 0.0);
    }

    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    for point in points {
        sum_x += point.x;
        sum_y += point.y;
    }

    Pos2::new(sum_x / points.len() as f32, sum_y / points.len() as f32)
}

fn inset_polygon(points: &[Pos2], inset_factor: f32) -> Vec<Pos2> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let centroid = polygon_centroid(points);
    points
        .iter()
        .map(|point| Pos2::new(
            point.x + (centroid.x - point.x) * inset_factor,
            point.y + (centroid.y - point.y) * inset_factor,
        ))
        .collect()
}

fn district_mode_label(mode: DistrictMode) -> &'static str {
    match mode {
        DistrictMode::Badge => "Badge",
        DistrictMode::Halo => "Halo",
        DistrictMode::Label => "Label",
        DistrictMode::Outline => "Outline",
    }
}

fn road_setting<'a>(roads: &'a RoadLayers, class: &crate::model::SegmentClass) -> &'a LayerSetting {
    match class {
        crate::model::SegmentClass::Highway => &roads.highways,
        crate::model::SegmentClass::LargeRoad
        | crate::model::SegmentClass::MediumRoad
        | crate::model::SegmentClass::SmallRoad => &roads.surface_roads,
        crate::model::SegmentClass::PedestrianStreet
        | crate::model::SegmentClass::PedestrianPath
        | crate::model::SegmentClass::PedestrianWay => &roads.pedestrian,
        crate::model::SegmentClass::TrainTrack => &roads.rail,
        crate::model::SegmentClass::MetroTrack => &roads.metro,
        crate::model::SegmentClass::ElectricityWire => &roads.utility,
        crate::model::SegmentClass::AirplaneRunway
        | crate::model::SegmentClass::AirplaneLine
        | crate::model::SegmentClass::HelicopterPath
        | crate::model::SegmentClass::BlimpPath => &roads.aviation,
        crate::model::SegmentClass::FloodWall => &roads.utility,
        crate::model::SegmentClass::BeautificationItem => &roads.beautification,
        crate::model::SegmentClass::Miscellaneous | crate::model::SegmentClass::Unknown(_) => &roads.miscellaneous,
    }
}

fn transit_setting<'a>(transit: &'a TransitLayers, kind: crate::model::TransportKind) -> &'a LayerSetting {
    match kind {
        crate::model::TransportKind::Metro => &transit.metro,
        crate::model::TransportKind::Pedestrian => &transit.pedestrian,
        crate::model::TransportKind::Other => &transit.other,
    }
}

fn route_links_for_route<'a>(
    scene: &'a Scene,
    route: &'a crate::model::TransportRecord,
) -> Vec<&'a crate::model::RouteLinkRecord> {
    if route.stops.len() < 2 {
        return Vec::new();
    }

    scene
        .document
        .route_links
        .iter()
        .filter(|link| {
            route
                .stops
                .windows(2)
                .any(|pair| pair[0] == link.start_node && pair[1] == link.end_node)
        })
        .collect()
}

fn point_in_polygon(point: WorldPoint, polygon: &[WorldPoint]) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let mut inside = false;
    let mut previous = *polygon.last().unwrap();
    for current in polygon {
        let intersects = ((current.y > point.y) != (previous.y > point.y))
            && (point.x < (previous.x - current.x) * (point.y - current.y) / ((previous.y - current.y).max(f32::EPSILON)) + current.x);
        if intersects {
            inside = !inside;
        }
        previous = *current;
    }

    inside
}
