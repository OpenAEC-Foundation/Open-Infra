//! Talking to Open CAD Studio from worker and window threads.
//!
//! `dispatch` (the host's call into the plugin) hands us a
//! `PluginRequestSender` for the active document tab; the workbench window and
//! import workers use it to add layers and entities while the host stays
//! responsive. Each import is one undo step.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use ocs_plugin_api::host::{
    codec::{Color, EntityType, Handle, LineWeight},
    ExtendedDataRecord, HostApi, LayerConfig, PluginRequestSender, TableOperation, XDataValue, XRecordEntry, XRecordValue,
};
use ocs_plugin_api::ipc::protocol::{PluginRequest, PluginResponse};

use crate::gis::site::{unit_factor_for_insunits, Site};

/// The document the workbench currently works on.
#[derive(Clone)]
pub struct Target {
    pub sender: Arc<dyn PluginRequestSender>,
    pub tab_id: u64,
    pub unit_factor: f64,
    pub doc_path: Option<PathBuf>,
}

#[derive(Default)]
struct Shared {
    target: Option<Target>,
    site: Option<Site>,
    /// NLCS layers and linetypes already set up, per tab.
    made: HashSet<(u64, String)>,
    /// The NLCS layers imports drew on, per tab, for the legend.
    nlcs_layers: Vec<(u64, crate::nlcs::LayerSpec)>,
    /// Data sources of the imports, per tab, for the title block.
    sources: Vec<(u64, String)>,
}

fn shared() -> &'static Mutex<Shared> {
    static S: OnceLock<Mutex<Shared>> = OnceLock::new();
    S.get_or_init(Default::default)
}

const SITE_DICT: &[&str] = &["OPENINFRA"];
const SITE_RECORD: &str = "SITE";

/// Called from `dispatch`: remember the active tab and read its site.
pub fn attach(host: &mut dyn HostApi) {
    let Some(sender) = host.plugin_request_sender() else { return };
    let tab_id = host.tab_id();
    let unit_factor = units(host, tab_id, false);
    let stored = host
        .read_xrecord(SITE_DICT, SITE_RECORD)
        .ok()
        .and_then(|entries| {
            entries.into_iter().find_map(|e| match e.value {
                XRecordValue::String(s) if e.code == 1 => serde_json::from_str::<Site>(&s).ok(),
                _ => None,
            })
        });
    let mut s = shared().lock().unwrap();
    let same_tab = s.target.as_ref().is_some_and(|t| t.tab_id == tab_id);
    s.target = Some(Target { sender: Arc::from(sender), tab_id, unit_factor, doc_path: host.document_path(tab_id) });
    match stored {
        Some(site) => {
            site.register_crs();
            s.site = Some(site);
        }
        // A new drawing without a site keeps nothing from another drawing.
        None if !same_tab => s.site = None,
        None => {}
    }
}

/// Drawing units → metres factor for the tab. Open CAD Studio's plugin API
/// does not hand out INSUNITS as a system variable (v2026.42 knows CLAYER,
/// SNAPANG and CTAB), so it comes from the drawing header: read once per
/// drawing, again when `fresh` (a new project location), because the header
/// means fetching the document.
pub fn units(host: &mut dyn HostApi, tab_id: u64, fresh: bool) -> f64 {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<u64, f64>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if !fresh {
        if let Some(f) = cache.lock().unwrap().get(&tab_id) {
            return *f;
        }
    }
    let f = match host.system_variable("INSUNITS") {
        Some(ocs_plugin_api::host::HostSettingValue::Number(n)) => unit_factor_for_insunits(n as i64),
        Some(ocs_plugin_api::host::HostSettingValue::Text(t)) => unit_factor_for_insunits(t.trim().parse().unwrap_or(0)),
        None => unit_factor_for_insunits(host.document().header.insertion_units as i64),
    };
    cache.lock().unwrap().insert(tab_id, f);
    f
}

pub fn target() -> Option<Target> {
    shared().lock().unwrap().target.clone()
}

pub fn site() -> Option<Site> {
    shared().lock().unwrap().site.clone()
}

