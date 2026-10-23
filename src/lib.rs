//! Open Infra — GIS workbench for Open CAD Studio.
//!
//! A "GIS" ribbon tab modelled on the FreeCAD GIS(2BIM) workbench: set the
//! project site and coordinate system, then pull WMS/tile map layers, WFS
//! vector data and OpenStreetMap into the drawing — the Netherlands (PDOK)
//! and worldwide. The workbench window runs in this plugin's own process.

pub mod bridge;
pub mod emit;
pub mod gis;
pub mod import;
pub mod basisdata;
pub mod nlcs;
pub mod tekenblad;
mod ui;

use std::sync::OnceLock;

use ocs_plugin_api::host::{BuiltinPlugin, CommandStep, HostApi, InteractiveCommand, PreviewWire};
use ocs_plugin_api::manifest::{ApiVersion, PluginManifest};
use ocs_plugin_api::ribbon::{CadModule, IconKind, ModuleEvent, RibbonGroup, RibbonItem, ToolDef};

use gis::site::Site;

static MANIFEST: PluginManifest = PluginManifest {
    id: "openaec.open-infra",
    name: "Open Infra — GIS",
    version: env!("CARGO_PKG_VERSION"),
    description: "GIS-workbench voor infra en civiele techniek: projectlocatie en coördinatenstelsels, WMS/WFS-servers, OpenStreetMap. Nederland (PDOK) en wereldwijd.",
    api_version: ApiVersion::CURRENT,
    ribbon_order: 60,
    xdata_apps: &[],
    command_prefixes: &["GIS_"],
};

const I_LOCATION: IconKind = IconKind::Svg(include_bytes!("../assets/location.svg"));
const I_AREA: IconKind = IconKind::Svg(include_bytes!("../assets/area.svg"));
const I_MAP: IconKind = IconKind::Svg(include_bytes!("../assets/map.svg"));
const I_LAYERS: IconKind = IconKind::Svg(include_bytes!("../assets/layers.svg"));
const I_VECTOR: IconKind = IconKind::Svg(include_bytes!("../assets/vector.svg"));
const I_GLOBE: IconKind = IconKind::Svg(include_bytes!("../assets/globe.svg"));
const I_POINT: IconKind = IconKind::Svg(include_bytes!("../assets/point.svg"));
const I_SERVER: IconKind = IconKind::Svg(include_bytes!("../assets/server.svg"));
const I_QUICK: IconKind = IconKind::Svg(include_bytes!("../assets/quick.svg"));
const I_PDOK: IconKind = IconKind::Svg(include_bytes!("../assets/pdok.svg"));
const I_BGT: IconKind = IconKind::Svg(include_bytes!("../assets/bgt.svg"));
const I_KADASTER: IconKind = IconKind::Svg(include_bytes!("../assets/kadaster.svg"));
const I_KLIC: IconKind = IconKind::Svg(include_bytes!("../assets/klic.svg"));
const I_SHEET: IconKind = IconKind::Svg(include_bytes!("../assets/sheet.svg"));

/// One-click imports from the ribbon (no window): command → what to fetch.
enum Quick {
    Data(import::Dataset),
    Osm,
}

const QUICK: &[(&str, &str, Quick)] = &[
    ("GIS_Q_LUCHTFOTO", "Luchtfoto (PDOK)", Quick::Data(import::Dataset::Raster { server: "pdok-luchtfoto" })),
    ("GIS_Q_KADASTER", "Kadastrale kaart (PDOK)", Quick::Data(import::Dataset::Raster { server: "pdok-kadastralekaart-wms" })),
    ("GIS_Q_AHN", "AHN hoogtekaart (PDOK)", Quick::Data(import::Dataset::Raster { server: "pdok-ahn-wms" })),
    ("GIS_Q_OSMKAART", "OpenStreetMap-kaart (wereld)", Quick::Data(import::Dataset::Raster { server: "osm-tiles" })),
    ("GIS_Q_SATELLIET", "Sentinel-2 satelliet (wereld)", Quick::Data(import::Dataset::Raster { server: "eox-s2-wms" })),
    ("GIS_Q_BAG", "BAG panden (PDOK)", Quick::Data(import::Dataset::Wfs { server: "pdok-bag-wfs", typename: "bag:pand", label: None, aci: 30 })),
    ("GIS_Q_PERCELEN", "Kadastrale percelen (PDOK)", Quick::Data(import::Dataset::Wfs { server: "pdok-kadastralekaart-wfs", typename: "kadastralekaart:Perceel", label: Some("perceelnummer"), aci: 3 })),
    ("GIS_Q_NWB", "NWB wegvakken (RWS)", Quick::Data(import::Dataset::Wfs { server: "pdok-nwbwegen-wfs", typename: "nwbwegen:wegvakken", label: Some("stt_naam"), aci: 2 })),
    ("GIS_Q_HECTO", "NWB hectometerpunten (RWS)", Quick::Data(import::Dataset::Wfs { server: "pdok-nwbwegen-wfs", typename: "nwbwegen:hectopunten", label: Some("hectomtrng"), aci: 2 })),
    ("GIS_Q_SPOOR", "Spoorassen (ProRail)", Quick::Data(import::Dataset::Wfs { server: "pdok-spoorwegen-wfs", typename: "spoorwegen:spooras", label: None, aci: 6 })),
    ("GIS_Q_OSM", "OpenStreetMap-vectoren (wereld)", Quick::Osm),
];

