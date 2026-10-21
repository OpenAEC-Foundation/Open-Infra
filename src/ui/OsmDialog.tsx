import { useState } from 'react';
import { Globe } from 'lucide-react';
import { Modal, Button, Field, NumberInput, Status, css, errorText } from './kit';
import { closeDialog, openDialog } from './dialogState';
import { useSite, siteDrawingRect } from '../gis/site';
import { OSM_CATEGORIES, fetchOsm } from '../gis/osm';
import { dataTransform, featuresToShapes } from '../gis/vector';
import { ensureLayer, commit } from '../gis/place';
import type { Shape } from 'open-2d-studio';

export function OsmDialog() {
  const site = useSite();
  const [selected, setSelected] = useState(() => new Set(OSM_CATEGORIES.filter((c) => c.defaultOn).map((c) => c.id)));
  const [labels, setLabels] = useState(true);
  const [labelHeight, setLabelHeight] = useState(2);
  const [clip, setClip] = useState(true);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState<string | null>(null);

  const toggle = (id: string) => setSelected((s) => {
    const n = new Set(s);
    if (n.has(id)) n.delete(id); else n.add(id);
    return n;
  });

  const run = async () => {
    if (!site) return;
    setBusy(true); setError(null);
    const all: Shape[] = [];
    const summary: string[] = [];
    try {
      const cats = OSM_CATEGORIES.filter((c) => selected.has(c.id));
      setMessage('OpenStreetMap-data ophalen (Overpass, kan even duren)…');
      const byCat = await fetchOsm(site, cats);
      for (const cat of cats) {
        const features = byCat.get(cat.id) ?? [];
        summary.push(`${cat.label}: ${features.length}`);
        if (!features.length) continue;
        const layerId = ensureLayer(cat.layer, cat.color);
        all.push(...featuresToShapes(site, features, dataTransform(site, 'EPSG:4326', features), {
          layerId, color: cat.color, labelField: labels ? cat.labelTag : undefined, labelHeight,
          clip: clip ? siteDrawingRect(site) : undefined,
        }));
      }
      commit(all);
      setMessage(summary.join(' · '));
      closeDialog();
    } catch (err) {
      if (all.length) commit(all);
      setError(errorText(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title="OpenStreetMap importeren (wereldwijd)"
      icon={<Globe size={14} />}
      width={480}
      onClose={closeDialog}
      footer={<>
        <Status busy={busy} message={message} error={error} />
        <Button onClick={closeDialog}>Sluiten</Button>
        <Button primary disabled={busy || !site || !selected.size} onClick={run}>Importeren</Button>
      </>}
    >
      {!site && (
        <div style={{ ...css.section, borderColor: '#e5c07b' }}>
          Nog geen projectlocatie ingesteld. <Button onClick={() => openDialog('location')}>Locatie instellen…</Button>
        </div>
      )}
      <div style={css.section}>
        <div style={css.sectionTitle}>Thema's (elk op een eigen laag)</div>
        {OSM_CATEGORIES.map((c) => (
          <label key={c.id} style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
            <input type="checkbox" checked={selected.has(c.id)} onChange={() => toggle(c.id)} />
            <span style={{ width: 10, height: 10, background: c.color, display: 'inline-block' }} />
            {c.label}
            <span style={{ ...css.muted, marginLeft: 'auto' }}>{c.layer}</span>
          </label>
        ))}
      </div>
      <div style={css.row}>
        <label style={{ display: 'flex', gap: 6, alignItems: 'center', flex: 1 }}>
          <input type="checkbox" checked={labels} onChange={(e) => setLabels(e.target.checked)} />
          Namen / huisnummers als tekst
        </label>
        <label style={{ display: 'flex', gap: 6, alignItems: 'center', flex: 1 }}>
          <input type="checkbox" checked={clip} onChange={(e) => setClip(e.target.checked)} />
          Bijsnijden op importgebied
        </label>
        <Field label="Teksthoogte (m)"><NumberInput value={labelHeight} step={0.5} min={0.1} onChange={(v) => setLabelHeight(Math.max(0.1, v))} /></Field>
      </div>
      {site && <div style={css.muted}>Gebied {site.width} × {site.height} m rond “{site.label}”, geplaatst in {site.crs}. Data © OpenStreetMap-bijdragers (ODbL), via Overpass API.</div>}
    </Modal>
  );
}
