use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use resvg::tiny_skia::{Pixmap, Transform};

use crate::scene::Scene;
use crate::svg::scene_to_svg;
use crate::model::WorldBounds;
use std::sync::{Arc, OnceLock};

static FONT_DB: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();

fn font_db() -> Arc<resvg::usvg::fontdb::Database> {
    FONT_DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    }).clone()
}

#[derive(Debug, Clone, Copy)]
pub struct LayerSetting {
    pub enabled: bool,
    pub opacity: f32,
}

impl LayerSetting {
    pub const fn new(enabled: bool, opacity: f32) -> Self {
        Self { enabled, opacity }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LayerSettings {
    pub terrain: LayerSetting,
    pub forests: LayerSetting,
    pub contours: LayerSetting,
    pub districts: LayerSetting,
    pub parks: LayerSetting,
    pub roads: LayerSetting,
    pub transit: LayerSetting,
    pub buildings: LayerSetting,
    pub nodes: LayerSetting,
    pub labels: LayerSetting,
    pub grid: LayerSetting,
}

#[derive(Debug, Clone, Copy)]
pub struct RoadLayers {
    pub highways: LayerSetting,
    pub surface_roads: LayerSetting,
    pub pedestrian: LayerSetting,
    pub rail: LayerSetting,
    pub metro: LayerSetting,
    pub aviation: LayerSetting,
    pub utility: LayerSetting,
    pub beautification: LayerSetting,
    pub miscellaneous: LayerSetting,
}

#[derive(Debug, Clone, Copy)]
pub struct TransitLayers {
    pub metro: LayerSetting,
    pub pedestrian: LayerSetting,
    pub other: LayerSetting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MapMode {
    Colour = 0,
    Planning = 1,
    Contour = 2,
}

impl MapMode {
    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn from_index(index: u8) -> Self {
        match index {
            1 => Self::Planning,
            2 => Self::Contour,
            _ => Self::Colour,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Colour => "Colour",
            Self::Planning => "Planning",
            Self::Contour => "Contour",
        }
    }
}

impl Default for MapMode {
    fn default() -> Self {
        Self::Colour
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistrictMode {
    Badge,
    Halo,
    Label,
    Outline,
}

#[derive(Debug, Clone, Copy)]
pub struct DistrictSetting {
    pub enabled: bool,
    pub opacity: f32,
    pub mode: DistrictMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Svg,
    Png,
    Pdf,
}

#[derive(Debug, Clone)]
pub struct ExportSettings {
    pub width: u32,
    pub height: u32,
    pub padding: f32,
    pub zoom: f32,
    pub frame: Option<WorldBounds>,
    pub mode: MapMode,
    pub layers: LayerSettings,
    pub roads: RoadLayers,
    pub transit: TransitLayers,
    pub districts: BTreeMap<u64, DistrictSetting>,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            width: 4096,
            height: 4096,
            padding: 120.0,
            zoom: 1.0,
            frame: None,
            mode: MapMode::default(),
            layers: LayerSettings::default(),
            roads: RoadLayers::default(),
            transit: TransitLayers::default(),
            districts: BTreeMap::new(),
        }
    }
}

impl Default for RoadLayers {
    fn default() -> Self {
        Self {
            highways: LayerSetting::new(true, 1.0),
            surface_roads: LayerSetting::new(true, 0.95),
            pedestrian: LayerSetting::new(true, 0.85),
            rail: LayerSetting::new(true, 0.9),
            metro: LayerSetting::new(true, 0.9),
            aviation: LayerSetting::new(true, 0.85),
            utility: LayerSetting::new(true, 0.8),
            beautification: LayerSetting::new(true, 0.85),
            miscellaneous: LayerSetting::new(true, 0.8),
        }
    }
}

impl Default for TransitLayers {
    fn default() -> Self {
        Self {
            metro: LayerSetting::new(true, 0.95),
            pedestrian: LayerSetting::new(true, 0.85),
            other: LayerSetting::new(true, 0.8),
        }
    }
}

impl Default for DistrictSetting {
    fn default() -> Self {
        Self {
            enabled: true,
            opacity: 0.55,
            mode: DistrictMode::Badge,
        }
    }
}

impl Default for LayerSettings {
    fn default() -> Self {
        Self {
            terrain: LayerSetting::new(true, 0.95),
            forests: LayerSetting::new(true, 0.62),
            contours: LayerSetting::new(false, 0.72),
            districts: LayerSetting::new(true, 0.68),
            parks: LayerSetting::new(true, 0.72),
            roads: LayerSetting::new(true, 0.95),
            transit: LayerSetting::new(true, 0.9),
            buildings: LayerSetting::new(true, 0.84),
            nodes: LayerSetting::new(true, 0.9),
            labels: LayerSetting::new(true, 0.82),
            grid: LayerSetting::new(false, 0.28),
        }
    }
}

pub fn export_scene(scene: &Scene, kind: ExportKind, settings: &ExportSettings, cached_svg: Option<String>) -> Result<(Vec<u8>, String)> {
    let svg = cached_svg.unwrap_or_else(|| scene_to_svg(scene, settings));
    let bytes = match kind {
        ExportKind::Svg => svg.as_bytes().to_vec(),
        ExportKind::Png => svg_to_png(&svg)?,
        ExportKind::Pdf => svg_to_pdf(&svg)?,
    };
    Ok((bytes, svg))  // return the svg so the caller can cache it
}

pub fn export_to_file(scene: &Scene, kind: ExportKind, settings: &ExportSettings, path: impl AsRef<Path>, cached_svg: Option<String>) -> Result<String> {
    let (bytes, svg) = export_scene(scene, kind, settings, cached_svg)?;
    fs::write(path.as_ref(), &bytes)
        .with_context(|| format!("failed to write {}", path.as_ref().display()))?;
    Ok(svg)  // caller gets the SVG string back for caching
}

pub fn export_path(input: &Path, kind: ExportKind) -> PathBuf {
    let mut output = input.with_extension(match kind {
        ExportKind::Svg => "svg",
        ExportKind::Png => "png",
        ExportKind::Pdf => "pdf",
    });

    if output == input {
        output.set_extension(match kind {
            ExportKind::Svg => "svg",
            ExportKind::Png => "png",
            ExportKind::Pdf => "pdf",
        });
    }

    output
}

fn svg_to_png(svg: &str) -> Result<Vec<u8>> {
    let mut options = resvg::usvg::Options::default();
    //
    options.fontdb_mut().clone_from(&font_db());
    let tree = resvg::usvg::Tree::from_str(svg, &options).context("failed to parse generated SVG")?;

    let mut pixmap = Pixmap::new(tree.size().width() as u32, tree.size().height() as u32)
        .ok_or_else(|| anyhow::anyhow!("failed to allocate PNG pixmap"))?;
    let mut pixmap_mut = pixmap.as_mut();
    resvg::render(&tree, Transform::identity(), &mut pixmap_mut);
    Ok(pixmap.encode_png().context("failed to encode PNG")?)
}

fn svg_to_pdf(svg: &str) -> Result<Vec<u8>> {
    let mut options = svg2pdf::usvg::Options::default();
    // NEW
    options.fontdb_mut().clone_from(&font_db());
    let tree = svg2pdf::usvg::Tree::from_str(svg, &options).context("failed to parse generated SVG for PDF")?;
    let pdf = svg2pdf::to_pdf(&tree, svg2pdf::ConversionOptions::default(), svg2pdf::PageOptions::default())
        .map_err(|error| anyhow::anyhow!("failed to convert SVG to PDF: {error}"))?;
    Ok(pdf)
}
