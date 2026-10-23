//! Workbench window, laid out like the dialogs of the FreeCAD GIS(2BIM)
//! workbench: a map with the location and bounding box on top, then
//! "Locatie / Begrenzing", "Webserver / Verzoek" and "Import-instellingen"
//! groups, and OK / Test / Annuleren at the bottom.

use std::sync::mpsc::Receiver;
use std::sync::Arc;

use eframe::egui::{self, Color32, RichText, TextureHandle, Vec2};

use super::jobs::{poll, Job};
use super::map::{LonLat, MapEvent, MapView, Overlay};
use super::{Tab, UiMsg};
use crate::bridge;
use crate::gis::caps::{self, WfsCaps, WfsType, WmsCaps, WmsLayer};
use crate::gis::crs::{self, CrsInfo, Transformer};
use crate::gis::geocode::{self, Hit};
use crate::gis::osm::THEMES;
use crate::gis::raster::{self, Preview, MAX_TOTAL_PX};
use crate::gis::servers::{self, MapServer, ServerKind};
use crate::gis::site::Site;
use crate::gis::vector::{self, Feature};
use crate::gis::wfs::WfsResult;
use crate::basisdata::{self, Cadastre, KlicOptions, NlcsOptions};
use crate::gis::{bgt, klic};
use crate::import::{self, VectorStyle, PDOK};
use crate::nlcs::{self, Element, Status};
use crate::tekenblad::{self, SheetInfo};

const ACI_CHOICES: &[(i16, &str)] = &[(1, "rood"), (2, "geel"), (3, "groen"), (4, "cyaan"), (5, "blauw"), (6, "magenta"), (7, "wit/zwart"), (8, "grijs"), (30, "oranje")];
const ACCENT: Color32 = Color32::from_rgb(33, 150, 220);
const OK_GREEN: Color32 = Color32::from_rgb(152, 195, 121);
const ERR_RED: Color32 = Color32::from_rgb(224, 108, 117);
const WARN: Color32 = Color32::from_rgb(229, 192, 123);
/// Map height on the import pages; the location page gets most of the window,
/// like the map pane of FreeCAD's "Set Geographic Location" dialog.
const MAP_H: f32 = 360.0;

fn aci_color(aci: i16) -> Color32 {
    match aci {
        1 => Color32::from_rgb(230, 60, 60),
        2 => Color32::from_rgb(235, 200, 40),
        3 => Color32::from_rgb(40, 180, 70),
        4 => Color32::from_rgb(40, 190, 210),
        5 => Color32::from_rgb(60, 100, 230),
        6 => Color32::from_rgb(210, 70, 200),
        8 => Color32::from_gray(110),
        30 => Color32::from_rgb(240, 140, 40),
        _ => Color32::from_gray(60),
    }
}

/// Toolbar-style icons from the plugin's SVGs, recoloured for the dark theme.
fn icon(name: &str) -> egui::Image<'static> {
    let svg: &str = match name {
        "location" => include_str!("../../assets/location.svg"),
        "map" => include_str!("../../assets/map.svg"),
        "vector" => include_str!("../../assets/vector.svg"),
        "globe" => include_str!("../../assets/globe.svg"),
        "pdok" => include_str!("../../assets/pdok.svg"),
        "point" => include_str!("../../assets/point.svg"),
        "server" => include_str!("../../assets/server.svg"),
        "area" => include_str!("../../assets/area.svg"),
        "nlcs" => include_str!("../../assets/bgt.svg"),
        _ => include_str!("../../assets/list.svg"),
    };
    let bytes = svg.replace("currentColor", "#dfe5ea").into_bytes();
    egui::Image::from_bytes(format!("bytes://open-infra/{name}.svg"), bytes).fit_to_exact_size(Vec2::splat(18.0))
}

/// The import area as a lon/lat ring (edges sampled, so it follows the
/// projection's curvature on the web map).
fn site_ring(site: &Site) -> Option<Vec<LonLat>> {
    let t = Transformer::new(&site.crs, "EPSG:4326").ok()?;
    let b = site.bbox();
    let mut ring = Vec::with_capacity(33);
    const N: usize = 8;
    let edges = [([b[0], b[3]], [b[2], b[3]]), ([b[2], b[3]], [b[2], b[1]]), ([b[2], b[1]], [b[0], b[1]]), ([b[0], b[1]], [b[0], b[3]])];
    for (a, c) in edges {
        for i in 0..N {
            let f = i as f64 / N as f64;
            ring.push(t.apply([a[0] + (c[0] - a[0]) * f, a[1] + (c[1] - a[1]) * f]).ok()?);
        }
    }
    Some(ring)
}

// ── Page states ─────────────────────────────────────────────────────────────

#[derive(Default)]
struct LocationPage {
    query: String,
    hits: Vec<Hit>,
    search: Option<Job<Vec<Hit>>>,
    custom_epsg: String,
    crs_job: Option<Job<CrsInfo>>,
    geo_origin: bool,
    aerial: bool,
    apply: Option<Job<String>>,
}

struct RasterPage {
    server: Option<MapServer>,
    caps: Option<WmsCaps>,
    caps_job: Option<Job<WmsCaps>>,
    layer: Option<WmsLayer>,
    filter: String,
    format: String,
    resolution: f64,
    fade: u8,
    preview: Option<(TextureHandle, [LonLat; 4])>,
    preview_job: Option<Job<Preview>>,
    run: Option<Job<String>>,
}

struct WfsPage {
    server: Option<MapServer>,
    caps: Option<WfsCaps>,
    caps_job: Option<Job<WfsCaps>>,
    ft: Option<WfsType>,
    filter: String,
    limit: usize,
    fetch: Option<Job<(WfsResult, Vec<Vec<LonLat>>)>>,
    result: Option<WfsResult>,
    preview: Vec<(Vec<LonLat>, Color32)>,
    label: String,
    text_m: f64,
    aci: i16,
    clip: bool,
    place: Option<Job<String>>,
}

