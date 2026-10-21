/**
 * Raster import — WMS GetMap and XYZ tiles, placed as underlays in the project CRS.
 *
 * When a server cannot deliver the project CRS (e.g. OSM tiles in Web
 * Mercator), the image is reprojected client-side with a control-point grid
 * so it lines up with the vector data.
 */

import { httpImage, imageSize } from './http';
import { isGeographic, transformer, type XY } from './crs';
import { siteBbox, toDrawing, type GisSite } from './site';
import type { WmsCapabilities, WmsLayer } from './capabilities';
import { withParams, type MapServer } from './servers';
import { ensureLayer, underlay, commit } from './place';
import type { ImageShape } from 'open-2d-studio';

type Bbox = [number, number, number, number];

export interface RasterOptions {
  /** Ground resolution in metres per pixel. */
  resolution: number;
  opacity: number;
  format?: string;
  layerName?: string;
}

const MAX_TILE_PX = 2048;

interface Img { dataUrl: string; width: number; height: number }

/** Split the site extent into request tiles of at most MAX_TILE_PX pixels. */
function tiles(bbox: Bbox, res: number): Array<{ bbox: Bbox; w: number; h: number }> {
  const [minE, minN, maxE, maxN] = bbox;
  const tileM = MAX_TILE_PX * res;
  const out: Array<{ bbox: Bbox; w: number; h: number }> = [];
  for (let e = minE; e < maxE - 1e-6; e += tileM) {
    for (let n = minN; n < maxN - 1e-6; n += tileM) {
      const e2 = Math.min(e + tileM, maxE);
      const n2 = Math.min(n + tileM, maxN);
      out.push({ bbox: [e, n, e2, n2], w: Math.max(1, Math.round((e2 - e) / res)), h: Math.max(1, Math.round((n2 - n) / res)) });
    }
  }
  return out;
}

function placeTile(site: GisSite, layerId: string, bbox: Bbox, img: Img, label: string, opacity: number): ImageShape {
  const tl = toDrawing(site, [bbox[0], bbox[3]]);
  const br = toDrawing(site, [bbox[2], bbox[1]]);
  return underlay(layerId, tl, br.x - tl.x, br.y - tl.y, img, label, opacity);
}

/** Envelope of a bbox after transforming its edges (sampled, so curved edges are covered). */
function envelope(bbox: Bbox, fwd: (p: XY) => XY): Bbox {
  const [a, b, c, d] = bbox;
  const xs: number[] = [];
  const ys: number[] = [];
  const N = 8;
  for (let i = 0; i <= N; i++) {
    const t = i / N;
    for (const p of [[a + (c - a) * t, b], [a + (c - a) * t, d], [a, b + (d - b) * t], [c, b + (d - b) * t]] as XY[]) {
      const q = fwd(p);
      xs.push(q[0]); ys.push(q[1]);
    }
  }
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
}

/**
 * Reproject `src` (covering `srcBbox` in `srcCrs`) onto a w×h raster covering
 * `dstBbox` in `dstCrs`. Uses a 33×33 control grid with bilinear interpolation.
 */