/// Store the site in memory and in the drawing (NOD XRecord, round-trips DWG).
pub fn save_site(site: &Site) -> Result<(), String> {
    shared().lock().unwrap().site = Some(site.clone());
    let t = target().ok_or("geen tekening actief")?;
    let json = serde_json::to_string(site).map_err(|e| e.to_string())?;
    let entries = vec![XRecordEntry::string(1, json)];
    let path: Vec<String> = SITE_DICT.iter().map(|s| s.to_string()).collect();
    let update = PluginRequest::UpdateXRecord { dictionary_path: path.clone(), name: SITE_RECORD.into(), entries: entries.clone() };
    if let Ok(PluginResponse::XRecordUpdateResult(Ok(_))) = t.sender.request(update) {
        return Ok(());
    }
    // Not there yet: create it (and the OPENINFRA dictionary).
    match t.sender.request(PluginRequest::CreateXRecord { dictionary_path: path, name: SITE_RECORD.into(), entries }) {
        Ok(PluginResponse::XRecordCreateResult(Ok(_))) => Ok(()),
        Ok(PluginResponse::XRecordCreateResult(Err(e))) | Ok(PluginResponse::Error(e)) => Err(e),
        Ok(other) => Err(format!("onverwacht antwoord: {other:?}")),
        Err(e) => Err(e.to_string()),
    }
}

fn request(req: PluginRequest) -> Result<PluginResponse, String> {
    let t = target().ok_or("geen tekening actief — klik eerst in Open CAD Studio op het GIS-tabblad")?;
    match t.sender.request(req) {
        Ok(PluginResponse::Error(e)) => Err(e),
        Ok(r) => Ok(r),
        Err(e) => Err(e.to_string()),
    }
}

pub fn info(msg: &str) {
    let _ = request(PluginRequest::PushInfo(format!("[Open Infra] {msg}")));
}

pub fn error(msg: &str) {
    let _ = request(PluginRequest::PushError(format!("[Open Infra] {msg}")));
}

pub fn ensure_layer(name: &str, color: Color) {
    let _ = request(PluginRequest::AddLayer(LayerConfig { name: name.into(), color: Some(color), ..Default::default() }));
}

/// Application name of the object data (XDATA) on imported entities.
pub const XDATA_APP: &str = "OPENINFRA";

/// Create an NLCS layer with its colour, lineweight and linetype. The NLCS
/// linetype is defined in paper millimetres when the drawing lacks it (shape
/// and text elements left out); an undefinable one leaves the layer
/// Continuous.
pub fn ensure_nlcs_layer(spec: &crate::nlcs::LayerSpec) {
    let Some(tab) = target().map(|t| t.tab_id) else { return };
    {
        let mut s = shared().lock().unwrap();
        if !s.made.insert((tab, format!("LA:{}", spec.name))) {
            return;
        }
        s.nlcs_layers.push((tab, spec.clone()));
    }
    let linetype = crate::nlcs::linetype_pattern(&spec.linetype).and_then(|pattern| {
        if !shared().lock().unwrap().made.insert((tab, format!("LT:{}", spec.linetype))) {
            return Some(spec.linetype.clone());
        }
        let description = if crate::nlcs::linetype_is_complex(&spec.linetype) {
            format!("NLCS {} (vereenvoudigd, zonder symbolen)", spec.linetype)
        } else {
            format!("NLCS {}", spec.linetype)
        };
        let op = TableOperation::LinetypeCreate { name: spec.linetype.clone(), description, pattern };
        match request(PluginRequest::TableOperation { operation: op }) {
            Ok(PluginResponse::TableResult(Ok(_))) => Some(spec.linetype.clone()),
            // Already in the drawing (an NLCS template, an earlier import).
            Ok(PluginResponse::TableResult(Err(e))) if e.contains("already exists") => Some(spec.linetype.clone()),
            _ => None,
        }
    });
    let config = |linetype: Option<String>| LayerConfig {
        name: spec.name.clone(),
        color: Some(match spec.true_color {
            Some((r, g, b)) => Color::Rgb { r, g, b },
            None => Color::Index(spec.color.max(1)),
        }),
        linetype,
        lineweight: spec.lineweight.map(LineWeight::from_value),
        description: Some(format!("NLCS object {}", spec.object_id)),
        ..Default::default()
    };
    let made = matches!(request(PluginRequest::AddLayer(config(linetype.clone()))), Ok(PluginResponse::OptHandle(Some(_))));
    if !made && linetype.is_some() {
        let _ = request(PluginRequest::AddLayer(config(None)));
    }
}

/// The NLCS layers set up in the active drawing during this session.
pub fn nlcs_layers() -> Vec<crate::nlcs::LayerSpec> {
    let Some(tab) = target().map(|t| t.tab_id) else { return Vec::new() };
    shared().lock().unwrap().nlcs_layers.iter().filter(|(t, _)| *t == tab).map(|(_, s)| s.clone()).collect()
}

