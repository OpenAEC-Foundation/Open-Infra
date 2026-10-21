import { useState } from 'react';
import { Server, Trash2 } from 'lucide-react';
import { Modal, Button, Field, Status, css, errorText } from './kit';
import { closeDialog } from './dialogState';
import { addServer, cleanServiceUrl, removeServer, useServers, type ServerKind } from '../gis/servers';
import { fetchWfsCapabilities, fetchWmsCapabilities } from '../gis/capabilities';

export function ServerManagerDialog() {
  const servers = useServers();
  const [kind, setKind] = useState<ServerKind>('wms');
  const [url, setUrl] = useState('');
  const [name, setName] = useState('');
  const [region, setRegion] = useState('Eigen servers');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState<string | null>(null);

  const add = async () => {
    setBusy(true); setError(null);
    try {
      let finalUrl = url.trim();
      let title = name.trim();
      if (kind === 'xyz') {
        if (!/\{z\}.*\{x\}.*\{y\}/.test(finalUrl)) throw new Error('Tegel-URL moet {z}, {x} en {y} bevatten');
      } else {
        finalUrl = cleanServiceUrl(finalUrl);
        setMessage('Server testen (GetCapabilities)…');
        const caps = kind === 'wms' ? await fetchWmsCapabilities(finalUrl) : await fetchWfsCapabilities(finalUrl);
        title ||= caps.title;
      }
      addServer({ kind, url: finalUrl, name: title || finalUrl, region: region.trim() || 'Eigen servers' });
      setUrl(''); setName('');
      setMessage('Server toegevoegd.');
    } catch (err) {
      setError(errorText(err)); setMessage('');
    } finally {
      setBusy(false);
    }
  };

  const regions = Array.from(new Set(servers.map((s) => s.region)));

  return (
    <Modal
      title="Serverbeheer (WMS / WFS / tegels)"
      icon={<Server size={14} />}
      width={660}
      onClose={closeDialog}
      footer={<>
        <Status busy={busy} message={message} error={error} />
        <Button onClick={closeDialog}>Sluiten</Button>
      </>}
    >
      <div style={css.section}>
        <div style={css.sectionTitle}>Server toevoegen</div>
        <div style={css.row}>
          <Field label="Type" style={{ flex: '0 0 110px' }}>
            <select style={css.input} value={kind} onChange={(e) => setKind(e.target.value as ServerKind)}>
              <option value="wms">WMS</option>
              <option value="wfs">WFS</option>
              <option value="xyz">XYZ-tegels</option>
            </select>
          </Field>
          <Field label={kind === 'xyz' ? 'Tegel-URL ({z}/{x}/{y})' : 'Service-URL (GetCapabilities wordt getest)'}>
            <input style={css.input} value={url} placeholder={kind === 'xyz' ? 'https://…/{z}/{x}/{y}.png' : 'https://…/wms'} onChange={(e) => setUrl(e.target.value)} />
          </Field>
        </div>
        <div style={css.row}>
          <Field label="Naam (leeg = titel uit capabilities)"><input style={css.input} value={name} onChange={(e) => setName(e.target.value)} /></Field>
          <Field label="Groep / regio"><input style={css.input} value={region} onChange={(e) => setRegion(e.target.value)} /></Field>
          <Button primary disabled={busy || !url.trim()} onClick={add}>Toevoegen</Button>
        </div>
      </div>

      <div style={{ ...css.list, maxHeight: 340 }}>
        {regions.map((r) => (
          <div key={r}>
            <div style={{ ...css.sectionTitle, padding: '6px 6px 2px', background: 'var(--theme-surface)' }}>{r}</div>
            {servers.filter((s) => s.region === r).map((s) => (
              <div key={s.id} style={{ display: 'flex', gap: 8, alignItems: 'center', padding: '3px 6px', borderBottom: '1px solid var(--theme-border)' }}>
                <span style={{ width: 34, fontSize: 10, color: 'var(--theme-text-dim)' }}>{s.kind.toUpperCase()}</span>
                <span style={{ flex: 1, minWidth: 0 }}>
                  {s.name}
                  <div style={{ ...css.muted, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{s.url}</div>
                </span>
                {s.builtin
                  ? <span style={css.muted}>standaard</span>
                  : <button title="Verwijderen" style={{ background: 'none', border: 'none', color: 'var(--theme-text-dim)' }} onClick={() => removeServer(s.id)}><Trash2 size={13} /></button>}
              </div>
            ))}
          </div>
        ))}
      </div>
    </Modal>
  );
}
