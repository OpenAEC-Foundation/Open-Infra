/**
 * OpenStreetMap import via the Overpass API — worldwide buildings, roads,
 * railways, water, land use, utilities and addresses.
 */

import { httpText } from './http';
import { siteBboxLonLat, type GisSite } from './site';
import type { Feature } from './vector';

interface OsmElement {
  type: 'node' | 'way' | 'relation';
  id: number;
  lat?: number;
  lon?: number;
  tags?: Record<string, string>;
  geometry?: Array<{ lat: number; lon: number } | null>;
  members?: Array<{ type: string; role: string; geometry?: Array<{ lat: number; lon: number } | null> }>;
}

export interface OsmCategory {
  id: string;
  label: string;
  layer: string;
  color: string;
  /** Overpass statements; `{{bbox}}` is replaced by the site extent. */
  query: string;
  /** Client-side test, used to sort the results of the combined query. */
  match: (tags: Record<string, string>, type: OsmElement['type']) => boolean;
  labelTag?: string;
  defaultOn: boolean;
}

export const OSM_CATEGORIES: OsmCategory[] = [
  { id: 'buildings', label: 'Gebouwen', layer: 'OSM-Gebouwen', color: '#d08770', defaultOn: true,
    query: 'way["building"]({{bbox}});relation["building"]({{bbox}});',
    match: (t) => !!t.building },
  { id: 'roads', label: 'Wegen en paden', layer: 'OSM-Wegen', color: '#ebcb8b', defaultOn: true, labelTag: 'name',
    query: 'way["highway"]({{bbox}});',
    match: (t) => !!t.highway },
  { id: 'rail', label: 'Spoor, tram en metro', layer: 'OSM-Spoor', color: '#b48ead', defaultOn: true,
    query: 'way["railway"~"^(rail|tram|light_rail|subway|narrow_gauge|monorail)$"]({{bbox}});',
    match: (t) => /^(rail|tram|light_rail|subway|narrow_gauge|monorail)$/.test(t.railway ?? '') },
  { id: 'water', label: 'Water en waterlopen', layer: 'OSM-Water', color: '#5e81ac', defaultOn: true, labelTag: 'name',
    query: 'way["waterway"]({{bbox}});way["natural"="water"]({{bbox}});relation["natural"="water"]({{bbox}});',
    match: (t) => !!t.waterway || t.natural === 'water' },
  { id: 'bridges', label: 'Kunstwerken (bruggen, tunnels, viaducten)', layer: 'OSM-Kunstwerken', color: '#bf616a', defaultOn: false, labelTag: 'name',
    query: 'way["man_made"="bridge"]({{bbox}});way["bridge"]["bridge"!="no"]({{bbox}});way["tunnel"]["tunnel"!="no"]({{bbox}});',
    match: (t) => t.man_made === 'bridge' || (!!t.bridge && t.bridge !== 'no') || (!!t.tunnel && t.tunnel !== 'no') },
  { id: 'landuse', label: 'Landgebruik', layer: 'OSM-Landgebruik', color: '#a3be8c', defaultOn: false,
    query: 'way["landuse"]({{bbox}});relation["landuse"]({{bbox}});',
    match: (t) => !!t.landuse },
  { id: 'utilities', label: 'Kabels en leidingen (hoogspanning, pijpleidingen)', layer: 'OSM-Kabels-Leidingen', color: '#d8dee9', defaultOn: false,
    query: 'way["power"~"^(line|minor_line|cable)$"]({{bbox}});way["man_made"="pipeline"]({{bbox}});node["power"="tower"]({{bbox}});',
    match: (t) => /^(line|minor_line|cable|tower)$/.test(t.power ?? '') || t.man_made === 'pipeline' },
  { id: 'parcels', label: 'Terreingrenzen en hekken', layer: 'OSM-Grenzen', color: '#88c0d0', defaultOn: false,
    query: 'way["barrier"~"^(fence|wall|hedge|retaining_wall|guard_rail)$"]({{bbox}});',
    match: (t) => /^(fence|wall|hedge|retaining_wall|guard_rail)$/.test(t.barrier ?? '') },
  { id: 'addresses', label: 'Huisnummers', layer: 'OSM-Adressen', color: '#e5e9f0', defaultOn: false, labelTag: 'addr:housenumber',
    query: 'node["addr:housenumber"]({{bbox}});',
    match: (t, type) => type === 'node' && !!t['addr:housenumber'] },
];