fn is_raster(q: &Quick) -> bool {
    matches!(q, Quick::Data(import::Dataset::Raster { .. }))
}

fn tool(id: &'static str, label: &'static str, icon: IconKind) -> RibbonItem {
    RibbonItem::LargeTool(ToolDef { id, label, icon, event: ModuleEvent::Command(id.to_string()) })
}

struct GisModule;

impl CadModule for GisModule {
    fn id(&self) -> &'static str {
        "open_infra_gis"
    }
    fn title(&self) -> &'static str {
        "GIS"
    }
    fn ribbon_groups(&self) -> &[RibbonGroup] {
        static GROUPS: OnceLock<Vec<RibbonGroup>> = OnceLock::new();
        GROUPS.get_or_init(|| {
            let quick = |filter: fn(&Quick) -> bool| {
                QUICK.iter().filter(|(_, _, q)| filter(q)).map(|(c, l, _)| (*c, *l, I_QUICK)).collect::<Vec<_>>()
            };
            vec![
                // Like the FreeCAD GIS workbench toolbars.
                RibbonGroup {
                    title: "Webservices",
                    tools: vec![
                        tool("GIS_LOCATIE", "Geo-locatie", I_LOCATION),
                        tool("GIS_KAART", "WMS / TMS", I_MAP),
                        tool("GIS_WFS", "WFS", I_VECTOR),
                        tool("GIS_OSM", "OpenStreetMap", I_GLOBE),
                        tool("GIS_PDOK", "PDOK (NL)", I_PDOK),
                    ],
                },
                RibbonGroup {
                    title: "Snel importeren",
                    tools: vec![
                        RibbonItem::LargeDropdown {
                            id: "GIS_Q_RASTER",
                            label: "Kaartbeeld",
                            icon: I_LAYERS,
                            items: quick(is_raster),
                            default: "GIS_Q_LUCHTFOTO",
                        },
                        RibbonItem::LargeDropdown {
                            id: "GIS_Q_VECTOR",
                            label: "Vectordata",
                            icon: I_QUICK,
                            items: quick(|q| !is_raster(q)),
                            default: "GIS_Q_BAG",
                        },
                    ],
                },
                // The InfraCAD Map base data, drawn conform NLCS.
                RibbonGroup {
                    title: "Basisdata NL (NLCS)",
                    tools: vec![
                        tool("GIS_BGT", "BGT · BRK · KLIC", I_BGT),
                        tool("GIS_Q_BGT", "BGT", I_BGT),
                        tool("GIS_Q_BRK", "Kadaster", I_KADASTER),
                        RibbonItem::LargeTool(ToolDef {
                            id: "GIS_KLIC",
                            label: "KLIC",
                            icon: I_KLIC,
                            event: ModuleEvent::PluginFileDialog {
                                command: "GIS_KLIC".into(),
                                title: "KLIC-levering openen".into(),
                                filter_name: "KLIC-levering (zip, xml)".into(),
                                extensions: vec!["zip".into(), "xml".into()],
                            },
                        }),
                        tool("GIS_TEKENBLAD", "Tekenblad", I_SHEET),
                    ],
                },
                RibbonGroup {
                    title: "Hulpmiddelen",
                    tools: vec![
                        tool("GIS_GEBIED", "Gebied", I_AREA),
                        tool("GIS_PUNT", "Punt info", I_POINT),
                        tool("GIS_COORD", "Coördinaten", I_POINT),
                        tool("GIS_SERVERS", "Servers", I_SERVER),
                    ],
                },
            ]
        })
    }
}

