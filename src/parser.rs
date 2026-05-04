use std::io::BufRead;

use anyhow::{anyhow, Context, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::model::{
    AreaRecord, DistrictRecord, MapDocument, NodeKind, NodeRecord, RasterKind,
    RasterSource, RgbaColor, RouteLinkRecord, SegmentClass, SegmentPoint, SegmentRecord,
    TransportKind, TransportRecord, WorldBounds, WorldPoint,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextTarget {
    City,
    Generated,
    SeaLevel,
    SegmentName,
    AreaName,
    AreaType,
    RouteLinkSegmentId,
    TerrainData,
    ForestRow,
}

#[derive(Debug, Clone)]
struct NodeDraft {
    id: u64,
    name: Option<String>,
    position: Option<WorldPoint>,
    elevation: f32,
    elevation_hint: f32,
    service: String,
    subtype: String,
    underground: bool,
    overground: bool,
    dist: Option<u64>,
}

#[derive(Debug, Clone)]
struct SegmentDraft {
    id: u64,
    name: Option<String>,
    start_node: u64,
    end_node: u64,
    raw_class: String,
    class: SegmentClass,
    width: f32,
    points: Vec<SegmentPoint>,
    bounds: WorldBounds,
    route_segment_ids: Vec<u64>,
    underground: bool,
    overground: bool,
    elevated_hint: bool,
}

#[derive(Debug, Clone)]
struct AreaDraft {
    id: u64,
    name: Option<String>,
    service: String,
    subtype: String,
    area_type: Option<String>,
    points: Vec<WorldPoint>,
    bounds: WorldBounds,
    elevation: f32,
}

#[derive(Debug, Clone)]
struct DistrictDraft {
    id: u64,
    name: String,
    anchor: Option<WorldPoint>,
}

#[derive(Debug, Clone)]
struct RouteDraft {
    id: String,
    name: String,
    kind: TransportKind,
    color: RgbaColor,
    stops: Vec<u64>,
}

#[derive(Debug, Clone)]
struct RouteLinkDraft {
    start_node: u64,
    end_node: u64,
    segment_ids: Vec<u64>,
}

#[derive(Default)]
struct ParseState {
    path: Vec<String>,
    current_text_target: Option<TextTarget>,
    text_buffer: String,
    current_node: Option<NodeDraft>,
    current_segment: Option<SegmentDraft>,
    current_area: Option<AreaDraft>,
    current_district: Option<DistrictDraft>,
    current_route: Option<RouteDraft>,
    current_route_link: Option<RouteLinkDraft>,
    current_terrain: Option<String>,
    current_forest_row: Option<String>,
    document: MapDocument,
}

fn local_name(name: &[u8]) -> String {
    match name {
        [] => String::new(),
        bytes => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn attr_value(element: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref() == key)
        .map(|attribute| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
}

fn parse_bool(value: Option<String>) -> bool {
    matches!(value.as_deref(), Some("true") | Some("1"))
}

fn parse_u64(value: Option<String>) -> Option<u64> {
    value.and_then(|text| text.parse::<u64>().ok())
}

fn parse_f32(value: Option<String>) -> Option<f32> {
    value.and_then(|text| text.parse::<f32>().ok())
}

fn parse_point(element: &BytesStart<'_>) -> Option<WorldPoint> {
    let x = parse_f32(attr_value(element, b"x"))?;
    let z = parse_f32(attr_value(element, b"z"))?;
    Some(WorldPoint::new(x, -z))
}

fn parse_segment_point(element: &BytesStart<'_>) -> Option<SegmentPoint> {
    let x = parse_f32(attr_value(element, b"x"))?;
    let z = parse_f32(attr_value(element, b"z"))?;
    let elevation = parse_f32(attr_value(element, b"y"))?;
    Some(SegmentPoint {
        position: WorldPoint::new(x, -z),
        elevation,
    })
}

fn parse_service_node_kind(service: &str, subtype: &str) -> NodeKind {
    NodeKind::from_service(service, subtype)
}

fn parse_route_kind(value: &str) -> TransportKind {
    TransportKind::from_str(value)
}

pub fn parse_csl_xml<R: BufRead>(reader: R) -> Result<MapDocument> {
    let mut reader = Reader::from_reader(reader);
    reader.config_mut().trim_text(true);

    let mut state = ParseState {
        document: MapDocument::new(),
        ..Default::default()
    };
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => handle_start(&mut state, &event),
            Ok(Event::Empty(event)) => {
                handle_start(&mut state, &event);
                handle_end(&mut state, &local_name(event.name().as_ref()));
            }
            Ok(Event::End(event)) => handle_end(&mut state, &local_name(event.name().as_ref())),
            Ok(Event::Text(event)) => handle_text(&mut state, &String::from_utf8_lossy(event.as_ref())),
            Ok(Event::CData(event)) => handle_text(&mut state, &String::from_utf8_lossy(event.as_ref())),
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(anyhow!(error)).context("failed to parse CSL XML"),
        }
    }

    finalize(&mut state)?;
    Ok(state.document)
}

pub fn parse_csl_file(path: impl AsRef<std::path::Path>) -> Result<MapDocument> {
    let file = std::fs::File::open(path.as_ref())
        .with_context(|| format!("failed to open {}", path.as_ref().display()))?;
    let reader = std::io::BufReader::new(file);
    parse_csl_xml(reader)
}

fn push_text_target(state: &mut ParseState, target: TextTarget) {
    state.current_text_target = Some(target);
    state.text_buffer.clear();
}

fn path_contains(state: &ParseState, name: &str) -> bool {
    state.path.iter().any(|segment| segment == name)
}

fn handle_start(state: &mut ParseState, event: &BytesStart<'_>) {
    let name = local_name(event.name().as_ref());
    state.path.push(name.clone());

    match name.as_str() {
        "City" => push_text_target(state, TextTarget::City),
        "Generated" => push_text_target(state, TextTarget::Generated),
        "SeaLevel" => push_text_target(state, TextTarget::SeaLevel),
        "Name" if path_contains(state, "Seg") => push_text_target(state, TextTarget::SegmentName),
        "Name" if path_contains(state, "Buil") || path_contains(state, "Park") => {
            push_text_target(state, TextTarget::AreaName)
        }
        "type" if path_contains(state, "Park") => push_text_target(state, TextTarget::AreaType),
        // "Sg" inside "Seg": handled below alongside route-link initialisation.
        "Ter" if path_contains(state, "Terrains") => push_text_target(state, TextTarget::TerrainData),
        "Forest" if path_contains(state, "Forests") => push_text_target(state, TextTarget::ForestRow),
        "Node" => {
            state.current_node = Some(NodeDraft {
                id: parse_u64(attr_value(event, b"id")).unwrap_or(0),
                name: attr_value(event, b"name"),
                position: None,
                elevation: parse_f32(attr_value(event, b"y")).unwrap_or(0.0),
                elevation_hint: parse_f32(attr_value(event, b"elev")).unwrap_or(0.0),
                service: attr_value(event, b"srv").unwrap_or_default(),
                subtype: attr_value(event, b"subsrv").unwrap_or_default(),
                underground: parse_bool(attr_value(event, b"ug")),
                overground: parse_bool(attr_value(event, b"og")),
                dist: parse_u64(attr_value(event, b"dist")),
            });
        }
        "Pos" if path_contains(state, "Node") => {
            if let Some(node) = state.current_node.as_mut() {
                let x = parse_f32(attr_value(event, b"x")).unwrap_or(0.0);
                let z = parse_f32(attr_value(event, b"z")).unwrap_or(0.0);
                let elevation = parse_f32(attr_value(event, b"y")).unwrap_or(0.0);
                node.position = Some(WorldPoint::new(x, -z));
                node.elevation = elevation;
            }
        }
        "Seg" => {
            let raw_class = attr_value(event, b"icls").unwrap_or_default();
            let class = SegmentClass::from_icls(&raw_class);
            state.current_segment = Some(SegmentDraft {
                id: parse_u64(attr_value(event, b"id")).unwrap_or(0),
                name: attr_value(event, b"name"),
                start_node: parse_u64(attr_value(event, b"sn")).unwrap_or(0),
                end_node: parse_u64(attr_value(event, b"en")).unwrap_or(0),
                raw_class: raw_class.clone(),
                class,
                width: parse_f32(attr_value(event, b"width")).unwrap_or(0.0),
                points: Vec::new(),
                bounds: WorldBounds::default(),
                route_segment_ids: Vec::new(),
                underground: raw_class.to_ascii_lowercase().contains("tunnel"),
                overground: raw_class.to_ascii_lowercase().contains("elevated")
                    || raw_class.to_ascii_lowercase().contains("bridge"),
                elevated_hint: raw_class.to_ascii_lowercase().contains("elevated")
                    || raw_class.to_ascii_lowercase().contains("bridge")
                    || raw_class.to_ascii_lowercase().contains("ramp"),
            });
        }
        "P" if path_contains(state, "Seg")
            || path_contains(state, "Buil")
            || path_contains(state, "Park")
            || path_contains(state, "Dist") =>
        {
            if let Some(segment) = state.current_segment.as_mut() {
                if let Some(point) = parse_segment_point(event) {
                    segment.bounds.include_point(point.position);
                    segment.points.push(point);
                }
            }
            if let Some(area) = state.current_area.as_mut() {
                if let Some(point) = parse_point(event) {
                    area.bounds.include_point(point);
                    area.points.push(point);
                    if area.elevation == 0.0 {
                        area.elevation = parse_f32(attr_value(event, b"y")).unwrap_or(0.0);
                    }
                }
            }
            if let Some(district) = state.current_district.as_mut() {
                if district.anchor.is_none() {
                    district.anchor = parse_point(event);
                }
            }
        }
        "Buil" => {
            state.current_area = Some(AreaDraft {
                id: parse_u64(attr_value(event, b"id")).unwrap_or(0),
                name: attr_value(event, b"name"),
                service: attr_value(event, b"srv").unwrap_or_default(),
                subtype: attr_value(event, b"subsrv").unwrap_or_default(),
                area_type: None,
                points: Vec::new(),
                bounds: WorldBounds::default(),
                elevation: 0.0,
            });
        }
        "Park" => {
            state.current_area = Some(AreaDraft {
                id: parse_u64(attr_value(event, b"id")).unwrap_or(0),
                name: attr_value(event, b"name"),
                service: String::from("Park"),
                subtype: String::new(),
                area_type: None,
                points: Vec::new(),
                bounds: WorldBounds::default(),
                elevation: 0.0,
            });
        }
        "Dist" => {
            state.current_district = Some(DistrictDraft {
                id: parse_u64(attr_value(event, b"id")).unwrap_or(0),
                name: attr_value(event, b"name").unwrap_or_default(),
                anchor: None,
            });
        }
        "Trans" => {
            let kind = parse_route_kind(&attr_value(event, b"type").unwrap_or_default());
            let color = RgbaColor::rgba(
                attr_value(event, b"r").and_then(|value| value.parse().ok()).unwrap_or(128),
                attr_value(event, b"g").and_then(|value| value.parse().ok()).unwrap_or(128),
                attr_value(event, b"b").and_then(|value| value.parse().ok()).unwrap_or(128),
                attr_value(event, b"a").and_then(|value| value.parse().ok()).unwrap_or(255),
            );
            state.current_route = Some(RouteDraft {
                id: attr_value(event, b"id").unwrap_or_default(),
                name: attr_value(event, b"name").unwrap_or_default(),
                kind,
                color,
                stops: Vec::new(),
            });
        }
        "color" if path_contains(state, "Trans") => {
            if let Some(route) = state.current_route.as_mut() {
                route.color = RgbaColor::rgba(
                    attr_value(event, b"r").and_then(|value| value.parse().ok()).unwrap_or(route.color.r),
                    attr_value(event, b"g").and_then(|value| value.parse().ok()).unwrap_or(route.color.g),
                    attr_value(event, b"b").and_then(|value| value.parse().ok()).unwrap_or(route.color.b),
                    attr_value(event, b"a").and_then(|value| value.parse().ok()).unwrap_or(route.color.a),
                );
            }
        }
        "Stop" if path_contains(state, "Trans") => {
            if let Some(route) = state.current_route.as_mut() {
                if let Some(node) = parse_u64(attr_value(event, b"node")) {
                    route.stops.push(node);
                }
            }
        }
        "P" => {
            state.current_route_link = Some(RouteLinkDraft {
                start_node: state.current_segment.as_ref().map(|segment| segment.start_node).unwrap_or(0),
                end_node: state.current_segment.as_ref().map(|segment| segment.end_node).unwrap_or(0),
                segment_ids: Vec::new(),
            });
            push_text_target(state, TextTarget::RouteLinkSegmentId);
        }
        _ => {}
    }
}

fn handle_text(state: &mut ParseState, text: &str) {
    if state.current_text_target.is_some() {
        state.text_buffer.push_str(text);
    }
}

fn finish_text_target(state: &mut ParseState) {
    let text = state.text_buffer.trim().to_owned();
    match state.current_text_target.take() {
        Some(TextTarget::City) => state.document.metadata.city = text,
        Some(TextTarget::Generated) => {
            state.document.metadata.generated = if text.is_empty() { None } else { Some(text) }
        }
        Some(TextTarget::SeaLevel) => {
            state.document.metadata.sea_level = text.parse::<f32>().unwrap_or(0.0)
        }
        Some(TextTarget::SegmentName) => {
            if let Some(segment) = state.current_segment.as_mut() {
                if !text.is_empty() {
                    segment.name = Some(text);
                }
            }
        }
        Some(TextTarget::AreaName) => {
            if let Some(area) = state.current_area.as_mut() {
                if !text.is_empty() {
                    area.name = Some(text);
                }
            }
        }
        Some(TextTarget::AreaType) => {
            if let Some(area) = state.current_area.as_mut() {
                if !text.is_empty() {
                    area.area_type = Some(text);
                }
            }
        }
        Some(TextTarget::RouteLinkSegmentId) => {
            if let Some(route_link) = state.current_route_link.as_mut() {
                if let Ok(id) = text.parse::<u64>() {
                    route_link.segment_ids.push(id);
                }
            }
        }
        Some(TextTarget::TerrainData) => {
            state.current_terrain.get_or_insert_with(String::new).push_str(&text);
        }
        Some(TextTarget::ForestRow) => {
            state.current_forest_row.get_or_insert_with(String::new).push_str(&text);
        }
        None => {}
    }
    state.text_buffer.clear();
}

fn handle_end(state: &mut ParseState, name: &str) {
    if matches!(
        state.current_text_target,
        Some(TextTarget::City)
            | Some(TextTarget::Generated)
            | Some(TextTarget::SeaLevel)
            | Some(TextTarget::SegmentName)
            | Some(TextTarget::AreaName)
            | Some(TextTarget::AreaType)
            | Some(TextTarget::RouteLinkSegmentId)
            | Some(TextTarget::TerrainData)
            | Some(TextTarget::ForestRow)
    )
        && matches!(
            name,
            "City"
                | "Generated"
                | "SeaLevel"
                | "Name"
                | "type"
                | "Sg"
                | "Ter"
                | "Forest"
        )
    {
        finish_text_target(state);
    }

    match name {
        "Node" => {
            if let Some(draft) = state.current_node.take() {
                if let Some(position) = draft.position {
                    let kind = parse_service_node_kind(&draft.service, &draft.subtype);
                    let record = NodeRecord {
                        id: draft.id,
                        name: draft.name,
                        position,
                        elevation: draft.elevation,
                        elevation_hint: draft.elevation_hint,
                        service: draft.service,
                        subtype: draft.subtype,
                        kind,
                        underground: draft.underground,
                        overground: draft.overground,
                        dist: draft.dist,
                    };
                    state.document.bounds.include_point(position);
                    state.document.nodes.push(record);
                }
            }
        }
        "Seg" => {
            if let Some(draft) = state.current_segment.take() {
                let record = SegmentRecord {
                    id: draft.id,
                    name: draft.name,
                    start_node: draft.start_node,
                    end_node: draft.end_node,
                    raw_class: draft.raw_class,
                    class: draft.class,
                    width: draft.width,
                    points: draft.points,
                    bounds: draft.bounds,
                    route_segment_ids: draft.route_segment_ids,
                    underground: draft.underground,
                    overground: draft.overground,
                    elevated_hint: draft.elevated_hint,
                };
                state.document.bounds.include_bounds(&record.bounds);
                state.document.segments.push(record);
            }
        }
        "Buil" => {
            if let Some(draft) = state.current_area.take() {
                let record = AreaRecord {
                    id: draft.id,
                    name: draft.name,
                    service: draft.service,
                    subtype: draft.subtype,
                    area_type: draft.area_type,
                    points: draft.points,
                    bounds: draft.bounds,
                    elevation: draft.elevation,
                };
                state.document.bounds.include_bounds(&record.bounds);
                state.document.buildings.push(record);
            }
        }
        "Park" => {
            if let Some(draft) = state.current_area.take() {
                let record = AreaRecord {
                    id: draft.id,
                    name: draft.name,
                    service: draft.service,
                    subtype: draft.subtype,
                    area_type: draft.area_type,
                    points: draft.points,
                    bounds: draft.bounds,
                    elevation: draft.elevation,
                };
                state.document.bounds.include_bounds(&record.bounds);
                state.document.parks.push(record);
            }
        }
        "Dist" => {
            if let Some(draft) = state.current_district.take() {
                if let Some(anchor) = draft.anchor {
                    state.document.bounds.include_point(anchor);
                    state.document.districts.push(DistrictRecord {
                        id: draft.id,
                        name: draft.name,
                        anchor,
                    });
                }
            }
        }
        "Trans" => {
            if let Some(route) = state.current_route.take() {
                state.document.transports.push(TransportRecord {
                    id: route.id,
                    name: route.name,
                    kind: route.kind,
                    color: route.color,
                    stops: route.stops,
                });
            }
        }
        "Sg" => {
            if let Some(route_link) = state.current_route_link.take() {
                if !route_link.segment_ids.is_empty() {
                    state.document.route_links.push(RouteLinkRecord {
                        start_node: route_link.start_node,
                        end_node: route_link.end_node,
                        segment_ids: route_link.segment_ids,
                    });
                }
            }
        }
        "Ter" => {
            if let Some(text) = state.current_terrain.take() {
                state.document.terrain = Some(RasterSource {
                    kind: RasterKind::Terrain,
                    packed: text,
                    rows: Vec::new(),
                });
            }
        }
        "Forest" => {
            if let Some(row) = state.current_forest_row.take() {
                let source = state.document.forests.get_or_insert(RasterSource {
                    kind: RasterKind::Forest,
                    packed: String::new(),
                    rows: Vec::new(),
                });
                source.rows.push(row);
            }
        }
        _ => {}
    }

    state.path.pop();
}

fn finalize(state: &mut ParseState) -> Result<()> {
    if state.document.metadata.city.is_empty() {
        return Err(anyhow!("missing City tag in CSL export"));
    }

    if state.document.bounds.is_empty() {
        for node in &state.document.nodes {
            state.document.bounds.include_point(node.position);
        }
        for segment in &state.document.segments {
            state.document.bounds.include_bounds(&segment.bounds);
        }
        for area in &state.document.buildings {
            state.document.bounds.include_bounds(&area.bounds);
        }
        for area in &state.document.parks {
            state.document.bounds.include_bounds(&area.bounds);
        }
        for district in &state.document.districts {
            state.document.bounds.include_point(district.anchor);
        }
    }

    Ok(())
}
