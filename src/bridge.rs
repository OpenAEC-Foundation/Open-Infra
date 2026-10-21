//! Talking to Open CAD Studio from worker and window threads.
//!
//! `dispatch` (the host's call into the plugin) hands us a
//! `PluginRequestSender` for the active document tab; the workbench window and
//! import workers use it to add layers and entities while the host stays
//! responsive. Each import is one undo step.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use ocs_plugin_api::host::{
    codec::{Color, EntityType, Handle},
    HostApi, LayerConfig, PluginRequestSender, XRecordEntry, XRecordValue,
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
    let unit_factor = match host.system_variable("INSUNITS") {
        Some(ocs_plugin_api::host::HostSettingValue::Number(n)) => unit_factor_for_insunits(n as i64),
        Some(ocs_plugin_api::host::HostSettingValue::Text(t)) => unit_factor_for_insunits(t.trim().parse().unwrap_or(0)),
        None => 1.0,
    };
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