type OsmFetched = (Vec<&'static str>, Vec<Vec<Feature>>, Vec<(Vec<LonLat>, Color32)>);

struct OsmPage {
    on: Vec<bool>,
    labels: bool,
    text_m: f64,
    clip: bool,
    fetch: Option<Job<OsmFetched>>,
    fetched: Option<OsmFetched>,
    run: Option<Job<String>>,
}

struct PdokPage {
    on: Vec<bool>,
    run: Option<Job<String>>,
}

type Preview2 = Vec<(Vec<LonLat>, Color32)>;
/// Fetched BGT/BRK for the chosen groups, with its map preview.
type NlcsFetched = (Vec<&'static str>, bool, Arc<Vec<(String, Vec<Feature>)>>, Option<Arc<Cadastre>>, Preview2);

struct NlcsPage {
    status: Status,
    discipline: String,
    scale: f64,
    object_data: bool,
    labels: bool,
    clip: bool,
    bgt_on: Vec<bool>,
    brk: bool,
    klic_path: String,
    klic: Option<Arc<klic::Levering>>,
    klic_preview: Preview2,
    klic_job: Option<Job<(klic::Levering, Preview2)>>,
    klic_on: Vec<(String, usize, bool)>,
    klic_discipline: String,
    imkl_colors: bool,
    sheet: bool,
    info: SheetInfo,
    fetch: Option<Job<NlcsFetched>>,
    fetched: Option<NlcsFetched>,
    run: Option<Job<String>>,
}

impl Default for NlcsPage {
    fn default() -> Self {
        let o = NlcsOptions::default();
        Self {
            status: o.status,
            discipline: o.discipline,
            scale: o.scale,
            object_data: o.object_data,
            labels: o.labels,
            clip: o.clip,
            bgt_on: bgt::GROUPS.iter().map(|g| g.default_on).collect(),
            brk: true,
            klic_path: String::new(),
            klic: None,
            klic_preview: Vec::new(),
            klic_job: None,
            klic_on: Vec::new(),
            klic_discipline: "OI".into(),
            imkl_colors: false,
            sheet: true,
            info: SheetInfo::default(),
            fetch: None,
            fetched: None,
            run: None,
        }
    }
}

impl NlcsPage {
    fn options(&self) -> NlcsOptions {
        NlcsOptions { status: self.status, discipline: self.discipline.clone(), scale: self.scale, object_data: self.object_data, labels: self.labels, clip: self.clip }
    }
    fn klic_options(&self) -> KlicOptions {
        let mut nlcs = self.options();
        nlcs.discipline = self.klic_discipline.clone();
        nlcs.clip = false;
        KlicOptions { nlcs, imkl_colors: self.imkl_colors, themes: self.klic_on.iter().filter(|t| t.2).map(|t| t.0.clone()).collect() }
    }
    fn collections(&self) -> Vec<&'static str> {
        bgt::GROUPS.iter().zip(&self.bgt_on).filter(|(_, on)| **on).flat_map(|(g, _)| g.collections.iter().copied()).collect()
    }
}

fn preview_color(rgb: (u8, u8, u8)) -> Color32 {
    // NLCS colour 7 is white on a dark CAD background: dark on the map.
    if rgb == (255, 255, 255) { Color32::from_gray(40) } else { Color32::from_rgb(rgb.0, rgb.1, rgb.2) }
}

fn rd_lines(to: &dyn Fn([f64; 2]) -> Option<[f64; 2]>, f: &Feature, color: Color32, out: &mut Preview2) {
    for (line, closed) in &f.lines {
        let mut pts: Vec<LonLat> = line.iter().filter_map(|p| to(*p)).collect();
        if *closed {
            if let Some(first) = pts.first().copied() {
                pts.push(first);
            }
        }
        if pts.len() > 1 {
            out.push((pts, color));
        }
    }
}

fn bgt_preview(site: &Site, data: &[(String, Vec<Feature>)], cad: Option<&Cadastre>) -> Result<Preview2, String> {
    let to = vector::data_to(site, "EPSG:28992", "EPSG:4326", &[])?;
    let mut out = Vec::new();
    for (c, feats) in data {
        if bgt::is_label(c) {
            continue;
        }
        for f in feats {
            let rgb = nlcs::bgt_lookup(c, &f.props).map(|x| x.0).or_else(|| nlcs::bgt_default(c)).and_then(nlcs::object).map(|o| nlcs::layer(o, Status::B, "XX", Element::G).rgb).unwrap_or((128, 128, 128));
            rd_lines(&*to, f, preview_color(rgb), &mut out);
        }
    }
    if let Some(cad) = cad {
        let rgb = nlcs::object_by_name("KG", "GRENS_PERCEEL").map(|o| nlcs::layer(o, Status::B, "XX", Element::G).rgb).unwrap_or((255, 0, 0));
        for f in &cad.boundaries {
            rd_lines(&*to, f, preview_color(rgb), &mut out);
        }
    }
    Ok(out)
}

fn klic_preview(site: &Site, lev: &klic::Levering) -> Result<Preview2, String> {
    let to = vector::data_to(site, "EPSG:28992", "EPSG:4326", &[])?;
    let mut out = Vec::new();
    for f in &lev.features {
        rd_lines(&*to, f, preview_color(nlcs::imkl_rgb(klic::thema(f))), &mut out);
    }
    Ok(out)
}

/// The RD extent of a delivery.
fn klic_extent(lev: &klic::Levering) -> Option<[f64; 4]> {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for f in &lev.features {
        for p in f.lines.iter().flat_map(|(l, _)| l.iter()).chain(f.points.iter()) {
            b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
        }
    }
    (b[0] < b[2]).then_some(b)
}

#[derive(Default)]
struct ServersPage {
    kind: Option<ServerKind>,
    url: String,
    name: String,
    region: String,
    test: Option<Job<String>>,
}

struct CoordsPage {
    crs: String,
    xy: [f64; 2],
    picked: Option<LonLat>,
    job: Option<Job<CrsInfo>>,
}

pub struct App {
    rx: Receiver<UiMsg>,
    tab: Tab,
    /// Location page edits a draft; the other pages use the saved site.
    draft: Site,
    has_site: bool,
    message: Option<(bool, String)>,
    map: MapView,
    fit_pending: bool,
    loc: LocationPage,
    raster: RasterPage,
    wfs: WfsPage,
    osm: OsmPage,
    pdok: PdokPage,
    nlcs: NlcsPage,
    srv: ServersPage,
    coords: CoordsPage,
    servers: Vec<MapServer>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, rx: Receiver<UiMsg>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let mut visuals = egui::Visuals::dark();
        visuals.selection.bg_fill = ACCENT;
        visuals.hyperlink_color = ACCENT;
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.all_styles_mut(|s| {
            s.spacing.item_spacing = Vec2::new(8.0, 6.0);
            s.spacing.button_padding = Vec2::new(10.0, 4.0);
        });
        let site = bridge::site();
        let draft = site.clone().unwrap_or_default();
        Self {
            rx,
            tab: Tab::Location,
            has_site: site.is_some(),
            map: MapView::new([draft.lon, draft.lat], 15.0),
            draft,
            fit_pending: true,
            message: None,
            loc: LocationPage::default(),
            raster: RasterPage { server: None, caps: None, caps_job: None, layer: None, filter: String::new(), format: "image/png".into(), resolution: 0.25, fade: 0, preview: None, preview_job: None, run: None },
            wfs: WfsPage { server: None, caps: None, caps_job: None, ft: None, filter: String::new(), limit: 5000, fetch: None, result: None, preview: Vec::new(), label: String::new(), text_m: 1.0, aci: 2, clip: true, place: None },
            osm: OsmPage { on: THEMES.iter().map(|t| t.default_on).collect(), labels: true, text_m: 2.0, clip: true, fetch: None, fetched: None, run: None },
            pdok: PdokPage { on: PDOK.iter().map(|d| d.default_on).collect(), run: None },
            nlcs: NlcsPage::default(),
            srv: ServersPage { region: "Eigen servers".into(), ..Default::default() },
            coords: CoordsPage { crs: "EPSG:4326".into(), xy: [4.660_612, 51.814_158], picked: None, job: None },
            servers: servers::all(),
        }
    }

    fn ok(&mut self, msg: impl Into<String>) {
        let m = msg.into();
        bridge::info(&m);
        self.message = Some((true, m));
    }

    fn err(&mut self, msg: impl Into<String>) {
        self.message = Some((false, msg.into()));
    }

    fn reload_site(&mut self) {
        match bridge::site() {
            Some(s) => {
                self.draft = s;
                self.has_site = true;
            }
            None => self.has_site = false,
        }
        self.fit_pending = true;
    }

    fn handle_messages(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                UiMsg::Show(tab) => {
                    self.tab = tab;
                    self.reload_site();
                    self.servers = servers::all();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                UiMsg::Picked(p) => {
                    self.tab = Tab::Coords;
                    if let Ok(ll) = crs::transform(&self.draft.crs, "EPSG:4326", p) {
                        self.coords.picked = Some(ll);
                        self.map.center = ll;
                    }
                    if let Ok(q) = crs::transform(&self.draft.crs, &self.coords.crs, p) {
                        self.coords.xy = q;
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                UiMsg::SiteChanged => self.reload_site(),
                UiMsg::Klic(path) => {
                    self.tab = Tab::Nlcs;
                    self.reload_site();
                    self.nlcs.klic_path = path.to_string_lossy().into_owned();
                    self.load_klic(ctx);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
        }
    }

    /// The site for import pages (saved in the drawing), or a hint to set one.
    fn saved_site(&mut self, ui: &mut egui::Ui) -> Option<Site> {
        let s = bridge::site();
        if s.is_none() {
            group(ui, "Projectlocatie ontbreekt", |ui| {
                ui.colored_label(WARN, "Deze tekening heeft nog geen projectlocatie. Stel die eerst in.");
                if ui.button("Geografische locatie instellen…").clicked() {
                    self.tab = Tab::Location;
                }
            });
        }
        s
    }

    /// Map toolbar + map. `site` gives the bounding box drawn on the map.
    fn map_section(&mut self, ui: &mut egui::Ui, site: Option<&Site>, marker: Option<LonLat>, image: Option<&(TextureHandle, [LonLat; 4])>, lines: &[(Vec<LonLat>, Color32)]) -> Option<MapEvent> {
        let ring = site.and_then(site_ring);
        let map_h = if self.tab == Tab::Location { (ui.ctx().content_rect().height() * 0.62).max(460.0) } else { MAP_H };
        if self.fit_pending {
            if let Some(r) = &ring {
                self.map.fit(r, Vec2::new(ui.available_width(), map_h));
                self.fit_pending = false;
            }
        }
        let mut event = None;
        group(ui, "Kaart", |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Button::image_and_text(icon("area"), "Gebied tekenen").selected(self.map.area_mode))
                    .on_hover_text("Sleep op de kaart een rechthoek als importgebied (of houd Shift ingedrukt)")
                    .clicked()
                    .then(|| self.map.area_mode = !self.map.area_mode);
                if ui.add_enabled(ring.is_some(), egui::Button::new("Zoom naar gebied")).clicked() {
                    if let Some(r) = &ring {
                        self.map.fit(r, Vec2::new(ui.available_width(), map_h));
                    }
                }
                if let Some(h) = self.map.hover() {
                    let mut txt = format!("{:.6}° N  {:.6}° O", h[1], h[0]);
                    if let Some(s) = site {
                        if let Ok(p) = crs::transform("EPSG:4326", &s.crs, h) {
                            txt.push_str(&format!("   |   {}  X {:.1}  Y {:.1}", s.crs, p[0], p[1]));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(txt).monospace().small().weak());
                    });
                }
            });
            event = self.map.show(ui, map_h, Overlay { marker, bbox: ring.clone(), image: image.map(|(t, c)| (t, *c)), lines });
        });
        event
    }

    // ── Geografische locatie ──────────────────────────────────────────────
    fn location(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if let Some(r) = poll(&mut self.loc.search) {
            match r {
                Ok(h) if h.is_empty() => self.err("Adres niet gevonden."),
                Ok(h) => self.loc.hits = h,
                Err(e) => self.err(e),
            }
        }
        if let Some(r) = poll(&mut self.loc.crs_job) {
            match r {
                Ok(info) if info.proj4.contains("+proj=longlat") => self.err("Kies een geprojecteerd stelsel (in meters) als projectstelsel."),
                Ok(info) => {
                    self.draft.crs = info.code;
                    self.draft.reproject_origin();
                }
                Err(e) => self.err(e),
            }
        }
        if let Some(r) = poll(&mut self.loc.apply) {
            match r {
                Ok(m) => self.ok(m),
                Err(e) => self.err(e),
            }
        }

        page_header(ui, "location", "Geografische locatie instellen", "Kies de projectlocatie op de kaart of zoek een adres; teken of typ de begrenzing van het importgebied.");
        let draft = self.draft.clone();
        match self.map_section(ui, Some(&draft), Some([draft.lon, draft.lat]), None, &[]) {
            Some(MapEvent::Clicked(ll)) => {
                self.draft.lon = ll[0];
                self.draft.lat = ll[1];
                self.draft.label = format!("Kaartpunt {:.5}, {:.5}", ll[1], ll[0]);
                self.draft.reproject_origin();
            }
            Some(MapEvent::RectDrawn(a, b)) => {
                if let (Ok(pa), Ok(pb)) = (crs::transform("EPSG:4326", &self.draft.crs, a), crs::transform("EPSG:4326", &self.draft.crs, b)) {
                    self.draft.set_origin(((pa[0] + pb[0]) / 2.0).round(), ((pa[1] + pb[1]) / 2.0).round());
                    self.draft.width = (pa[0] - pb[0]).abs().round().max(10.0);
                    self.draft.height = (pa[1] - pb[1]).abs().round().max(10.0);
                    self.draft.label = format!("Getekend gebied {} × {} m", self.draft.width, self.draft.height);
                    self.map.area_mode = false;
                }
            }
            None => {}
        }

        group(ui, "Locatie", |ui| {
            egui::Grid::new("loc").num_columns(2).spacing([14.0, 7.0]).min_col_width(170.0).show(ui, |ui| {
                ui.label("Zoek plaats / adres");
                ui.horizontal(|ui| {
                    let edit = ui.add(egui::TextEdit::singleline(&mut self.loc.query).hint_text("bijv. Lange Geldersekade 2 Dordrecht, A2 Vught, Hamburg Hafen, 51.814, 4.660").desired_width(ui.available_width() - 90.0));
                    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (ui.add_enabled(self.loc.search.is_none(), egui::Button::new("Zoeken")).clicked() || enter) && !self.loc.query.trim().is_empty() {
                        let q = self.loc.query.clone();
                        self.loc.search = Some(Job::spawn(&ctx, "Zoeken…", move |_| geocode::search(&q)));
                    }
                });
                ui.end_row();

                if !self.loc.hits.is_empty() {
                    ui.label("");
                    let mut picked = None;
                    egui::Frame::new().fill(ui.visuals().extreme_bg_color).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("hits").max_height(130.0).show(ui, |ui| {
                            for (i, h) in self.loc.hits.iter().enumerate() {
                                let src = RichText::new(format!("{:<4}", h.source)).monospace().small().color(ACCENT);
                                ui.horizontal(|ui| {
                                    ui.label(src);
                                    if ui.selectable_label(false, &h.label).clicked() {
                                        picked = Some(i);
                                    }
                                });
                            }
                        });
                    });
                    if let Some(i) = picked {
                        let h = self.loc.hits[i].clone();
                        self.draft.label = h.label.clone();
                        self.draft.lon = h.lon;
                        self.draft.lat = h.lat;
                        let code = crs::suggest_for(&h.country, h.lon, h.lat);
                        if crs::known(&code).is_some() {
                            self.draft.crs = code;
                            self.draft.reproject_origin();
                        } else {
                            self.loc.crs_job = Some(Job::spawn(&ctx, "Stelsel laden…", move |_| crs::ensure(&code)));
                        }
                        self.map.center = [h.lon, h.lat];
                        self.fit_pending = true;
                        self.loc.hits.clear();
                    }
                    ui.end_row();
                }

                ui.label("Coördinatenstelsel (CRS)");
                ui.horizontal(|ui| {
                    let suggested = crs::suggest(self.draft.lon, self.draft.lat);
                    let mut list = crs::builtin_list();
                    for c in [&suggested, &self.draft.crs] {
                        if !list.iter().any(|i| &i.code == c) {
                            if let Some(i) = crs::known(c) {
                                list.push(i);
                            }
                        }
                    }
                    let current = crs::known(&self.draft.crs).map(|i| format!("{} — {}", i.code, i.name)).unwrap_or(self.draft.crs.clone());
                    let mut chosen = None;
                    egui::ComboBox::from_id_salt("crs").width(340.0).height(420.0).selected_text(current).show_ui(ui, |ui| {
                        for i in list.iter().filter(|i| !i.proj4.contains("+proj=longlat")) {
                            let star = if i.code == suggested { "  ★ aanbevolen" } else { "" };
                            if ui.selectable_label(i.code == self.draft.crs, format!("{} — {} ({}){star}", i.code, i.name, i.region)).clicked() {
                                chosen = Some(i.code.clone());
                            }
                        }
                    });
                    if let Some(c) = chosen {
                        self.draft.crs = c;
                        self.draft.reproject_origin();
                    }
                    ui.add(egui::TextEdit::singleline(&mut self.loc.custom_epsg).hint_text("EPSG-code").desired_width(80.0));
                    if ui.add_enabled(self.loc.crs_job.is_none() && !self.loc.custom_epsg.trim().is_empty(), egui::Button::new("Laden")).on_hover_text("Definitie ophalen via epsg.io").clicked() {
                        let code = self.loc.custom_epsg.clone();
                        self.loc.crs_job = Some(Job::spawn(&ctx, "Stelsel laden…", move |_| crs::ensure(&code)));
                    }
                });
                ui.end_row();

                ui.label("Breedtegraad / lengtegraad");
                ui.horizontal(|ui| {
                    let a = ui.add(egui::DragValue::new(&mut self.draft.lat).speed(0.0001).fixed_decimals(7).prefix("lat "));
                    let b = ui.add(egui::DragValue::new(&mut self.draft.lon).speed(0.0001).fixed_decimals(7).prefix("lon "));
                    if a.changed() || b.changed() {
                        self.draft.reproject_origin();
                    }
                });
                ui.end_row();

                ui.label("Omschrijving");
                ui.add(egui::TextEdit::singleline(&mut self.draft.label).desired_width(f32::INFINITY));
                ui.end_row();
            });
        });

        group(ui, "Begrenzing en plaatsing", |ui| {
            egui::Grid::new("bbox").num_columns(2).spacing([14.0, 7.0]).min_col_width(170.0).show(ui, |ui| {
                ui.label("Basispunt X / Y (m)");
                ui.horizontal(|ui| {
                    let (mut e, mut n) = (self.draft.origin_e, self.draft.origin_n);
                    let a = ui.add(egui::DragValue::new(&mut e).speed(1.0).fixed_decimals(3).prefix("X "));
                    let b = ui.add(egui::DragValue::new(&mut n).speed(1.0).fixed_decimals(3).prefix("Y "));
                    if a.changed() || b.changed() {
                        self.draft.set_origin(e, n);
                    }
                });
                ui.end_row();
                ui.label("Begrenzing breedte × hoogte (m)");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.draft.width).speed(10.0).range(10.0..=20_000.0));
                    ui.label("×");
                    ui.add(egui::DragValue::new(&mut self.draft.height).speed(10.0).range(10.0..=20_000.0));
                    for s in [250.0, 500.0, 1000.0, 2000.0] {
                        if ui.small_button(format!("{s:.0}")).clicked() {
                            self.draft.width = s;
                            self.draft.height = s;
                        }
                    }
                });
                ui.end_row();
                ui.label("Coördinaten in de tekening");
                ui.vertical(|ui| {
                    ui.radio_value(&mut self.draft.absolute, true, "Absoluut — CRS-coördinaten (bijv. RD) direct in de tekening");
                    ui.radio_value(&mut self.draft.absolute, false, "Relatief — basispunt = 0,0");
                });
                ui.end_row();
                ui.label("Na opslaan");
                ui.vertical(|ui| {
                    ui.checkbox(&mut self.loc.geo_origin, "Basispunt-markering in de tekening plaatsen (GeoOrigin)");
                    ui.checkbox(&mut self.loc.aerial, "Luchtfoto importeren (PDOK in NL, Sentinel-2 elders)");
                });
                ui.end_row();
            });
            let units = bridge::target().map(|t| t.unit_factor).unwrap_or(1.0);
            ui.label(RichText::new(format!(
                "Tekeneenheid: {} (INSUNITS){}",
                if (units - 1.0).abs() < 1e-9 { "meter".to_string() } else if (units - 1000.0).abs() < 1e-9 { "millimeter".into() } else { format!("{units} per meter") },
                if self.has_site { "" } else { " · deze tekening heeft nog geen projectlocatie" }
            )).small().weak());
        });

    }

    // ── Rasterdata (WMS / TMS) ──────────────────────────────────────────────
    fn raster(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if let Some(r) = poll(&mut self.raster.caps_job) {
            match r {
                Ok(c) => {
                    let pre = self.raster.server.as_ref().map(|s| s.default_layer.clone()).unwrap_or_default();
                    self.raster.layer = c.layers.iter().find(|l| l.name == pre).or(c.layers.first()).cloned();
                    if let Some(l) = &self.raster.layer {
                        self.raster.format = raster::default_format(&c, &l.name);
                    }
                    self.message = Some((true, format!("{} lagen (WMS {}).", c.layers.len(), c.version)));
                    self.raster.caps = Some(c);
                }
                Err(e) => self.err(e),
            }
        }
        if let Some(r) = poll(&mut self.raster.preview_job) {
            match r {
                Ok(p) => {
                    let img = egui::ColorImage::from_rgba_unmultiplied(p.size, &p.rgba);
                    self.raster.preview = Some((ctx.load_texture("wms-preview", img, egui::TextureOptions::LINEAR), p.corners));
                    self.message = Some((true, "Voorbeeld getoond op de kaart.".into()));
                }
                Err(e) => self.err(format!("Test mislukt: {e}")),
            }
        }
        if let Some(r) = poll(&mut self.raster.run) {
            match r {
                Ok(m) => self.ok(m),
                Err(e) => self.err(e),
            }
        }

        page_header(ui, "map", "Rasterdata laden met WMS / TMS", "Luchtfoto's, kaarten en hoogtebeelden als onderlegger in de tekening.");
        let Some(site) = self.saved_site(ui) else { return };
        let preview = self.raster.preview.take();
        self.map_section(ui, Some(&site), None, preview.as_ref(), &[]);
        self.raster.preview = preview;

        group(ui, "Webserver / verzoek", |ui| {
            egui::Grid::new("wms").num_columns(2).spacing([14.0, 7.0]).min_col_width(170.0).show(ui, |ui| {
                ui.label("Voorgedefinieerde server");
                if let Some(s) = server_combo(ui, "wms_srv", &self.servers, &[ServerKind::Wms, ServerKind::Xyz], self.raster.server.as_ref()) {
                    self.raster.caps = None;
                    self.raster.layer = None;
                    self.raster.preview = None;
                    self.raster.filter.clear();
                    if s.kind == ServerKind::Wms {
                        let url = s.url.clone();
                        self.raster.caps_job = Some(Job::spawn(&ctx, "Capabilities ophalen…", move |_| caps::fetch_wms(&url)));
                        self.raster.resolution = 0.25;
                    } else {
                        self.raster.resolution = 0.5;
                    }
                    self.raster.server = Some(s);
                }
                ui.end_row();
                if let Some(s) = &self.raster.server {
                    ui.label("Bron");
                    ui.label(RichText::new(format!("{}  ·  {}", s.attribution, s.url)).small().weak());
                    ui.end_row();
                }
                if let Some(j) = &self.raster.caps_job {
                    ui.label("");
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(j.status());
                    });
                    ui.end_row();
                }
                if let (Some(c), Some(ServerKind::Wms)) = (&self.raster.caps, self.raster.server.as_ref().map(|s| s.kind)) {
                    ui.label("Laag");
                    ui.label(RichText::new(self.raster.layer.as_ref().map(|l| format!("{}  ({})", l.title, l.name)).unwrap_or_else(|| "— kies hieronder —".into())).strong());
                    ui.end_row();
                    ui.label("Formaat");
                    egui::ComboBox::from_id_salt("fmt").selected_text(self.raster.format.clone()).show_ui(ui, |ui| {
                        for f in c.formats.iter().filter(|f| f.starts_with("image/png") || f.starts_with("image/jpeg")) {
                            ui.selectable_value(&mut self.raster.format, f.clone(), f);
                        }
                    });
                    ui.end_row();
                }
            });
            if let (Some(c), Some(ServerKind::Wms)) = (&self.raster.caps, self.raster.server.as_ref().map(|s| s.kind)) {
                ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut self.raster.filter).hint_text(format!("Filter {} lagen…", c.layers.len())).desired_width(f32::INFINITY));
                let mut chosen = None;
                list_frame(ui, 170.0, "wms_layers", |ui| {
                    let f = self.raster.filter.to_lowercase();
                    for l in c.layers.iter().filter(|l| f.is_empty() || l.name.to_lowercase().contains(&f) || l.title.to_lowercase().contains(&f)) {
                        let sel = self.raster.layer.as_ref().is_some_and(|x| x.name == l.name);
                        let reproj = !l.crs.is_empty() && !l.crs.contains(&site.crs);
                        let txt = format!("{}{}{}", l.title, if l.title != l.name { format!("   ({})", l.name) } else { String::new() }, if reproj { "   · herprojectie" } else { "" });
                        if ui.selectable_label(sel, txt).clicked() {
                            chosen = Some(l.clone());
                        }
                    }
                });
                if let Some(l) = chosen {
                    self.raster.format = raster::default_format(c, &l.name);
                    self.raster.layer = Some(l);
                    self.raster.preview = None;
                }
            }
        });

        let mut too_big = false;
        if self.raster.server.is_some() {
            group(ui, "Import-instellingen", |ui| {
                egui::Grid::new("wms_imp").num_columns(2).spacing([14.0, 7.0]).min_col_width(170.0).show(ui, |ui| {
                    ui.label("Resolutie");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut self.raster.resolution).speed(0.01).range(0.02..=100.0).suffix(" m/px"));
                        for r in [0.08, 0.25, 0.5, 1.0] {
                            if ui.small_button(format!("{r}")).clicked() {
                                self.raster.resolution = r;
                            }
                        }
                    });
                    ui.end_row();
                    ui.label("Pixels");
                    let (w, h) = (site.width / self.raster.resolution, site.height / self.raster.resolution);
                    too_big = w * h > MAX_TOTAL_PX;
                    ui.label(RichText::new(format!("{w:.0} × {h:.0} px voor {} × {} m{}", site.width, site.height, if too_big { " — te groot: verhoog de resolutie of verklein het gebied" } else { "" }))
                        .color(if too_big { ERR_RED } else { ui.visuals().weak_text_color() }));
                    ui.end_row();
                    ui.label("Vervagen");
                    ui.add(egui::Slider::new(&mut self.raster.fade, 0..=80).suffix(" %"));
                    ui.end_row();
                    ui.label("Laag in tekening");
                    let name = match (&self.raster.server, &self.raster.layer) {
                        (Some(s), _) if s.kind == ServerKind::Xyz => crate::emit::layer_name("GIS", &s.name),
                        (_, Some(l)) => crate::emit::layer_name("GIS-WMS", &l.title),
                        _ => "—".into(),
                    };
                    ui.label(RichText::new(name).monospace());
                    ui.end_row();
                });
            });
        }

    }

    // ── Vectordata (WFS) ────────────────────────────────────────────────────
    fn wfs(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if let Some(r) = poll(&mut self.wfs.caps_job) {
            match r {
                Ok(c) => {
                    let pre = self.wfs.server.as_ref().map(|s| s.default_layer.clone()).unwrap_or_default();
                    self.wfs.ft = c.types.iter().find(|t| t.name == pre).or(c.types.first()).cloned();
                    self.message = Some((true, format!("{} objecttypen (WFS {}).", c.types.len(), c.version)));
                    self.wfs.caps = Some(c);
                }
                Err(e) => self.err(e),
            }
        }
        if let Some(r) = poll(&mut self.wfs.fetch) {
            match r {
                Ok((res, lines)) => {
                    let names = vector::property_names(&res.features);
                    const HINTS: &[&str] = &["perceelnummer", "huisnummer", "hectomtrng", "kmGeocodeT", "stt_naam", "naam", "name"];
                    self.wfs.label = HINTS.iter().find(|h| names.iter().any(|n| n == *h)).map(|h| h.to_string()).unwrap_or_default();
                    let color = aci_color(self.wfs.aci);
                    self.wfs.preview = lines.into_iter().map(|l| (l, color)).collect();
                    self.message = Some((true, format!("{} objecten opgehaald{} — zie de kaart.", res.features.len(), if res.truncated { " (limiet bereikt)" } else { "" })));
                    self.wfs.result = Some(res);
                }
                Err(e) => self.err(e),
            }
        }
        if let Some(r) = poll(&mut self.wfs.place) {
            match r {
                Ok(m) => self.ok(m),
                Err(e) => self.err(e),
            }
        }

        page_header(ui, "vector", "2D-vectordata laden met WFS", "Percelen, panden, wegvakken, spoor en andere objecten als lijnen met labels.");
        let Some(site) = self.saved_site(ui) else { return };
        let preview = std::mem::take(&mut self.wfs.preview);
        self.map_section(ui, Some(&site), None, None, &preview);
        self.wfs.preview = preview;

        group(ui, "Webserver / verzoek", |ui| {
            egui::Grid::new("wfs").num_columns(2).spacing([14.0, 7.0]).min_col_width(170.0).show(ui, |ui| {
                ui.label("Voorgedefinieerde server");
                if let Some(s) = server_combo(ui, "wfs_srv", &self.servers, &[ServerKind::Wfs], self.wfs.server.as_ref()) {
                    self.wfs.caps = None;
                    self.wfs.ft = None;
                    self.wfs.result = None;
                    self.wfs.preview.clear();
                    let url = s.url.clone();
                    self.wfs.caps_job = Some(Job::spawn(&ctx, "Capabilities ophalen…", move |_| caps::fetch_wfs(&url)));
                    self.wfs.server = Some(s);
                }
                ui.end_row();
                if let Some(j) = &self.wfs.caps_job {
                    ui.label("");
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(j.status());
                    });
                    ui.end_row();
                }
                if let Some(_c) = &self.wfs.caps {
                    ui.label("Objecttype");
                    ui.label(RichText::new(self.wfs.ft.as_ref().map(|t| format!("{}  ({})", t.title, t.name)).unwrap_or_else(|| "— kies hieronder —".into())).strong());
                    ui.end_row();
                    ui.label("Max. aantal objecten");
                    ui.add(egui::DragValue::new(&mut self.wfs.limit).speed(100).range(1..=200_000));
                    ui.end_row();
                }
            });
            if let Some(c) = &self.wfs.caps {
                ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut self.wfs.filter).hint_text(format!("Filter {} objecttypen…", c.types.len())).desired_width(f32::INFINITY));
                let mut chosen = None;
                list_frame(ui, 150.0, "wfs_types", |ui| {
                    let f = self.wfs.filter.to_lowercase();
                    for t in c.types.iter().filter(|t| f.is_empty() || t.name.to_lowercase().contains(&f) || t.title.to_lowercase().contains(&f)) {
                        let sel = self.wfs.ft.as_ref().is_some_and(|x| x.name == t.name);
                        if ui.selectable_label(sel, format!("{}   ({})", t.title, t.name)).clicked() {
                            chosen = Some(t.clone());
                        }
                    }
                });
                if let Some(t) = chosen {
                    self.wfs.ft = Some(t);
                    self.wfs.result = None;
                    self.wfs.preview.clear();
                }
            }
        });

        group(ui, "Import-instellingen", |ui| {
            egui::Grid::new("wfs_imp").num_columns(2).spacing([14.0, 7.0]).min_col_width(170.0).show(ui, |ui| {
                ui.label("Label (attribuut)");
                let names = self.wfs.result.as_ref().map(|r| vector::property_names(&r.features)).unwrap_or_default();
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("wfs_label").selected_text(if self.wfs.label.is_empty() { "— geen —".to_string() } else { self.wfs.label.clone() }).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.wfs.label, String::new(), "— geen —");
                        for n in &names {
                            ui.selectable_value(&mut self.wfs.label, n.clone(), n);
                        }
                    });
                    if names.is_empty() {
                        ui.label(RichText::new("eerst Test om attributen te zien").small().weak());
                    }
                });
                ui.end_row();
                ui.label("Teksthoogte");
                ui.add(egui::DragValue::new(&mut self.wfs.text_m).speed(0.1).range(0.05..=100.0).suffix(" m"));
                ui.end_row();
                ui.label("Lijnkleur (laag)");
                if aci_combo(ui, "wfs_aci", &mut self.wfs.aci) {
                    let c = aci_color(self.wfs.aci);
                    self.wfs.preview.iter_mut().for_each(|(_, col)| *col = c);
                }
                ui.end_row();
                ui.label("Begrenzing");
                ui.checkbox(&mut self.wfs.clip, "Bijsnijden op het importgebied");
                ui.end_row();
            });
        });

    }

    // ── OpenStreetMap ───────────────────────────────────────────────────────
    fn osm(&mut self, ui: &mut egui::Ui) {
        if let Some(r) = poll(&mut self.osm.fetch) {
            match r {
                Ok(f) => {
                    let n: usize = f.1.iter().map(Vec::len).sum();
                    self.message = Some((true, format!("{n} OpenStreetMap-objecten opgehaald — zie de kaart.")));
                    self.osm.fetched = Some(f);
                }
                Err(e) => self.err(e),
            }
        }
        if let Some(r) = poll(&mut self.osm.run) {
            match r {
                Ok(m) => self.ok(m),
                Err(e) => self.err(e),
            }
        }

        page_header(ui, "globe", "OpenStreetMap importeren", "Wereldwijd: gebouwen, wegen, spoor, water, kunstwerken, leidingen en meer — elk thema op een eigen laag.");
        let Some(site) = self.saved_site(ui) else { return };
        let fetched = self.osm.fetched.take();
        self.map_section(ui, Some(&site), None, None, fetched.as_ref().map(|f| f.2.as_slice()).unwrap_or(&[]));
        self.osm.fetched = fetched;

        group(ui, "Thema's", |ui| {
            egui::Grid::new("osm").num_columns(3).spacing([14.0, 5.0]).show(ui, |ui| {
                for (i, t) in THEMES.iter().enumerate() {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, aci_color(t.aci));
                    ui.checkbox(&mut self.osm.on[i], t.label);
                    ui.label(RichText::new(t.layer).monospace().small().weak());
                    ui.end_row();
                }
            });
        });
        group(ui, "Import-instellingen", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.osm.labels, "Namen en huisnummers als tekst");
                ui.add(egui::DragValue::new(&mut self.osm.text_m).speed(0.1).range(0.1..=50.0).prefix("hoogte ").suffix(" m"));
            });
            ui.checkbox(&mut self.osm.clip, "Bijsnijden op het importgebied");
            ui.label(RichText::new("Data © OpenStreetMap-bijdragers (ODbL), via de Overpass API. Publieke Overpass-servers zijn soms druk; ophalen kan een minuut duren.").small().weak());
        });

    }

    // ── PDOK (NL) ───────────────────────────────────────────────────────────
    fn pdok(&mut self, ui: &mut egui::Ui) {
        if let Some(r) = poll(&mut self.pdok.run) {
            match r {
                Ok(m) => self.ok(m),
                Err(e) => self.err(e),
            }
        }
        page_header(ui, "pdok", "Nederlandse basisregistraties importeren (PDOK)", "Luchtfoto, kadaster, BAG, NWB en ProRail in één keer — kies wat u nodig hebt.");
        let Some(site) = self.saved_site(ui) else { return };
        self.map_section(ui, Some(&site), None, None, &[]);
        if !(site.lon > 3.2 && site.lon < 7.3 && site.lat > 50.7 && site.lat < 53.7) {
            ui.colored_label(WARN, "Let op: de projectlocatie ligt buiten Nederland; PDOK levert hier geen data.");
        }
        let mut groups: Vec<&str> = Vec::new();
        for d in PDOK {
            if !groups.contains(&d.group) {
                groups.push(d.group);
            }
        }
        ui.columns(groups.len().max(1), |cols| {
            for (col, g) in cols.iter_mut().zip(&groups) {
                group(col, g, |ui| {
                    for (i, d) in PDOK.iter().enumerate().filter(|(_, d)| d.group == *g) {
                        ui.checkbox(&mut self.pdok.on[i], d.label);
                    }
                });
            }
        });
    }

    // ── Coördinaten ─────────────────────────────────────────────────────────
    fn coords(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if let Some(r) = poll(&mut self.coords.job) {
            match r {
                Ok(info) => {
                    if let Ok(q) = crs::transform(&self.coords.crs, &info.code, self.coords.xy) {
                        self.coords.xy = q;
                    }
                    self.coords.crs = info.code;
                }
                Err(e) => self.err(e),
            }
        }
        page_header(ui, "point", "Coördinaten omrekenen", "Klik op de kaart, typ een coördinaat of gebruik \"Punt info\" in het lint om een punt in de tekening aan te wijzen.");
        let site = bridge::site();
        let marker = self.coords.picked.or_else(|| crs::transform(&self.coords.crs, "EPSG:4326", self.coords.xy).ok());
        if let Some(MapEvent::Clicked(ll)) = self.map_section(ui, site.as_ref(), marker, None, &[]) {
            self.coords.picked = Some(ll);
            if let Ok(q) = crs::transform("EPSG:4326", &self.coords.crs, ll) {
                self.coords.xy = q;
            }
        }
        group(ui, "Invoer", |ui| {
            let mut codes: Vec<String> = crs::builtin_list().into_iter().map(|i| i.code).collect();
            if let Some(s) = &site {
                if !codes.contains(&s.crs) {
                    codes.push(s.crs.clone());
                }
            }
            let geo = crs::is_geographic(&self.coords.crs);
            ui.horizontal(|ui| {
                ui.label("Stelsel");
                let mut chosen = None;
                egui::ComboBox::from_id_salt("coord_crs").width(320.0).selected_text(crs::known(&self.coords.crs).map(|i| format!("{} — {}", i.code, i.name)).unwrap_or(self.coords.crs.clone())).show_ui(ui, |ui| {
                    for c in &codes {
                        if let Some(i) = crs::known(c) {
                            if ui.selectable_label(*c == self.coords.crs, format!("{} — {}", i.code, i.name)).clicked() {
                                chosen = Some(c.clone());
                            }
                        }
                    }
                });
                if let Some(c) = chosen {
                    self.coords.job = Some(Job::spawn(&ctx, "…", move |_| crs::ensure(&c)));
                }
                let a = ui.add(egui::DragValue::new(&mut self.coords.xy[0]).speed(if geo { 0.0001 } else { 1.0 }).fixed_decimals(if geo { 8 } else { 3 }).prefix(if geo { "lon " } else { "X " }));
                let b = ui.add(egui::DragValue::new(&mut self.coords.xy[1]).speed(if geo { 0.0001 } else { 1.0 }).fixed_decimals(if geo { 8 } else { 3 }).prefix(if geo { "lat " } else { "Y " }));
                if a.changed() || b.changed() {
                    self.coords.picked = None;
                }
            });
        });
        group(ui, "Resultaat", |ui| {
            let mut targets: Vec<String> = vec!["EPSG:4326".into(), "EPSG:28992".into(), "EPSG:3857".into()];
            if let Some(s) = &site {
                targets.insert(1, s.crs.clone());
            }
            if let Ok(ll) = crs::transform(&self.coords.crs, "EPSG:4326", self.coords.xy) {
                targets.push(crs::utm_code_for(ll[0], ll[1]));
            }
            targets.dedup();
            egui::Grid::new("coord_out").num_columns(3).striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
                for t in targets {
                    let Some(info) = crs::known(&t) else { continue };
                    ui.label(format!("{} — {}", info.code, info.name));
                    match crs::transform(&self.coords.crs, &t, self.coords.xy) {
                        Ok(p) if crs::is_geographic(&t) => {
                            ui.monospace(format!("lon {:.8}", p[0]));
                            ui.monospace(format!("lat {:.8}", p[1]));
                        }
                        Ok(p) => {
                            ui.monospace(format!("X {:.3}", p[0]));
                            ui.monospace(format!("Y {:.3}", p[1]));
                        }
                        Err(_) => {
                            ui.label("—");
                            ui.label("—");
                        }
                    }
                    ui.end_row();
                }
            });
        });
    }

    fn location_actions(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let busy = self.loc.apply.is_some() || self.loc.crs_job.is_some() || self.loc.search.is_some();
        button_bar(ui, |ui| {
            if ui.add_enabled(!busy, primary_button("OK")).on_hover_text("Projectlocatie opslaan in de tekening").clicked() {
                match bridge::save_site(&self.draft) {
                    Ok(()) => {
                        self.has_site = true;
                        let site = self.draft.clone();
                        let (origin, aerial) = (self.loc.geo_origin, self.loc.aerial);
                        let summary = format!("Projectlocatie opgeslagen: {} — {} X {:.2} Y {:.2}, {}×{} m.", site.label, site.crs, site.origin_e, site.origin_n, site.width, site.height);
                        if origin || aerial {
                            self.loc.apply = Some(Job::spawn(&ctx, "Bezig…", move |p| {
                                let mut out = vec![summary];
                                if origin {
                                    out.push(import::geo_origin(&site)?);
                                }
                                if aerial {
                                    out.push(import::aerial(&site, p)?);
                                }
                                Ok(out.join(" "))
                            }));
                        } else {
                            self.ok(summary);
                        }
                    }
                    Err(e) => self.err(format!("Opslaan mislukt: {e}")),
                }
            }
            if ui.button("Annuleren").on_hover_text("Wijzigingen terugdraaien naar de opgeslagen locatie").clicked() {
                self.reload_site();
                self.loc.hits.clear();
            }
            job_status(ui, &self.loc.apply);
            job_status(ui, &self.loc.search);
        });
    }

    fn raster_actions(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let Some(site) = bridge::site() else { return };
        let too_big = (site.width / self.raster.resolution) * (site.height / self.raster.resolution) > MAX_TOTAL_PX;
        let is_xyz = self.raster.server.as_ref().is_some_and(|s| s.kind == ServerKind::Xyz);
        let ready = self.raster.server.is_some() && (is_xyz || self.raster.layer.is_some());
        button_bar(ui, |ui| {
            if ui.add_enabled(ready && !too_big && self.raster.run.is_none(), primary_button("OK en importeren")).clicked() {
                let server = self.raster.server.clone().unwrap();
                let opt = raster::Options { resolution: self.raster.resolution, format: self.raster.format.clone() };
                let (fade, caps, layer, s) = (self.raster.fade, self.raster.caps.clone(), self.raster.layer.clone(), site.clone());
                self.raster.run = Some(Job::spawn(&ctx, "Importeren…", move |p| match server.kind {
                    ServerKind::Xyz => import::xyz(&s, &server, &opt, fade, p),
                    _ => import::wms(&s, &server, caps.as_ref().ok_or("geen capabilities")?, layer.as_ref().ok_or("geen laag")?, &opt, fade, p),
                }));
            }
            if ui.add_enabled(ready && self.raster.preview_job.is_none(), egui::Button::new("Test — voorbeeld op kaart")).clicked() {
                let (server, caps, layer, fmt, s) = (self.raster.server.clone().unwrap(), self.raster.caps.clone(), self.raster.layer.clone(), self.raster.format.clone(), site.clone());
                self.raster.preview_job = Some(Job::spawn(&ctx, "Voorbeeld ophalen…", move |_| match server.kind {
                    ServerKind::Xyz => raster::preview_xyz(&s, &server),
                    _ => raster::preview_wms(&s, caps.as_ref().ok_or("geen capabilities")?, layer.as_ref().ok_or("geen laag")?, &fmt, 900),
                }));
            }
            if self.raster.preview.is_some() && ui.button("Voorbeeld wissen").clicked() {
                self.raster.preview = None;
            }
            job_status(ui, &self.raster.run);
            job_status(ui, &self.raster.preview_job);
        });
    }

    fn wfs_actions(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let Some(site) = bridge::site() else { return };
        button_bar(ui, |ui| {
            let can = self.wfs.ft.is_some() && self.wfs.place.is_none() && self.wfs.fetch.is_none();
            if ui.add_enabled(can, primary_button("OK en importeren")).clicked() {
                let style = VectorStyle { aci: self.wfs.aci, label_field: (!self.wfs.label.is_empty()).then(|| self.wfs.label.clone()), text_m: self.wfs.text_m, clip: self.wfs.clip };
                let (s, ft, caps, limit, cached) = (site.clone(), self.wfs.ft.clone().unwrap(), self.wfs.caps.clone(), self.wfs.limit, self.wfs.result.take());
                self.wfs.preview.clear();
                self.wfs.place = Some(Job::spawn(&ctx, "Importeren…", move |p| {
                    let res = match cached {
                        Some(r) => r,
                        None => import::wfs_fetch(&s, caps.as_ref().ok_or("geen capabilities")?, &ft, limit, p)?,
                    };
                    import::wfs_place(&s, &ft, &res, &style)
                }));
            }
            if ui.add_enabled(can, egui::Button::new("Test — ophalen en tonen")).clicked() {
                let (s, c, t, n) = (site.clone(), self.wfs.caps.clone().unwrap(), self.wfs.ft.clone().unwrap(), self.wfs.limit);
                self.wfs.fetch = Some(Job::spawn(&ctx, "Objecten ophalen…", move |p| {
                    let res = import::wfs_fetch(&s, &c, &t, n, p)?;
                    let lines = vector::preview_lines(&s, &res.data_crs, &res.features)?;
                    Ok((res, lines))
                }));
            }
            job_status(ui, &self.wfs.place);
            job_status(ui, &self.wfs.fetch);
        });
    }

    fn osm_actions(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let Some(site) = bridge::site() else { return };
        let ids: Vec<&'static str> = THEMES.iter().zip(&self.osm.on).filter(|(_, on)| **on).map(|(t, _)| t.id).collect();
        let busy = self.osm.fetch.is_some() || self.osm.run.is_some();
        button_bar(ui, |ui| {
            if ui.add_enabled(!ids.is_empty() && !busy, primary_button("OK en importeren")).clicked() {
                let cached = self.osm.fetched.take().filter(|f| f.0 == ids).map(|f| f.1);
                let (s, ids2, labels, h, clip) = (site.clone(), ids.clone(), self.osm.labels, self.osm.text_m, self.osm.clip);
                self.osm.run = Some(Job::spawn(&ctx, "OpenStreetMap…", move |p| {
                    let per_theme = match cached {
                        Some(c) => c,
                        None => import::osm_fetch(&s, &ids2, p)?,
                    };
                    import::osm_place(&s, &ids2, &per_theme, labels, h, clip)
                }));
            }
            if ui.add_enabled(!ids.is_empty() && !busy, egui::Button::new("Test — ophalen en tonen")).clicked() {
                let (s, ids2) = (site.clone(), ids.clone());
                self.osm.fetch = Some(Job::spawn(&ctx, "OpenStreetMap ophalen…", move |p| {
                    let per_theme = import::osm_fetch(&s, &ids2, p)?;
                    let mut lines = Vec::new();
                    for (id, feats) in ids2.iter().zip(&per_theme) {
                        let color = THEMES.iter().find(|t| t.id == *id).map(|t| aci_color(t.aci)).unwrap_or(Color32::GRAY);
                        lines.extend(vector::preview_lines(&s, "EPSG:4326", feats)?.into_iter().map(|l| (l, color)));
                    }
                    Ok((ids2, per_theme, lines))
                }));
            }
            job_status(ui, &self.osm.run);
            job_status(ui, &self.osm.fetch);
        });
    }

    fn pdok_actions(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let Some(site) = bridge::site() else { return };
        button_bar(ui, |ui| {
            let chosen: Vec<usize> = (0..PDOK.len()).filter(|i| self.pdok.on[*i]).collect();
            if ui.add_enabled(!chosen.is_empty() && self.pdok.run.is_none(), primary_button("OK en importeren")).clicked() {
                let s = site.clone();
                self.pdok.run = Some(Job::spawn(&ctx, "PDOK…", move |p| {
                    let mut done = Vec::new();
                    for (k, i) in chosen.iter().enumerate() {
                        let d = &PDOK[*i];
                        p(format!("{}/{}: {}…", k + 1, chosen.len(), d.label));
                        match import::run_dataset(&s, &d.data, &|m: String| p(format!("{}/{}: {} — {m}", k + 1, chosen.len(), d.label))) {
                            Ok(_) => done.push(d.label.to_string()),
                            Err(e) => done.push(format!("{} (mislukt: {e})", d.label)),
                        }
                    }
                    Ok(format!("PDOK geïmporteerd: {}.", done.join("; ")))
                }));
            }
            if let Some(j) = &self.pdok.run {
                ui.spinner();
                ui.label(j.status());
            }
        });
    }

    // ── BGT · Kadaster · KLIC (NLCS) ────────────────────────────────────────
    fn load_klic(&mut self, ctx: &egui::Context) {
        let path = std::path::PathBuf::from(self.nlcs.klic_path.trim().trim_matches('"'));
        let Some(site) = bridge::site().or_else(|| Some(self.draft.clone())) else { return };
        self.nlcs.klic = None;
        self.nlcs.klic_preview.clear();
        self.nlcs.klic_job = Some(Job::spawn(ctx, "KLIC-levering lezen…", move |_| {
            let lev = klic::read(&path)?;
            let preview = klic_preview(&site, &lev)?;
            Ok((lev, preview))
        }));
    }

    fn nlcs_page(&mut self, ui: &mut egui::Ui) {
        if let Some(r) = poll(&mut self.nlcs.klic_job) {
            match r {
                Ok((lev, preview)) => {
                    self.message = Some((true, format!("KLIC {}: {} objecten van {} netbeheerders.", lev.meldnummer, lev.features.len(), lev.operators.len())));
                    self.nlcs.klic_on = lev.themes.iter().map(|(t, n)| (t.clone(), *n, true)).collect();
                    self.nlcs.klic = Some(Arc::new(lev));
                    self.nlcs.klic_preview = preview;
                }
                Err(e) => self.err(format!("KLIC: {e}")),
            }
        }
        if let Some(r) = poll(&mut self.nlcs.fetch) {
            match r {
                Ok(f) => {
                    let n: usize = f.2.iter().map(|(_, v)| v.len()).sum();
                    self.message = Some((true, format!("{n} BGT-objecten opgehaald — zie de kaart.")));
                    self.nlcs.fetched = Some(f);
                }
                Err(e) => self.err(e),
            }
        }
        if let Some(r) = poll(&mut self.nlcs.run) {
            match r {
                Ok(m) => self.ok(m),
                Err(e) => self.err(e),
            }
        }
        // A delivery dropped on the window.
        let dropped: Vec<std::path::PathBuf> = ui.ctx().input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if let Some(p) = dropped.into_iter().next() {
            self.nlcs.klic_path = p.to_string_lossy().into_owned();
            let ctx = ui.ctx().clone();
            self.load_klic(&ctx);
        }

        page_header(ui, "nlcs", "BGT, kadaster en KLIC conform NLCS", "Basisdata voor een infra-ontwerp: elk object op zijn NLCS-laag, met kleur, lijndikte, lijntype en objectgegevens.");
        let Some(site) = self.saved_site(ui) else { return };
        let mut lines: Preview2 = self.nlcs.fetched.as_ref().map(|f| f.4.clone()).unwrap_or_default();
        lines.extend(self.nlcs.klic_preview.iter().cloned());
        self.map_section(ui, Some(&site), None, None, &lines);
        if !(site.lon > 3.2 && site.lon < 7.3 && site.lat > 50.7 && site.lat < 53.7) {
            ui.colored_label(WARN, "Let op: BGT, BRK en KLIC bestaan alleen voor Nederland.");
        }

        group(ui, "Tekenstandaard NLCS 5.02", |ui| {
            egui::Grid::new("nlcs_std").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label("Tekenstatus");
                egui::ComboBox::from_id_salt("nlcs_status").selected_text(self.nlcs.status.label()).show_ui(ui, |ui| {
                    for st in Status::ALL {
                        ui.selectable_value(&mut self.nlcs.status, st, st.label());
                    }
                });
                ui.label("Schaal");
                egui::ComboBox::from_id_salt("nlcs_scale").selected_text(format!("1:{}", self.nlcs.scale)).show_ui(ui, |ui| {
                    for sc in [100.0, 200.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0] {
                        ui.selectable_value(&mut self.nlcs.scale, sc, format!("1:{sc}"));
                    }
                });
                ui.end_row();
                let disciplines = nlcs::disciplines();
                let combo = |ui: &mut egui::Ui, id: &str, value: &mut String| {
                    let current = disciplines.iter().find(|d| d.0 == value.as_str()).map(|d| format!("{} — {}", d.0, d.1)).unwrap_or(value.clone());
                    egui::ComboBox::from_id_salt(id).width(260.0).selected_text(current).show_ui(ui, |ui| {
                        for (c, d) in &disciplines {
                            ui.selectable_value(value, c.to_string(), format!("{c} — {d}"));
                        }
                    });
                };
                ui.label("Discipline BGT/BRK");
                combo(ui, "nlcs_disc", &mut self.nlcs.discipline);
                ui.label("Discipline KLIC");
                combo(ui, "nlcs_disc_klic", &mut self.nlcs.klic_discipline);
                ui.end_row();
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.nlcs.object_data, "Objectgegevens (XDATA)");
                ui.checkbox(&mut self.nlcs.labels, "Teksten: straatnamen, huisnummers, perceelnummers, leidinglabels");
                ui.checkbox(&mut self.nlcs.clip, "Bijsnijden op het importgebied");
            });
            ui.label(RichText::new("Laagnaam: STATUS-DISCIPLINE-HOOFDGROEP-OBJECT-ELEMENT, bijv. B-XX-VH-GESLOTENVERHARDING_ASFALT-G. Lijntypen in papier-mm met de lijntypeschaal van de tekenschaal; teksten krijgen de stijl van hun objectlaag, de teksten van het tekenblad volgen de NLCS-tekstklassen.").small().weak());
        });

        ui.columns(2, |cols| {
            group(&mut cols[0], "BGT (PDOK, actueel)", |ui| {
                for (i, g) in bgt::GROUPS.iter().enumerate() {
                    ui.checkbox(&mut self.nlcs.bgt_on[i], g.label);
                }
            });
            group(&mut cols[1], "Kadastrale kaart (BRK)", |ui| {
                ui.checkbox(&mut self.nlcs.brk, "Perceelgrenzen en -nummers (KG-GRENS_PERCEEL)");
            });
            group(&mut cols[1], "Tekenblad", |ui| {
                ui.checkbox(&mut self.nlcs.sheet, "Kader, titelblok, legenda, noordpijl, schaalbalk en RD-kruisen");
                let (paper, _, _) = tekenblad::paper_for(&site, self.nlcs.scale);
                ui.label(RichText::new(format!("Formaat bij 1:{}: {paper} liggend", self.nlcs.scale)).small().weak());
                egui::Grid::new("sheet").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                    ui.label("Project");
                    ui.add(egui::TextEdit::singleline(&mut self.nlcs.info.project).hint_text(site.label.as_str()).desired_width(240.0));
                    ui.end_row();
                    ui.label("Onderwerp");
                    ui.add(egui::TextEdit::singleline(&mut self.nlcs.info.subject).desired_width(240.0));
                    ui.end_row();
                    ui.label("Tekeningnummer");
                    ui.add(egui::TextEdit::singleline(&mut self.nlcs.info.number).desired_width(240.0));
                    ui.end_row();
                    ui.label("Getekend");
                    ui.add(egui::TextEdit::singleline(&mut self.nlcs.info.drawn_by).desired_width(240.0));
                    ui.end_row();
                });
            });
        });

        group(ui, "KLIC-levering (WIBON, IMKL 2.0)", |ui| {
            ui.horizontal(|ui| {
                ui.label("Bestand");
                ui.add(egui::TextEdit::singleline(&mut self.nlcs.klic_path).hint_text("Levering_….zip of GI_gebiedsinformatielevering_….xml — of sleep het bestand op dit venster").desired_width(ui.available_width() - 90.0));
                if ui.add_enabled(!self.nlcs.klic_path.trim().is_empty() && self.nlcs.klic_job.is_none(), egui::Button::new("Lezen")).clicked() {
                    let ctx = ui.ctx().clone();
                    self.load_klic(&ctx);
                }
            });
            job_status(ui, &self.nlcs.klic_job);
            if let Some(lev) = self.nlcs.klic.clone() {
                ui.label(RichText::new(format!("Melding {} · {} objecten · {}", lev.meldnummer, lev.features.len(), lev.operators.join(", "))).small());
                egui::Grid::new("klic_themes").num_columns(3).spacing([14.0, 4.0]).show(ui, |ui| {
                    for (i, (t, n, on)) in self.nlcs.klic_on.iter_mut().enumerate() {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, preview_color(nlcs::imkl_rgb(t)));
                        ui.checkbox(on, format!("{} ({n})", klic::theme_label(t)));
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.nlcs.imkl_colors, "IMKL-themakleuren in plaats van NLCS-kleuren");
                    if ui.button("Importgebied op deze levering zetten").clicked() {
                        if let Some(b) = klic_extent(&lev) {
                            let mut s = site.clone();
                            let c = [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0];
                            let c = if s.crs == "EPSG:28992" { Ok(c) } else { crs::transform("EPSG:28992", &s.crs, c) };
                            if let Ok(c) = c {
                                s.set_origin((c[0] * 10.0).round() / 10.0, (c[1] * 10.0).round() / 10.0);
                                s.width = ((b[2] - b[0]) + 40.0).ceil();
                                s.height = ((b[3] - b[1]) + 40.0).ceil();
                                s.label = format!("KLIC {}", lev.meldnummer);
                                match bridge::save_site(&s) {
                                    Ok(()) => {
                                        self.draft = s;
                                        self.fit_pending = true;
                                        self.nlcs.fetched = None;
                                        self.ok("Importgebied aangepast aan de KLIC-levering.");
                                    }
                                    Err(e) => self.err(e),
                                }
                            }
                        }
                    }
                });
            }
        });
    }

    fn nlcs_actions(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let Some(site) = bridge::site() else { return };
        let collections = self.nlcs.collections();
        let brk = self.nlcs.brk;
        let klic_ready = self.nlcs.klic.is_some() && self.nlcs.klic_on.iter().any(|t| t.2);
        let busy = self.nlcs.fetch.is_some() || self.nlcs.run.is_some();
        let anything = !collections.is_empty() || brk || klic_ready;
        button_bar(ui, |ui| {
            if ui.add_enabled(anything && !busy, primary_button("OK en importeren")).clicked() {
                let cached = self.nlcs.fetched.take().filter(|f| f.0 == collections && f.1 == brk);
                let (opt, kopt, lev, sheet, info) = (self.nlcs.options(), self.nlcs.klic_options(), self.nlcs.klic.clone().filter(|_| klic_ready), self.nlcs.sheet, self.nlcs.info.clone());
                let (s, cols) = (site.clone(), collections.clone());
                self.nlcs.run = Some(Job::spawn(&ctx, "Basisdata…", move |p| {
                    let mut done = Vec::new();
                    let (bgt_data, cad) = match cached {
                        Some(c) => (c.2, c.3),
                        None => {
                            let b = if cols.is_empty() { Arc::new(Vec::new()) } else { Arc::new(bgt::fetch(&s, &cols, p)?) };
                            let c = if brk { Some(Arc::new(basisdata::cadastre_fetch(&s, opt.labels, p)?)) } else { None };
                            (b, c)
                        }
                    };
                    if !bgt_data.is_empty() {
                        p("BGT tekenen…".into());
                        done.push(basisdata::bgt_place(&s, &bgt_data, &opt)?);
                    }
                    if let Some(c) = cad {
                        p("Kadaster tekenen…".into());
                        done.push(basisdata::cadastre_place(&s, &c, &opt)?);
                    }
                    if let Some(lev) = lev {
                        p("KLIC tekenen…".into());
                        done.push(basisdata::klic_place(&s, &lev, &kopt)?);
                    }
                    if sheet {
                        p("Tekenblad…".into());
                        done.push(tekenblad::draw(&s, &opt, &info)?);
                    }
                    bridge::zoom_extents();
                    Ok(done.join(" "))
                }));
            }
            if ui.add_enabled((!collections.is_empty() || brk) && !busy, egui::Button::new("Test — ophalen en tonen")).clicked() {
                let (s, cols, labels) = (site.clone(), collections.clone(), self.nlcs.labels);
                self.nlcs.fetch = Some(Job::spawn(&ctx, "BGT ophalen…", move |p| {
                    let data = if cols.is_empty() { Vec::new() } else { bgt::fetch(&s, &cols, p)? };
                    let cad = if brk { Some(basisdata::cadastre_fetch(&s, labels, p)?) } else { None };
                    let preview = bgt_preview(&s, &data, cad.as_ref())?;
                    Ok((cols, brk, Arc::new(data), cad.map(Arc::new), preview))
                }));
            }
            job_status(ui, &self.nlcs.run);
            job_status(ui, &self.nlcs.fetch);
        });
    }

    // ── Servers ─────────────────────────────────────────────────────────────
    fn servers_page(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if let Some(r) = poll(&mut self.srv.test) {
            match r {
                Ok(title) => {
                    let kind = self.srv.kind.unwrap_or(ServerKind::Wms);
                    let mut user = servers::load_user();
                    let name = if self.srv.name.trim().is_empty() { title } else { self.srv.name.trim().to_string() };
                    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                    user.push(MapServer {
                        id: format!("user-{stamp}"),
                        kind,
                        name: name.clone(),
                        region: if self.srv.region.trim().is_empty() { "Eigen servers".into() } else { self.srv.region.trim().into() },
                        url: if kind == ServerKind::Xyz { self.srv.url.trim().to_string() } else { crate::gis::http::clean_service_url(&self.srv.url) },
                        attribution: String::new(),
                        default_layer: String::new(),
                        max_zoom: (kind == ServerKind::Xyz).then_some(19),
                        builtin: false,
                    });
                    match servers::save_user(&user) {
                        Ok(()) => {
                            self.servers = servers::all();
                            self.srv.url.clear();
                            self.srv.name.clear();
                            self.ok(format!("Server \"{name}\" toegevoegd."));
                        }
                        Err(e) => self.err(e),
                    }
                }
                Err(e) => self.err(format!("Server niet toegevoegd: {e}")),
            }
        }
        page_header(ui, "server", "Serverbeheer", "Voorgedefinieerde servers (Nederland, Vlaanderen, wereld) en uw eigen WMS-, WFS- en tegelservers.");
        group(ui, "Server toevoegen", |ui| {
            ui.horizontal(|ui| {
                let kind = self.srv.kind.get_or_insert(ServerKind::Wms);
                for k in [ServerKind::Wms, ServerKind::Wfs, ServerKind::Xyz] {
                    ui.radio_value(kind, k, k.label());
                }
            });
            let xyz = self.srv.kind == Some(ServerKind::Xyz);
            ui.add(egui::TextEdit::singleline(&mut self.srv.url).hint_text(if xyz { "https://…/{z}/{x}/{y}.png" } else { "https://…/wms of …/wfs (GetCapabilities wordt getest)" }).desired_width(f32::INFINITY));
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.srv.name).hint_text("naam (leeg = titel van de server)"));
                ui.add(egui::TextEdit::singleline(&mut self.srv.region).hint_text("groep"));
                if ui.add_enabled(self.srv.test.is_none() && !self.srv.url.trim().is_empty(), primary_button("Testen en toevoegen")).clicked() {
                    let (kind, url) = (self.srv.kind.unwrap_or(ServerKind::Wms), self.srv.url.trim().to_string());
                    self.srv.test = Some(Job::spawn(&ctx, "Server testen…", move |_| match kind {
                        ServerKind::Wms => caps::fetch_wms(&crate::gis::http::clean_service_url(&url)).map(|c| c.title),
                        ServerKind::Wfs => caps::fetch_wfs(&crate::gis::http::clean_service_url(&url)).map(|c| c.title),
                        ServerKind::Xyz => {
                            if !(url.contains("{z}") && url.contains("{x}") && url.contains("{y}")) {
                                return Err("tegel-URL moet {z}, {x} en {y} bevatten".into());
                            }
                            crate::gis::http::get_image(&url.replace("{z}", "0").replace("{x}", "0").replace("{y}", "0")).map(|_| "Tegelserver".to_string())
                        }
                    }));
                }
                if let Some(j) = &self.srv.test {
                    ui.spinner();
                    ui.label(j.status());
                }
            });
        });
        let mut remove = None;
        group(ui, "Servers", |ui| {
            let mut regions: Vec<String> = Vec::new();
            for s in &self.servers {
                if !regions.contains(&s.region) {
                    regions.push(s.region.clone());
                }
            }
            egui::Grid::new("srv_list").num_columns(4).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                for r in regions {
                    ui.label(RichText::new(r.to_uppercase()).small().strong().color(ACCENT));
                    ui.end_row();
                    for s in self.servers.iter().filter(|s| s.region == r) {
                        ui.label(RichText::new(s.kind.label()).monospace().small());
                        ui.label(&s.name);
                        ui.label(RichText::new(&s.url).small().weak());
                        if s.builtin {
                            ui.label(RichText::new("standaard").small().weak());
                        } else if ui.small_button("Verwijderen").clicked() {
                            remove = Some(s.id.clone());
                        }
                        ui.end_row();
                    }
                }
            });
        });
        if let Some(id) = remove {
            let user: Vec<MapServer> = servers::load_user().into_iter().filter(|s| s.id != id).collect();
            match servers::save_user(&user) {
                Ok(()) => self.servers = servers::all(),
                Err(e) => self.err(e),
            }
        }
    }
}

