//! Workbench window, laid out like the FreeCAD GIS(2BIM) workbench: set the
//! site first, then pull map layers, vector data and OpenStreetMap into the
//! drawing.

use std::sync::mpsc::Receiver;

use eframe::egui::{self, Color32, RichText};

use super::jobs::{poll, Job};
use super::{Tab, UiMsg};
use crate::bridge;
use crate::gis::caps::{self, WfsCaps, WfsType, WmsCaps, WmsLayer};
use crate::gis::crs::{self, CrsInfo};
use crate::gis::geocode::{self, Hit};
use crate::gis::osm::THEMES;
use crate::gis::raster::{self, MAX_TOTAL_PX};
use crate::gis::servers::{self, MapServer, ServerKind};
use crate::gis::site::Site;
use crate::gis::wfs::WfsResult;
use crate::import::{self, VectorStyle};

const ACI_CHOICES: &[(i16, &str)] = &[(1, "rood"), (2, "geel"), (3, "groen"), (4, "cyaan"), (5, "blauw"), (6, "magenta"), (7, "wit/zwart"), (8, "grijs"), (30, "oranje")];

fn aci_color(aci: i16) -> Color32 {
    match aci {
        1 => Color32::from_rgb(230, 60, 60),
        2 => Color32::from_rgb(230, 210, 60),
        3 => Color32::from_rgb(60, 190, 80),
        4 => Color32::from_rgb(60, 200, 210),
        5 => Color32::from_rgb(70, 110, 230),
        6 => Color32::from_rgb(210, 70, 200),
        8 => Color32::GRAY,
        30 => Color32::from_rgb(240, 140, 40),
        _ => Color32::LIGHT_GRAY,
    }
}

#[derive(Default)]
struct LocationTab {
    query: String,
    hits: Vec<Hit>,
    search: Option<Job<Vec<Hit>>>,
    custom_epsg: String,
    crs_job: Option<Job<CrsInfo>>,
}

#[derive(Default)]
struct RasterTab {
    server: Option<MapServer>,
    caps: Option<WmsCaps>,
    caps_job: Option<Job<WmsCaps>>,
    layer: Option<WmsLayer>,
    filter: String,
    format: String,
    resolution: f64,
    fade: u8,
    run: Option<Job<String>>,
}

#[derive(Default)]
struct WfsTab {
    server: Option<MapServer>,
    caps: Option<WfsCaps>,
    caps_job: Option<Job<WfsCaps>>,
    ft: Option<WfsType>,
    filter: String,
    limit: usize,
    fetch: Option<Job<WfsResult>>,
    result: Option<WfsResult>,
    label: String,
    text_m: f64,
    aci: i16,
    clip: bool,
    place: Option<Job<String>>,
}

struct OsmTab {
    on: Vec<bool>,
    labels: bool,
    text_m: f64,
    clip: bool,
    run: Option<Job<String>>,
}

#[derive(Default)]
struct ServersTab {
    kind: Option<ServerKind>,
    url: String,
    name: String,
    region: String,
    test: Option<Job<String>>,
}

struct CoordsTab {
    crs: String,
    xy: [f64; 2],
    job: Option<Job<CrsInfo>>,
}

