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

const EMPTY_DASH: &[f32] = &[];
const UNDERGROUND_DASH: &[f32] = &[18.0, 12.0];
const RAIL_DASH: &[f32] = &[12.0, 10.0];
const WIRE_DASH: &[f32] = &[8.0, 8.0];
const RUNWAY_DASH: &[f32] = &[30.0, 14.0];

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

pub fn segment_style(class: &SegmentClass, elevation: ElevationState) -> SegmentStyle {
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

pub fn route_style(kind: TransportKind, color: RgbaColor) -> RouteStyle {
    let routes = &theme().routes;
    let (stroke_width, opacity) = match kind {
        TransportKind::Metro => (routes.metro_width, routes.metro_opacity),
        TransportKind::Pedestrian => (routes.pedestrian_width, routes.pedestrian_opacity),
        TransportKind::Other => (routes.other_width, routes.other_opacity),
    };

    RouteStyle {
        fill: color.with_alpha(50),
        stroke: color,
        stroke_width,
        opacity,
    }
}

pub fn node_style(kind: &NodeKind) -> NodeStyle {
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

pub fn building_style(service: &str, subtype: &str) -> AreaStyle {
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

pub fn park_style(area_type: Option<&str>) -> AreaStyle {
    let parks = &theme().parks;
    match area_type.map(|value| value.trim()) {
        Some("PedestrianZone") => area_palette(&parks.pedestrian_zone),
        Some("Forestry") => area_palette(&parks.forestry),
        _ => area_palette(&parks.park),
    }
}

pub fn park_path_style() -> SegmentStyle {
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

pub fn district_style() -> AreaStyle {
    let district = &theme().districts;
    AreaStyle {
        fill: district.fill.to_color(),
        stroke: district.stroke.to_color(),
        stroke_width: district.stroke_width,
        opacity: district.opacity,
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
