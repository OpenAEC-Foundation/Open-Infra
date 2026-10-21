/**
 * Small UI kit for the GIS dialogs. Uses inline styles on the host's theme
 * variables: Tailwind in the host only contains classes it uses itself, so a
 * runtime-loaded extension cannot rely on arbitrary utility classes.
 */

import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import { X } from 'lucide-react';

export const css = {
  overlay: { position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.45)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 60 } as CSSProperties,
  modal: { background: 'var(--theme-surface)', border: '1px solid var(--theme-border)', boxShadow: '0 10px 30px rgba(0,0,0,0.35)', color: 'var(--theme-text)', display: 'flex', flexDirection: 'column', maxHeight: '88vh', fontSize: 12 } as CSSProperties,
  header: { display: 'flex', alignItems: 'center', borderBottom: '1px solid var(--theme-border)', userSelect: 'none', cursor: 'move' } as CSSProperties,
  title: { flex: 1, padding: '6px 10px', fontWeight: 600, display: 'flex', alignItems: 'center', gap: 6 } as CSSProperties,
  body: { padding: 12, overflow: 'auto', display: 'flex', flexDirection: 'column', gap: 10 } as CSSProperties,
  footer: { padding: '8px 12px', borderTop: '1px solid var(--theme-border)', display: 'flex', justifyContent: 'flex-end', gap: 8, alignItems: 'center' } as CSSProperties,
  input: { width: '100%', padding: '4px 6px', fontSize: 12, background: 'var(--theme-input-bg)', border: '1px solid var(--theme-border)', color: 'var(--theme-text)', boxSizing: 'border-box' } as CSSProperties,
  label: { color: 'var(--theme-text-dim)', fontSize: 11, marginBottom: 3, display: 'block' } as CSSProperties,
  section: { border: '1px solid var(--theme-border)', padding: 8, display: 'flex', flexDirection: 'column', gap: 8 } as CSSProperties,
  sectionTitle: { fontWeight: 600, fontSize: 11, textTransform: 'uppercase', letterSpacing: 0.4, color: 'var(--theme-text-dim)' } as CSSProperties,
  row: { display: 'flex', gap: 8, alignItems: 'flex-end' } as CSSProperties,
  muted: { color: 'var(--theme-text-muted)', fontSize: 11 } as CSSProperties,
  list: { border: '1px solid var(--theme-border)', background: 'var(--theme-input-bg)', overflow: 'auto' } as CSSProperties,
};

export function Modal({ title, icon, width = 520, onClose, children, footer }: {
  title: string; icon?: ReactNode; width?: number; onClose: () => void; children: ReactNode; footer?: ReactNode;
}) {
  const [pos, setPos] = useState({ x: 0, y: 0 });
  const drag = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    const onMove = (e: MouseEvent) => {
      if (drag.current) setPos({ x: e.clientX - drag.current.x, y: e.clientY - drag.current.y });
    };
    const onUp = () => { drag.current = null; };
    window.addEventListener('keydown', onKey);
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
  }, [onClose]);

  return (
    <div style={css.overlay} onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div style={{ ...css.modal, width, transform: `translate(${pos.x}px, ${pos.y}px)` }} onKeyDown={(e) => e.stopPropagation()}>
        <div style={css.header} onMouseDown={(e) => { drag.current = { x: e.clientX - pos.x, y: e.clientY - pos.y }; }}>
          <div style={css.title}>{icon}{title}</div>
          <button style={{ background: 'none', border: 'none', color: 'var(--theme-text-dim)', padding: '6px 8px', cursor: 'default' }} onClick={onClose} title="Sluiten">
            <X size={14} />
          </button>
        </div>
        <div style={css.body}>{children}</div>
        {footer && <div style={css.footer}>{footer}</div>}
      </div>
    </div>
  );
}

export function Button({ children, onClick, primary, disabled, title, style }: {
  children: ReactNode; onClick?: () => void; primary?: boolean; disabled?: boolean; title?: string; style?: CSSProperties;
}) {
  return (
    <button
      title={title}
      disabled={disabled}
      onClick={onClick}
      style={{
        padding: '4px 12px', fontSize: 12, cursor: 'default', whiteSpace: 'nowrap',
        border: primary ? '1px solid var(--theme-accent)' : '1px solid var(--theme-border)',
        background: primary ? 'var(--theme-accent)' : 'var(--theme-input-bg)',
        color: primary ? '#fff' : 'var(--theme-text)',
        opacity: disabled ? 0.5 : 1,
        ...style,
      }}
    >
      {children}
    </button>
  );
}

export function Field({ label, children, style }: { label: string; children: ReactNode; style?: CSSProperties }) {
  return (
    <label style={{ display: 'block', flex: 1, ...style }}>
      <span style={css.label}>{label}</span>
      {children}
    </label>
  );
}

export function NumberInput({ value, onChange, step, min }: { value: number; onChange: (v: number) => void; step?: number; min?: number }) {
  const [draft, setDraft] = useState(String(value));
  useEffect(() => { if (Number(draft) !== value) setDraft(String(value)); }, [value]); // eslint-disable-line react-hooks/exhaustive-deps
  return (
    <input
      style={css.input}
      type="number"
      step={step}
      min={min}
      value={draft}
      onChange={(e) => {
        setDraft(e.target.value);
        const n = Number(e.target.value.replace(',', '.'));
        if (e.target.value !== '' && Number.isFinite(n)) onChange(n);
      }}
    />
  );
}

export function Status({ busy, message, error }: { busy?: boolean; message?: string; error?: string | null }) {
  if (error) return <div style={{ color: '#e06c75', fontSize: 11, flex: 1 }}>{error}</div>;
  if (!message) return <div style={{ flex: 1 }} />;
  return <div style={{ ...css.muted, flex: 1 }}>{busy ? '⏳ ' : ''}{message}</div>;
}

export function errorText(err: unknown): string {
  const msg = err instanceof Error ? err.message : String(err);
  if (/Failed to fetch|NetworkError|Load failed/i.test(msg)) {
    return `${msg} — server onbereikbaar of blokkeert CORS (in de desktop-app werkt dit wel).`;
  }
  return msg;
}