/// `GIS_GEBIED`: pick two opposite corners of the import area in the drawing.
struct AreaCmd {
    first: Option<[f64; 2]>,
    site: Site,
    unit_factor: f64,
}

impl InteractiveCommand for AreaCmd {
    fn prompt(&self) -> String {
        if self.first.is_none() { "GIS: eerste hoek van het importgebied".into() } else { "GIS: tegenoverliggende hoek".into() }
    }
    fn on_point(&mut self, pt: [f64; 3]) -> CommandStep {
        let Some(a) = self.first else {
            self.first = Some([pt[0], pt[1]]);
            return CommandStep::NeedPoint;
        };
        let b = [pt[0], pt[1]];
        let pa = self.site.from_drawing(a, self.unit_factor);
        let pb = self.site.from_drawing(b, self.unit_factor);
        let (w, h) = ((pa[0] - pb[0]).abs(), (pa[1] - pb[1]).abs());
        if w < 1.0 || h < 1.0 {
            return CommandStep::Cancel;
        }
        let mut site = self.site.clone();
        site.set_origin((pa[0] + pb[0]) / 2.0, (pa[1] + pb[1]) / 2.0);
        site.width = (w * 10.0).round() / 10.0;
        site.height = (h * 10.0).round() / 10.0;
        // Write from a worker: this callback runs inside the host's input
        // handling and must not wait on another host round-trip.
        std::thread::spawn(move || match bridge::save_site(&site) {
            Ok(()) => {
                bridge::info(&format!("Importgebied {} × {} m rond X {:.2} Y {:.2} ({}).", site.width, site.height, site.origin_e, site.origin_n, site.crs));
                ui::send(ui::UiMsg::SiteChanged);
            }
            Err(e) => bridge::error(&format!("Gebied niet opgeslagen: {e}")),
        });
        CommandStep::Done
    }
    fn on_cursor_move(&mut self, pt: [f64; 3]) -> Vec<PreviewWire> {
        let Some(a) = self.first else { return Vec::new() };
        let p = |x: f64, y: f64| [x, y, 0.0];
        vec![PreviewWire::with_color(
            vec![p(a[0], a[1]), p(pt[0], a[1]), p(pt[0], pt[1]), p(a[0], pt[1]), p(a[0], a[1])],
            [0.95, 0.6, 0.15, 1.0],
        )]
    }
}

/// `GIS_PUNT`: report a picked point in the project CRS, WGS 84 and RD/UTM.
struct PointCmd {
    site: Site,
    unit_factor: f64,
}

impl InteractiveCommand for PointCmd {
    fn prompt(&self) -> String {
        "GIS: wijs een punt aan".into()
    }
    fn on_point(&mut self, pt: [f64; 3]) -> CommandStep {
        let p = self.site.from_drawing([pt[0], pt[1]], self.unit_factor);
        let (site, unit) = (self.site.crs.clone(), self.unit_factor);
        std::thread::spawn(move || {
            let mut lines = vec![format!("{site}: X {:.3}  Y {:.3}", p[0], p[1])];
            if let Ok(ll) = gis::crs::transform(&site, "EPSG:4326", p) {
                lines.push(format!("WGS 84: {:.8} N  {:.8} E", ll[1], ll[0]));
                let utm = gis::crs::utm_code_for(ll[0], ll[1]);
                for other in ["EPSG:28992".to_string(), utm] {
                    if other != site {
                        if let (Ok(q), Some(i)) = (gis::crs::transform(&site, &other, p), gis::crs::known(&other)) {
                            lines.push(format!("{}: X {:.3}  Y {:.3}", i.name, q[0], q[1]));
                        }
                    }
                }
            }
            let _ = unit;
            bridge::info(&lines.join("  |  "));
            ui::send(ui::UiMsg::Picked(p));
        });
        CommandStep::Done
    }
}

struct OpenInfra;

fn require_site(host: &mut dyn HostApi) -> Option<Site> {
    let s = bridge::site();
    if s.is_none() {
        host.push_error("[Open Infra] Stel eerst de projectlocatie in: GIS → Locatie.");
        let _ = ui::open(ui::Tab::Location);
    }
    s
}

