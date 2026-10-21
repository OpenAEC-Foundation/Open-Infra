/**
 * Address search — PDOK Locatieserver for Dutch addresses and parcels,
 * OpenStreetMap Nominatim for the rest of the world.
 */

import { httpJson } from './http';

export interface GeocodeResult {
  label: string;
  lon: number;
  lat: number;
  source: 'PDOK' | 'OSM';
  countryCode?: string;
}

async function pdok(q: string): Promise<GeocodeResult[]> {
  const url = `https://api.pdok.nl/bzk/locatieserver/search/v3_1/free?rows=6&fl=weergavenaam,centroide_ll,type&q=${encodeURIComponent(q)}`;
  const json = await httpJson<{ response: { docs: Array<{ weergavenaam: string; centroide_ll: string; type: string }> } }>(url);
  return json.response.docs.flatMap((d) => {
    const m = /POINT\(([-\d.]+) ([-\d.]+)\)/.exec(d.centroide_ll);
    return m ? [{ label: `${d.weergavenaam} (${d.type})`, lon: Number(m[1]), lat: Number(m[2]), source: 'PDOK' as const }] : [];
  });
}

async function nominatim(q: string): Promise<GeocodeResult[]> {
  const url = `https://nominatim.openstreetmap.org/search?format=jsonv2&addressdetails=1&limit=6&q=${encodeURIComponent(q)}`;
  const json = await httpJson<Array<{ display_name: string; lon: string; lat: string; address?: { country_code?: string } }>>(url, {
    headers: { 'Accept-Language': 'nl,en' },
  });
  return json.map((r) => ({
    label: r.display_name, lon: Number(r.lon), lat: Number(r.lat), source: 'OSM' as const, countryCode: r.address?.country_code,
  }));
}

/** Coordinates typed directly: "52.09, 5.11" (lat, lon). */
function parseLatLon(q: string): GeocodeResult | null {
  const m = /^\s*(-?\d+(?:\.\d+)?)\s*[,; ]\s*(-?\d+(?:\.\d+)?)\s*$/.exec(q);
  if (!m) return null;
  const lat = Number(m[1]);
  const lon = Number(m[2]);
  if (Math.abs(lat) > 90 || Math.abs(lon) > 180) return null;
  return { label: `${lat}, ${lon}`, lon, lat, source: 'OSM' };
}

export async function geocode(q: string): Promise<GeocodeResult[]> {
  const direct = parseLatLon(q);
  if (direct) return [direct];
  const [nl, world] = await Promise.allSettled([pdok(q), nominatim(q)]);
  const nlHits = nl.status === 'fulfilled' ? nl.value : [];
  const worldHits = world.status === 'fulfilled' ? world.value : [];
  // PDOK always returns (fuzzy) Dutch matches; list them first only when the
  // worldwide search also points to the Netherlands.
  const dutch = !worldHits.length || worldHits[0].countryCode === 'nl';
  const out = dutch ? [...nlHits, ...worldHits] : [...worldHits, ...nlHits];
  if (!out.length && nl.status === 'rejected' && world.status === 'rejected') {
    throw new Error('Zoekdiensten niet bereikbaar');
  }
  return out;
}