// ── Layout helpers ──────────────────────────────────────────────────────────

fn page_header(ui: &mut egui::Ui, icon_name: &str, title: &str, subtitle: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add(icon(icon_name).fit_to_exact_size(Vec2::splat(26.0)));
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(18.0).strong());
            ui.label(RichText::new(subtitle).weak());
        });
    });
    ui.add_space(6.0);
}

/// A titled group box, like the QGroupBox sections of the FreeCAD dialogs.
fn group(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(10)).corner_radius(4.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title.to_uppercase()).small().strong().color(ACCENT));
        ui.add_space(2.0);
        add(ui);
    });
    ui.add_space(6.0);
}

fn list_frame(ui: &mut egui::Ui, height: f32, id: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(ui.visuals().extreme_bg_color).inner_margin(egui::Margin::same(4)).corner_radius(3.0).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt(id).max_height(height).auto_shrink([false, true]).show(ui, add);
    });
}

fn button_bar(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(add);
}

fn job_status<T: Send + 'static>(ui: &mut egui::Ui, job: &Option<Job<T>>) {
    if let Some(j) = job {
        ui.spinner();
        ui.label(j.status());
    }
}

fn primary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).strong().color(Color32::WHITE)).fill(ACCENT).min_size(Vec2::new(130.0, 28.0))
}

