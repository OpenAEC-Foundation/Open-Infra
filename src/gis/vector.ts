/**
 * Vector import — GeoJSON (from WFS or OpenStreetMap) to polylines, points and labels.
 */

import type { Shape, Point } from 'open-2d-studio';
import { transformer, type XY } from './crs';
import { toDrawing, type GisSite } from './site';
import { polyline, point, text } from './place';

export interface Geometry {
  type: string;
  coordinates?: any;
  geometries?: Geometry[];
}

export interface Feature {
  type: 'Feature';
  geometry: Geometry | null;
  properties?: Record<string, unknown> | null;
}

export interface VectorStyle {
  layerId: string;
  color: string;
  /** Property used for a text label at the feature centre. */
  labelField?: string;
  /** Text height and point-symbol size in metres. */
  labelHeight?: number;
  /** Clip geometry to this rectangle (drawing coordinates). */
  clip?: Rect;
}

export interface Rect { minX: number; minY: number; maxX: number; maxY: number }

const inside = (p: Point, r: Rect) => p.x >= r.minX && p.x <= r.maxX && p.y >= r.minY && p.y <= r.maxY;

/** Liang–Barsky: clip a polyline to a rectangle; returns the visible pieces. */
export function clipLine(pts: Point[], r: Rect): Point[][] {
  const out: Point[][] = [];
  let cur: Point[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const a = pts[i], b = pts[i + 1];
    const dx = b.x - a.x, dy = b.y - a.y;
    let t0 = 0, t1 = 1;
    let visible = true;
    for (const [p, q] of [[-dx, a.x - r.minX], [dx, r.maxX - a.x], [-dy, a.y - r.minY], [dy, r.maxY - a.y]]) {
      if (p === 0) { if (q < 0) { visible = false; break; } continue; }
      const t = q / p;
      if (p < 0) { if (t > t1) { visible = false; break; } if (t > t0) t0 = t; }
      else { if (t < t0) { visible = false; break; } if (t < t1) t1 = t; }
    }
    if (!visible) { if (cur.length > 1) out.push(cur); cur = []; continue; }
    const s = { x: a.x + t0 * dx, y: a.y + t0 * dy };
    const e = { x: a.x + t1 * dx, y: a.y + t1 * dy };
    if (!cur.length) cur.push(s);
    cur.push(e);
    if (t1 < 1) { if (cur.length > 1) out.push(cur); cur = []; }
  }
  if (cur.length > 1) out.push(cur);
  return out;
}

/** Sutherland–Hodgman: clip a closed ring to a rectangle. */
export function clipRing(pts: Point[], r: Rect): Point[] {
  const edges: Array<[(p: Point) => boolean, (a: Point, b: Point) => Point]> = [
    [(p) => p.x >= r.minX, (a, b) => ({ x: r.minX, y: a.y + ((b.y - a.y) * (r.minX - a.x)) / (b.x - a.x) })],
    [(p) => p.x <= r.maxX, (a, b) => ({ x: r.maxX, y: a.y + ((b.y - a.y) * (r.maxX - a.x)) / (b.x - a.x) })],
    [(p) => p.y >= r.minY, (a, b) => ({ x: a.x + ((b.x - a.x) * (r.minY - a.y)) / (b.y - a.y), y: r.minY })],
    [(p) => p.y <= r.maxY, (a, b) => ({ x: a.x + ((b.x - a.x) * (r.maxY - a.y)) / (b.y - a.y), y: r.maxY })],
  ];
  let ring = pts;
  for (const [isIn, cut] of edges) {
    if (!ring.length) break;
    const next: Point[] = [];
    for (let i = 0; i < ring.length; i++) {
      const a = ring[(i + ring.length - 1) % ring.length], b = ring[i];
      if (isIn(b)) {
        if (!isIn(a)) next.push(cut(a, b));
        next.push(b);
      } else if (isIn(a)) {
        next.push(cut(a, b));
      }
    }
    ring = next;
  }
  return ring;
}

