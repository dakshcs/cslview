use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

use cslview::export::{export_to_file, ExportKind, ExportSettings, LayerSettings, MapMode, RoadLayers, TransitLayers};
use cslview::parser::parse_csl_file;
use cslview::scene::build_scene;

#[derive(Parser, Debug)]
#[command(author, version, about = "CSLView - Cities: Skylines XML renderer")]
struct Cli {
    #[arg(value_name = "INPUT")]
    input: Option<PathBuf>,

    #[arg(long, value_enum)]
    export: Option<ExportFormat>,

    #[arg(long)]
    output: Option<PathBuf>,

    #[arg(long, default_value_t = 4096)]
    width: u32,

    #[arg(long, default_value_t = 4096)]
    height: u32,

    #[arg(long, default_value_t = 80.0)]
    padding: f32,

    #[arg(long, default_value_t = 1.0)]
    zoom: f32,

    #[arg(long)]
    frame: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExportFormat {
    Svg,
    Png,
    Pdf,
}

impl From<ExportFormat> for ExportKind {
    fn from(value: ExportFormat) -> Self {
        match value {
            ExportFormat::Svg => ExportKind::Svg,
            ExportFormat::Png => ExportKind::Png,
            ExportFormat::Pdf => ExportKind::Pdf,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(format) = cli.export {
        let input = cli.input.as_ref().context("an input XML file is required for export")?;
        let document = parse_csl_file(input)?;
        let scene = build_scene(document);
        let output = cli.output.unwrap_or_else(|| default_output_path(input, format.into()));
        let settings = ExportSettings {
            width: cli.width,
            height: cli.height,
            padding: cli.padding,
            zoom: cli.zoom,
            frame: parse_frame(cli.frame.as_deref()),
            mode: MapMode::default(),
            layers: LayerSettings::default(),
            roads: RoadLayers::default(),
            transit: TransitLayers::default(),
            districts: BTreeMap::new(),
        };

        export_to_file(&scene, format.into(), &settings, output, None)?;
        return Ok(());
    }

    run_desktop(cli.input)
}

fn run_desktop(initial_file: Option<PathBuf>) -> Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "CSLView",
        native_options,
        Box::new(move |cc| Ok(Box::new(cslview::app::CslViewApp::new(cc, initial_file.clone())))),
    )
    .context("failed to start the desktop app")
}

fn parse_frame(value: Option<&str>) -> Option<cslview::model::WorldBounds> {
    let value = value?.trim();
    let mut parts = value.split(',').filter(|part| !part.trim().is_empty());
    let min_x = parts.next()?.trim().parse::<f32>().ok()?;
    let min_y = parts.next()?.trim().parse::<f32>().ok()?;
    let max_x = parts.next()?.trim().parse::<f32>().ok()?;
    let max_y = parts.next()?.trim().parse::<f32>().ok()?;
    Some(cslview::model::WorldBounds::from_corners(min_x, min_y, max_x, max_y))
}

fn default_output_path(input: &Path, kind: ExportKind) -> PathBuf {
    let extension = match kind {
        ExportKind::Svg => "svg",
        ExportKind::Png => "png",
        ExportKind::Pdf => "pdf",
    };
    input.with_extension(extension)
}
