//! OGC GetCapabilities parsing for WMS (1.1.1 / 1.3.0) and WFS (1.1 / 2.0).

use roxmltree::{Document, Node};

use super::crs::normalize;
use super::http;

#[derive(Clone, Debug)]
pub struct WmsLayer {
    pub name: String,
    pub title: String,
    pub crs: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct WmsCaps {
    pub version: String,
    pub title: String,
    pub get_map_url: String,
    pub formats: Vec<String>,
    pub layers: Vec<WmsLayer>,
}

#[derive(Clone, Debug)]
pub struct WfsType {
    pub name: String,
    pub title: String,
    pub crs: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct WfsCaps {
    pub version: String,
    pub title: String,
    pub get_feature_url: String,
    pub output_formats: Vec<String>,
    pub types: Vec<WfsType>,
}

fn children<'a, 'i>(n: Node<'a, 'i>, local: &'a str) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    n.children().filter(move |c| c.is_element() && c.tag_name().name() == local)
}

fn child<'a, 'i>(n: Node<'a, 'i>, local: &'a str) -> Option<Node<'a, 'i>> {
    children(n, local).next()
}

fn text_of(n: Option<Node>, local: &str) -> String {
    n.and_then(|n| child(n, local)).and_then(|c| c.text()).unwrap_or("").trim().to_string()
}

fn parse(xml: &str) -> Result<Document<'_>, String> {
    if let Some(e) = http::service_error(xml) {
        return Err(e);
    }
    Document::parse(xml).map_err(|e| format!("ongeldig antwoord van de server: {e}"))
}

fn href(n: Node) -> Option<String> {
    n.attributes().find(|a| a.name() == "href").map(|a| a.value().to_string())
}

/// URL of an operation (WMS DCPType or OWS OperationsMetadata).
fn operation_url(root: Node, op: &str) -> Option<String> {
    for n in root.descendants().filter(|n| n.is_element()) {
        let is_op = n.tag_name().name() == op
            || (n.tag_name().name() == "Operation" && n.attribute("name") == Some(op));
        if !is_op {
            continue;
        }
        if let Some(r) = n.descendants().find(|d| d.tag_name().name() == "OnlineResource").and_then(href) {
            return Some(r);
        }
        if let Some(r) = n.descendants().find(|d| d.tag_name().name() == "Get").and_then(href) {
            return Some(r);
        }
    }
    None
}

/// Keep https when the capabilities advertise a plain-http endpoint.
fn same_scheme(service: &str, advertised: Option<String>) -> String {
    match advertised {
        Some(a) if service.starts_with("https:") && a.starts_with("http:") => format!("https:{}", &a[5..]),
        Some(a) if !a.is_empty() => a,
        _ => service.to_string(),
    }
}

pub fn fetch_wms(url: &str) -> Result<WmsCaps, String> {
    let xml = http::get_text(&http::with_params(url, &[("SERVICE", "WMS".into()), ("REQUEST", "GetCapabilities".into())]))?;
    let doc = parse(&xml)?;
    let root = doc.root_element();
    let version = root.attribute("version").unwrap_or("1.3.0").to_string();
    let capability = child(root, "Capability").ok_or("geen WMS-capabilities gevonden")?;
    let formats = child(capability, "Request")
        .and_then(|r| child(r, "GetMap"))
        .map(|g| children(g, "Format").filter_map(|f| f.text()).map(|t| t.trim().to_string()).collect())
        .unwrap_or_default();

    let mut layers = Vec::new();
    fn walk(n: Node, inherited: &[String], out: &mut Vec<WmsLayer>) {
        let mut crs: Vec<String> = inherited.to_vec();
        for c in children(n, "CRS").chain(children(n, "SRS")) {
            for t in c.text().unwrap_or("").split_whitespace() {
                let code = normalize(t);
                if !crs.contains(&code) {
                    crs.push(code);
                }
            }
        }
        let name = text_of(Some(n), "Name");
        if !name.is_empty() {
            let title = text_of(Some(n), "Title");
            out.push(WmsLayer { title: if title.is_empty() { name.clone() } else { title }, name, crs: crs.clone() });
        }
        for sub in children(n, "Layer") {
            walk(sub, &crs, out);
        }
    }
    for top in children(capability, "Layer") {
        walk(top, &[], &mut layers);
    }
    Ok(WmsCaps {
        version,
        title: text_of(child(root, "Service"), "Title"),
        get_map_url: same_scheme(url, operation_url(capability, "GetMap")),
        formats,
        layers,
    })
}

pub fn fetch_wfs(url: &str) -> Result<WfsCaps, String> {
    let xml = http::get_text(&http::with_params(url, &[("SERVICE", "WFS".into()), ("REQUEST", "GetCapabilities".into())]))?;
    let doc = parse(&xml)?;
    let root = doc.root_element();
    let version = root.attribute("version").unwrap_or("2.0.0").to_string();
    let ident = child(root, "ServiceIdentification").or_else(|| child(root, "Service"));
    let types = root
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "FeatureType")
        .filter_map(|ft| {
            let name = text_of(Some(ft), "Name");
            if name.is_empty() {
                return None;
            }
            let crs = ft
                .children()
                .filter(|c| c.is_element() && ["DefaultCRS", "OtherCRS", "DefaultSRS", "OtherSRS", "SRS"].contains(&c.tag_name().name()))
                .filter_map(|c| c.text())
                .map(normalize)
                .collect();
            let title = text_of(Some(ft), "Title");
            Some(WfsType { title: if title.is_empty() { name.clone() } else { title }, name, crs })
        })
        .collect();
    let mut output_formats: Vec<String> = Vec::new();
    for p in root.descendants().filter(|n| n.is_element() && n.tag_name().name() == "Parameter") {
        if p.attribute("name").is_some_and(|a| a.eq_ignore_ascii_case("outputFormat")) {
            for v in p.descendants().filter(|v| v.tag_name().name() == "Value") {
                if let Some(t) = v.text() {
                    output_formats.push(t.trim().to_string());
                }
            }
        }
    }
    Ok(WfsCaps {
        version,
        title: text_of(ident, "Title"),
        get_feature_url: same_scheme(url, operation_url(root, "GetFeature")),
        output_formats,
        types,
    })
}

/// A GeoJSON output format from what the server advertises.
pub fn pick_geojson_format(formats: &[String]) -> String {
    let lower: Vec<String> = formats.iter().map(|f| f.to_ascii_lowercase()).collect();
    for want in ["application/json", "geojson", "json"] {
        if let Some(i) = lower.iter().position(|f| f == want || (want != "application/json" && f.contains(want))) {
            return formats[i].clone();
        }
    }
    "application/json".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geojson_format_preference() {
        let f = vec!["text/xml; subtype=gml/3.2".to_string(), "application/json; subtype=geojson".to_string(), "application/json".to_string()];
        assert_eq!(pick_geojson_format(&f), "application/json");
        assert_eq!(pick_geojson_format(&["GML2".into(), "geojson".into()]), "geojson");
    }
}
