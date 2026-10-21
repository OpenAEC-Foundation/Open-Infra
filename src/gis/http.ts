/**
 * HTTP helper — in the desktop app requests go through the Tauri HTTP plugin
 * (no CORS restrictions, so any WMS/WFS server works); in the browser build
 * the regular fetch is used, which needs servers that send CORS headers.
 */

import { fetch as tauriFetch } from '@tauri-apps/plugin-http';

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function httpFetch(url: string, init?: RequestInit): Promise<Response> {
  if (isTauri) {
    try {
      return await tauriFetch(url, init);
    } catch (err) {
      // Scope not granted for this host — fall back to the webview's fetch.
      console.warn('[Open Infra] Tauri fetch failed, using webview fetch:', err);
    }
  }
  return fetch(url, init);
}

async function checked(url: string, init?: RequestInit): Promise<Response> {
  const res = await httpFetch(url, init);
  if (!res.ok) throw new Error(`HTTP ${res.status} voor ${url.split('?')[0]}`);
  return res;
}

export async function httpText(url: string, init?: RequestInit): Promise<string> {
  return (await checked(url, init)).text();
}

export async function httpJson<T = any>(url: string, init?: RequestInit): Promise<T> {
  return (await checked(url, init)).json();
}

/** Fetch an image and return it as data URL plus its pixel size. */
export async function httpImage(url: string): Promise<{ dataUrl: string; width: number; height: number }> {
  const res = await checked(url);
  const type = res.headers.get('content-type') || '';
  const blob = await res.blob();
  if (!type.startsWith('image/')) {
    const text = await blob.text();
    throw new Error(serviceError(text) || `Server gaf geen afbeelding terug (${type || 'onbekend type'})`);
  }
  const dataUrl = await new Promise<string>((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(r.result as string);
    r.onerror = () => reject(r.error);
    r.readAsDataURL(blob);
  });
  const { width, height } = await imageSize(dataUrl);
  return { dataUrl, width, height };
}

export function imageSize(src: string): Promise<{ width: number; height: number; img: HTMLImageElement }> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve({ width: img.naturalWidth, height: img.naturalHeight, img });
    img.onerror = () => reject(new Error('Afbeelding kon niet worden gelezen'));
    img.src = src;
  });
}

/** Extract the message of an OGC ServiceException / ExceptionReport. */
export function serviceError(text: string): string | null {
  const m = /<(?:\w+:)?(?:ServiceException|ExceptionText)[^>]*>([\s\S]*?)<\/(?:\w+:)?(?:ServiceException|ExceptionText)>/.exec(text);
  return m ? m[1].replace(/<!\[CDATA\[|\]\]>/g, '').trim() : null;
}
