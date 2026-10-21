/**
 * WFS GetFeature — GeoJSON in the project CRS when the server offers it,
 * otherwise in WGS 84, with paging (startIndex) for servers with a page limit.
 */

import { httpText, serviceError } from './http';
import { siteBbox, siteBboxLonLat, type GisSite } from './site';
import { pickGeoJsonFormat, type WfsCapabilities, type WfsFeatureType } from './capabilities';
import { withParams, type MapServer } from './servers';
import type { Feature } from './vector';

export interface WfsResult {
  features: Feature[];
  dataCrs: string;
  truncated: boolean;
}

export async function fetchWfsFeatures(
  site: GisSite,
  server: MapServer,
  caps: WfsCapabilities,
  ft: WfsFeatureType,
  limit: number,
  onProgress?: (msg: string) => void,
): Promise<WfsResult> {
  const v2 = caps.version.startsWith('2');
  const useSite = !ft.crs.length || ft.crs.includes(site.crs);
  const dataCrs = useSite ? site.crs : 'EPSG:4326';
  let bbox: string;
  if (useSite) {
    bbox = [...siteBbox(site), site.crs].join(',');
  } else {
    const [w, s, e, n] = siteBboxLonLat(site);
    // urn form = lat/lon axis order (WFS 1.1 / 2.0).
    bbox = [s, w, n, e, 'urn:ogc:def:crs:EPSG::4326'].join(',');
  }
  let base = caps.getFeatureUrl || server.url;
  if (server.url.startsWith('https:') && base.startsWith('http:')) base = 'https:' + base.slice(5);

  const pageSize = Math.min(limit, 1000);
  const features: Feature[] = [];
  let truncated = false;
  for (let start = 0; start < limit; start += pageSize) {
    const want = Math.min(pageSize, limit - start);
    const params: Record<string, string | number> = {
      SERVICE: 'WFS',
      VERSION: v2 ? '2.0.0' : '1.1.0',
      REQUEST: 'GetFeature',
      [v2 ? 'TYPENAMES' : 'TYPENAME']: ft.name,
      SRSNAME: dataCrs,
      BBOX: bbox,
      OUTPUTFORMAT: pickGeoJsonFormat(caps.outputFormats),
      [v2 ? 'COUNT' : 'MAXFEATURES']: want,
    };
    if (start > 0) params.STARTINDEX = start;
    onProgress?.(`Objecten ophalen… (${features.length})`);
    const body = await httpText(withParams(base, params));
    let json: { features?: Feature[] };
    try {
      json = JSON.parse(body);
    } catch {
      throw new Error(serviceError(body) || 'Server gaf geen GeoJSON terug');
    }
    const page = json.features ?? [];
    features.push(...page);
    if (page.length < want) break;
    if (start + pageSize >= limit) truncated = true;
    // WFS 1.x has no reliable paging; stop after the first page.
    if (!v2) { truncated = page.length >= pageSize; break; }
  }
  return { features, dataCrs, truncated };
}