pub struct App {
    rx: Receiver<UiMsg>,
    tab: Tab,
    draft: Site,
    has_site: bool,
    message: Option<(bool, String)>,
    loc: LocationTab,
    raster: RasterTab,
    wfs: WfsTab,
    osm: OsmTab,
    srv: ServersTab,
    coords: CoordsTab,
    servers: Vec<MapServer>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, rx: Receiver<UiMsg>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let site = bridge::site();
        Self {
            rx,
            tab: Tab::Location,
            has_site: site.is_some(),
            draft: site.unwrap_or_default(),
            message: None,
            loc: LocationTab::default(),
            raster: RasterTab { resolution: 0.25, format: "image/png".into(), ..Default::default() },
            wfs: WfsTab { limit: 5000, text_m: 1.0, aci: 2, clip: true, ..Default::default() },
            osm: OsmTab { on: THEMES.iter().map(|t| t.default_on).collect(), labels: true, text_m: 2.0, clip: true, run: None },
            srv: ServersTab { region: "Eigen servers".into(), ..Default::default() },
            coords: CoordsTab { crs: "EPSG:4326".into(), xy: [5.1214, 52.0907], job: None },
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
        if let Some(s) = bridge::site() {
            self.draft = s;
            self.has_site = true;
        } else {
            self.has_site = false;
        }
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
                    if let Ok(q) = crs::transform(&self.draft.crs, &self.coords.crs, p) {
                        self.coords.xy = q;
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                UiMsg::SiteChanged => self.reload_site(),
            }
        }
    }

    /// The saved site, or a hint to set one first.
    fn saved_site(&self, ui: &mut egui::Ui) -> Option<Site> {
        let s = bridge::site();
        if s.is_none() {
            ui.colored_label(Color32::from_rgb(229, 192, 123), "Let op: stel eerst de projectlocatie in en sla die op (tabblad Locatie).");
        }
        s
    }

    // ── Locatie ──────────────────────────────────────────────────────────
    fn location(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if let Some(r) = poll(&mut self.loc.search) {
            match r {
                Ok(h) if h.is_empty() => self.err("Geen resultaten."),
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

        ui.heading("Projectlocatie");
        ui.label(RichText::new("Adres, plaats, perceel, weg of \"lat, lon\". Nederland via PDOK, wereldwijd via OpenStreetMap.").weak());
        ui.horizontal(|ui| {
            let edit = ui.add(egui::TextEdit::singleline(&mut self.loc.query).hint_text("bijv. Domplein 21 Utrecht, Maasvlakte, Nairobi, 52.09, 5.11").desired_width(ui.available_width() - 90.0));
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.add_enabled(self.loc.search.is_none(), egui::Button::new("Zoeken")).clicked() || enter) && !self.loc.query.trim().is_empty() {
                let q = self.loc.query.clone();
                self.loc.search = Some(Job::spawn(&ctx, "Zoeken…", move |_| geocode::search(&q)));
            }
        });
        if let Some(j) = &self.loc.search {
            ui.horizontal(|ui| { ui.spinner(); ui.label(j.status()); });
        }
        if !self.loc.hits.is_empty() {
            let mut picked = None;
            egui::ScrollArea::vertical().id_salt("hits").max_height(130.0).show(ui, |ui| {
                for (i, h) in self.loc.hits.iter().enumerate() {
                    if ui.selectable_label(false, format!("[{}] {}", h.source, h.label)).clicked() {
                        picked = Some(i);
                    }
                }
            });
            if let Some(i) = picked {
                let h = self.loc.hits[i].clone();
                self.draft.label = h.label;
                self.draft.lon = h.lon;
                self.draft.lat = h.lat;
                let code = crs::suggest_for(&h.country, h.lon, h.lat);
                if crs::known(&code).is_some() {
                    self.draft.crs = code;
                    self.draft.reproject_origin();
                } else {
                    self.loc.crs_job = Some(Job::spawn(&ctx, "Stelsel laden…", move |_| crs::ensure(&code)));
                }
                self.loc.hits.clear();
            }
        }
        ui.separator();

        egui::Grid::new("site").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Omschrijving");
            ui.add(egui::TextEdit::singleline(&mut self.draft.label).desired_width(420.0));
            ui.end_row();

            ui.label("Breedte / lengte (WGS 84)");
            ui.horizontal(|ui| {
                let a = ui.add(egui::DragValue::new(&mut self.draft.lat).speed(0.0001).fixed_decimals(7).prefix("lat "));
                let b = ui.add(egui::DragValue::new(&mut self.draft.lon).speed(0.0001).fixed_decimals(7).prefix("lon "));
                if a.changed() || b.changed() {
                    self.draft.reproject_origin();
                }
            });
            ui.end_row();

            ui.label("Coördinatenstelsel");
            ui.horizontal(|ui| {
                let suggested = crs::suggest(self.draft.lon, self.draft.lat);
                let mut list = crs::builtin_list();
                for c in [&suggested, &self.draft.crs] {
                    if !list.iter().any(|i| &i.code == c) {
                        if let Some(i) = crs::known(c) { list.push(i); }
                    }
                }
                let current = crs::known(&self.draft.crs).map(|i| format!("{} — {}", i.code, i.name)).unwrap_or(self.draft.crs.clone());
                let mut chosen = None;
                egui::ComboBox::from_id_salt("crs").width(330.0).selected_text(current).show_ui(ui, |ui| {
                    for i in list.iter().filter(|i| !i.proj4.contains("+proj=longlat")) {
                        let star = if i.code == suggested { " ★" } else { "" };
                        if ui.selectable_label(i.code == self.draft.crs, format!("{} — {} ({}){star}", i.code, i.name, i.region)).clicked() {
                            chosen = Some(i.code.clone());
                        }
                    }
                });
                if let Some(c) = chosen {
                    self.draft.crs = c;
                    self.draft.reproject_origin();
                }
                ui.add(egui::TextEdit::singleline(&mut self.loc.custom_epsg).hint_text("ander EPSG").desired_width(80.0));
                if ui.add_enabled(self.loc.crs_job.is_none() && !self.loc.custom_epsg.trim().is_empty(), egui::Button::new("Laden")).on_hover_text("Definitie ophalen via epsg.io").clicked() {
                    let code = self.loc.custom_epsg.clone();
                    self.loc.crs_job = Some(Job::spawn(&ctx, "Stelsel laden…", move |_| crs::ensure(&code)));
                }
            });
            ui.end_row();

            ui.label("Basispunt (m)");
            ui.horizontal(|ui| {
                let (mut e, mut n) = (self.draft.origin_e, self.draft.origin_n);
                let a = ui.add(egui::DragValue::new(&mut e).speed(1.0).fixed_decimals(3).prefix("X "));
                let b = ui.add(egui::DragValue::new(&mut n).speed(1.0).fixed_decimals(3).prefix("Y "));
                if a.changed() || b.changed() {
                    self.draft.set_origin(e, n);
                }
            });
            ui.end_row();

            ui.label("Importgebied (m)");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.draft.width).speed(10.0).range(10.0..=20_000.0).prefix("breedte "));
                ui.add(egui::DragValue::new(&mut self.draft.height).speed(10.0).range(10.0..=20_000.0).prefix("hoogte "));
            });
            ui.end_row();

            ui.label("Plaatsing");
            ui.vertical(|ui| {
                ui.radio_value(&mut self.draft.absolute, true, "Absolute coördinaten (bijv. RD direct in de tekening) — gebruikelijk in de infra");
                ui.radio_value(&mut self.draft.absolute, false, "Relatief: basispunt = 0,0 in de tekening");
            });
            ui.end_row();
        });
        if let Some(j) = &self.loc.crs_job {
            ui.horizontal(|ui| { ui.spinner(); ui.label(j.status()); });
        }
        let units = bridge::target().map(|t| t.unit_factor).unwrap_or(1.0);
        ui.label(RichText::new(format!(
            "Tekeneenheid: {} (INSUNITS). Gebied ook aan te wijzen in de tekening met de knop \"Gebied\" in het GIS-lint.",
            if (units - 1.0).abs() < 1e-9 { "meter".to_string() } else if (units - 1000.0).abs() < 1e-9 { "millimeter".into() } else { format!("{units} per meter") }
        )).weak());
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button(RichText::new("Opslaan in tekening").strong()).clicked() {
                match bridge::save_site(&self.draft) {
                    Ok(()) => {
                        self.has_site = true;
                        let m = format!("Projectlocatie opgeslagen: {} — {} X {:.2} Y {:.2}, {}×{} m.", self.draft.label, self.draft.crs, self.draft.origin_e, self.draft.origin_n, self.draft.width, self.draft.height);
                        self.ok(m);
                    }
                    Err(e) => self.err(format!("Opslaan mislukt: {e}")),
                }
            }
            if !self.has_site {
                ui.colored_label(Color32::from_rgb(229, 192, 123), "nog niet opgeslagen in deze tekening");
            }
        });
    }

    // ── Kaartlagen (WMS / tegels) ─────────────────────────────────────────
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
        if let Some(r) = poll(&mut self.raster.run) {
            match r { Ok(m) => self.ok(m), Err(e) => self.err(e) }
        }

        ui.heading("Kaartlaag als onderlegger (WMS / tegels)");
        let Some(site) = self.saved_site(ui) else { return };
        let picked = server_combo(ui, "wms_srv", &self.servers, &[ServerKind::Wms, ServerKind::Xyz], self.raster.server.as_ref());
        if let Some(s) = picked {
            self.raster.caps = None;
            self.raster.layer = None;
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
        if let Some(s) = &self.raster.server {
            if !s.attribution.is_empty() {
                ui.label(RichText::new(format!("Bron: {}", s.attribution)).weak());
            }
        }
        if let Some(j) = &self.raster.caps_job {
            ui.horizontal(|ui| { ui.spinner(); ui.label(j.status()); });
        }
        if let (Some(c), Some(ServerKind::Wms)) = (&self.raster.caps, self.raster.server.as_ref().map(|s| s.kind)) {
            ui.add(egui::TextEdit::singleline(&mut self.raster.filter).hint_text("Filter lagen…"));
            let f = self.raster.filter.to_lowercase();
            let mut chosen = None;
            egui::ScrollArea::vertical().id_salt("wms_layers").max_height(200.0).show(ui, |ui| {
                for l in c.layers.iter().filter(|l| f.is_empty() || l.name.to_lowercase().contains(&f) || l.title.to_lowercase().contains(&f)) {
                    let sel = self.raster.layer.as_ref().is_some_and(|x| x.name == l.name);
                    let reproj = !l.crs.is_empty() && !l.crs.contains(&site.crs);
                    let text = format!("{}{}{}", l.title, if l.title != l.name { format!("  ({})", l.name) } else { String::new() }, if reproj { "   (herprojectie)" } else { "" });
                    if ui.selectable_label(sel, text).clicked() {
                        chosen = Some(l.clone());
                    }
                }
            });
            if let Some(l) = chosen {
                self.raster.format = raster::default_format(c, &l.name);
                self.raster.layer = Some(l);
            }
            ui.horizontal(|ui| {
                ui.label("Formaat");
                egui::ComboBox::from_id_salt("fmt").selected_text(self.raster.format.clone()).show_ui(ui, |ui| {
                    for f in c.formats.iter().filter(|f| f.starts_with("image/png") || f.starts_with("image/jpeg")) {
                        ui.selectable_value(&mut self.raster.format, f.clone(), f);
                    }
                });
            });
        }
        if self.raster.server.is_some() {
            ui.horizontal(|ui| {
                ui.label("Resolutie");
                ui.add(egui::DragValue::new(&mut self.raster.resolution).speed(0.01).range(0.02..=100.0).suffix(" m/px"));
                ui.label("Vervagen");
                ui.add(egui::Slider::new(&mut self.raster.fade, 0..=80).suffix(" %"));
            });
            let px = (site.width / self.raster.resolution) * (site.height / self.raster.resolution);
            let too_big = px > MAX_TOTAL_PX;
            ui.label(RichText::new(format!(
                "Gebied {} × {} m rond \"{}\" in {}: {:.0} × {:.0} px{}",
                site.width, site.height, site.label, site.crs, site.width / self.raster.resolution, site.height / self.raster.resolution,
                if too_big { " — te groot: verhoog de resolutie of verklein het gebied" } else { "" }
            )).color(if too_big { Color32::from_rgb(224, 108, 117) } else { Color32::GRAY }));
            let ready = self.raster.run.is_none() && !too_big
                && (self.raster.server.as_ref().is_some_and(|s| s.kind == ServerKind::Xyz) || self.raster.layer.is_some());
            if ui.add_enabled(ready, egui::Button::new(RichText::new("Importeren in tekening").strong())).clicked() {
                let server = self.raster.server.clone().unwrap();
                let opt = raster::Options { resolution: self.raster.resolution, format: self.raster.format.clone() };
                let fade = self.raster.fade;
                let (caps, layer) = (self.raster.caps.clone(), self.raster.layer.clone());
                self.raster.run = Some(Job::spawn(&ctx, "Importeren…", move |p| match server.kind {
                    ServerKind::Xyz => import::xyz(&site, &server, &opt, fade, p),
                    _ => import::wms(&site, &server, caps.as_ref().ok_or("geen capabilities")?, layer.as_ref().ok_or("geen laag")?, &opt, fade, p),
                }));
            }
        }
        if let Some(j) = &self.raster.run {
            ui.horizontal(|ui| { ui.spinner(); ui.label(j.status()); });
        }
    }

    // ── Vectordata (WFS) ─────────────────────────────────────────────────
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
                Ok(res) => {
                    let names = crate::gis::vector::property_names(&res.features);
                    const HINTS: &[&str] = &["perceelnummer", "huisnummer", "wegnummer", "stt_naam", "naam", "name"];
                    self.wfs.label = HINTS.iter().find(|h| names.iter().any(|n| n == *h)).map(|h| h.to_string()).unwrap_or_default();
                    self.message = Some((true, format!("{} objecten opgehaald{}.", res.features.len(), if res.truncated { " (limiet bereikt)" } else { "" })));
                    self.wfs.result = Some(res);
                }
                Err(e) => self.err(e),
            }
        }
        if let Some(r) = poll(&mut self.wfs.place) {
            match r { Ok(m) => self.ok(m), Err(e) => self.err(e) }
        }

        ui.heading("Vectordata (WFS)");
        let Some(site) = self.saved_site(ui) else { return };
        if let Some(s) = server_combo(ui, "wfs_srv", &self.servers, &[ServerKind::Wfs], self.wfs.server.as_ref()) {
            self.wfs.caps = None;
            self.wfs.ft = None;
            self.wfs.result = None;
            let url = s.url.clone();
            self.wfs.caps_job = Some(Job::spawn(&ctx, "Capabilities ophalen…", move |_| caps::fetch_wfs(&url)));
            self.wfs.server = Some(s);
        }
        if let Some(j) = &self.wfs.caps_job {
            ui.horizontal(|ui| { ui.spinner(); ui.label(j.status()); });
        }
        if let Some(c) = &self.wfs.caps {
            ui.add(egui::TextEdit::singleline(&mut self.wfs.filter).hint_text("Filter objecttypen…"));
            let f = self.wfs.filter.to_lowercase();
            let mut chosen = None;
            egui::ScrollArea::vertical().id_salt("wfs_types").max_height(150.0).show(ui, |ui| {
                for t in c.types.iter().filter(|t| f.is_empty() || t.name.to_lowercase().contains(&f) || t.title.to_lowercase().contains(&f)) {
                    let sel = self.wfs.ft.as_ref().is_some_and(|x| x.name == t.name);
                    if ui.selectable_label(sel, format!("{}  ({})", t.title, t.name)).clicked() {
                        chosen = Some(t.clone());
                    }
                }
            });
            if let Some(t) = chosen {
                self.wfs.ft = Some(t);
                self.wfs.result = None;
            }
            ui.horizontal(|ui| {
                ui.label("Max. objecten");
                ui.add(egui::DragValue::new(&mut self.wfs.limit).speed(100).range(1..=200_000));
                if ui.add_enabled(self.wfs.fetch.is_none() && self.wfs.ft.is_some(), egui::Button::new("Ophalen")).clicked() {
                    let (s, c, t, n) = (site.clone(), c.clone(), self.wfs.ft.clone().unwrap(), self.wfs.limit);
                    self.wfs.fetch = Some(Job::spawn(&ctx, "Objecten ophalen…", move |p| import::wfs_fetch(&s, &c, &t, n, p)));
                }
            });
        }
        if let Some(j) = &self.wfs.fetch {
            ui.horizontal(|ui| { ui.spinner(); ui.label(j.status()); });
        }
        if let Some(res) = &self.wfs.result {
            ui.separator();
            ui.label(format!("{} objecten klaar om te plaatsen.", res.features.len()));
            let names = crate::gis::vector::property_names(&res.features);
            ui.horizontal(|ui| {
                ui.label("Label");
                egui::ComboBox::from_id_salt("wfs_label").selected_text(if self.wfs.label.is_empty() { "— geen —".to_string() } else { self.wfs.label.clone() }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.wfs.label, String::new(), "— geen —");
                    for n in &names {
                        ui.selectable_value(&mut self.wfs.label, n.clone(), n);
                    }
                });
                ui.label("Teksthoogte");
                ui.add(egui::DragValue::new(&mut self.wfs.text_m).speed(0.1).range(0.05..=100.0).suffix(" m"));
            });
            ui.horizontal(|ui| {
                ui.label("Laagkleur");
                aci_combo(ui, "wfs_aci", &mut self.wfs.aci);
                ui.checkbox(&mut self.wfs.clip, "Bijsnijden op importgebied");
            });
            if ui.add_enabled(self.wfs.place.is_none(), egui::Button::new(RichText::new("Plaatsen in tekening").strong())).clicked() {
                let style = VectorStyle { aci: self.wfs.aci, label_field: (!self.wfs.label.is_empty()).then(|| self.wfs.label.clone()), text_m: self.wfs.text_m, clip: self.wfs.clip };
                let (s, ft, r) = (site.clone(), self.wfs.ft.clone().unwrap(), self.wfs.result.take().unwrap());
                self.wfs.place = Some(Job::spawn(&ctx, "Plaatsen…", move |_| import::wfs_place(&s, &ft, &r, &style)));
            }
        }
    }

    // ── OpenStreetMap ────────────────────────────────────────────────────
    fn osm(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if let Some(r) = poll(&mut self.osm.run) {
            match r { Ok(m) => self.ok(m), Err(e) => self.err(e) }
        }
        ui.heading("OpenStreetMap (wereldwijd)");
        let Some(site) = self.saved_site(ui) else { return };
        ui.label(RichText::new("Elk thema komt op een eigen laag.").weak());
        egui::Grid::new("osm").num_columns(2).show(ui, |ui| {
            for (i, t) in THEMES.iter().enumerate() {
                ui.checkbox(&mut self.osm.on[i], t.label);
                ui.colored_label(aci_color(t.aci), t.layer);
                ui.end_row();
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.osm.labels, "Namen en huisnummers als tekst");
            ui.add(egui::DragValue::new(&mut self.osm.text_m).speed(0.1).range(0.1..=50.0).prefix("hoogte ").suffix(" m"));
            ui.checkbox(&mut self.osm.clip, "Bijsnijden op importgebied");
        });
        ui.label(RichText::new(format!("Gebied {} × {} m rond \"{}\" in {}. Data © OpenStreetMap-bijdragers (ODbL), via de Overpass API.", site.width, site.height, site.label, site.crs)).weak());
        let any = self.osm.on.iter().any(|b| *b);
        if ui.add_enabled(self.osm.run.is_none() && any, egui::Button::new(RichText::new("Importeren in tekening").strong())).clicked() {
            let ids: Vec<&'static str> = THEMES.iter().zip(&self.osm.on).filter(|(_, on)| **on).map(|(t, _)| t.id).collect();
            let (labels, h, clip) = (self.osm.labels, self.osm.text_m, self.osm.clip);
            self.osm.run = Some(Job::spawn(&ctx, "OpenStreetMap ophalen…", move |p| import::osm(&site, &ids, labels, h, clip, p)));
        }
        if let Some(j) = &self.osm.run {
            ui.horizontal(|ui| { ui.spinner(); ui.label(j.status()); });
        }
    }

    // ── Servers ──────────────────────────────────────────────────────────
    fn servers_tab(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if let Some(r) = poll(&mut self.srv.test) {
            match r {
                Ok(title) => {
                    let kind = self.srv.kind.unwrap_or(ServerKind::Wms);
                    let mut user = servers::load_user();
                    let name = if self.srv.name.trim().is_empty() { title } else { self.srv.name.trim().to_string() };
                    user.push(MapServer {
                        id: format!("user-{}", user.len() + 1 + std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as usize).unwrap_or(0)),
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
        ui.heading("Serverbeheer");
        egui::CollapsingHeader::new("Server toevoegen").default_open(true).show(ui, |ui| {
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
                if ui.add_enabled(self.srv.test.is_none() && !self.srv.url.trim().is_empty(), egui::Button::new("Testen en toevoegen")).clicked() {
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
            });
            if let Some(j) = &self.srv.test {
                ui.horizontal(|ui| { ui.spinner(); ui.label(j.status()); });
            }
        });
        ui.separator();
        let mut remove = None;
        egui::ScrollArea::vertical().id_salt("srv_list").show(ui, |ui| {
            let mut regions: Vec<String> = Vec::new();
            for s in &self.servers {
                if !regions.contains(&s.region) { regions.push(s.region.clone()); }
            }
            for r in regions {
                ui.label(RichText::new(r.to_uppercase()).small().strong());
                for s in self.servers.iter().filter(|s| s.region == r) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(s.kind.label()).monospace().weak());
                        ui.label(&s.name).on_hover_text(&s.url);
                        if !s.builtin && ui.small_button("Verwijderen").clicked() {
                            remove = Some(s.id.clone());
                        }
                    });
                }
            }
        });
        if let Some(id) = remove {
            let user: Vec<MapServer> = servers::load_user().into_iter().filter(|s| s.id != id).collect();
            match servers::save_user(&user) {
                Ok(()) => self.servers = servers::all(),
                Err(e) => self.err(e),
            }
        }
    }

    // ── Coördinaten ──────────────────────────────────────────────────────
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
        ui.heading("Coördinaten omrekenen");
        ui.label(RichText::new("Of klik \"Punt info\" in het GIS-lint en wijs een punt aan in de tekening.").weak());
        let mut codes: Vec<String> = crs::builtin_list().into_iter().map(|i| i.code).collect();
        if let Some(s) = bridge::site() {
            if !codes.contains(&s.crs) { codes.push(s.crs.clone()); }
        }
        let geo = crs::is_geographic(&self.coords.crs);
        ui.horizontal(|ui| {
            ui.label("Invoer in");
            let mut chosen = None;
            egui::ComboBox::from_id_salt("coord_crs").width(300.0).selected_text(crs::known(&self.coords.crs).map(|i| format!("{} — {}", i.code, i.name)).unwrap_or(self.coords.crs.clone())).show_ui(ui, |ui| {
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
        });
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.coords.xy[0]).speed(if geo { 0.0001 } else { 1.0 }).fixed_decimals(if geo { 8 } else { 3 }).prefix(if geo { "lon " } else { "X " }));
            ui.add(egui::DragValue::new(&mut self.coords.xy[1]).speed(if geo { 0.0001 } else { 1.0 }).fixed_decimals(if geo { 8 } else { 3 }).prefix(if geo { "lat " } else { "Y " }));
        });
        ui.separator();
        let mut targets: Vec<String> = vec!["EPSG:4326".into(), "EPSG:28992".into(), "EPSG:3857".into()];
        if let Some(s) = bridge::site() { targets.insert(1, s.crs); }
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
    }
}

