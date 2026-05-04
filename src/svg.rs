use std::collections::BTreeMap;
use std::fmt::Write;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::export::{DistrictMode, DistrictSetting, ExportSettings, LayerSetting, RoadLayers, TransitLayers};
use crate::model::ElevationState;
use crate::model::WorldPoint;
use crate::scene::{RasterLayer, Scene};
use crate::style::{
    building_style, district_style, elevation_state, node_style, park_style, route_style,
    segment_style, NodeShape,
};
use crate::viewport::{polygon_points, polyline_path, ViewportTransform};

pub fn scene_to_svg(scene: &Scene, settings: &ExportSettings) -> String {
    let bounds = settings.frame.unwrap_or(scene.bounds);
    let transform = ViewportTransform::new(
        bounds,
        settings.width,
        settings.height,
        settings.padding,
        settings.zoom,
    );

    let mut output = String::new();
    let layers = &settings.layers;
    let _ = write!(
        output,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{}\" height=\"{}\" viewBox=\"{}\" shape-rendering=\"geometricPrecision\">\n<defs>\n  <style>\n    .text-ui {{ font-family: system-ui, sans-serif; }}\n    .label-shadow {{ paint-order: stroke fill; stroke: rgba(0, 0, 0, 0.58); stroke-width: 6; stroke-linejoin: round; }}\n    .segment {{ fill: none; stroke-linecap: round; stroke-linejoin: round; }}\n    .area {{ stroke-linejoin: round; stroke-linecap: round; }}\n  </style>\n</defs>\n",
        settings.width,
        settings.height,
        transform.screen_rect(),
    );

    let _ = writeln!(output, "<rect width=\"100%\" height=\"100%\" fill=\"#0d1015\"/>");

    if let Some(terrain) = scene.terrain.as_ref() {
        output.push_str(&raster_layer_svg(terrain, &transform, layers.terrain.opacity));
    }
    if let Some(forests) = scene.forests.as_ref() {
        output.push_str(&raster_layer_svg(forests, &transform, layers.forests.opacity));
    }

    output.push_str(&render_districts(scene, &transform, &settings.districts, layers.districts.opacity));
    output.push_str(&render_parks(scene, &transform, layers.parks.opacity));
    output.push_str(&render_segments(scene, &transform, &settings.roads, layers.roads.opacity));
    output.push_str(&render_routes(scene, &transform, &settings.transit, layers.transit.opacity));
    output.push_str(&render_buildings(scene, &transform, layers.buildings.opacity));
    output.push_str(&render_nodes(scene, &transform, layers.nodes.opacity));
    output.push_str(&render_labels(scene, &transform, layers.labels.opacity));

    output.push_str("</svg>");
    output
}