fn server_combo(ui: &mut egui::Ui, id: &str, all: &[MapServer], kinds: &[ServerKind], current: Option<&MapServer>) -> Option<MapServer> {
    let mut picked = None;
    egui::ComboBox::from_id_salt(id)
        .width(ui.available_width().min(520.0))
        .height(480.0)
        .selected_text(current.map(|s| s.name.clone()).unwrap_or_else(|| "— kies een server —".into()))
        .show_ui(ui, |ui| {
            let mut region = "";
            for s in all.iter().filter(|s| kinds.contains(&s.kind)) {
                if s.region != region {
                    region = &s.region;
                    ui.label(RichText::new(region.to_uppercase()).small().color(ACCENT));
                }
                let label = if s.kind == ServerKind::Xyz { format!("{}   [tegels]", s.name) } else { s.name.clone() };
                if ui.selectable_label(current.is_some_and(|c| c.id == s.id), label).clicked() {
                    picked = Some(s.clone());
                }
            }
        });
    picked
}

/// Returns true when the colour changed.
fn aci_combo(ui: &mut egui::Ui, id: &str, aci: &mut i16) -> bool {
    let before = *aci;
    let name = ACI_CHOICES.iter().find(|(a, _)| a == aci).map(|(_, n)| *n).unwrap_or("?");
    egui::ComboBox::from_id_salt(id).selected_text(RichText::new(name).color(aci_color(*aci))).show_ui(ui, |ui| {
        for (a, n) in ACI_CHOICES {
            ui.selectable_value(aci, *a, RichText::new(*n).color(aci_color(*a)));
        }
    });
    before != *aci
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_messages(ctx);
        // Closing only hides: the event loop cannot be created twice.
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("nav").exact_size(196.0).resizable(false).show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(icon("location").fit_to_exact_size(Vec2::splat(22.0)));
                ui.label(RichText::new("Open Infra · GIS").size(15.0).strong());
            });
            ui.add_space(10.0);
            let mut nav = |ui: &mut egui::Ui, tab: Tab, ic: &str, label: &str| {
                let sel = self.tab == tab;
                let b = egui::Button::image_and_text(icon(ic), RichText::new(label).size(13.5)).selected(sel).frame(sel).min_size(Vec2::new(ui.available_width(), 30.0));
                if ui.add(b).clicked() {
                    self.tab = tab;
                }
            };
            ui.label(RichText::new("WEBSERVICES").small().weak());
            nav(ui, Tab::Location, "location", "Geografische locatie");
            nav(ui, Tab::Raster, "map", "WMS / TMS (raster)");
            nav(ui, Tab::Wfs, "vector", "WFS (vector)");
            nav(ui, Tab::Osm, "globe", "OpenStreetMap");
            nav(ui, Tab::Pdok, "pdok", "PDOK (NL)");
            ui.add_space(8.0);
            ui.label(RichText::new("BASISDATA NL").small().weak());
            nav(ui, Tab::Nlcs, "nlcs", "BGT · Kadaster · KLIC");
            ui.add_space(8.0);
            ui.label(RichText::new("HULPMIDDELEN").small().weak());
            nav(ui, Tab::Coords, "point", "Coördinaten");
            nav(ui, Tab::Servers, "server", "Servers");
            ui.add_space(14.0);

            egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("PROJECTLOCATIE").small().strong().color(ACCENT));
                if let Some(s) = bridge::site() {
                    ui.label(RichText::new(&s.label).small());
                    ui.label(RichText::new(format!("{}\nX {:.1}\nY {:.1}\n{} × {} m · {}", s.crs, s.origin_e, s.origin_n, s.width, s.height, if s.absolute { "absoluut" } else { "relatief" })).small().monospace().weak());
                } else {
                    ui.label(RichText::new("Nog niet ingesteld").small().color(WARN));
                }
                if let Some(t) = bridge::target() {
                    let doc = t.doc_path.and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_else(|| "(niet opgeslagen tekening)".into());
                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("Tekening: {doc}")).small().weak());
                }
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                match &self.message {
                    Some((true, m)) => ui.label(RichText::new(m).color(OK_GREEN)),
                    Some((false, m)) => ui.label(RichText::new(m).color(ERR_RED)),
                    None => ui.label(RichText::new("Klaar.").weak()),
                };
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            // OK / Test / Annuleren stay visible at the bottom, as in the FreeCAD dialogs.
            let has_actions = matches!(self.tab, Tab::Location | Tab::Raster | Tab::Wfs | Tab::Osm | Tab::Pdok | Tab::Nlcs);
            if has_actions {
                egui::Panel::bottom("actions").frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(4, 8))).show(ui, |ui| match self.tab {
                    Tab::Location => self.location_actions(ui),
                    Tab::Raster => self.raster_actions(ui),
                    Tab::Wfs => self.wfs_actions(ui),
                    Tab::Osm => self.osm_actions(ui),
                    Tab::Pdok => self.pdok_actions(ui),
                    Tab::Nlcs => self.nlcs_actions(ui),
                    _ => {}
                });
            }
            egui::ScrollArea::vertical().scroll_source(egui::scroll_area::ScrollSource::MOUSE_WHEEL).auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(ui.available_width() - 6.0);
                match self.tab {
                    Tab::Location => self.location(ui),
                    Tab::Raster => self.raster(ui),
                    Tab::Wfs => self.wfs(ui),
                    Tab::Osm => self.osm(ui),
                    Tab::Pdok => self.pdok(ui),
                    Tab::Nlcs => self.nlcs_page(ui),
                    Tab::Servers => self.servers_page(ui),
                    Tab::Coords => self.coords(ui),
                }
            });
        });
    }
}
