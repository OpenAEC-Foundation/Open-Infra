/**
 * Project site — the GIS location every import works from (comparable to the
 * Site object of the FreeCAD GIS2BIM workbench): a geographic position, the
 * project CRS, the base point and the extent of the area to import.
 *
 * Drawing units are millimetres with Y pointing down, so a projected
 * coordinate (E, N) in metres lands at x = (E − E0)·1000, y = −(N − N0)·1000.
 */

import { useSyncExternalStore } from 'react';
import { transform, type XY } from './crs';

export interface GisSite {
  label: string;
  lon: number;
  lat: number;
  crs: string;
  /** Base point in the project CRS (metres). */
  originE: number;
  originN: number;
  /** Extent of the import area around the base point (metres). */
  width: number;
  height: number;
  /** true = absolute CRS coordinates in the drawing, false = relative to the base point. */
  absolute: boolean;
}

const STORAGE_KEY = 'open-infra:site';

const DEFAULT_SITE: GisSite = {
  label: 'Utrecht Centraal',
  lon: 5.110000,
  lat: 52.089444,
  crs: 'EPSG:28992',
  originE: 136100,
  originN: 455900,
  width: 500,
  height: 500,
  absolute: false,
};

function load(): GisSite | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? { ...DEFAULT_SITE, ...JSON.parse(raw) } : null;
  } catch {
    return null;
  }
}

let site: GisSite | null = load();
const listeners = new Set<() => void>();

export function getSite(): GisSite | null {
  return site;
}

export function requireSite(): GisSite {
  if (!site) throw new Error('Stel eerst de projectlocatie in (GIS → Locatie).');
  return site;
}

export function defaultSite(): GisSite {
  return { ...DEFAULT_SITE };
}

export function setSite(next: GisSite): void {
  site = { ...next };
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(site)); } catch { /* private mode */ }
  listeners.forEach((l) => l());
}

export function useSite(): GisSite | null {
  return useSyncExternalStore(
    (cb) => { listeners.add(cb); return () => listeners.delete(cb); },
    () => site,
  );
}

/** Import extent in the project CRS: [minE, minN, maxE, maxN]. */
export function siteBbox(s: GisSite): [number, number, number, number] {
  return [s.originE - s.width / 2, s.originN - s.height / 2, s.originE + s.width / 2, s.originN + s.height / 2];
}

/** Import extent as lon/lat [minLon, minLat, maxLon, maxLat] (envelope of the corners). */
export function siteBboxLonLat(s: GisSite): [number, number, number, number] {
  const [a, b, c, d] = siteBbox(s);
  const pts = ([[a, b], [c, b], [c, d], [a, d]] as XY[]).map((p) => transform(s.crs, 'EPSG:4326', p));
  const lons = pts.map((p) => p[0]);
  const lats = pts.map((p) => p[1]);
  return [Math.min(...lons), Math.min(...lats), Math.max(...lons), Math.max(...lats)];
}

/** Project-CRS metres → drawing millimetres. */
export function toDrawing(s: GisSite, p: XY): { x: number; y: number } {
  const e0 = s.absolute ? 0 : s.originE;
  const n0 = s.absolute ? 0 : s.originN;
  return { x: (p[0] - e0) * 1000, y: -(p[1] - n0) * 1000 };
}

/** Drawing millimetres → project-CRS metres. */
export function fromDrawing(s: GisSite, x: number, y: number): XY {
  const e0 = s.absolute ? 0 : s.originE;
  const n0 = s.absolute ? 0 : s.originN;
  return [x / 1000 + e0, -y / 1000 + n0];
}

/** Import extent as a rectangle in drawing coordinates (for clipping). */
export function siteDrawingRect(s: GisSite): { minX: number; minY: number; maxX: number; maxY: number } {
  const [a, b, c, d] = siteBbox(s);
  const p = toDrawing(s, [a, d]);
  const q = toDrawing(s, [c, b]);
  return { minX: p.x, minY: p.y, maxX: q.x, maxY: q.y };
}
