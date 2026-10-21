import { useEffect, useState } from 'react';
import { Shapes } from 'lucide-react';
import { Modal, Button, Field, NumberInput, Status, css, errorText } from './kit';
import { ServerPicker, LayerList } from './ServerPicker';
import { closeDialog, openDialog } from './dialogState';
import { useSite, siteDrawingRect } from '../gis/site';
import { allServers, type MapServer } from '../gis/servers';
import { fetchWfsCapabilities, type WfsCapabilities, type WfsFeatureType } from '../gis/capabilities';
import { fetchWfsFeatures, type WfsResult } from '../gis/wfs';
import { dataTransform, featuresToShapes, propertyNames } from '../gis/vector';
import { ensureLayer, commit } from '../gis/place';

const LAST_KEY = 'open-infra:last-wfs';

/** Label fields that are useful for common Dutch datasets. */
const LABEL_HINTS = ['perceelnummer', 'huisnummer', 'wegnummer', 'stt_naam', 'naam', 'name', 'identificatie'];

export function WfsDialog() {
  const site = useSite();
  const [server, setServer] = useState<MapServer | null>(null);
  const [caps, setCaps] = useState<WfsCapabilities | null>(null);
  const [ft, setFt] = useState<WfsFeatureType | null>(null);
  const [filter, setFilter] = useState('');
  const [limit, setLimit] = useState(5000);
  const [result, setResult] = useState<WfsResult | null>(null);
  const [labelField, setLabelField] = useState('');
  const [labelHeight, setLabelHeight] = useState(1);
  const [color, setColor] = useState('#e5c07b');
  const [clip, setClip] = useState(true);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState<string | null>(null);

  const selectServer = async (s: MapServer) => {
    setServer(s); setCaps(null); setFt(null); setResult(null); setError(null); setFilter('');
    try { localStorage.setItem(LAST_KEY, s.id); } catch { /* ignore */ }
    setBusy(true); setMessage('Capabilities ophalen…');
    try {
      const c = await fetchWfsCapabilities(s.url);
      setCaps(c);
      setFt(c.featureTypes.find((f) => f.name === s.defaultLayer) ?? c.featureTypes[0] ?? null);
      setMessage(`${c.featureTypes.length} objecttypen (WFS ${c.version})`);
    } catch (err) {
      setError(errorText(err)); setMessage('');
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    let last: string | null = null;
    try { last = localStorage.getItem(LAST_KEY); } catch { /* ignore */ }
    const s = allServers().find((x) => x.id === last && x.kind === 'wfs');
    if (s) selectServer(s);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const fetchFeatures = async () => {
    if (!site || !server || !caps || !ft) return;
    setBusy(true); setError(null); setResult(null);
    try {
      const r = await fetchWfsFeatures(site, server, caps, ft, limit, setMessage);
      setResult(r);
      const props = propertyNames(r.features);
      setLabelField(LABEL_HINTS.find((h) => props.includes(h)) ?? '');
      setMessage(`${r.features.length} objecten opgehaald${r.truncated ? ' (limiet bereikt)' : ''}.`);
    } catch (err) {
      setError(errorText(err)); setMessage('');
    } finally {
      setBusy(false);
    }
  };

  const place = () => {
    if (!site || !result || !ft) return;
    try {
      const layerId = ensureLayer(`GIS-${ft.title || ft.name}`.replace(/[:/\\]/g, '-').slice(0, 60), color);
      const shapes = featuresToShapes(site, result.features, dataTransform(site, result.dataCrs, result.features), {
        layerId, color, labelField: labelField || undefined, labelHeight,
        clip: clip ? siteDrawingRect(site) : undefined,
      });
      commit(shapes);
      closeDialog();
    } catch (err) {
      setError(errorText(err));
    }
  };

  const props = result ? propertyNames(result.features) : [];

  return (
    <Modal
      title="Vectordata (WFS) importeren"
      icon={<Shapes size={14} />}
      width={600}
      onClose={closeDialog}
      footer={<>
        <Status busy={busy} message={message} error={error} />
        <Button onClick={closeDialog}>Sluiten</Button>
        <Button disabled={busy || !site || !ft} onClick={fetchFeatures}>Ophalen</Button>
        <Button primary disabled={busy || !result?.features.length} onClick={place}>Plaatsen</Button>
      </>}
    >
      {!site && (
        <div style={{ ...css.section, borderColor: '#e5c07b' }}>
          Nog geen projectlocatie ingesteld. <Button onClick={() => openDialog('location')}>Locatie instellen…</Button>
        </div>
      )}
      <ServerPicker kinds={['wfs']} value={server?.id ?? ''} onChange={selectServer} />
      {server?.attribution && <div style={css.muted}>Bron: {server.attribution}</div>}
      {caps && (
        <Field label={`Objecttype — ${caps.title || server?.name}`}>
          <input style={{ ...css.input, marginBottom: 4 }} placeholder="Filter…" value={filter} onChange={(e) => setFilter(e.target.value)} />
          <LayerList items={caps.featureTypes} value={ft?.name ?? ''} filter={filter} onChange={(f) => { setFt(f); setResult(null); }} height={150} />
        </Field>
      )}
      <div style={css.row}>
        <Field label="Max. aantal objecten"><NumberInput value={limit} step={1000} min={1} onChange={(v) => setLimit(Math.max(1, Math.round(v)))} /></Field>
        <Field label="Kleur"><input type="color" style={{ ...css.input, padding: 0, height: 24 }} value={color} onChange={(e) => setColor(e.target.value)} /></Field>
      </div>
      <label style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
        <input type="checkbox" checked={clip} onChange={(e) => setClip(e.target.checked)} />
        Bijsnijden op importgebied (anders volledige objecten die het gebied raken)
      </label>
      {result && (
        <div style={css.row}>
          <Field label="Label (attribuut)">
            <select style={css.input} value={labelField} onChange={(e) => setLabelField(e.target.value)}>
              <option value="">— geen label —</option>
              {props.map((p) => <option key={p}>{p}</option>)}
            </select>
          </Field>
          <Field label="Teksthoogte (m)"><NumberInput value={labelHeight} step={0.25} min={0.1} onChange={(v) => setLabelHeight(Math.max(0.05, v))} /></Field>
        </div>
      )}
      {site && <div style={css.muted}>Gebied {site.width} × {site.height} m rond “{site.label}”, geplaatst in {site.crs}.</div>}
    </Modal>
  );
}
