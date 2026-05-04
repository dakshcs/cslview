use crate::export::MapMode;
use crate::model::{ElevationState, NodeKind, RgbaColor, SegmentClass, TransportKind};
use crate::theme::theme;

#[derive(Debug, Clone, Copy)]
pub enum NodeShape {
    Circle,
    Square,
    Diamond,
    Hexagon,
    Triangle,
}

#[derive(Debug, Clone, Copy)]
pub struct SegmentStyle {
    pub casing: RgbaColor,
    pub body: RgbaColor,
    pub casing_extra: f32,
    pub body_scale: f32,
    pub opacity: f32,
    pub dash: &'static [f32],
}

impl SegmentStyle {
    pub const fn new(
        casing: RgbaColor,
        body: RgbaColor,
        casing_extra: f32,
        body_scale: f32,
        opacity: f32,
        dash: &'static [f32],
    ) -> Self {
        Self {
            casing,
            body,
            casing_extra,
            body_scale,
            opacity,
            dash,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AreaStyle {
    pub fill: RgbaColor,
    pub stroke: RgbaColor,
    pub stroke_width: f32,
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct NodeStyle {
    pub fill: RgbaColor,
    pub stroke: RgbaColor,
    pub text: RgbaColor,
    pub radius: f32,
    pub shape: NodeShape,
    pub letter: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct RouteStyle {
    pub fill: RgbaColor,
    pub stroke: RgbaColor,
    pub stroke_width: f32,
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct DistrictStyle {
    pub fill: RgbaColor,
    pub stroke: RgbaColor,
    pub label: RgbaColor,
    pub halo: RgbaColor,
    pub stroke_width: f32,
    pub opacity: f32,
    pub label_size: f32,
    pub halo_radius: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct LabelStyle {
    pub fill: RgbaColor,
    pub shadow: RgbaColor,
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct ContourStyle {
    pub stroke: RgbaColor,
    pub width: f32,
    pub opacity: f32,
}

const EMPTY_DASH: &[f32] = &[];
const UNDERGROUND_DASH: &[f32] = &[18.0, 12.0];
const RAIL_DASH: &[f32] = &[12.0, 10.0];
const WIRE_DASH: &[f32] = &[8.0, 8.0];
const RUNWAY_DASH: &[f32] = &[30.0, 14.0];

fn is_colour(mode: MapMode) -> bool {
    matches!(mode, MapMode::Colour)
}

fn neutral(value: u8) -> RgbaColor {
    RgbaColor::rgb(value, value, value)
}

fn planning_segment_palette(class: &SegmentClass) -> SegmentStyle {
    match class {
        SegmentClass::Highway => SegmentStyle::new(neutral(26), neutral(0), 1.8, 0.82, 1.0, EMPTY_DASH),
        SegmentClass::LargeRoad => SegmentStyle::new(neutral(58), neutral(26), 1.4, 0.78, 0.95, EMPTY_DASH),
        SegmentClass::MediumRoad => SegmentStyle::new(neutral(85), neutral(51), 1.2, 0.72, 0.9, EMPTY_DASH),
        SegmentClass::SmallRoad => SegmentStyle::new(neutral(119), neutral(85), 1.0, 0.65, 0.85, EMPTY_DASH),
        SegmentClass::PedestrianStreet => SegmentStyle::new(neutral(136), neutral(102), 1.0, 0.58, 0.82, EMPTY_DASH),
        SegmentClass::PedestrianPath | SegmentClass::PedestrianWay => SegmentStyle::new(neutral(160), neutral(120), 0.9, 0.52, 0.76, EMPTY_DASH),
        SegmentClass::TrainTrack => SegmentStyle::new(neutral(102), neutral(68), 1.2, 0.82, 0.9, RAIL_DASH),
        SegmentClass::MetroTrack => SegmentStyle::new(neutral(110), neutral(78), 1.2, 0.76, 0.9, RAIL_DASH),
        SegmentClass::ElectricityWire => SegmentStyle::new(neutral(170), neutral(118), 0.9, 0.26, 0.85, WIRE_DASH),
        SegmentClass::AirplaneRunway => SegmentStyle::new(neutral(186), neutral(80), 1.6, 1.10, 0.9, RUNWAY_DASH),
        SegmentClass::AirplaneLine | SegmentClass::HelicopterPath | SegmentClass::BlimpPath => {
            SegmentStyle::new(neutral(140), neutral(90), 1.2, 0.44, 0.85, EMPTY_DASH)
        }
        SegmentClass::FloodWall => SegmentStyle::new(neutral(110), neutral(70), 1.2, 0.54, 0.9, EMPTY_DASH),
        SegmentClass::BeautificationItem => SegmentStyle::new(neutral(144), neutral(96), 1.0, 0.54, 0.84, EMPTY_DASH),
        SegmentClass::Miscellaneous | SegmentClass::Unknown(_) => SegmentStyle::new(neutral(128), neutral(80), 1.0, 0.6, 0.85, EMPTY_DASH),
    }
}

fn planning_area_style(fill: u8, stroke: u8, opacity: f32) -> AreaStyle {
    AreaStyle {
        fill: neutral(fill),
        stroke: neutral(stroke),
        stroke_width: 1.0,
        opacity,
    }
}

fn planning_node_style(radius: f32, shape: NodeShape, letter: &'static str) -> NodeStyle {
    NodeStyle {
        fill: neutral(238),
        stroke: neutral(96),
        text: neutral(24),
        radius,
        shape,
        letter,
    }
}

fn segment_palette(palette: &crate::theme::SegmentPalette, dash: &'static [f32]) -> SegmentStyle {
    SegmentStyle::new(
        palette.casing.to_color(),
        palette.body.to_color(),
        palette.casing_extra,
        palette.body_scale,
        palette.opacity,
        dash,
    )
}

fn area_palette(palette: &crate::theme::AreaPalette) -> AreaStyle {
    AreaStyle {
        fill: palette.fill.to_color(),
        stroke: palette.stroke.to_color(),
        stroke_width: palette.stroke_width,
        opacity: palette.opacity,
    }
}

fn node_palette(palette: &crate::theme::NodePalette, shape: NodeShape, letter: &'static str) -> NodeStyle {
    NodeStyle {
        fill: palette.fill.to_color(),
        stroke: palette.stroke.to_color(),
        text: palette.text.to_color(),
        radius: palette.radius,
        shape,
        letter,
    }
}

pub fn segment_style(class: &SegmentClass, elevation: ElevationState, mode: MapMode) -> SegmentStyle {
    if !is_colour(mode) {
        return planning_segment_palette(class);
    }

    let roads = &theme().roads;
    let mut style = match class {
        SegmentClass::Highway => segment_palette(&roads.highway, EMPTY_DASH),
        SegmentClass::LargeRoad => segment_palette(&roads.large_road, EMPTY_DASH),
        SegmentClass::MediumRoad => segment_palette(&roads.medium_road, EMPTY_DASH),
        SegmentClass::SmallRoad => segment_palette(&roads.small_road, EMPTY_DASH),
        SegmentClass::PedestrianStreet => segment_palette(&roads.pedestrian_street, EMPTY_DASH),
        SegmentClass::PedestrianPath => segment_palette(&roads.pedestrian_path, EMPTY_DASH),
        SegmentClass::PedestrianWay => segment_palette(&roads.pedestrian_way, EMPTY_DASH),
        SegmentClass::TrainTrack => segment_palette(&roads.train_track, RAIL_DASH),
        SegmentClass::MetroTrack => segment_palette(&roads.metro_track, RAIL_DASH),
        SegmentClass::ElectricityWire => segment_palette(&roads.electricity_wire, WIRE_DASH),
        SegmentClass::AirplaneRunway => segment_palette(&roads.airplane_runway, RUNWAY_DASH),
        SegmentClass::AirplaneLine => segment_palette(&roads.airplane_line, EMPTY_DASH),
        SegmentClass::HelicopterPath => segment_palette(&roads.helicopter_path, EMPTY_DASH),
        SegmentClass::BlimpPath => segment_palette(&roads.blimp_path, EMPTY_DASH),
        SegmentClass::FloodWall => segment_palette(&roads.flood_wall, EMPTY_DASH),
        SegmentClass::BeautificationItem => segment_palette(&roads.beautification, EMPTY_DASH),
        SegmentClass::Miscellaneous | SegmentClass::Unknown(_) => segment_palette(&roads.miscellaneous, EMPTY_DASH),
    };

    match elevation {
        ElevationState::Underground => {
            style.body = style.body.mix(roads.underground_body.to_color(), 0.5);
            style.casing = style.casing.mix(roads.underground_casing.to_color(), 0.5);
            style.opacity *= 0.7;
            if style.dash.is_empty() {
                style.dash = UNDERGROUND_DASH;
            }
        }
        ElevationState::Elevated => {
            style.body = style.body.mix(roads.elevated_body.to_color(), 0.12);
            style.casing = style.casing.mix(roads.elevated_casing.to_color(), 0.06);
            style.opacity *= 1.0;
        }
        ElevationState::Surface => {}
    }

    style
}

pub fn route_style(kind: TransportKind, color: RgbaColor, mode: MapMode) -> RouteStyle {
    let routes = &theme().routes;
    let (stroke_width, opacity) = match kind {
        TransportKind::Metro => (routes.metro_width, routes.metro_opacity),
        TransportKind::Pedestrian => (routes.pedestrian_width, routes.pedestrian_opacity),
        TransportKind::Other => (routes.other_width, routes.other_opacity),
    };

    if !is_colour(mode) {
        let mono = neutral(color.luminance().round().clamp(0.0, 255.0) as u8);
        return RouteStyle {
            fill: mono.with_alpha(48),
            stroke: mono,
            stroke_width,
            opacity,
        };
    }

    RouteStyle {
        fill: color.with_alpha(50),
        stroke: color,
        stroke_width,
        opacity,
    }
}

pub fn node_style(kind: &NodeKind, mode: MapMode) -> NodeStyle {
    if !is_colour(mode) {
        return match kind {
            NodeKind::TrainStation => planning_node_style(15.0, NodeShape::Square, "T"),
            NodeKind::MetroStation => planning_node_style(15.0, NodeShape::Hexagon, "M"),
            NodeKind::Harbor => planning_node_style(15.0, NodeShape::Circle, "H"),
            NodeKind::Airport => planning_node_style(15.0, NodeShape::Triangle, "A"),
            NodeKind::Electricity => planning_node_style(14.0, NodeShape::Diamond, "E"),
            NodeKind::Beautification => planning_node_style(13.0, NodeShape::Diamond, "P"),
            NodeKind::Water => planning_node_style(12.0, NodeShape::Circle, "W"),
            NodeKind::Road => planning_node_style(10.0, NodeShape::Circle, "R"),
            NodeKind::PublicTransport => planning_node_style(12.0, NodeShape::Circle, "P"),
            NodeKind::Unknown(_, _) => planning_node_style(10.0, NodeShape::Circle, "?"),
        };
    }

    let nodes = &theme().nodes;
    match kind {
        NodeKind::TrainStation => node_palette(&nodes.train_station, NodeShape::Square, "T"),
        NodeKind::MetroStation => node_palette(&nodes.metro_station, NodeShape::Hexagon, "M"),
        NodeKind::Harbor => node_palette(&nodes.harbor, NodeShape::Circle, "H"),
        NodeKind::Airport => node_palette(&nodes.airport, NodeShape::Triangle, "A"),
        NodeKind::Electricity => node_palette(&nodes.electricity, NodeShape::Diamond, "E"),
        NodeKind::Beautification => node_palette(&nodes.beautification, NodeShape::Diamond, "P"),
        NodeKind::Water => node_palette(&nodes.water, NodeShape::Circle, "W"),
        NodeKind::Road => node_palette(&nodes.road, NodeShape::Circle, "R"),
        NodeKind::PublicTransport => node_palette(&nodes.public_transport, NodeShape::Circle, "P"),
        NodeKind::Unknown(_, _) => node_palette(&nodes.unknown, NodeShape::Circle, "?"),
    }
}

pub fn building_style(service: &str, subtype: &str, mode: MapMode) -> AreaStyle {
    if !is_colour(mode) {
        return AreaStyle {
            fill: neutral(232),
            stroke: neutral(153),
            stroke_width: 1.0,
            opacity: 0.9,
        };
    }

    let buildings = &theme().buildings;
    match (service.trim(), subtype.trim()) {
        ("Residential", "ResidentialLowEco") => area_palette(&buildings.residential_low_eco),
        ("Residential", "ResidentialHighEco") => area_palette(&buildings.residential_high_eco),
        ("Residential", _) => area_palette(&buildings.residential),
        ("Commercial", "CommercialEco") => area_palette(&buildings.commercial_eco),
        ("Commercial", "CommercialLow") => area_palette(&buildings.commercial_low),
        ("Commercial", _) => area_palette(&buildings.commercial),
        ("Industrial", "IndustrialForestry") | ("PlayerIndustry", "PlayerIndustryForestry") =>
            area_palette(&buildings.industrial_forestry),
        ("Industrial", _) | ("PlayerIndustry", _) => area_palette(&buildings.industrial),
        ("Office", _) => area_palette(&buildings.office),
        ("Education", _) => area_palette(&buildings.education),
        ("HealthCare", _) => area_palette(&buildings.health),
        ("FireDepartment", _) => area_palette(&buildings.fire),
        ("PoliceDepartment", _) => area_palette(&buildings.police),
        ("Electricity", _) => area_palette(&buildings.electricity),
        ("Water", _) => area_palette(&buildings.water),
        ("Beautification", _) => area_palette(&buildings.beautification),
        ("PublicTransport", _) => area_palette(&buildings.public_transport),
        ("Hotel", _) => area_palette(&buildings.hotel),
        _ => area_palette(&buildings.default),
    }
}

pub fn park_style(area_type: Option<&str>, mode: MapMode) -> AreaStyle {
    if !is_colour(mode) {
        return match area_type.map(|value| value.trim()) {
            Some("PedestrianZone") => planning_area_style(226, 150, 0.78),
            Some("Forestry") => planning_area_style(218, 146, 0.72),
            _ => planning_area_style(234, 162, 0.74),
        };
    }

    let parks = &theme().parks;
    match area_type.map(|value| value.trim()) {
        Some("PedestrianZone") => area_palette(&parks.pedestrian_zone),
        Some("Forestry") => area_palette(&parks.forestry),
        _ => area_palette(&parks.park),
    }
}

pub fn park_path_style(mode: MapMode) -> SegmentStyle {
    if !is_colour(mode) {
        return SegmentStyle::new(neutral(112), neutral(72), 1.2, 0.5, 0.9, EMPTY_DASH);
    }

    let path = &theme().parks.path;
    SegmentStyle::new(
        path.casing.to_color(),
        path.body.to_color(),
        path.casing_extra,
        path.body_scale,
        path.opacity,
        EMPTY_DASH,
    )
}

pub fn district_style(mode: MapMode) -> DistrictStyle {
    if !is_colour(mode) {
        return DistrictStyle {
            fill: RgbaColor::rgba(232, 232, 232, 190),
            stroke: neutral(132),
            label: neutral(24),
            halo: RgbaColor::rgba(255, 255, 255, 96),
            stroke_width: 1.2,
            opacity: 0.38,
            label_size: 15.0,
            halo_radius: 18.0,
        };
    }

    let district = &theme().districts;
    DistrictStyle {
        fill: district.fill.to_color(),
        stroke: district.stroke.to_color(),
        label: district.label.to_color(),
        halo: district.halo.to_color(),
        stroke_width: district.stroke_width,
        opacity: district.opacity,
        label_size: district.label_size,
        halo_radius: district.halo_radius,
    }
}

pub fn label_style(mode: MapMode) -> LabelStyle {
    if !is_colour(mode) {
        return LabelStyle {
            fill: neutral(20),
            shadow: RgbaColor::rgba(0, 0, 0, 0),
            opacity: 0.9,
        };
    }

    let labels = &theme().labels;
    LabelStyle {
        fill: labels.fill.to_color(),
        shadow: labels.shadow.to_color(),
        opacity: labels.opacity,
    }
}

pub fn contour_style(mode: MapMode, is_index: bool) -> ContourStyle {
    if !is_colour(mode) {
        return if is_index {
            ContourStyle {
                stroke: neutral(36),
                width: 1.4,
                opacity: 0.8,
            }
        } else {
            ContourStyle {
                stroke: neutral(96),
                width: 0.75,
                opacity: 0.55,
            }
        };
    }

    if is_index {
        ContourStyle {
            stroke: RgbaColor::rgb(88, 66, 44),
            width: 1.4,
            opacity: 0.72,
        }
    } else {
        ContourStyle {
            stroke: RgbaColor::rgb(132, 104, 72),
            width: 0.75,
            opacity: 0.46,
        }
    }
}

pub fn background_color(mode: MapMode) -> RgbaColor {
    if is_colour(mode) {
        RgbaColor::rgb(13, 16, 21)
    } else {
        RgbaColor::rgb(248, 247, 242)
    }
}

pub fn elevation_state(underground: bool, overground: bool, elevated_hint: bool, max_elevation: f32, sea_level: f32) -> ElevationState {
    if underground || max_elevation < sea_level - 5.0 {
        ElevationState::Underground
    } else if elevated_hint || overground || max_elevation > sea_level + 18.0 {
        ElevationState::Elevated
    } else {
        ElevationState::Surface
    }
}
