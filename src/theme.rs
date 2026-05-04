use std::sync::OnceLock;

use serde::Deserialize;

use crate::model::RgbaColor;

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ColorSpec {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default = "ColorSpec::default_alpha")]
    pub a: u8,
}

impl ColorSpec {
    const fn default_alpha() -> u8 {
        255
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_color(self) -> RgbaColor {
        RgbaColor::rgba(self.r, self.g, self.b, self.a)
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct SegmentPalette {
    pub casing: ColorSpec,
    pub body: ColorSpec,
    pub casing_extra: f32,
    pub body_scale: f32,
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AreaPalette {
    pub fill: ColorSpec,
    pub stroke: ColorSpec,
    pub stroke_width: f32,
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct NodePalette {
    pub fill: ColorSpec,
    pub stroke: ColorSpec,
    pub text: ColorSpec,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct TerrainPalette {
    pub water_light: ColorSpec,
    pub water_dark: ColorSpec,
    pub land_low: ColorSpec,
    pub land_mid: ColorSpec,
    pub land_high: ColorSpec,
    pub snow: ColorSpec,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ForestPalette {
    pub color: ColorSpec,
    pub blur_radius: usize,
    pub blur_passes: usize,
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct RoadTheme {
    pub highway: SegmentPalette,
    pub large_road: SegmentPalette,
    pub medium_road: SegmentPalette,
    pub small_road: SegmentPalette,
    pub pedestrian_street: SegmentPalette,
    pub pedestrian_path: SegmentPalette,
    pub pedestrian_way: SegmentPalette,
    pub train_track: SegmentPalette,
    pub metro_track: SegmentPalette,
    pub electricity_wire: SegmentPalette,
    pub airplane_runway: SegmentPalette,
    pub airplane_line: SegmentPalette,
    pub helicopter_path: SegmentPalette,
    pub blimp_path: SegmentPalette,
    pub flood_wall: SegmentPalette,
    pub beautification: SegmentPalette,
    pub miscellaneous: SegmentPalette,
    pub underground_casing: ColorSpec,
    pub underground_body: ColorSpec,
    pub elevated_casing: ColorSpec,
    pub elevated_body: ColorSpec,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct BuildingTheme {
    pub residential_low_eco: AreaPalette,
    pub residential_high_eco: AreaPalette,
    pub residential: AreaPalette,
    pub commercial_eco: AreaPalette,
    pub commercial_low: AreaPalette,
    pub commercial: AreaPalette,
    pub industrial_forestry: AreaPalette,
    pub industrial: AreaPalette,
    pub office: AreaPalette,
    pub education: AreaPalette,
    pub health: AreaPalette,
    pub fire: AreaPalette,
    pub police: AreaPalette,
    pub electricity: AreaPalette,
    pub water: AreaPalette,
    pub beautification: AreaPalette,
    pub public_transport: AreaPalette,
    pub hotel: AreaPalette,
    pub default: AreaPalette,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ParkTheme {
    pub park: AreaPalette,
    pub pedestrian_zone: AreaPalette,
    pub forestry: AreaPalette,
    pub path: SegmentPalette,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct DistrictTheme {
    pub fill: ColorSpec,
    pub stroke: ColorSpec,
    pub label: ColorSpec,
    pub halo: ColorSpec,
    pub stroke_width: f32,
    pub opacity: f32,
    pub label_size: f32,
    pub halo_radius: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct LabelTheme {
    pub fill: ColorSpec,
    pub shadow: ColorSpec,
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct NodeTheme {
    pub road: NodePalette,
    pub water: NodePalette,
    pub train_station: NodePalette,
    pub metro_station: NodePalette,
    pub harbor: NodePalette,
    pub airport: NodePalette,
    pub electricity: NodePalette,
    pub beautification: NodePalette,
    pub public_transport: NodePalette,
    pub unknown: NodePalette,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct RouteTheme {
    pub metro_opacity: f32,
    pub pedestrian_opacity: f32,
    pub other_opacity: f32,
    pub metro_width: f32,
    pub pedestrian_width: f32,
    pub other_width: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Theme {
    pub terrain: TerrainPalette,
    pub forest: ForestPalette,
    pub roads: RoadTheme,
    pub buildings: BuildingTheme,
    pub parks: ParkTheme,
    pub districts: DistrictTheme,
    pub labels: LabelTheme,
    pub nodes: NodeTheme,
    pub routes: RouteTheme,
}

impl Theme {
    pub fn load() -> Self {
        let toml = include_str!("../assets/style.toml");
        toml::from_str(toml).expect("invalid assets/style.toml")
    }
}

static THEME: OnceLock<Theme> = OnceLock::new();

pub fn theme() -> &'static Theme {
    THEME.get_or_init(Theme::load)
}