fn server_combo(ui: &mut egui::Ui, id: &str, all: &[MapServer], kinds: &[ServerKind], current: Option<&MapServer>) -> Option<MapServer> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.label("Server");
        egui::ComboBox::from_id_salt(id)
            .width(ui.available_width() - 10.0)
            .height(480.0)
            .selected_text(current.map(|s| s.name.clone()).unwrap_or_else(|| "— kies een server —".into()))
            .show_ui(ui, |ui| {
                let mut region = "";
                for s in all.iter().filter(|s| kinds.contains(&s.kind)) {
                    if s.region != region {
                        region = &s.region;
                        ui.label(RichText::new(region.to_uppercase()).small().weak());
                    }
                    let label = if s.kind == ServerKind::Xyz { format!("{}  [tegels]", s.name) } else { s.name.clone() };
                    if ui.selectable_label(current.is_some_and(|c| c.id == s.id), label).clicked() {
                        picked = Some(s.clone());
                    }
                }
            });
    });
    picked
}

fn aci_combo(ui: &mut egui::Ui, id: &str, aci: &mut i16) {
    let name = ACI_CHOICES.iter().find(|(a, _)| a == aci).map(|(_, n)| *n).unwrap_or("?");
    egui::ComboBox::from_id_salt(id).selected_text(RichText::new(name).color(aci_color(*aci))).show_ui(ui, |ui| {
        for (a, n) in ACI_CHOICES {
            ui.selectable_value(aci, *a, RichText::new(*n).color(aci_color(*a)));
        }
    });
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
        egui::Panel::left("nav").exact_size(150.0).resizable(false).show(ui, |ui| {
            ui.add_space(6.0);
            ui.label(RichText::new("OPEN INFRA · GIS").strong());
            ui.add_space(6.0);
            for (tab, label) in [
                (Tab::Location, "Locatie"),
                (Tab::Raster, "Kaartlagen"),
                (Tab::Wfs, "Vectordata (WFS)"),
                (Tab::Osm, "OpenStreetMap"),
                (Tab::Coords, "Coördinaten"),
                (Tab::Servers, "Servers"),
            ] {
                if ui.selectable_label(self.tab == tab, label).clicked() {
                    self.tab = tab;
                }
            }
            ui.add_space(12.0);
            ui.separator();
            if let Some(s) = bridge::site() {
                ui.label(RichText::new(&s.label).small());
                ui.label(RichText::new(format!("{}\nX {:.1}\nY {:.1}\n{} × {} m\n{}", s.crs, s.origin_e, s.origin_n, s.width, s.height, if s.absolute { "absoluut" } else { "relatief" })).small().weak());
            } else {
                ui.label(RichText::new("Geen locatie in deze tekening").small().weak());
            }
            if let Some(t) = bridge::target() {
                let doc = t.doc_path.and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_else(|| "(niet opgeslagen tekening)".into());
                ui.add_space(6.0);
                ui.label(RichText::new(format!("Tekening: {doc}")).small().weak());
            }
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            match &self.message {
                Some((true, m)) => ui.label(RichText::new(m).color(Color32::from_rgb(152, 195, 121))),
                Some((false, m)) => ui.label(RichText::new(m).color(Color32::from_rgb(224, 108, 117))),
                None => ui.label(RichText::new("Klaar.").weak()),
            };
        });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.tab {
                Tab::Location => self.location(ui),
                Tab::Raster => self.raster(ui),
                Tab::Wfs => self.wfs(ui),
                Tab::Osm => self.osm(ui),
                Tab::Servers => self.servers_tab(ui),
                Tab::Coords => self.coords(ui),
            });
        });
    }
}