fn open(host: &mut dyn HostApi, tab: ui::Tab) {
    if let Err(e) = ui::open(tab) {
        host.push_error(&format!("[Open Infra] {e}"));
    }
}

fn quick(host: &mut dyn HostApi, q: &'static Quick, label: &'static str) {
    let Some(site) = require_site(host) else { return };
    host.push_info(&format!("[Open Infra] {label} ophalen…"));
    std::thread::spawn(move || {
        let progress = |_: String| {};
        let result = match q {
            Quick::Data(d) => import::run_dataset(&site, d, &progress),
            Quick::Osm => {
                let ids: Vec<&str> = gis::osm::THEMES.iter().filter(|t| t.default_on).map(|t| t.id).collect();
                import::osm(&site, &ids, true, 2.0, true, &progress)
            }
        };
        match result {
            Ok(m) => bridge::info(&m),
            Err(e) => bridge::error(&format!("{label}: {e}")),
        }
    });
}

/// `GIS_SITE <lon> <lat> [breedte] [hoogte] [EPSG:code] [naam]` or
/// `GIS_SITE RD <x> <y> [breedte] [hoogte] [naam]`: set the project location
/// from a script, without the window.
fn site_command(host: &mut dyn HostApi, cmd: &str) {
    let args: Vec<&str> = cmd.split_whitespace().skip(1).collect();
    let num = |i: usize| args.get(i).and_then(|a| a.replace(',', ".").parse::<f64>().ok());
    let mut site = bridge::site().unwrap_or_default();
    let rd = args.first().is_some_and(|a| a.eq_ignore_ascii_case("RD"));
    let at = if rd { 1 } else { 0 };
    let (Some(a), Some(b)) = (num(at), num(at + 1)) else {
        host.push_error("[Open Infra] GIS_SITE <lon> <lat> [breedte] [hoogte] [EPSG:code]  of  GIS_SITE RD <x> <y> [breedte] [hoogte]");
        return;
    };
    if let Some(code) = args.iter().find(|a| a.to_ascii_uppercase().starts_with("EPSG:")) {
        site.crs = code.to_ascii_uppercase();
    } else if rd {
        site.crs = "EPSG:28992".into();
    }
    site.width = num(at + 2).unwrap_or(site.width);
    site.height = num(at + 3).unwrap_or(site.width);
    // Words after the numbers name the location (title block, window).
    let name: Vec<&str> = args.iter().skip(at + 2).filter(|a| a.replace(',', ".").parse::<f64>().is_err() && !a.to_ascii_uppercase().starts_with("EPSG:")).copied().collect();
    site.label = if !name.is_empty() {
        name.join(" ")
    } else if rd {
        format!("RD {a:.0} {b:.0}")
    } else {
        format!("{b:.5} N {a:.5} E")
    };
    if rd {
        site.reproject_origin();
        site.set_origin(a, b);
    } else {
        site.lon = a;
        site.lat = b;
        site.reproject_origin();
    }
    std::thread::spawn(move || match bridge::save_site(&site) {
        Ok(()) => {
            bridge::info(&format!("Projectlocatie {}: X {:.2} Y {:.2} ({}), {} × {} m.", site.label, site.origin_e, site.origin_n, site.crs, site.width, site.height));
            ui::send(ui::UiMsg::SiteChanged);
        }
        Err(e) => bridge::error(&format!("GIS_SITE: {e}")),
    });
}

/// Base data from the command line or a ribbon button, without the window.
fn base_data(host: &mut dyn HostApi, verb: &str, arg: &str) {
    let Some(site) = require_site(host) else { return };
    let verb = verb.to_string();
    let arg = arg.trim().trim_matches('"').to_string();
    host.push_info(&format!("[Open Infra] {verb} …"));
    std::thread::spawn(move || {
        let progress = |_: String| {};
        let opt = basisdata::NlcsOptions::default();
        let result = match verb.as_str() {
            "GIS_Q_BGT" => {
                let cols: Vec<&str> = gis::bgt::GROUPS.iter().filter(|g| g.default_on).flat_map(|g| g.collections.iter().copied()).collect();
                basisdata::bgt(&site, &cols, &opt, &progress)
            }
            "GIS_Q_BRK" => basisdata::cadastre(&site, &opt, &progress),
            "GIS_KLIC_IMPORT" => basisdata::klic(&site, std::path::Path::new(&arg), &basisdata::KlicOptions::default()),
            _ => {
                let mut info = tekenblad::SheetInfo::default();
                if !arg.is_empty() {
                    info.subject = arg.clone();
                }
                tekenblad::draw(&site, &opt, &info)
            }
        };
        match result {
            Ok(m) => bridge::info(&m),
            Err(e) => bridge::error(&format!("{verb}: {e}")),
        }
    });
}

