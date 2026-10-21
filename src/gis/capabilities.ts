/**
 * OGC GetCapabilities parsing for WMS (1.1.1 / 1.3.0) and WFS (1.1 / 2.0).
 */

import { httpText, serviceError } from './http';
import { withParams } from './servers';
import { normalizeCode } from './crs';

export interface WmsLayer {
  name: string;
  title: string;
  crs: string[];
  /** lon/lat extent when advertised. */
  bbox?: [number, number, number, number];
}

export interface WmsCapabilities {
  version: string;
  title: string;
  getMapUrl: string;
  formats: string[];
  layers: WmsLayer[];
}

export interface WfsFeatureType {
  name: string;
  title: string;
  crs: string[];
}

export interface WfsCapabilities {
  version: string;
  title: string;
  getFeatureUrl: string;
  outputFormats: string[];
  featureTypes: WfsFeatureType[];
}

const children = (el: Element, local: string) =>
  Array.from(el.children).filter((c) => c.localName === local);
const child = (el: Element, local: string) => children(el, local)[0];
const text = (el: Element | undefined, local: string) => (el ? child(el, local)?.textContent?.trim() ?? '' : '');

function parseXml(xml: string): Document {
  const doc = new DOMParser().parseFromString(xml, 'application/xml');
  const err = serviceError(xml);
  if (err) throw new Error(err);
  if (doc.getElementsByTagName('parsererror').length) throw new Error('Ongeldig antwoord van de server (geen XML)');
  return doc;
}

function onlineResource(root: Element, op: string): string | null {
  const els = root.getElementsByTagNameNS('*', op);
  for (const el of Array.from(els)) {
    const res = el.getElementsByTagNameNS('*', 'OnlineResource')[0];
    const href = res?.getAttributeNS('http://www.w3.org/1999/xlink', 'href') || res?.getAttribute('xlink:href');
    if (href) return href;
    // WFS 1.1/2.0 OperationsMetadata
    if (el.getAttribute('name') === op) {
      const get = el.getElementsByTagNameNS('*', 'Get')[0];
      const h = get?.getAttributeNS('http://www.w3.org/1999/xlink', 'href') || get?.getAttribute('xlink:href');
      if (h) return h;
    }
  }
  return null;
}

export async function fetchWmsCapabilities(url: string): Promise<WmsCapabilities> {
  const xml = await httpText(withParams(url, { service: 'WMS', request: 'GetCapabilities' }));
  const doc = parseXml(xml);
  const root = doc.documentElement;
  const version = root.getAttribute('version') || '1.3.0';
  const service = child(root, 'Service');
  const capability = child(root, 'Capability');
  if (!capability) throw new Error('Geen WMS Capabilities gevonden');
  const request = child(capability, 'Request');
  const getMap = request ? child(request, 'GetMap') : undefined;
  const formats = getMap ? children(getMap, 'Format').map((f) => f.textContent?.trim() ?? '') : [];

  const layers: WmsLayer[] = [];
  const walk = (el: Element, inheritedCrs: string[]) => {
    const own = [...children(el, 'CRS'), ...children(el, 'SRS')]
      .flatMap((c) => (c.textContent ?? '').split(/\s+/))
      .filter(Boolean)
      .map(normalizeCode);
    const crs = Array.from(new Set([...inheritedCrs, ...own]));
    const name = text(el, 'Name');
    if (name) {
      const layer: WmsLayer = { name, title: text(el, 'Title') || name, crs };
      const geo = child(el, 'EX_GeographicBoundingBox');
      const ll = child(el, 'LatLonBoundingBox');
      if (geo) {
        layer.bbox = [
          Number(text(geo, 'westBoundLongitude')), Number(text(geo, 'southBoundLatitude')),
          Number(text(geo, 'eastBoundLongitude')), Number(text(geo, 'northBoundLatitude')),
        ];
      } else if (ll) {
        layer.bbox = ['minx', 'miny', 'maxx', 'maxy'].map((k) => Number(ll.getAttribute(k))) as WmsLayer['bbox'];
      }
      layers.push(layer);
    }
    for (const sub of children(el, 'Layer')) walk(sub, crs);
  };
  for (const top of children(capability, 'Layer')) walk(top, []);

  return {
    version,
    title: text(service, 'Title'),
    getMapUrl: onlineResource(capability, 'GetMap') || url,
    formats,
    layers,
  };
}

export async function fetchWfsCapabilities(url: string): Promise<WfsCapabilities> {
  const xml = await httpText(withParams(url, { service: 'WFS', request: 'GetCapabilities' }));
  const doc = parseXml(xml);
  const root = doc.documentElement;
  const version = root.getAttribute('version') || '2.0.0';
  const ident = child(root, 'ServiceIdentification') || child(root, 'Service');

  const featureTypes: WfsFeatureType[] = Array.from(doc.getElementsByTagNameNS('*', 'FeatureType')).map((ft) => {
    const crs = [
      ...children(ft, 'DefaultCRS'), ...children(ft, 'OtherCRS'),
      ...children(ft, 'DefaultSRS'), ...children(ft, 'OtherSRS'), ...children(ft, 'SRS'),
    ].map((c) => normalizeCode(c.textContent ?? '')).filter(Boolean);
    const name = text(ft, 'Name');
    return { name, title: text(ft, 'Title') || name, crs };
  }).filter((f) => f.name);

  const outputFormats = new Set<string>();
  for (const p of Array.from(doc.getElementsByTagNameNS('*', 'Parameter'))) {
    if (/^outputFormat$/i.test(p.getAttribute('name') || '')) {
      for (const v of Array.from(p.getElementsByTagNameNS('*', 'Value'))) outputFormats.add(v.textContent?.trim() ?? '');
    }
  }
  for (const f of Array.from(doc.getElementsByTagNameNS('*', 'Format'))) outputFormats.add(f.textContent?.trim() ?? '');

  return {
    version,
    title: text(ident, 'Title'),
    getFeatureUrl: onlineResource(root, 'GetFeature') || url,
    outputFormats: Array.from(outputFormats).filter(Boolean),
    featureTypes,
  };
}

/** Choose a GeoJSON output format from what the server advertises. */
export function pickGeoJsonFormat(formats: string[]): string {
  const prefs = [/^application\/json$/i, /geo\+?json/i, /^json$/i, /json/i];
  for (const re of prefs) {
    const f = formats.find((x) => re.test(x));
    if (f) return f;
  }
  return 'application/json';
}