/// Record a data source of an import into the active drawing.
pub fn note_source(source: &str) {
    let Some(tab) = target().map(|t| t.tab_id) else { return };
    let mut s = shared().lock().unwrap();
    if !s.sources.iter().any(|(t, v)| *t == tab && v == source) {
        s.sources.push((tab, source.to_string()));
    }
}

pub fn sources() -> Vec<String> {
    let Some(tab) = target().map(|t| t.tab_id) else { return Vec::new() };
    shared().lock().unwrap().sources.iter().filter(|(t, _)| *t == tab).map(|(_, v)| v.clone()).collect()
}

/// An XDATA record with `key=value` strings (each at most 255 characters).
pub fn object_data<'a>(pairs: impl IntoIterator<Item = (&'a str, String)>) -> ExtendedDataRecord {
    let mut r = ExtendedDataRecord::new(XDATA_APP);
    for (k, v) in pairs {
        let v = v.replace(['\n', '\r'], " ");
        if v.trim().is_empty() {
            continue;
        }
        let mut s = format!("{k}={}", v.trim());
        if s.len() > 255 {
            let mut cut = 255;
            while !s.is_char_boundary(cut) {
                cut -= 1;
            }
            s.truncate(cut);
        }
        r.add_value(XDataValue::String(s));
    }
    r
}

/// The host registers an XDATA application name only through WriteRecord:
/// rewrite one entity's record so the drawing carries the APPID.
pub fn register_object_data(handle: Handle, record: ExtendedDataRecord) {
    let _ = request(PluginRequest::WriteRecord { handle, record });
}

/// Add entities as one undo step, in chunks that stay well under the IPC
/// frame limit.
pub fn add_entities(label: &str, entities: Vec<EntityType>) -> Result<usize, String> {
    if entities.is_empty() {
        return Ok(0);
    }
    Ok(add_entities_handles(label, entities)?.len())
}

/// Like [`add_entities`], returning the new handles.
pub fn add_entities_handles(label: &str, entities: Vec<EntityType>) -> Result<Vec<Handle>, String> {
    if entities.is_empty() {
        return Ok(Vec::new());
    }
    // Imports run on their own threads: one at a time keeps each its own
    // undo step.
    static WRITING: Mutex<()> = Mutex::new(());
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    request(PluginRequest::PushUndo { label: label.into() })?;
    let mut handles = Vec::with_capacity(entities.len());
    let mut it = entities.into_iter().peekable();
    while it.peek().is_some() {
        let chunk: Vec<EntityType> = it.by_ref().take(2000).collect();
        if let PluginResponse::Handles(h) = request(PluginRequest::AddEntities(chunk))? {
            handles.extend(h);
        }
    }
    let _ = request(PluginRequest::SetDirty);
    // Many fast additions can leave the host's draw cache partly stale
    // (seen with an image underlay hiding newer lines): force a rebuild.
    let _ = request(PluginRequest::BumpGeometry);
    Ok(handles)
}

/// Map images are underlays: send them behind the vector data.
pub fn send_to_back(handles: &[Handle]) {
    if handles.is_empty() {
        return;
    }
    if request(PluginRequest::SetSelection { handles: handles.to_vec() }).is_ok() {
        let _ = request(PluginRequest::ExecuteCommand("DRAWORDER_BACK".into()));
        let _ = request(PluginRequest::SetSelection { handles: Vec::new() });
        let _ = request(PluginRequest::BumpGeometry);
    }
}

/// Run a command line in the active drawing (tokens answer its prompts).
pub fn execute(command: &str) -> Result<(), String> {
    match request(PluginRequest::ExecuteCommand(command.into()))? {
        PluginResponse::Bool(false) => Err(format!("Open CAD Studio voerde '{}' niet uit", command.split_whitespace().next().unwrap_or(""))),
        _ => Ok(()),
    }
}

pub fn zoom_extents() {
    let _ = request(PluginRequest::ExecuteCommand("ZOOMEXTENTS".into()));
}

/// Folder for downloaded map images: beside the drawing when it is saved,
/// otherwise in the plugin's config folder.
pub fn image_dir() -> PathBuf {
    let doc = target().and_then(|t| t.doc_path);
    match doc {
        Some(p) => {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("tekening").to_string();
            p.parent().map(|d| d.join(format!("{stem}_GIS"))).unwrap_or_else(|| crate::gis::servers::config_dir().join("kaarten"))
        }
        None => crate::gis::servers::config_dir().join("kaarten"),
    }
}