fn render_segments(scene: &Scene, transform: &ViewportTransform, roads: &RoadLayers, opacity: f32) -> String {
    let mut output = String::new();
    let sea_level = scene.document.metadata.sea_level;

    output.push_str("<g id=\"segments\">");
    for segment in &scene.document.segments {
        if !segment.bounds.intersects(&transform.world) {
            continue;
        }

        let elevation = elevation_state(
            segment.underground,
            segment.overground,
            segment.elevated_hint,
            segment
                .points
                .iter()
                .map(|point| point.elevation)
                .fold(f32::NEG_INFINITY, f32::max),
            sea_level,
        );
        let style = segment_style(&segment.class, elevation);
        let road = road_setting(roads, &segment.class);
        if !road.enabled {
            continue;
        }
        let points = segment
            .points
            .iter()
            .map(|point| transform.map(point.position))
            .collect::<Vec<_>>();
        let path = polyline_path(&points);
        let dash_attr = if style.dash.is_empty() {
            String::new()
        } else {
            format!(
                " stroke-dasharray=\"{}\"",
                style
                    .dash
                    .iter()
                    .map(|value| format!("{value:.1}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };

        let _ = writeln!(
            output,
            "<path class=\"segment\" d=\"{path}\" stroke=\"{}\" stroke-width=\"{:.2}\" opacity=\"{:.3}\"{} data-id=\"{}\" data-class=\"{}\"/>",
            style.casing.to_css(),
            transform.map_size(segment.width + style.casing_extra),
            style.opacity * opacity * road.opacity,
            dash_attr,
            segment.id,
            escape_xml(segment.class.raw_label()),
        );
        let _ = writeln!(
            output,
            "<path class=\"segment\" d=\"{path}\" stroke=\"{}\" stroke-width=\"{:.2}\" opacity=\"{:.3}\"{} data-id=\"{}\" data-class=\"{}\"/>",
            style.body.to_css(),
            transform.map_size(segment.width * style.body_scale),
            style.opacity * opacity * road.opacity,
            dash_attr,
            segment.id,
            escape_xml(segment.class.raw_label()),
        );
    }
    output.push_str("</g>");
    output
}

fn render_routes(scene: &Scene, transform: &ViewportTransform, transit: &TransitLayers, opacity: f32) -> String {
    if scene.document.transports.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    output.push_str("<g id=\"transit\">");

    for route in &scene.document.transports {
        let style = route_style(route.kind, route.color);
        let setting = transit_setting(transit, route.kind);
        if !setting.enabled {
            continue;
        }
        let route_label = escape_xml(&route.name);

        for link in route_links_for_route(scene, route) {
            for segment_id in &link.segment_ids {
                if let Some(segment) = scene.document.segments.iter().find(|segment| segment.id == *segment_id) {
                    let points = segment
                        .points
                        .iter()
                        .map(|point| transform.map(point.position))
                        .collect::<Vec<_>>();
                    let path = polyline_path(&points);
                    let _ = writeln!(
                        output,
                        "<path class=\"segment\" d=\"{path}\" stroke=\"{}\" stroke-width=\"{:.2}\" opacity=\"{:.3}\" data-route=\"{}\" data-link=\"{}-{}\"/>",
                        style.stroke.to_css(),
                        transform.map_size((segment.width * 0.6).max(style.stroke_width)),
                        style.opacity * opacity * setting.opacity,
                        route_label,
                        link.start_node,
                        link.end_node,
                    );
                }
            }
        }
    }

    output.push_str("</g>");
    output
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

fn render_buildings(scene: &Scene, transform: &ViewportTransform, opacity: f32) -> String {
    let mut output = String::new();
    output.push_str("<g id=\"buildings\">");
    for area in &scene.document.buildings {
        if !area.bounds.intersects(&transform.world) {
            continue;
        }

        let style = building_style(&area.service, &area.subtype);
        let points = area
            .points
            .iter()
            .map(|point| transform.map(*point))
            .collect::<Vec<_>>();
        let _ = writeln!(
            output,
            "<polygon class=\"area\" points=\"{}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\" data-id=\"{}\" data-service=\"{}\" data-subservice=\"{}\"/>",
            polygon_points(&points),
            style.fill.to_css(),
            style.opacity * opacity,
            style.stroke.to_css(),
            style.opacity * opacity,
            transform.map_size(style.stroke_width),
            area.id,
            escape_xml(&area.service),
            escape_xml(&area.subtype),
        );
    }
    output.push_str("</g>");
    output
}

fn render_parks(scene: &Scene, transform: &ViewportTransform, opacity: f32) -> String {
    let mut output = String::new();
    if scene.document.parks.is_empty() {
        return output;
    }

    output.push_str("<g id=\"parks\">");
    for area in &scene.document.parks {
        if !area.bounds.intersects(&transform.world) {
            continue;
        }

        let style = park_style(area.area_type.as_deref());

        let points = area
            .points
            .iter()
            .map(|point| transform.map(*point))
            .collect::<Vec<_>>();
        let _ = writeln!(
            output,
            "<polygon class=\"area\" points=\"{}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\" data-id=\"{}\" data-type=\"{}\"/>",
            polygon_points(&points),
            style.fill.to_css(),
            style.opacity * opacity,
            style.stroke.to_css(),
            style.opacity * opacity,
            transform.map_size(style.stroke_width),
            area.id,
            escape_xml(area.area_type.as_deref().unwrap_or("Park")),
        );
    }
    output.push_str("</g>");
    output
}

fn render_districts(scene: &Scene, transform: &ViewportTransform, districts: &BTreeMap<u64, DistrictSetting>, opacity: f32) -> String {
    let mut output = String::new();
    if scene.document.districts.is_empty() {
        return output;
    }

    output.push_str("<g id=\"districts\">");
    let style = district_style();
    for district in &scene.document.districts {
        let setting = districts.get(&district.id).copied().unwrap_or_default();
        if !setting.enabled {
            continue;
        }
        let (x, y) = transform.map(district.anchor);
        let circle_radius = transform.map_size(3.0);
        let fill_opacity = style.opacity * opacity * setting.opacity;
        let stroke_opacity = style.opacity * opacity * setting.opacity;
        let label = escape_xml(&district.name);
        match setting.mode {
            DistrictMode::Badge => {
                let _ = writeln!(
                    output,
                    "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{circle_radius:.2}\" fill=\"{}\" fill-opacity=\"{fill_opacity:.3}\" stroke=\"{}\" stroke-opacity=\"{stroke_opacity:.3}\" stroke-width=\"{:.2}\"/>",
                    style.fill.to_css(),
                    style.stroke.to_css(),
                    transform.map_size(style.stroke_width),
                );
                let _ = writeln!(
                    output,
                    "<text class=\"text-ui label-shadow\" x=\"{x:.2}\" y=\"{:.2}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"{:.2}\" fill=\"#f4f2ea\" fill-opacity=\"{:.3}\">{label}</text>",
                    y + circle_radius * 1.15,
                    transform.map_size(15.0),
                    fill_opacity.clamp(0.0, 1.0),
                );
            }
            DistrictMode::Halo => {
                let _ = writeln!(
                    output,
                    "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\"/>",
                    transform.map_size(7.5),
                    style.fill.to_css(),
                    (fill_opacity * 0.35).clamp(0.0, 1.0),
                );
                let _ = writeln!(
                    output,
                    "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{circle_radius:.2}\" fill=\"{}\" fill-opacity=\"{fill_opacity:.3}\" stroke=\"{}\" stroke-opacity=\"{stroke_opacity:.3}\" stroke-width=\"{:.2}\"/>",
                    style.fill.to_css(),
                    style.stroke.to_css(),
                    transform.map_size(style.stroke_width),
                );
            }
            DistrictMode::Label => {
                let _ = writeln!(
                    output,
                    "<text class=\"text-ui label-shadow\" x=\"{x:.2}\" y=\"{y:.2}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"{:.2}\" fill=\"#f4f2ea\" fill-opacity=\"{:.3}\">{label}</text>",
                    transform.map_size(15.0),
                    fill_opacity.clamp(0.0, 1.0),
                );
            }
            DistrictMode::Outline => {
                let _ = writeln!(
                    output,
                    "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{circle_radius:.2}\" fill=\"none\" stroke=\"{}\" stroke-opacity=\"{stroke_opacity:.3}\" stroke-width=\"{:.2}\"/>",
                    style.stroke.to_css(),
                    transform.map_size(style.stroke_width),
                );
            }
        }
    }
    output.push_str("</g>");
    output
}

fn render_nodes(scene: &Scene, transform: &ViewportTransform, opacity: f32) -> String {
    let mut output = String::new();
    output.push_str("<g id=\"nodes\">");

    for node in &scene.document.nodes {
        if !transform.world.contains_point(node.position) {
            continue;
        }

        let elevation = elevation_state(
            node.underground,
            node.overground,
            false,
            node.elevation,
            scene.document.metadata.sea_level,
        );
        let style = node_style(&node.kind);
        let (x, y) = transform.map(node.position);
        let radius = transform.map_size(style.radius * if elevation == ElevationState::Elevated { 1.1 } else { 1.0 });
        let text_color = style.text.to_css();
        let fill_opacity = opacity.clamp(0.0, 1.0);
        let stroke_opacity = opacity.clamp(0.0, 1.0);

        let shape = match style.shape {
            NodeShape::Circle => format!(
                "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{radius:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\"/>",
                style.fill.to_css(),
                fill_opacity,
                style.stroke.to_css(),
                stroke_opacity,
                transform.map_size(1.6)
            ),
            NodeShape::Square => format!(
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" rx=\"{:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\" transform=\"rotate(45 {:.2} {:.2})\"/>",
                x - radius,
                y - radius,
                radius * 2.0,
                radius * 2.0,
                radius * 0.22,
                style.fill.to_css(),
                fill_opacity,
                style.stroke.to_css(),
                stroke_opacity,
                transform.map_size(1.5),
                x,
                y
            ),
            NodeShape::Diamond => format!(
                "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\"/>",
                x,
                y - radius,
                x + radius,
                y,
                x,
                y + radius,
                x - radius,
                y,
                style.fill.to_css(),
                fill_opacity,
                style.stroke.to_css(),
                stroke_opacity,
                transform.map_size(1.6)
            ),
            NodeShape::Hexagon => {
                let half = radius * 0.92;
                format!(
                    "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\"/>",
                    x - half * 0.5,
                    y - radius,
                    x + half * 0.5,
                    y - radius,
                    x + radius,
                    y,
                    x + half * 0.5,
                    y + radius,
                    x - half * 0.5,
                    y + radius,
                    x - radius,
                    y,
                    style.fill.to_css(),
                    fill_opacity,
                    style.stroke.to_css(),
                    stroke_opacity,
                    transform.map_size(1.6)
                )
            }
            NodeShape::Triangle => format!(
                "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\"/>",
                x,
                y - radius,
                x + radius,
                y + radius,
                x - radius,
                y + radius,
                style.fill.to_css(),
                fill_opacity,
                style.stroke.to_css(),
                stroke_opacity,
                transform.map_size(1.6)
            ),
        };
        output.push_str(&shape);

        let _ = writeln!(
            output,
            "<text class=\"text-ui\" x=\"{x:.2}\" y=\"{:.2}\" font-size=\"{:.2}\" text-anchor=\"middle\" dominant-baseline=\"central\" fill=\"{}\" fill-opacity=\"{:.3}\">{}</text>",
            y + radius * 0.1,
            transform.map_size(style.radius * 1.15),
            text_color,
            opacity.clamp(0.0, 1.0),
            style.letter,
        );
    }

    output.push_str("</g>");
    output
}

fn render_labels(scene: &Scene, transform: &ViewportTransform, opacity: f32) -> String {
    let mut output = String::new();
    if scene.document.districts.is_empty() && scene.document.buildings.is_empty() && scene.document.parks.is_empty() && scene.document.transports.is_empty() {
        return output;
    }

    output.push_str("<g id=\"labels\">");
    for area in &scene.document.buildings {
        if !area.bounds.intersects(&transform.world) {
            continue;
        }
        let Some(name) = area.name.as_deref() else {
            continue;
        };
        let points = area
            .points
            .iter()
            .map(|point| transform.map(*point))
            .collect::<Vec<_>>();
        let (x, y) = polygon_centroid(&points);
        let _ = writeln!(
            output,
            "<text class=\"text-ui label-shadow\" x=\"{x:.2}\" y=\"{y:.2}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"{:.2}\" fill=\"#f4f2ea\" fill-opacity=\"{:.3}\">{}</text>",
            transform.map_size(13.0),
            opacity.clamp(0.0, 1.0),
            escape_xml(name),
        );
    }

    for area in &scene.document.parks {
        if !area.bounds.intersects(&transform.world) {
            continue;
        }
        let label = area.name.as_deref().unwrap_or("Park");
        let points = area
            .points
            .iter()
            .map(|point| transform.map(*point))
            .collect::<Vec<_>>();
        let (x, y) = polygon_centroid(&points);
        let _ = writeln!(
            output,
            "<text class=\"text-ui label-shadow\" x=\"{x:.2}\" y=\"{y:.2}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"{:.2}\" fill=\"#f4f2ea\" fill-opacity=\"{:.3}\">{}</text>",
            transform.map_size(12.5),
            opacity.clamp(0.0, 1.0),
            escape_xml(label),
        );
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
            .map(|point| transform.map(point.position))
            .collect::<Vec<_>>();
        let (x, y) = polygon_centroid(&points);
        let _ = writeln!(
            output,
            "<text class=\"text-ui label-shadow\" x=\"{x:.2}\" y=\"{y:.2}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"{:.2}\" fill=\"#f4f2ea\" fill-opacity=\"{:.3}\">{}</text>",
            transform.map_size(11.0),
            opacity.clamp(0.0, 1.0),
            escape_xml(&route.name),
        );
    }
    output.push_str("</g>");
    output
}

fn polygon_centroid(points: &[(f32, f32)]) -> (f32, f32) {
    if points.is_empty() {
        return (0.0, 0.0);
    }

    let mut twice_area = 0.0;
    let mut centroid_x = 0.0;
    let mut centroid_y = 0.0;

    for index in 0..points.len() {
        let (x0, y0) = points[index];
        let (x1, y1) = points[(index + 1) % points.len()];
        let cross = x0 * y1 - x1 * y0;
        twice_area += cross;
        centroid_x += (x0 + x1) * cross;
        centroid_y += (y0 + y1) * cross;
    }

    if twice_area.abs() < f32::EPSILON {
        let sum_x = points.iter().map(|(x, _)| *x).sum::<f32>();
        let sum_y = points.iter().map(|(_, y)| *y).sum::<f32>();
        return (sum_x / points.len() as f32, sum_y / points.len() as f32);
    }

    let area = twice_area * 0.5;
    (centroid_x / (6.0 * area), centroid_y / (6.0 * area))
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

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn raster_layer_svg(layer: &RasterLayer, transform: &ViewportTransform, opacity: f32) -> String {
    if opacity <= f32::EPSILON || layer.world_bounds.is_empty() {
        return String::new();
    }

    let png = match layer.to_png_bytes() {
        Ok(bytes) => bytes,
        Err(_) => return String::new(),
    };

    let encoded = STANDARD.encode(png);
    let top_left = transform.map(WorldPoint::new(layer.world_bounds.min_x, layer.world_bounds.max_y));
    let bottom_right = transform.map(WorldPoint::new(layer.world_bounds.max_x, layer.world_bounds.min_y));
    let x = top_left.0.min(bottom_right.0);
    let y = top_left.1.min(bottom_right.1);
    let width = (bottom_right.0 - top_left.0).abs();
    let height = (bottom_right.1 - top_left.1).abs();

    format!(
        "<image x=\"{x:.2}\" y=\"{y:.2}\" width=\"{width:.2}\" height=\"{height:.2}\" opacity=\"{opacity:.3}\" preserveAspectRatio=\"none\" xlink:href=\"data:image/png;base64,{encoded}\"/>"
    )
}
/*
use std::fmt::Write;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::export::ExportSettings;
use crate::model::{ElevationState, WorldPoint};
use crate::scene::{RasterLayer, Scene};
use crate::style::{
    building_style, district_style, elevation_state, node_style, park_style, route_style,
    segment_style, NodeShape,
};
use crate::viewport::{polygon_points, polyline_path, ViewportTransform};

pub fn scene_to_svg(scene: &Scene, settings: &ExportSettings) -> String {
    let bounds = settings.frame.unwrap_or(scene.bounds);
    let transform = ViewportTransform::new(
        bounds,
        settings.width,
        settings.height,
        settings.padding,
        settings.zoom,
    );

    let mut output = String::new();
    let _ = write!(
        output,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{}\" height=\"{}\" viewBox=\"{}\" shape-rendering=\"geometricPrecision\">\n<defs>\n  <style>\n    .text-ui {{ font-family: system-ui, sans-serif; }}\n    .label-shadow {{ paint-order: stroke fill; stroke: rgba(0, 0, 0, 0.58); stroke-width: 6; stroke-linejoin: round; }}\n    .segment {{ fill: none; stroke-linecap: round; stroke-linejoin: round; }}\n    .area {{ stroke-linejoin: round; stroke-linecap: round; }}\n  </style>\n</defs>\n",
        settings.width,
        settings.height,
        transform.screen_rect(),
    );

    let _ = writeln!(output, "<rect width=\"100%\" height=\"100%\" fill=\"#0d1015\"/>");

    if let Some(terrain) = scene.terrain.as_ref() {
        let grid = terrain.sample_grid(bounds, terrain_resolution);
        output.push_str(&grid.to_svg(&transform));
    }
    if let Some(forests) = scene.forests.as_ref() {
        let grid = forests.sample_grid(bounds, terrain_resolution);
        output.push_str(&grid.to_svg(&transform));
    }

    output.push_str(&render_districts(scene, &transform));
    output.push_str(&render_parks(scene, &transform));
    output.push_str(&render_segments(scene, &transform));
    output.push_str(&render_routes(scene, &transform));
    output.push_str(&render_buildings(scene, &transform));
    output.push_str(&render_nodes(scene, &transform));
    output.push_str(&render_labels(scene, &transform));

    output.push_str("</svg>");
    output
}

fn render_segments(scene: &Scene, transform: &ViewportTransform) -> String {
    let mut output = String::new();
    let sea_level = scene.document.metadata.sea_level;

    output.push_str("<g id=\"segments\">");
    for segment in &scene.document.segments {
        if !segment.bounds.intersects(&transform.world) {
            continue;
        }

        let elevation = elevation_state(
            segment.underground,
            segment.overground,
            segment.elevated_hint,
            segment
                .points
                .iter()
                .map(|point| point.elevation)
                .fold(f32::NEG_INFINITY, f32::max),
            sea_level,
        );
        let style = segment_style(&segment.class, elevation);
        let points = segment
            .points
            .iter()
            .map(|point| transform.map(point.position))
            .collect::<Vec<_>>();
        let path = polyline_path(&points);
        let dash_attr = if style.dash.is_empty() {
            String::new()
        } else {
            format!(
                " stroke-dasharray=\"{}\"",
                style
                    .dash
                    .iter()
                    .map(|value| format!("{value:.1}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };

        let _ = writeln!(
            output,
            "<path class=\"segment\" d=\"{path}\" stroke=\"{}\" stroke-width=\"{:.2}\" opacity=\"{:.3}\"{} data-id=\"{}\" data-class=\"{}\"/>",
            style.casing.to_css(),
            transform.map_size(segment.width + style.casing_extra),
            style.opacity,
            dash_attr,
            segment.id,
            escape_xml(segment.class.raw_label()),
        );
        let _ = writeln!(
            output,
            "<path class=\"segment\" d=\"{path}\" stroke=\"{}\" stroke-width=\"{:.2}\" opacity=\"{:.3}\"{} data-id=\"{}\" data-class=\"{}\"/>",
            style.body.to_css(),
            transform.map_size(segment.width * style.body_scale),
            style.opacity,
            dash_attr,
            segment.id,
            escape_xml(segment.class.raw_label()),
        );
    }
    output.push_str("</g>");
    output
}

fn render_routes(scene: &Scene, transform: &ViewportTransform) -> String {
    if scene.document.transports.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    output.push_str("<g id=\"transit\">");

    for route in &scene.document.transports {
        let style = route_style(route.kind, route.color);
        let route_label = escape_xml(&route.name);

        for link in route_links_for_route(scene, route) {
            for segment_id in &link.segment_ids {
                if let Some(segment) = scene.document.segments.iter().find(|segment| segment.id == *segment_id) {
                    let points = segment
                        .points
                        .iter()
                        .map(|point| transform.map(point.position))
                        .collect::<Vec<_>>();
                    let path = polyline_path(&points);
                    let _ = writeln!(
                        output,
                        "<path class=\"segment\" d=\"{path}\" stroke=\"{}\" stroke-width=\"{:.2}\" opacity=\"{:.3}\" data-route=\"{}\" data-link=\"{}-{}\"/>",
                        style.stroke.to_css(),
                        transform.map_size((segment.width * 0.6).max(style.stroke_width)),
                        style.opacity,
                        route_label,
                        link.start_node,
                        link.end_node,
                    );
                }
            }
        }
    }

    output.push_str("</g>");
    output
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

fn render_buildings(scene: &Scene, transform: &ViewportTransform, opacity: f32) -> String {
    let mut output = String::new();
    output.push_str("<g id=\"buildings\">");
    for area in &scene.document.buildings {
        if !area.bounds.intersects(&transform.world) {
            continue;
        }

        let style = building_style(&area.service, &area.subtype);
        let points = area
            .points
            .iter()
            .map(|point| transform.map(*point))
            .collect::<Vec<_>>();
        let _ = writeln!(
            output,
            "<polygon class=\"area\" points=\"{}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\" data-id=\"{}\" data-service=\"{}\" data-subservice=\"{}\"/>",
            polygon_points(&points),
            style.fill.to_css(),
            style.opacity * opacity,
            style.stroke.to_css(),
            style.opacity * opacity,
            transform.map_size(style.stroke_width),
            area.id,
            escape_xml(&area.service),
            escape_xml(&area.subtype),
        );
    }
    output.push_str("</g>");
    output
}

fn render_parks(scene: &Scene, transform: &ViewportTransform, opacity: f32) -> String {
    let mut output = String::new();
    if scene.document.parks.is_empty() {
        return output;
    }

    output.push_str("<g id=\"parks\">");
    let style = park_style();
    for area in &scene.document.parks {
        if !area.bounds.intersects(&transform.world) {
            continue;
        }

        let points = area
            .points
            .iter()
            .map(|point| transform.map(*point))
            .collect::<Vec<_>>();
        let _ = writeln!(
            output,
            "<polygon class=\"area\" points=\"{}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\" data-id=\"{}\" data-type=\"{}\"/>",
            polygon_points(&points),
            style.fill.to_css(),
            style.opacity * opacity,
            style.stroke.to_css(),
            style.opacity * opacity,
            transform.map_size(style.stroke_width),
            area.id,
            escape_xml(area.area_type.as_deref().unwrap_or("Park")),
        );
    }
    output.push_str("</g>");
    output
}

fn render_districts(scene: &Scene, transform: &ViewportTransform, opacity: f32) -> String {
    let mut output = String::new();
    if scene.document.districts.is_empty() {
        return output;
    }

    output.push_str("<g id=\"districts\">");
    let style = district_style();
    for district in &scene.document.districts {
        let (x, y) = transform.map(district.anchor);
        let _ = writeln!(
            output,
            "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\"/>",
            transform.map_size(3.0),
            style.fill.to_css(),
            style.opacity * opacity,
            style.stroke.to_css(),
            style.opacity * opacity,
            transform.map_size(style.stroke_width),
        );
    }
    output.push_str("</g>");
    output
}

fn render_nodes(scene: &Scene, transform: &ViewportTransform) -> String {
    let mut output = String::new();
    output.push_str("<g id=\"nodes\">");

    for node in &scene.document.nodes {
        if !transform.world.contains_point(node.position) {
            continue;
        }

        let elevation = elevation_state(
            node.underground,
            node.overground,
            false,
            node.elevation,
            scene.document.metadata.sea_level,
        );
        let style = node_style(&node.kind);
        let (x, y) = transform.map(node.position);
        let radius = transform.map_size(style.radius * if elevation == ElevationState::Elevated { 1.1 } else { 1.0 });
        let text_color = style.text.to_css();

        let shape = match style.shape {
            NodeShape::Circle => format!(
                "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{radius:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{:.2}\"/>",
                style.fill.to_css(),
                style.stroke.to_css(),
                transform.map_size(1.6)
            ),
            NodeShape::Square => format!(
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" rx=\"{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{:.2}\" transform=\"rotate(45 {:.2} {:.2})\"/>",
                x - radius,
                y - radius,
                radius * 2.0,
                radius * 2.0,
                radius * 0.22,
                style.fill.to_css(),
                style.stroke.to_css(),
                transform.map_size(1.5),
                x,
                y
            ),
            NodeShape::Diamond => format!(
                "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{:.2}\"/>",
                x,
                y - radius,
                x + radius,
                y,
                x,
                y + radius,
                x - radius,
                y,
                style.fill.to_css(),
                style.stroke.to_css(),
                transform.map_size(1.6)
            ),
            NodeShape::Hexagon => {
                let half = radius * 0.92;
                format!(
                    "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{:.2}\"/>",
                    x - half * 0.5,
                    y - radius,
                    x + half * 0.5,
                    y - radius,
                    x + radius,
                    y,
                    x + half * 0.5,
                    y + radius,
                    x - half * 0.5,
                    y + radius,
                    x - radius,
                    y,
                    style.fill.to_css(),
                    style.stroke.to_css(),
                    transform.map_size(1.6)
                )
            }
            NodeShape::Triangle => format!(
                "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{:.2}\"/>",
                x,
                y - radius,
                x + radius,
                y + radius,
                x - radius,
                y + radius,
                style.fill.to_css(),
                style.stroke.to_css(),
                transform.map_size(1.6)
            ),
        };
        output.push_str(&shape);

        let _ = writeln!(
            output,
            "<text class=\"text-ui\" x=\"{x:.2}\" y=\"{:.2}\" font-size=\"{:.2}\" text-anchor=\"middle\" dominant-baseline=\"central\" fill=\"{}\">{}</text>",
            y + radius * 0.1,
            transform.map_size(style.radius * 1.15),
            text_color,
            style.letter,
        );
    }

    output.push_str("</g>");
    output
}

fn render_labels(scene: &Scene, transform: &ViewportTransform) -> String {
    let mut output = String::new();
    if scene.document.districts.is_empty() {
        return output;
    }

    output.push_str("<g id=\"labels\">");
    for district in &scene.document.districts {
        let (x, y) = transform.map(district.anchor);
        let _ = writeln!(
            output,
            "<text class=\"text-ui label-shadow\" x=\"{x:.2}\" y=\"{y:.2}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"{:.2}\" fill=\"#f4f2ea\">{}</text>",
            transform.map_size(15.0),
            escape_xml(&district.name),
        );
    }
    output.push_str("</g>");
    output
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
*/
