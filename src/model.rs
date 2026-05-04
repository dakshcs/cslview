#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldPoint {
    pub x: f32,
    pub y: f32,
}

impl WorldPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Self) -> f32 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldBounds {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    empty: bool,
}

impl Default for WorldBounds {
    fn default() -> Self {
        Self {
            min_x: f32::INFINITY,
            min_y: f32::INFINITY,
            max_x: f32::NEG_INFINITY,
            max_y: f32::NEG_INFINITY,
            empty: true,
        }
    }
}

impl WorldBounds {
    pub fn from_corners(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
            empty: false,
        }
    }

    pub fn include_point(&mut self, point: WorldPoint) {
        if self.empty {
            self.min_x = point.x;
            self.max_x = point.x;
            self.min_y = point.y;
            self.max_y = point.y;
            self.empty = false;
            return;
        }

        self.min_x = self.min_x.min(point.x);
        self.max_x = self.max_x.max(point.x);
        self.min_y = self.min_y.min(point.y);
        self.max_y = self.max_y.max(point.y);
    }

    pub fn include_bounds(&mut self, other: &WorldBounds) {
        if other.empty {
            return;
        }

        self.include_point(WorldPoint::new(other.min_x, other.min_y));
        self.include_point(WorldPoint::new(other.max_x, other.max_y));
    }

    pub fn width(&self) -> f32 {
        if self.empty {
            1.0
        } else {
            (self.max_x - self.min_x).max(1.0)
        }
    }

    pub fn height(&self) -> f32 {
        if self.empty {
            1.0
        } else {
            (self.max_y - self.min_y).max(1.0)
        }
    }

    pub fn center(&self) -> WorldPoint {
        if self.empty {
            WorldPoint::new(0.0, 0.0)
        } else {
            WorldPoint::new((self.min_x + self.max_x) * 0.5, (self.min_y + self.max_y) * 0.5)
        }
    }

    pub fn expand(&self, amount: f32) -> Self {
        if self.empty {
            return *self;
        }

        Self {
            min_x: self.min_x - amount,
            min_y: self.min_y - amount,
            max_x: self.max_x + amount,
            max_y: self.max_y + amount,
            empty: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.empty
    }

    pub fn intersects(&self, other: &WorldBounds) -> bool {
        if self.empty || other.empty {
            return false;
        }

        self.min_x <= other.max_x
            && self.max_x >= other.min_x
            && self.min_y <= other.max_y
            && self.max_y >= other.min_y
    }

    pub fn contains_point(&self, point: WorldPoint) -> bool {
        if self.empty {
            return false;
        }

        point.x >= self.min_x
            && point.x <= self.max_x
            && point.y >= self.min_y
            && point.y <= self.max_y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RgbaColor {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    pub fn to_css(self) -> String {
        format!("rgba({}, {}, {}, {:.3})", self.r, self.g, self.b, self.a as f32 / 255.0)
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub fn luminance(self) -> f32 {
        0.299 * self.r as f32 + 0.587 * self.g as f32 + 0.114 * self.b as f32
    }

    pub fn is_dark(self) -> bool {
        self.luminance() < 128.0
    }

    pub fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (a as f32 * (1.0 - t) + b as f32 * t).round() as u8;
        Self {
            r: lerp(self.r, other.r),
            g: lerp(self.g, other.g),
            b: lerp(self.b, other.b),
            a: lerp(self.a, other.a),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SegmentClass {
    Highway,
    LargeRoad,
    MediumRoad,
    SmallRoad,
    PedestrianStreet,
    PedestrianPath,
    PedestrianWay,
    TrainTrack,
    MetroTrack,
    ElectricityWire,
    AirplaneRunway,
    AirplaneLine,
    HelicopterPath,
    BlimpPath,
    FloodWall,
    BeautificationItem,
    Miscellaneous,
    Unknown(String),
}

impl SegmentClass {
    pub fn from_icls(icls: &str) -> Self {
        let normalized = icls.trim().to_ascii_lowercase();

        if normalized.contains("highway") {
            Self::Highway
        } else if normalized.contains("large road") {
            Self::LargeRoad
        } else if normalized.contains("medium road") {
            Self::MediumRoad
        } else if normalized.contains("small road") {
            Self::SmallRoad
        } else if normalized.contains("pedestrian street") {
            Self::PedestrianStreet
        } else if normalized.contains("pedestrian path") {
            Self::PedestrianPath
        } else if normalized.contains("pedestrian way") {
            Self::PedestrianWay
        } else if normalized.contains("train track") {
            Self::TrainTrack
        } else if normalized.contains("metro track") {
            Self::MetroTrack
        } else if normalized.contains("electricity wire") || normalized.contains("power line") {
            Self::ElectricityWire
        } else if normalized.contains("airplane runway") {
            Self::AirplaneRunway
        } else if normalized.contains("airplane line") {
            Self::AirplaneLine
        } else if normalized.contains("helicopter path") {
            Self::HelicopterPath
        } else if normalized.contains("blimp path") {
            Self::BlimpPath
        } else if normalized.contains("flood wall") {
            Self::FloodWall
        } else if normalized.contains("beautification item")
            || normalized.contains("fence")
            || normalized.contains("decoration")
        {
            Self::BeautificationItem
        } else if normalized.is_empty() {
            Self::Miscellaneous
        } else {
            Self::Unknown(icls.to_owned())
        }
    }

    pub fn raw_label(&self) -> &str {
        match self {
            Self::Highway => "Highway",
            Self::LargeRoad => "Large Road",
            Self::MediumRoad => "Medium Road",
            Self::SmallRoad => "Small Road",
            Self::PedestrianStreet => "Pedestrian Street",
            Self::PedestrianPath => "Pedestrian Path",
            Self::PedestrianWay => "Pedestrian Way",
            Self::TrainTrack => "Train Track",
            Self::MetroTrack => "Metro Track",
            Self::ElectricityWire => "Electricity Wire",
            Self::AirplaneRunway => "Airplane Runway",
            Self::AirplaneLine => "Airplane Line",
            Self::HelicopterPath => "Helicopter Path",
            Self::BlimpPath => "Blimp Path",
            Self::FloodWall => "Flood Wall",
            Self::BeautificationItem => "Beautification Item",
            Self::Miscellaneous => "Miscellaneous",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Road,
    Water,
    TrainStation,
    MetroStation,
    Harbor,
    Airport,
    Electricity,
    Beautification,
    PublicTransport,
    Unknown(String, String),
}

impl NodeKind {
    pub fn from_service(service: &str, subtype: &str) -> Self {
        match (service.trim(), subtype.trim()) {
            ("Road", _) => Self::Road,
            ("Water", _) => Self::Water,
            ("PublicTransport", "PublicTransportTrain") => Self::TrainStation,
            ("PublicTransport", "PublicTransportMetro") => Self::MetroStation,
            ("PublicTransport", "PublicTransportShip") => Self::Harbor,
            ("PublicTransport", "PublicTransportPlane") => Self::Airport,
            ("PublicTransport", _) => Self::PublicTransport,
            ("Electricity", _) => Self::Electricity,
            ("Beautification", _) => Self::Beautification,
            (service, subtype) => Self::Unknown(service.to_owned(), subtype.to_owned()),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Road => "Road",
            Self::Water => "Water",
            Self::TrainStation => "Train Station",
            Self::MetroStation => "Metro Station",
            Self::Harbor => "Harbor",
            Self::Airport => "Airport",
            Self::Electricity => "Electricity",
            Self::Beautification => "Beautification",
            Self::PublicTransport => "Public Transport",
            Self::Unknown(service, subtype) => {
                if subtype.is_empty() {
                    service.as_str()
                } else {
                    subtype.as_str()
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevationState {
    Underground,
    Surface,
    Elevated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Metro,
    Pedestrian,
    Other,
}

impl TransportKind {
    pub fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "metro" => Self::Metro,
            "pedestrian" => Self::Pedestrian,
            _ => Self::Other,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Metro => "Metro",
            Self::Pedestrian => "Pedestrian",
            Self::Other => "Other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterKind {
    Terrain,
    Forest,
}

#[derive(Debug, Clone)]
pub struct MapMetadata {
    pub city: String,
    pub generated: Option<String>,
    pub sea_level: f32,
}

#[derive(Debug, Clone)]
pub struct NodeRecord {
    pub id: u64,
    pub name: Option<String>,
    pub position: WorldPoint,
    pub elevation: f32,
    pub elevation_hint: f32,
    pub service: String,
    pub subtype: String,
    pub kind: NodeKind,
    pub underground: bool,
    pub overground: bool,
    pub dist: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SegmentPoint {
    pub position: WorldPoint,
    pub elevation: f32,
}

#[derive(Debug, Clone)]
pub struct SegmentRecord {
    pub id: u64,
    pub name: Option<String>,
    pub start_node: u64,
    pub end_node: u64,
    pub raw_class: String,
    pub class: SegmentClass,
    pub width: f32,
    pub points: Vec<SegmentPoint>,
    pub bounds: WorldBounds,
    pub route_segment_ids: Vec<u64>,
    pub underground: bool,
    pub overground: bool,
    pub elevated_hint: bool,
}

#[derive(Debug, Clone)]
pub struct AreaRecord {
    pub id: u64,
    pub name: Option<String>,
    pub service: String,
    pub subtype: String,
    pub area_type: Option<String>,
    pub points: Vec<WorldPoint>,
    pub bounds: WorldBounds,
    pub elevation: f32,
}

#[derive(Debug, Clone)]
pub struct DistrictRecord {
    pub id: u64,
    pub name: String,
    pub anchor: WorldPoint,
}

#[derive(Debug, Clone)]
pub struct TransportRecord {
    pub id: String,
    pub name: String,
    pub kind: TransportKind,
    pub color: RgbaColor,
    pub stops: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct RouteLinkRecord {
    pub start_node: u64,
    pub end_node: u64,
    pub segment_ids: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct RasterSource {
    pub kind: RasterKind,
    pub packed: String,
    pub rows: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MapDocument {
    pub metadata: MapMetadata,
    pub bounds: WorldBounds,
    pub nodes: Vec<NodeRecord>,
    pub segments: Vec<SegmentRecord>,
    pub buildings: Vec<AreaRecord>,
    pub parks: Vec<AreaRecord>,
    pub districts: Vec<DistrictRecord>,
    pub transports: Vec<TransportRecord>,
    pub route_links: Vec<RouteLinkRecord>,
    pub terrain: Option<RasterSource>,
    pub forests: Option<RasterSource>,
}

impl MapDocument {
    pub fn new() -> Self {
        Self {
            metadata: MapMetadata {
                city: String::new(),
                generated: None,
                sea_level: 0.0,
            },
            bounds: WorldBounds::default(),
            nodes: Vec::new(),
            segments: Vec::new(),
            buildings: Vec::new(),
            parks: Vec::new(),
            districts: Vec::new(),
            transports: Vec::new(),
            route_links: Vec::new(),
            terrain: None,
            forests: None,
        }
    }
}

impl Default for MapDocument {
    fn default() -> Self {
        Self::new()
    }
}
