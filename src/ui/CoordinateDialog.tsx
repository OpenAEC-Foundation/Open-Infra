import { useMemo, useState } from 'react';
import { Crosshair } from 'lucide-react';
import { Modal, Button, Field, NumberInput, Status, css, errorText } from './kit';
import { closeDialog } from './dialogState';
import { fromDrawing, toDrawing, useSite } from '../gis/site';
import { builtinCrsList, crsInfo, ensureCrs, isGeographic, transform, utmCodeFor, type XY } from '../gis/crs';
import { useAppStore } from 'open-2d-studio';
import { ensureLayer, commit, point, text } from '../gis/place';

/**
 * Coordinate converter: enter a coordinate in any CRS, see it in the others,
 * and optionally place a marked point in the drawing. With a selected point
 * or shape, it reads that location from the drawing.
 */
export function CoordinateDialog() {
  const site = useSite();
  const [crs, setCrs] = useState(site?.crs ?? 'EPSG:4326');
  const [xy, setXy] = useState<XY>(() => (site ? [site.originE, site.originN] : [5.11, 52.09]));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const targets = useMemo(() => {
    const list = ['EPSG:4326', ...(site ? [site.crs] : []), 'EPSG:28992', 'EPSG:3857'];
    try {
      const ll = transform(crs, 'EPSG:4326', xy);
      list.push(utmCodeFor(ll[0], ll[1]));
    } catch { /* ignore */ }
    return Array.from(new Set(list));
  }, [crs, xy, site]);

  const rows = targets.map((code) => {
    try {
      const p = transform(crs, code, xy);
      const geo = isGeographic(code);
      return { code, name: crsInfo(code).name, a: p[0].toFixed(geo ? 8 : 3), b: p[1].toFixed(geo ? 8 : 3), geo };
    } catch {
      return { code, name: crsInfo(code).name, a: '—', b: '—', geo: false };
    }
  });

  const change = async (code: string) => {
    setBusy(true); setError(null);
    try {
      const info = await ensureCrs(code);
      setXy(transform(crs, info.code, xy));
      setCrs(info.code);
    } catch (err) {
      setError(errorText(err));
    } finally {
      setBusy(false);
    }
  };

  const fromSelection = () => {
    if (!site) return;
    const s = useAppStore.getState();
    const shape: any = s.shapes.find((sh) => sh.id === s.selectedShapeIds[0]);
    const p = shape?.center ?? shape?.position ?? shape?.start ?? shape?.points?.[0] ?? shape?.topLeft;
    if (!p) { setError('Selecteer eerst een punt, cirkel, tekst, lijn of polylijn.'); return; }
    setXy(transform(site.crs, crs, fromDrawing(site, p.x, p.y)));
    setError(null);
  };

  const place = () => {
    if (!site) return;
    try {
      const p = toDrawing(site, transform(crs, site.crs, xy));
      const layerId = ensureLayer('GIS-Coördinaten', '#ff6b6b');
      const [e, n] = transform(crs, site.crs, xy);
      commit([
        point(layerId, '#ff6b6b', p, 1000),
        text(layerId, '#ff6b6b', { x: p.x, y: p.y - 2500 }, `X ${e.toFixed(2)}  Y ${n.toFixed(2)}`, 1500),
      ], false);
    } catch (err) {
      setError(errorText(err));
    }
  };

  const geo = isGeographic(crs);
  const choices = Array.from(new Set([...builtinCrsList().map((c) => c.code), ...targets, crs]));

  return (
    <Modal
      title="Coördinaten omrekenen"
      icon={<Crosshair size={14} />}
      width={560}
      onClose={closeDialog}
      footer={<>
        <Status busy={busy} error={error} />
        <Button disabled={!site} onClick={fromSelection} title="Lees de coördinaat van het geselecteerde object">Uit selectie</Button>
        <Button disabled={!site} onClick={place}>Punt plaatsen</Button>
        <Button onClick={closeDialog}>Sluiten</Button>
      </>}
    >
      <div style={css.row}>
        <Field label="Invoerstelsel">
          <select style={css.input} value={crs} onChange={(e) => change(e.target.value)}>
            {choices.map((c) => <option key={c} value={c}>{c} — {crsInfo(c).name}</option>)}
          </select>
        </Field>
      </div>
      <div style={css.row}>
        <Field label={geo ? 'Lengtegraad (lon)' : 'X / Oost (m)'}><NumberInput value={xy[0]} step={geo ? 0.0001 : 1} onChange={(v) => setXy([v, xy[1]])} /></Field>
        <Field label={geo ? 'Breedtegraad (lat)' : 'Y / Noord (m)'}><NumberInput value={xy[1]} step={geo ? 0.0001 : 1} onChange={(v) => setXy([xy[0], v])} /></Field>
      </div>
      <table style={{ width: '100%', borderCollapse: 'collapse' }}>
        <tbody>
          {rows.map((r) => (
            <tr key={r.code} style={{ borderBottom: '1px solid var(--theme-border)' }}>
              <td style={{ padding: '4px 4px' }}>{r.code}<div style={css.muted}>{r.name}</div></td>
              <td style={{ textAlign: 'right', fontFamily: 'monospace' }}>{r.geo ? 'lon ' : 'X '}{r.a}</td>
              <td style={{ textAlign: 'right', fontFamily: 'monospace' }}>{r.geo ? 'lat ' : 'Y '}{r.b}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Modal>
  );
}