// Public Overpass instances with worldwide coverage, tried in order.
const ENDPOINTS = [
  'https://overpass-api.de/api/interpreter',
  'https://overpass.private.coffee/api/interpreter',
  'https://maps.mail.ru/osm/tools/overpass/api/interpreter',
  'https://overpass.kumi.systems/api/interpreter',
];
const TIMEOUT_MS = 90_000;
const ROUNDS = 2;


function line(geom: Array<{ lat: number; lon: number } | null>): [number, number][] {
  return geom.filter((g): g is { lat: number; lon: number } => !!g).map((g) => [g.lon, g.lat]);
}

function toFeature(el: OsmElement): Feature | null {
  const properties = { ...(el.tags ?? {}), osm_id: `${el.type}/${el.id}` };
  if (el.type === 'node' && el.lon !== undefined && el.lat !== undefined) {
    return { type: 'Feature', properties, geometry: { type: 'Point', coordinates: [el.lon, el.lat] } };
  }
  if (el.type === 'way' && el.geometry) {
    const coords = line(el.geometry);
    if (coords.length < 2) return null;
    const first = coords[0], last = coords[coords.length - 1];
    const closed = coords.length > 3 && first[0] === last[0] && first[1] === last[1];
    return closed
      ? { type: 'Feature', properties, geometry: { type: 'Polygon', coordinates: [coords] } }
      : { type: 'Feature', properties, geometry: { type: 'LineString', coordinates: coords } };
  }
  if (el.type === 'relation' && el.members) {
    const lines = el.members.filter((m) => m.geometry).map((m) => line(m.geometry!)).filter((l) => l.length > 1);
    if (!lines.length) return null;
    return { type: 'Feature', properties, geometry: { type: 'MultiLineString', coordinates: lines } };
  }
  return null;
}

/**
 * Fetch all selected categories with one combined Overpass query (kinder to
 * the public servers) and sort the elements per category.
 */
export async function fetchOsm(site: GisSite, cats: OsmCategory[]): Promise<Map<string, Feature[]>> {
  const [w, s, e, n] = siteBboxLonLat(site);
  const bbox = [s, w, n, e].map((v) => v.toFixed(7)).join(',');
  const statements = cats.map((c) => c.query.split('{{bbox}}').join(bbox)).join('');
  const query = `[out:json][timeout:120];(${statements});out geom;`;
  let lastErr: unknown;
  // Public instances are often busy (429/502/504): rotate through them, with a
  // per-request timeout, and back off between rounds.
  for (let round = 0; round < ROUNDS; round++) {
    if (round > 0) await new Promise((r) => setTimeout(r, 2000 * round));
    for (const url of ENDPOINTS) {
      const ac = new AbortController();
      const timer = setTimeout(() => ac.abort(), TIMEOUT_MS);
      try {
        const body = await httpText(url, {
          method: 'POST',
          headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
          body: 'data=' + encodeURIComponent(query),
          signal: ac.signal,
        });
        const json = JSON.parse(body) as { elements: OsmElement[]; remark?: string };
        if (json.remark && /error|timed out/i.test(json.remark)) throw new Error(json.remark);
        const out = new Map<string, Feature[]>(cats.map((c) => [c.id, []]));
        for (const el of json.elements) {
          const tags = el.tags ?? {};
          const hits = cats.filter((c) => c.match(tags, el.type));
          if (!hits.length) continue;
          const f = toFeature(el);
          if (f) for (const c of hits) out.get(c.id)!.push(f);
        }
        return out;
      } catch (err) {
        lastErr = ac.signal.aborted ? new Error(`Overpass time-out (${url.split('/')[2]})`) : err;
      } finally {
        clearTimeout(timer);
      }
    }
  }
  throw new Error(`OpenStreetMap (Overpass) is nu overbelast of onbereikbaar — probeer het later opnieuw. (${lastErr instanceof Error ? lastErr.message : lastErr})`);
}
