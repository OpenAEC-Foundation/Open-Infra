import { useMemo, useState } from 'react';
import { MapPin, Search } from 'lucide-react';
import { Modal, Button, Field, NumberInput, Status, css, errorText } from './kit';
import { closeDialog } from './dialogState';
import { getSite, defaultSite, setSite, siteBboxLonLat, type GisSite } from '../gis/site';
import { builtinCrsList, crsInfo, ensureCrs, isGeographic, suggestCrs, transform } from '../gis/crs';
import { geocode, type GeocodeResult } from '../gis/geocode';

const fmt = (v: number, d: number) => v.toFixed(d);

export function LocationDialog() {
  const [site, setDraft] = useState<GisSite>(() => getSite() ?? withProjected(defaultSite()));
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<GeocodeResult[]>([]);
  const [customCode, setCustomCode] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState<string | null>(null);

  const crsOptions = useMemo(() => {
    const list = builtinCrsList();
    const suggested = suggestCrs(site.lon, site.lat);
    for (const code of [suggested, site.crs]) {
      if (!list.some((c) => c.code === code)) list.push(crsInfo(code));
    }
    return list;
  }, [site.lon, site.lat, site.crs]);

  const run = async (label: string, fn: () => Promise<void>) => {
    setBusy(true); setError(null); setMessage(label);
    try { await fn(); setMessage(''); } catch (err) { setError(errorText(err)); } finally { setBusy(false); }
  };

  const search = () => run('Zoeken…', async () => {
    const r = await geocode(query);
    setResults(r);
    if (!r.length) throw new Error('Geen resultaten');
  });

  const pick = (r: GeocodeResult) => run('Coördinatenstelsel laden…', async () => {
    const crs = suggestCrs(r.lon, r.lat);
    await ensureCrs(crs);
    setDraft((s) => withProjected({ ...s, label: r.label, lon: r.lon, lat: r.lat, crs }));
    setResults([]);
  });

  const changeCrs = (code: string) => run('Coördinatenstelsel laden…', async () => {
    const info = await ensureCrs(code);
    if (isGeographic(info.code)) throw new Error('Kies een geprojecteerd stelsel (in meters) als projectstelsel.');
    setDraft((s) => withProjected({ ...s, crs: info.code }));
  });

  const setLonLat = (lon: number, lat: number) => setDraft((s) => withProjected({ ...s, lon, lat }));
  const setProjected = (e: number, n: number) => setDraft((s) => {
    const [lon, lat] = transform(s.crs, 'EPSG:4326', [e, n]);
    return { ...s, originE: e, originN: n, lon, lat };
  });

  const bboxLL = siteBboxLonLat(site);

  return (
    <Modal
      title="Projectlocatie en coördinatenstelsel"
      icon={<MapPin size={14} />}
      width={560}
      onClose={closeDialog}
      footer={<>
        <Status busy={busy} message={message} error={error} />
        <Button onClick={closeDialog}>Annuleren</Button>
        <Button primary disabled={busy} onClick={() => { setSite(site); closeDialog(); }}>Opslaan</Button>
      </>}
    >
      <div style={css.section}>
        <div style={css.sectionTitle}>Zoeken (adres, plaats, perceel of "lat, lon")</div>
        <div style={css.row}>
          <input
            style={css.input}
            value={query}
            placeholder="bijv. Stationsplein 1 Utrecht, Rotterdam Maasvlakte, Paris, 52.09, 5.11"
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => { if (e.key === 'Enter' && query.trim()) search(); }}
          />
          <Button onClick={search} disabled={busy || !query.trim()}><Search size={12} /></Button>
        </div>
        {results.length > 0 && (
          <div style={{ ...css.list, maxHeight: 160 }}>
            {results.map((r, i) => (
              <div
                key={i}
                onClick={() => pick(r)}
                style={{ padding: '4px 6px', borderBottom: '1px solid var(--theme-border)', cursor: 'default' }}
                onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--theme-hover)')}
                onMouseLeave={(e) => (e.currentTarget.style.background = '')}
              >
                <span style={{ ...css.muted, marginRight: 6 }}>[{r.source}]</span>{r.label}
              </div>
            ))}
          </div>
        )}
        <div style={css.muted}>Nederland via PDOK Locatieserver, wereldwijd via OpenStreetMap Nominatim.</div>
      </div>

      <div style={css.section}>
        <div style={css.sectionTitle}>Locatie</div>
        <Field label="Omschrijving">
          <input style={css.input} value={site.label} onChange={(e) => setDraft({ ...site, label: e.target.value })} />
        </Field>
        <div style={css.row}>
          <Field label="Breedtegraad (lat, WGS 84)"><NumberInput value={Number(fmt(site.lat, 7))} step={0.0001} onChange={(v) => setLonLat(site.lon, v)} /></Field>
          <Field label="Lengtegraad (lon, WGS 84)"><NumberInput value={Number(fmt(site.lon, 7))} step={0.0001} onChange={(v) => setLonLat(v, site.lat)} /></Field>
        </div>
      </div>

      <div style={css.section}>
        <div style={css.sectionTitle}>Coördinatenstelsel (CRS)</div>
        <div style={css.row}>
          <Field label="Projectstelsel">
            <select style={css.input} value={site.crs} onChange={(e) => changeCrs(e.target.value)}>
              {crsOptions.map((c) => (
                <option key={c.code} value={c.code}>
                  {c.code} — {c.name}{c.region ? ` (${c.region})` : ''}{c.code === suggestCrs(site.lon, site.lat) ? ' ★' : ''}
                </option>
              ))}
            </select>
          </Field>
        </div>
        <div style={css.row}>
          <Field label="Ander EPSG-stelsel (wordt opgehaald via epsg.io)">
            <input style={css.input} value={customCode} placeholder="bijv. 25833, 3035, 32631" onChange={(e) => setCustomCode(e.target.value)} />
          </Field>
          <Button disabled={busy || !customCode.trim()} onClick={() => changeCrs(customCode)}>Laden</Button>
        </div>
        <div style={css.muted}>★ = aanbevolen voor deze locatie (nationaal stelsel waar bekend, anders UTM).</div>
        <div style={css.row}>
          <Field label="Basispunt X / Oost (m)"><NumberInput value={Number(fmt(site.originE, 3))} step={1} onChange={(v) => setProjected(v, site.originN)} /></Field>
          <Field label="Basispunt Y / Noord (m)"><NumberInput value={Number(fmt(site.originN, 3))} step={1} onChange={(v) => setProjected(site.originE, v)} /></Field>
        </div>
      </div>

      <div style={css.section}>
        <div style={css.sectionTitle}>Importgebied en plaatsing</div>
        <div style={css.row}>
          <Field label="Breedte (m)"><NumberInput value={site.width} min={10} step={50} onChange={(v) => setDraft({ ...site, width: Math.max(1, v) })} /></Field>
          <Field label="Hoogte (m)"><NumberInput value={site.height} min={10} step={50} onChange={(v) => setDraft({ ...site, height: Math.max(1, v) })} /></Field>
        </div>
        <label style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
          <input type="radio" checked={!site.absolute} onChange={() => setDraft({ ...site, absolute: false })} />
          Relatief t.o.v. basispunt (basispunt = 0,0 in de tekening) — aanbevolen
        </label>
        <label style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
          <input type="radio" checked={site.absolute} onChange={() => setDraft({ ...site, absolute: true })} />
          Absolute coördinaten (bijv. RD-coördinaten direct in de tekening)
        </label>
        <div style={css.muted}>
          Tekeningeenheid: millimeter (1 m in het stelsel = 1000 mm). Gebied in WGS 84:
          {' '}{fmt(bboxLL[1], 5)}, {fmt(bboxLL[0], 5)} → {fmt(bboxLL[3], 5)}, {fmt(bboxLL[2], 5)}
        </div>
      </div>
    </Modal>
  );
}

function withProjected(s: GisSite): GisSite {
  try {
    const [e, n] = transform('EPSG:4326', s.crs, [s.lon, s.lat]);
    return { ...s, originE: Math.round(e * 1000) / 1000, originN: Math.round(n * 1000) / 1000 };
  } catch {
    return s;
  }
}
