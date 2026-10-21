import { useEffect, useState } from 'react';
import { Image as ImageIcon } from 'lucide-react';
import { Modal, Button, Field, NumberInput, Status, css, errorText } from './kit';
import { ServerPicker, LayerList } from './ServerPicker';
import { closeDialog, openDialog } from './dialogState';
import { useSite } from '../gis/site';
import { allServers, type MapServer } from '../gis/servers';
import { fetchWmsCapabilities, type WmsCapabilities, type WmsLayer } from '../gis/capabilities';
import { defaultFormat, importWms, importXyz } from '../gis/raster';

const LAST_KEY = 'open-infra:last-wms';

export function WmsDialog() {
  const site = useSite();
  const [server, setServer] = useState<MapServer | null>(null);
  const [caps, setCaps] = useState<WmsCapabilities | null>(null);
  const [layer, setLayer] = useState<WmsLayer | null>(null);
  const [filter, setFilter] = useState('');
  const [format, setFormat] = useState('image/png');
  const [resolution, setResolution] = useState(0.25);
  const [opacity, setOpacity] = useState(100);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState<string | null>(null);

  const selectServer = async (s: MapServer) => {
    setServer(s); setCaps(null); setLayer(null); setError(null); setFilter('');
    try { localStorage.setItem(LAST_KEY, s.id); } catch { /* ignore */ }
    if (s.kind === 'xyz') { setResolution(0.5); setMessage(''); return; }
    setBusy(true); setMessage('Capabilities ophalen…');
    try {
      const c = await fetchWmsCapabilities(s.url);
      setCaps(c);
      const pre = c.layers.find((l) => l.name === s.defaultLayer) ?? c.layers[0] ?? null;
      setLayer(pre);
      if (pre) setFormat(defaultFormat(c, pre.name));
      setMessage(`${c.layers.length} lagen (WMS ${c.version})`);
    } catch (err) {
      setError(errorText(err)); setMessage('');
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    let last: string | null = null;
    try { last = localStorage.getItem(LAST_KEY); } catch { /* ignore */ }
    const s = allServers().find((x) => x.id === last && x.kind !== 'wfs');
    if (s) selectServer(s);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const px = site ? Math.round(site.width / resolution) * Math.round(site.height / resolution) : 0;
  const tooBig = px > 64e6;

  const run = async () => {
    if (!site || !server) return;
    setBusy(true); setError(null);
    try {
      const opts = { resolution, opacity: opacity / 100, format };
      if (server.kind === 'xyz') {
        const n = await importXyz(site, server, opts, setMessage);
        setMessage(`${n} tegels geplaatst.`);
      } else if (caps && layer) {
        const n = await importWms(site, server, caps, layer, opts, setMessage);
        setMessage(`${n} kaartdeel/delen geplaatst als onderlegger.`);
      }
      closeDialog();
    } catch (err) {
      setError(errorText(err));
    } finally {
      setBusy(false);
    }
  };

  const reprojectNote = layer && site && layer.crs.length && !layer.crs.includes(site.crs)
    ? `Deze laag ondersteunt ${site.crs} niet; wordt opgehaald in ${layer.crs.includes('EPSG:3857') ? 'EPSG:3857' : layer.crs[0]} en geherprojecteerd.`
    : null;

  return (
    <Modal
      title="Kaartlaag (WMS / tegels) als onderlegger"
      icon={<ImageIcon size={14} />}
      width={600}
      onClose={closeDialog}
      footer={<>
        <Status busy={busy} message={message} error={error} />
        <Button onClick={closeDialog}>Sluiten</Button>
        <Button primary disabled={busy || !site || !server || (server.kind !== 'xyz' && !layer) || tooBig} onClick={run}>Importeren</Button>
      </>}
    >
      {!site && (
        <div style={{ ...css.section, borderColor: '#e5c07b' }}>
          Nog geen projectlocatie ingesteld. <Button onClick={() => openDialog('location')}>Locatie instellen…</Button>
        </div>
      )}
      <ServerPicker kinds={['wms', 'xyz']} value={server?.id ?? ''} onChange={selectServer} />
      {server?.attribution && <div style={css.muted}>Bron: {server.attribution}</div>}

      {server?.kind === 'wms' && caps && (
        <>
          <Field label={`Laag — ${caps.title || server.name}`}>
            <input style={{ ...css.input, marginBottom: 4 }} placeholder="Filter lagen…" value={filter} onChange={(e) => setFilter(e.target.value)} />
            <LayerList
              items={caps.layers}
              value={layer?.name ?? ''}
              filter={filter}
              onChange={(l) => { setLayer(l); setFormat(defaultFormat(caps, l.name)); }}
              badge={(l) => (site && l.crs.length && !l.crs.includes(site.crs) ? 'herprojectie' : null)}
            />
          </Field>
          {reprojectNote && <div style={css.muted}>{reprojectNote}</div>}
          <div style={css.row}>
            <Field label="Formaat">
              <select style={css.input} value={format} onChange={(e) => setFormat(e.target.value)}>
                {(caps.formats.length ? caps.formats : ['image/png', 'image/jpeg']).filter((f) => f.startsWith('image/')).map((f) => <option key={f}>{f}</option>)}
              </select>
            </Field>
          </div>
        </>
      )}

      {server && (
        <div style={css.row}>
          <Field label="Resolutie (m per pixel)"><NumberInput value={resolution} step={0.05} min={0.05} onChange={(v) => setResolution(Math.max(0.02, v))} /></Field>
          <Field label="Dekking (%)"><NumberInput value={opacity} step={10} min={5} onChange={(v) => setOpacity(Math.min(100, Math.max(5, v)))} /></Field>
        </div>
      )}
      {site && server && (
        <div style={{ ...css.muted, color: tooBig ? '#e06c75' : undefined }}>
          Gebied {site.width} × {site.height} m rond “{site.label}” in {site.crs} → {Math.round(site.width / resolution)} × {Math.round(site.height / resolution)} px
          {tooBig ? ' — te groot, verhoog de resolutie of verklein het gebied.' : ''}
        </div>
      )}
    </Modal>
  );
}