async function warp(src: Img, srcCrs: string, srcBbox: Bbox, dstCrs: string, dstBbox: Bbox, w: number, h: number, jpeg: boolean): Promise<Img> {
  const { img } = await imageSize(src.dataUrl);
  const sc = document.createElement('canvas');
  sc.width = src.width; sc.height = src.height;
  const sctx = sc.getContext('2d')!;
  sctx.drawImage(img, 0, 0);
  const sdata = sctx.getImageData(0, 0, src.width, src.height).data;

  const inv = transformer(dstCrs, srcCrs);
  const G = 32;
  const gu = new Float64Array((G + 1) * (G + 1));
  const gv = new Float64Array((G + 1) * (G + 1));
  const [dminx, dminy, dmaxx, dmaxy] = dstBbox;
  const [sminx, sminy, smaxx, smaxy] = srcBbox;
  for (let j = 0; j <= G; j++) {
    for (let i = 0; i <= G; i++) {
      const p = inv([dminx + (dmaxx - dminx) * (i / G), dmaxy - (dmaxy - dminy) * (j / G)]);
      gu[j * (G + 1) + i] = ((p[0] - sminx) / (smaxx - sminx)) * src.width;
      gv[j * (G + 1) + i] = ((smaxy - p[1]) / (smaxy - sminy)) * src.height;
    }
  }

  const dc = document.createElement('canvas');
  dc.width = w; dc.height = h;
  const dctx = dc.getContext('2d')!;
  const out = dctx.createImageData(w, h);
  const od = out.data;
  for (let y = 0; y < h; y++) {
    const fy = ((y + 0.5) / h) * G;
    const j = Math.min(G - 1, Math.floor(fy));
    const ty = fy - j;
    for (let x = 0; x < w; x++) {
      const fx = ((x + 0.5) / w) * G;
      const i = Math.min(G - 1, Math.floor(fx));
      const tx = fx - i;
      const k = j * (G + 1) + i;
      const u = (gu[k] * (1 - tx) + gu[k + 1] * tx) * (1 - ty) + (gu[k + G + 1] * (1 - tx) + gu[k + G + 2] * tx) * ty;
      const v = (gv[k] * (1 - tx) + gv[k + 1] * tx) * (1 - ty) + (gv[k + G + 1] * (1 - tx) + gv[k + G + 2] * tx) * ty;
      const su = Math.floor(u);
      const sv = Math.floor(v);
      if (su < 0 || sv < 0 || su >= src.width || sv >= src.height) continue;
      const si = (sv * src.width + su) * 4;
      const di = (y * w + x) * 4;
      od[di] = sdata[si]; od[di + 1] = sdata[si + 1]; od[di + 2] = sdata[si + 2]; od[di + 3] = sdata[si + 3];
    }
  }
  dctx.putImageData(out, 0, 0);
  return { dataUrl: dc.toDataURL(jpeg ? 'image/jpeg' : 'image/png', 0.9), width: w, height: h };
}

// ---------------------------------------------------------------------------
// WMS
// ---------------------------------------------------------------------------

function isPhoto(name: string): boolean {
  return /ortho|luchtfoto|lufo|s2cloud|sentinel|satell|imagery|bluemarble/i.test(name);
}

export function defaultFormat(caps: WmsCapabilities, layer: string): string {
  const f = caps.formats;
  if (isPhoto(layer) && f.includes('image/jpeg')) return 'image/jpeg';
  return f.includes('image/png') ? 'image/png' : f[0] || 'image/png';
}

/** CRS to request: the project CRS when supported, otherwise a common fallback. */
function requestCrs(layer: WmsLayer, siteCrs: string): string {
  if (!layer.crs.length || layer.crs.includes(siteCrs)) return siteCrs;
  for (const c of ['EPSG:3857', 'EPSG:900913', 'EPSG:4326', 'CRS:84']) if (layer.crs.includes(c)) return c === 'EPSG:900913' ? 'EPSG:3857' : c;
  return layer.crs[0];
}

function getMapUrl(server: MapServer, caps: WmsCapabilities, layer: string, crs: string, bbox: Bbox, w: number, h: number, format: string): string {
  // Capabilities sometimes advertise http:// endpoints; keep https when we came in over https.
  let base = caps.getMapUrl || server.url;
  if (server.url.startsWith('https:') && base.startsWith('http:')) base = 'https:' + base.slice(5);
  const v13 = caps.version.startsWith('1.3');
  const axisFlip = v13 && crs !== 'CRS:84' && isGeographic(crs);
  const b = axisFlip ? [bbox[1], bbox[0], bbox[3], bbox[2]] : bbox;
  return withParams(base, {
    SERVICE: 'WMS',
    VERSION: v13 ? '1.3.0' : '1.1.1',
    REQUEST: 'GetMap',
    LAYERS: layer,
    STYLES: '',
    [v13 ? 'CRS' : 'SRS']: crs,
    BBOX: b.join(','),
    WIDTH: w,
    HEIGHT: h,
    FORMAT: format,
    TRANSPARENT: format === 'image/jpeg' ? 'FALSE' : 'TRUE',
  });
}