/** Visit every coordinate ring/line of a geometry. */
function walk(g: Geometry, line: (pts: XY[], closed: boolean) => void, pt: (p: XY) => void): void {
  const c = g.coordinates;
  switch (g.type) {
    case 'Point': pt(c); break;
    case 'MultiPoint': c.forEach(pt); break;
    case 'LineString': line(c, false); break;
    case 'MultiLineString': c.forEach((l: XY[]) => line(l, false)); break;
    case 'Polygon': c.forEach((r: XY[]) => line(r, true)); break;
    case 'MultiPolygon': c.forEach((p: XY[][]) => p.forEach((r) => line(r, true))); break;
    case 'GeometryCollection': g.geometries?.forEach((s) => walk(s, line, pt)); break;
  }
}

function sample(features: Feature[], n = 200): XY[] {
  const out: XY[] = [];
  for (const f of features) {
    if (!f.geometry) continue;
    walk(f.geometry, (pts) => { if (pts[0]) out.push(pts[0]); }, (p) => out.push(p));
    if (out.length >= n) break;
  }
  return out;
}

/**
 * Coordinate transform from the data CRS to the project CRS. Some servers
 * return lat/lon order for EPSG:4326 — detect that by checking which order
 * lands closest to the site.
 */
export function dataTransform(site: GisSite, dataCrs: string, features: Feature[]): (p: XY) => XY {
  const fwd = transformer(dataCrs, site.crs);
  if (dataCrs === site.crs) return fwd;
  const pts = sample(features);
  if (!pts.length) return fwd;
  const score = (f: (p: XY) => XY) => {
    let near = 0;
    for (const p of pts) {
      try {
        const q = f(p);
        if (Math.hypot(q[0] - site.originE, q[1] - site.originN) < 50_000) near++;
      } catch { /* out of domain */ }
    }
    return near;
  };
  const swapped = (p: XY) => fwd([p[1], p[0]]);
  return score(swapped) > score(fwd) ? swapped : fwd;
}

function centroid(pts: Point[]): Point {
  let x = 0, y = 0;
  for (const p of pts) { x += p.x; y += p.y; }
  return { x: x / pts.length, y: y / pts.length };
}

export function featuresToShapes(site: GisSite, features: Feature[], toSite: (p: XY) => XY, style: VectorStyle): Shape[] {
  const shapes: Shape[] = [];
  const h = (style.labelHeight ?? 1) * 1000;
  const conv = (p: XY) => toDrawing(site, toSite([p[0], p[1]]));

  for (const f of features) {
    if (!f.geometry) continue;
    const anchor: Point[] = [];
    walk(
      f.geometry,
      (raw, closed) => {
        let pts = raw.map(conv);
        if (closed && pts.length > 2) {
          const a = pts[0], b = pts[pts.length - 1];
          if (Math.abs(a.x - b.x) < 1e-6 && Math.abs(a.y - b.y) < 1e-6) pts = pts.slice(0, -1);
        }
        if (pts.length < 2) return;
        const pieces = !style.clip ? [pts]
          : closed ? [clipRing(pts, style.clip)].filter((r) => r.length > 2)
          : clipLine(pts, style.clip);
        for (const piece of pieces) {
          shapes.push(polyline(style.layerId, style.color, piece, closed));
          if (!anchor.length) anchor.push(...piece);
        }
      },
      (p) => {
        const q = conv(p);
        if (style.clip && !inside(q, style.clip)) return;
        shapes.push(point(style.layerId, style.color, q, h * 0.5));
        anchor.push(q);
      },
    );
    const label = style.labelField ? f.properties?.[style.labelField] : undefined;
    if (label !== undefined && label !== null && label !== '' && anchor.length) {
      shapes.push(text(style.layerId, style.color, centroid(anchor), String(label), h));
    }
  }
  return shapes;
}

/** Property names present on the features (for the label picker). */
export function propertyNames(features: Feature[]): string[] {
  const names = new Set<string>();
  for (const f of features.slice(0, 50)) for (const k of Object.keys(f.properties ?? {})) names.add(k);
  return Array.from(names);
}
