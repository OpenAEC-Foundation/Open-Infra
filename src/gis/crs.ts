/**
 * Coordinate reference systems — proj4 definitions, UTM auto-selection and
 * on-demand lookup of any EPSG code via epsg.io.
 */

import proj4 from 'proj4';
import { httpText } from './http';

export interface CrsInfo {
  code: string;      // 'EPSG:28992'
  name: string;
  region: string;
}

/** Built-in definitions — the common ones for NL/BE/DE/UK plus global systems. */
const BUILTIN: Array<CrsInfo & { proj4: string }> = [
  {
    code: 'EPSG:28992', name: 'Amersfoort / RD New', region: 'Nederland',
    proj4: '+proj=sterea +lat_0=52.15616055555555 +lon_0=5.38763888888889 +k=0.9999079 +x_0=155000 +y_0=463000 +ellps=bessel +towgs84=565.4171,50.3319,465.5524,1.9342,-1.6677,9.1019,4.0725 +units=m +no_defs',
  },
  { code: 'EPSG:4326', name: 'WGS 84 (lat/lon)', region: 'Wereld', proj4: '+proj=longlat +datum=WGS84 +no_defs' },
  { code: 'EPSG:4258', name: 'ETRS89 (lat/lon)', region: 'Europa', proj4: '+proj=longlat +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +no_defs' },
  { code: 'EPSG:3857', name: 'WGS 84 / Pseudo-Mercator', region: 'Wereld (webkaarten)', proj4: '+proj=merc +a=6378137 +b=6378137 +lat_ts=0 +lon_0=0 +x_0=0 +y_0=0 +k=1 +units=m +nadgrids=@null +wktext +no_defs' },
  { code: 'EPSG:25831', name: 'ETRS89 / UTM zone 31N', region: 'Europa (0°–6°O)', proj4: '+proj=utm +zone=31 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs' },
  { code: 'EPSG:25832', name: 'ETRS89 / UTM zone 32N', region: 'Europa (6°–12°O)', proj4: '+proj=utm +zone=32 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs' },
  { code: 'EPSG:31370', name: 'Belge 1972 / Belgian Lambert 72', region: 'België', proj4: '+proj=lcc +lat_0=90 +lon_0=4.36748666666667 +lat_1=51.1666672333333 +lat_2=49.8333339 +x_0=150000.013 +y_0=5400088.438 +ellps=intl +towgs84=-106.8686,52.2978,-103.7239,0.3366,-0.457,1.8422,-1.2747 +units=m +no_defs' },
  { code: 'EPSG:3812', name: 'ETRS89 / Belgian Lambert 2008', region: 'België', proj4: '+proj=lcc +lat_0=50.797815 +lon_0=4.35921583333333 +lat_1=49.8333333333333 +lat_2=51.1666666666667 +x_0=649328 +y_0=665262 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs' },
  { code: 'EPSG:27700', name: 'OSGB36 / British National Grid', region: 'Verenigd Koninkrijk', proj4: '+proj=tmerc +lat_0=49 +lon_0=-2 +k=0.9996012717 +x_0=400000 +y_0=-100000 +ellps=airy +towgs84=446.448,-125.157,542.06,0.15,0.247,0.842,-20.489 +units=m +no_defs' },
  { code: 'EPSG:2056', name: 'CH1903+ / LV95', region: 'Zwitserland', proj4: '+proj=somerc +lat_0=46.9524055555556 +lon_0=7.43958333333333 +k_0=1 +x_0=2600000 +y_0=1200000 +ellps=bessel +towgs84=674.374,15.056,405.346,0,0,0,0 +units=m +no_defs' },
  { code: 'EPSG:2154', name: 'RGF93 / Lambert-93', region: 'Frankrijk', proj4: '+proj=lcc +lat_0=46.5 +lon_0=3 +lat_1=49 +lat_2=44 +x_0=700000 +y_0=6600000 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs' },
];

const known = new Map<string, CrsInfo>();
for (const c of BUILTIN) {
  proj4.defs(c.code, c.proj4);
  known.set(c.code, { code: c.code, name: c.name, region: c.region });
}

