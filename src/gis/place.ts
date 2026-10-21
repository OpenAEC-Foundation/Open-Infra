/**
 * Placing GIS data in the drawing: layers, polylines, texts and raster underlays.
 */

import { useAppStore, generateId } from 'open-2d-studio';
import type { Shape, ImageShape, Point } from 'open-2d-studio';

/** Find a layer by name in the active drawing, or create it with the given colour. */
export function ensureLayer(name: string, color: string): string {
  const store = useAppStore.getState();
  const existing = store.layers.find((l) => l.drawingId === store.activeDrawingId && l.name === name);
  if (existing) return existing.id;
  const previousActive = store.activeLayerId;
  store.addLayer(name);
  const s = useAppStore.getState();
  const created = s.layers.find((l) => l.drawingId === s.activeDrawingId && l.name === name);
  if (!created) throw new Error(`Laag "${name}" kon niet worden aangemaakt`);
  s.updateLayer(created.id, { color });
  // addLayer activates the new layer; keep the user's active layer.
  s.setActiveLayer?.(previousActive);
  return created.id;
}

function base(layerId: string, color: string, fill?: string) {
  return {
    id: generateId(),
    layerId,
    drawingId: useAppStore.getState().activeDrawingId,
    style: { strokeColor: color, strokeWidth: 1, lineStyle: 'solid' as const, ...(fill ? { fillColor: fill } : {}) },
    visible: true,
    locked: false,
  };
}

export function polyline(layerId: string, color: string, points: Point[], closed: boolean): Shape {
  return { ...base(layerId, color), type: 'polyline', points, closed } as Shape;
}

export function point(layerId: string, color: string, p: Point, size: number): Shape {
  return { ...base(layerId, color), type: 'circle', center: p, radius: size / 2 } as Shape;
}

export function text(layerId: string, color: string, p: Point, value: string, height: number): Shape {
  return {
    ...base(layerId, color),
    type: 'text',
    position: p,
    text: value,
    fontSize: height,
    fontFamily: 'Arial',
    rotation: 0,
    alignment: 'center',
    verticalAlignment: 'middle',
    bold: false,
    italic: false,
    underline: false,
    color,
    lineHeight: 1.2,
    // Height in drawing units (real-world size), not paper mm scaled by the drawing scale.
    isModelText: true,
  } as Shape;
}

export function underlay(
  layerId: string,
  topLeft: Point,
  width: number,
  height: number,
  image: { dataUrl: string; width: number; height: number },
  label: string,
  opacity = 1,
): ImageShape {
  return {
    ...base(layerId, '#ffffff'),
    type: 'image',
    position: topLeft,
    width,
    height,
    rotation: 0,
    imageData: image.dataUrl,
    originalWidth: image.width,
    originalHeight: image.height,
    opacity,
    maintainAspectRatio: true,
    isUnderlay: true,
    sourceFileName: label,
  };
}

export function commit(shapes: Shape[], zoom = true): void {
  if (!shapes.length) return;
  const s = useAppStore.getState();
  s.addShapes(shapes);
  if (zoom) s.zoomToFit();
}
