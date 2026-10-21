/** Which GIS dialog is open — module state shared by the ribbon tab and the dialog host. */

import { useSyncExternalStore } from 'react';

export type GisDialog = 'location' | 'wms' | 'wfs' | 'osm' | 'servers' | 'coords';

// A stack, so e.g. the server manager opened from the WMS dialog returns to it.
let stack: GisDialog[] = [];
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((l) => l());

export function openDialog(d: GisDialog, { over = false } = {}): void {
  stack = over ? [...stack, d] : [d];
  emit();
}

export function closeDialog(): void {
  stack = stack.slice(0, -1);
  emit();
}

/** All open dialogs, bottom to top (the top one is interactive). */
export function useDialogStack(): GisDialog[] {
  return useSyncExternalStore(
    (cb) => { listeners.add(cb); return () => listeners.delete(cb); },
    () => stack,
  );
}