export function builtinCrsList(): CrsInfo[] {
  return BUILTIN.map(({ code, name, region }) => ({ code, name, region }));
}

export function normalizeCode(code: string): string {
  const m = /(\d{4,6})\s*$/.exec(code.trim());
  return m ? `EPSG:${m[1]}` : code.trim().toUpperCase();
}

/** UTM zone (WGS 84) for a lon/lat, incl. the Norway/Svalbard exceptions. */
export function utmCodeFor(lon: number, lat: number): string {
  let zone = Math.floor((lon + 180) / 6) + 1;
  if (lat >= 56 && lat < 64 && lon >= 3 && lon < 12) zone = 32;
  if (lat >= 72 && lat < 84) {
    if (lon >= 0 && lon < 9) zone = 31;
    else if (lon >= 9 && lon < 21) zone = 33;
    else if (lon >= 21 && lon < 33) zone = 35;
    else if (lon >= 33 && lon < 42) zone = 37;
  }
  zone = Math.min(Math.max(zone, 1), 60);
  return `EPSG:${lat >= 0 ? 326 : 327}${String(zone).padStart(2, '0')}`;
}

function defineUtm(code: string): boolean {
  const m = /^EPSG:32([67])(\d{2})$/.exec(code);
  if (!m) return false;
  const south = m[1] === '7';
  const zone = Number(m[2]);
  proj4.defs(code, `+proj=utm +zone=${zone}${south ? ' +south' : ''} +datum=WGS84 +units=m +no_defs`);
  known.set(code, { code, name: `WGS 84 / UTM zone ${zone}${south ? 'S' : 'N'}`, region: 'Wereld (UTM)' });
  return true;
}

/** Suggested CRS for a location: national grid where we know one, otherwise UTM. */
export function suggestCrs(lon: number, lat: number): string {
  if (lon > 3.2 && lon < 7.3 && lat > 50.7 && lat < 53.7) return 'EPSG:28992';
  if (lon > 2.5 && lon < 6.5 && lat > 49.4 && lat < 51.55) return 'EPSG:31370';
  if (lon > -8.7 && lon < 1.8 && lat > 49.8 && lat < 60.9) return 'EPSG:27700';
  if (lon > 5.9 && lon < 10.5 && lat > 45.8 && lat < 47.9) return 'EPSG:2056';
  if (lon > -5.2 && lon < 9.6 && lat > 41.3 && lat < 51.1) return 'EPSG:2154';
  return utmCodeFor(lon, lat);
}

export function isDefined(code: string): boolean {
  return !!proj4.defs(code);
}

/** Make sure proj4 knows the CRS; fetches unknown EPSG codes from epsg.io. */
export async function ensureCrs(rawCode: string): Promise<CrsInfo> {
  const code = normalizeCode(rawCode);
  if (known.has(code) && isDefined(code)) return known.get(code)!;
  if (defineUtm(code)) return known.get(code)!;
  const num = code.replace('EPSG:', '');
  const def = (await httpText(`https://epsg.io/${num}.proj4`)).trim();
  if (!def.startsWith('+')) throw new Error(`Onbekend coördinatenstelsel ${code}`);
  proj4.defs(code, def);
  const info: CrsInfo = { code, name: code, region: 'epsg.io' };
  try {
    const json = JSON.parse(await httpText(`https://epsg.io/${num}.json`));
    if (json?.name) info.name = json.name;
  } catch { /* name is cosmetic */ }
  known.set(code, info);
  return info;
}

export function crsInfo(code: string): CrsInfo {
  return known.get(normalizeCode(code)) ?? { code, name: code, region: '' };
}

export function isGeographic(code: string): boolean {
  const def = proj4.defs(normalizeCode(code)) as { projName?: string } | undefined;
  return def?.projName === 'longlat';
}

export type XY = [number, number];

export function transform(from: string, to: string, p: XY): XY {
  if (from === to) return p;
  return proj4(from, to, p) as XY;
}

export function transformer(from: string, to: string): (p: XY) => XY {
  if (from === to) return (p) => p;
  const t = proj4(from, to);
  return (p) => t.forward(p) as XY;
}