export async function importWms(
  site: GisSite,
  server: MapServer,
  caps: WmsCapabilities,
  layer: WmsLayer,
  opts: RasterOptions,
  onProgress?: (msg: string) => void,
): Promise<number> {
  const format = opts.format || defaultFormat(caps, layer.name);
  const crs = requestCrs(layer, site.crs);
  const layerId = ensureLayer(opts.layerName || `GIS-WMS ${layer.title}`.slice(0, 60), '#9aa0a6');
  const parts = tiles(siteBbox(site), opts.resolution);
  const shapes: ImageShape[] = [];
  const label = `${server.name} — ${layer.title}`;

  for (let i = 0; i < parts.length; i++) {
    onProgress?.(`Kaartdeel ${i + 1}/${parts.length} ophalen…`);
    const t = parts[i];
    if (crs === site.crs) {
      const img = await httpImage(getMapUrl(server, caps, layer.name, crs, t.bbox, t.w, t.h, format));
      shapes.push(placeTile(site, layerId, t.bbox, img, label, opts.opacity));
    } else {
      const fwdCrs = crs === 'CRS:84' ? 'EPSG:4326' : crs;
      const srcBbox = envelope(t.bbox, transformer(site.crs, fwdCrs));
      const src = await httpImage(getMapUrl(server, caps, layer.name, crs, srcBbox, t.w, t.h, format));
      onProgress?.(`Kaartdeel ${i + 1}/${parts.length} herprojecteren naar ${site.crs}…`);
      const img = await warp(src, fwdCrs, srcBbox, site.crs, t.bbox, t.w, t.h, format === 'image/jpeg');
      shapes.push(placeTile(site, layerId, t.bbox, img, label, opts.opacity));
    }
  }
  commit(shapes);
  return shapes.length;
}

// ---------------------------------------------------------------------------
// XYZ tiles (Web Mercator)
// ---------------------------------------------------------------------------

const R = 6378137;
const ORIGIN = Math.PI * R;
const MAX_TILES = 64;

export async function importXyz(site: GisSite, server: MapServer, opts: RasterOptions, onProgress?: (msg: string) => void): Promise<number> {
  const bbox = siteBbox(site);
  const merc = envelope(bbox, transformer(site.crs, 'EPSG:3857'));
  const center = transformer(site.crs, 'EPSG:4326')([site.originE, site.originN]);
  const maxZoom = server.maxZoom ?? 19;
  // Mercator metres per pixel at the equator, scaled to ground resolution at this latitude.
  const zIdeal = Math.log2((2 * ORIGIN * Math.cos((center[1] * Math.PI) / 180)) / (256 * opts.resolution));
  let z = Math.min(maxZoom, Math.max(0, Math.ceil(zIdeal)));

  const tileRange = (zoom: number) => {
    const size = (2 * ORIGIN) / 2 ** zoom;
    return {
      size,
      x0: Math.floor((merc[0] + ORIGIN) / size), x1: Math.floor((merc[2] + ORIGIN) / size),
      y0: Math.floor((ORIGIN - merc[3]) / size), y1: Math.floor((ORIGIN - merc[1]) / size),
    };
  };
  let r = tileRange(z);
  while (z > 0 && (r.x1 - r.x0 + 1) * (r.y1 - r.y0 + 1) > MAX_TILES) r = tileRange(--z);

  const nx = r.x1 - r.x0 + 1;
  const ny = r.y1 - r.y0 + 1;
  const canvas = document.createElement('canvas');
  canvas.width = nx * 256; canvas.height = ny * 256;
  const ctx = canvas.getContext('2d')!;
  let done = 0;
  const jobs: Promise<void>[] = [];
  for (let ty = r.y0; ty <= r.y1; ty++) {
    for (let tx = r.x0; tx <= r.x1; tx++) {
      const url = server.url.replace('{z}', String(z)).replace('{x}', String(tx)).replace('{y}', String(ty));
      jobs.push(httpImage(url).then(async (t) => {
        const { img } = await imageSize(t.dataUrl);
        ctx.drawImage(img, (tx - r.x0) * 256, (ty - r.y0) * 256, 256, 256);
        onProgress?.(`Tegels ophalen ${++done}/${nx * ny} (zoom ${z})…`);
      }));
    }
  }
  await Promise.all(jobs);

  const srcBbox: Bbox = [
    r.x0 * r.size - ORIGIN, ORIGIN - (r.y1 + 1) * r.size,
    (r.x1 + 1) * r.size - ORIGIN, ORIGIN - r.y0 * r.size,
  ];
  const src: Img = { dataUrl: canvas.toDataURL('image/png'), width: canvas.width, height: canvas.height };
  onProgress?.(`Herprojecteren naar ${site.crs}…`);
  const res = Math.max(opts.resolution, Math.max(site.width, site.height) / 4096);
  const w = Math.round(site.width / res);
  const h = Math.round(site.height / res);
  const out = await warp(src, 'EPSG:3857', srcBbox, site.crs, bbox, w, h, false);
  const layerId = ensureLayer(opts.layerName || `GIS-${server.name}`.slice(0, 60), '#9aa0a6');
  commit([placeTile(site, layerId, bbox, out, `${server.name} (zoom ${z}) — ${server.attribution ?? ''}`, opts.opacity)]);
  return nx * ny;
}