impl BuiltinPlugin for OpenInfra {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn ribbon(&self) -> Box<dyn CadModule> {
        Box::new(GisModule)
    }

    fn on_load(&mut self, host: &mut dyn HostApi) {
        bridge::attach(host);
    }

    fn dispatch(&self, host: &mut dyn HostApi, cmd: &str) -> bool {
        let verb = cmd.split_whitespace().next().unwrap_or("").to_ascii_uppercase();
        if !verb.starts_with("GIS_") {
            return false;
        }
        bridge::attach(host);
        match verb.as_str() {
            "GIS_LOCATIE" | "GIS_WERKBANK" => open(host, ui::Tab::Location),
            "GIS_KAART" | "GIS_WMS" => open(host, ui::Tab::Raster),
            "GIS_WFS" => open(host, ui::Tab::Wfs),
            "GIS_OSM" => open(host, ui::Tab::Osm),
            "GIS_PDOK" => open(host, ui::Tab::Pdok),
            "GIS_SERVERS" => open(host, ui::Tab::Servers),
            "GIS_COORD" => open(host, ui::Tab::Coords),
            "GIS_BGT" | "GIS_NLCS" => open(host, ui::Tab::Nlcs),
            "GIS_SITE" => site_command(host, cmd),
            "GIS_Q_BGT" | "GIS_Q_BRK" | "GIS_TEKENBLAD" | "GIS_KLIC_IMPORT" => {
                let arg = cmd.split_once(char::is_whitespace).map(|(_, a)| a).unwrap_or("");
                base_data(host, &verb, arg);
            }
            "GIS_KLIC" => {
                // From the ribbon's file dialog: "GIS_KLIC <path>" (original case).
                let arg = cmd.split_once(char::is_whitespace).map(|(_, a)| a.trim().trim_matches('"')).unwrap_or("");
                if arg.is_empty() {
                    open(host, ui::Tab::Nlcs);
                } else if ui::open(ui::Tab::Nlcs).is_ok() {
                    ui::send(ui::UiMsg::Klic(arg.into()));
                } else {
                    base_data(host, "GIS_KLIC_IMPORT", arg);
                }
            }
            "GIS_GEBIED" | "GIS_PUNT" => {
                let site = bridge::site().unwrap_or_default();
                site.register_crs();
                let unit_factor = bridge::target().map(|t| t.unit_factor).unwrap_or(1.0);
                if verb == "GIS_GEBIED" {
                    host.start_interactive(Box::new(AreaCmd { first: None, site, unit_factor }));
                } else {
                    host.start_interactive(Box::new(PointCmd { site, unit_factor }));
                }
            }
            "GIS_HELP" => host.push_info(
                "[Open Infra] GIS_LOCATIE, GIS_GEBIED, GIS_PUNT, GIS_KAART, GIS_WFS, GIS_OSM, GIS_PDOK, GIS_COORD, GIS_SERVERS; \
                 snel: GIS_Q_LUCHTFOTO, GIS_Q_KADASTER, GIS_Q_AHN, GIS_Q_OSMKAART, GIS_Q_SATELLIET, GIS_Q_BAG, \
                 GIS_Q_PERCELEN, GIS_Q_NWB, GIS_Q_HECTO, GIS_Q_SPOOR, GIS_Q_OSM; \
                 GIS_SITE <lon> <lat> [b] [h] [EPSG:code] | GIS_SITE RD <x> <y> [b] [h]; \
                 NLCS: GIS_BGT (venster), GIS_Q_BGT, GIS_Q_BRK, GIS_KLIC [bestand], GIS_KLIC_IMPORT <bestand>, GIS_TEKENBLAD [onderwerp].",
            ),
            v => match QUICK.iter().find(|(c, ..)| *c == v) {
                Some((_, label, q)) => quick(host, q, label),
                None => {
                    host.push_error(&format!("[Open Infra] onbekend commando {v} — zie GIS_HELP"));
                }
            },
        }
        true
    }
}

ocs_plugin_api::export_plugin!(OpenInfra);
