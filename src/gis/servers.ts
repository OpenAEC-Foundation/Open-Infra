/**
 * Map server catalogue — presets for the Netherlands (PDOK), Flanders and
 * worldwide sources, plus servers the user adds in the server manager.
 */

import { useSyncExternalStore } from 'react';

export type ServerKind = 'wms' | 'wfs' | 'xyz';

export interface MapServer {
  id: string;
  kind: ServerKind;
  name: string;
  region: string;
  /** Service endpoint (WMS/WFS) or tile template with {z}/{x}/{y} (XYZ). */
  url: string;
  attribution?: string;
  /** Layer / feature type preselected in the dialog. */
  defaultLayer?: string;
  /** XYZ only: highest zoom level the server provides. */
  maxZoom?: number;
  builtin?: boolean;
}

const PRESETS: MapServer[] = [
  // --- Nederland: PDOK WMS ---
  { id: 'pdok-luchtfoto', kind: 'wms', region: 'Nederland', name: 'PDOK Luchtfoto RGB (actueel, 8 cm)', url: 'https://service.pdok.nl/hwh/luchtfotorgb/wms/v1_0', defaultLayer: 'Actueel_orthoHR', attribution: 'Beeldmateriaal.nl / PDOK' },
  { id: 'pdok-kadastralekaart-wms', kind: 'wms', region: 'Nederland', name: 'PDOK Kadastrale kaart', url: 'https://service.pdok.nl/kadaster/kadastralekaart/wms/v5_0', defaultLayer: 'Kadastralekaart', attribution: 'Kadaster / PDOK' },
  { id: 'pdok-bag-wms', kind: 'wms', region: 'Nederland', name: 'PDOK BAG (panden, verblijfsobjecten)', url: 'https://service.pdok.nl/lv/bag/wms/v2_0', defaultLayer: 'pand', attribution: 'Kadaster / PDOK' },
  { id: 'pdok-top10nl-wms', kind: 'wms', region: 'Nederland', name: 'PDOK TOP10NL topografie', url: 'https://service.pdok.nl/brt/top10nl/wms/v1_0', attribution: 'Kadaster / PDOK' },
  { id: 'pdok-ahn-wms', kind: 'wms', region: 'Nederland', name: 'PDOK AHN hoogtebestand', url: 'https://service.pdok.nl/rws/ahn/wms/v1_0', defaultLayer: 'dtm_05m', attribution: 'AHN / Rijkswaterstaat / PDOK' },
  // --- Nederland: PDOK WFS ---
  { id: 'pdok-bag-wfs', kind: 'wfs', region: 'Nederland', name: 'PDOK BAG — panden en adressen', url: 'https://service.pdok.nl/lv/bag/wfs/v2_0', defaultLayer: 'bag:pand', attribution: 'Kadaster / PDOK' },
  { id: 'pdok-kadastralekaart-wfs', kind: 'wfs', region: 'Nederland', name: 'PDOK Kadastrale kaart — percelen', url: 'https://service.pdok.nl/kadaster/kadastralekaart/wfs/v5_0', defaultLayer: 'kadastralekaart:Perceel', attribution: 'Kadaster / PDOK' },
  { id: 'pdok-nwbwegen-wfs', kind: 'wfs', region: 'Nederland', name: 'PDOK NWB Wegen (Rijkswaterstaat)', url: 'https://service.pdok.nl/rws/nwbwegen/wfs/v1_0', attribution: 'Rijkswaterstaat / PDOK' },
  { id: 'pdok-nwbvaarwegen-wfs', kind: 'wfs', region: 'Nederland', name: 'PDOK NWB Vaarwegen', url: 'https://service.pdok.nl/rws/nwbvaarwegen/wfs/v1_0', attribution: 'Rijkswaterstaat / PDOK' },
  { id: 'pdok-spoorwegen-wfs', kind: 'wfs', region: 'Nederland', name: 'PDOK Spoorwegen (ProRail)', url: 'https://service.pdok.nl/prorail/spoorwegen/wfs/v1_0', attribution: 'ProRail / PDOK' },
  { id: 'pdok-bestuurlijk-wfs', kind: 'wfs', region: 'Nederland', name: 'PDOK Bestuurlijke gebieden', url: 'https://service.pdok.nl/kadaster/bestuurlijkegebieden/wfs/v1_0', attribution: 'Kadaster / PDOK' },
  { id: 'pdok-natura2000-wfs', kind: 'wfs', region: 'Nederland', name: 'PDOK Natura 2000', url: 'https://service.pdok.nl/rvo/natura2000/wfs/v1_0', attribution: 'RVO / PDOK' },
  // --- Vlaanderen ---
  { id: 'vl-ortho-wms', kind: 'wms', region: 'België (Vlaanderen)', name: 'Digitaal Vlaanderen — Orthofoto (middenschalig)', url: 'https://geo.api.vlaanderen.be/OMWRGBMRVL/wms', attribution: 'Digitaal Vlaanderen' },
  { id: 'vl-grb-wfs', kind: 'wfs', region: 'België (Vlaanderen)', name: 'Digitaal Vlaanderen — GRB', url: 'https://geo.api.vlaanderen.be/GRB/wfs', attribution: 'Digitaal Vlaanderen' },
  // --- Wereldwijd ---
  { id: 'osm-tiles', kind: 'xyz', region: 'Wereld', name: 'OpenStreetMap (tegels)', url: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png', maxZoom: 19, attribution: '© OpenStreetMap-bijdragers (ODbL)' },
  { id: 'terrestris-osm-wms', kind: 'wms', region: 'Wereld', name: 'terrestris OSM WMS', url: 'https://ows.terrestris.de/osm/service', defaultLayer: 'OSM-WMS', attribution: '© terrestris, © OpenStreetMap-bijdragers' },
  { id: 'eox-s2-wms', kind: 'wms', region: 'Wereld', name: 'EOX Sentinel-2 cloudless (satelliet)', url: 'https://tiles.maps.eox.at/wms', defaultLayer: 's2cloudless-2023', attribution: 'Sentinel-2 cloudless by EOX (CC BY-NC-SA 4.0)' },
  { id: 'nasa-gibs-wms', kind: 'wms', region: 'Wereld', name: 'NASA GIBS (aardobservatie)', url: 'https://gibs.earthdata.nasa.gov/wms/epsg4326/best/wms.cgi', defaultLayer: 'BlueMarble_ShadedRelief_Bathymetry', attribution: 'NASA EOSDIS GIBS' },
  { id: 'gebco-wms', kind: 'wms', region: 'Wereld', name: 'GEBCO bathymetrie', url: 'https://wms.gebco.net/mapserv', defaultLayer: 'GEBCO_LATEST_SUB_ICE_TOPO', attribution: 'GEBCO Compilation Group' },
];

const STORAGE_KEY = 'open-infra:servers';

function loadUser(): MapServer[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? (JSON.parse(raw) as MapServer[]) : [];
  } catch {
    return [];
  }
}

