import { css, Field } from './kit';
import { useServers, type MapServer, type ServerKind } from '../gis/servers';
import { openDialog } from './dialogState';

/** Server dropdown grouped by region, with a shortcut to the server manager. */
export function ServerPicker({ kinds, value, onChange }: { kinds: ServerKind[]; value: string; onChange: (s: MapServer) => void }) {
  const servers = useServers(kinds);
  const regions = Array.from(new Set(servers.map((s) => s.region)));
  return (
    <div style={css.row}>
      <Field label="Server">
        <select
          style={css.input}
          value={value}
          onChange={(e) => {
            const s = servers.find((x) => x.id === e.target.value);
            if (s) onChange(s);
          }}
        >
          <option value="">— kies een server —</option>
          {regions.map((r) => (
            <optgroup key={r} label={r}>
              {servers.filter((s) => s.region === r).map((s) => (
                <option key={s.id} value={s.id}>{s.name}{s.kind === 'xyz' ? ' [tegels]' : ''}</option>
              ))}
            </optgroup>
          ))}
        </select>
      </Field>
      <button
        style={{ ...css.input, width: 'auto', cursor: 'default' }}
        title="Servers beheren / eigen server toevoegen"
        onClick={() => openDialog('servers', { over: true })}
      >
        Beheer…
      </button>
    </div>
  );
}

/** Filterable list of layer names/titles. */
export function LayerList<T extends { name: string; title: string }>({ items, value, onChange, filter, height = 180, badge }: {
  items: T[]; value: string; onChange: (item: T) => void; filter: string; height?: number; badge?: (item: T) => string | null;
}) {
  const f = filter.trim().toLowerCase();
  const shown = f ? items.filter((i) => i.name.toLowerCase().includes(f) || i.title.toLowerCase().includes(f)) : items;
  return (
    <div style={{ ...css.list, height }}>
      {shown.map((i) => {
        const selected = i.name === value;
        const b = badge?.(i);
        return (
          <div
            key={i.name}
            onClick={() => onChange(i)}
            style={{
              padding: '3px 6px', cursor: 'default', borderBottom: '1px solid var(--theme-border)',
              background: selected ? 'var(--theme-accent)' : undefined, color: selected ? '#fff' : undefined,
            }}
          >
            {i.title}
            {i.title !== i.name && <span style={{ opacity: 0.6, marginLeft: 6 }}>({i.name})</span>}
            {b && <span style={{ float: 'right', opacity: 0.7, fontSize: 10 }}>{b}</span>}
          </div>
        );
      })}
      {!shown.length && <div style={{ ...css.muted, padding: 6 }}>Geen lagen</div>}
    </div>
  );
}