let userServers = loadUser();
let snapshot: MapServer[] = [...PRESETS.map((s) => ({ ...s, builtin: true })), ...userServers];
const listeners = new Set<() => void>();

function commit() {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(userServers)); } catch { /* ignore */ }
  snapshot = [...PRESETS.map((s) => ({ ...s, builtin: true })), ...userServers];
  listeners.forEach((l) => l());
}

export function allServers(): MapServer[] {
  return snapshot;
}

export function useServers(kind?: ServerKind | ServerKind[]): MapServer[] {
  const list = useSyncExternalStore(
    (cb) => { listeners.add(cb); return () => listeners.delete(cb); },
    () => snapshot,
  );
  if (!kind) return list;
  const kinds = Array.isArray(kind) ? kind : [kind];
  return list.filter((s) => kinds.includes(s.kind));
}

export function addServer(s: Omit<MapServer, 'id' | 'builtin'>): MapServer {
  const server: MapServer = { ...s, id: `user-${Date.now().toString(36)}` };
  userServers = [...userServers, server];
  commit();
  return server;
}

export function removeServer(id: string): void {
  userServers = userServers.filter((s) => s.id !== id);
  commit();
}

/** Strip query parameters that the dialogs add themselves. */
export function cleanServiceUrl(url: string): string {
  try {
    const u = new URL(url.trim());
    for (const k of [...u.searchParams.keys()]) {
      if (/^(service|request|version|layers|typenames?|bbox|srs|crs|format|width|height|styles|outputformat)$/i.test(k)) {
        u.searchParams.delete(k);
      }
    }
    return u.toString().replace(/\?$/, '');
  } catch {
    return url.trim();
  }
}

export function withParams(url: string, params: Record<string, string | number>): string {
  const u = new URL(url);
  for (const [k, v] of Object.entries(params)) u.searchParams.set(k, String(v));
  return u.toString();
}
