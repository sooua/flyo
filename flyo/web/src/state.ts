/**
 * Global app state via @preact/signals.
 *
 * Keeping state in module-scope signals lets any component subscribe to the
 * pieces it needs without prop-drilling; rerenders are fine-grained per
 * signal read, so the UI stays cheap even on large directory listings.
 */

import { computed, signal } from '@preact/signals';
import type { Entry, WhoAmI } from './api';
import { normalize } from './api';

// --- session ---
export const user = signal<WhoAmI | null>(null);

// --- current directory ---
export const currentPath = signal<string>(parseHashPath());
export const entries = signal<Entry[]>([]);
export const loading = signal<boolean>(false);
export const loadError = signal<string | null>(null);

// --- selection ---
export const selectedNames = signal<Set<string>>(new Set());

export const selectionCount = computed(() => selectedNames.value.size);

// --- sort ---
export type SortKey = 'name' | 'size' | 'mtime';
export const sortKey = signal<SortKey>(
  (localStorage.getItem('flyo.sort') as SortKey) || 'name',
);
export const sortAsc = signal<boolean>(
  localStorage.getItem('flyo.sortAsc') !== 'false',
);

// --- theme ---
// Binary: light or dark. On first load we read the user's stored choice; if
// none exists we ask the OS via prefers-color-scheme, then persist that as an
// explicit value so the choice is sticky.
export type Theme = 'light' | 'dark';
const initialTheme: Theme = (() => {
  const stored = localStorage.getItem('flyo.theme');
  if (stored === 'light' || stored === 'dark') return stored;
  return window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
})();
export const theme = signal<Theme>(initialTheme);

/**
 * Switch theme with a short cross-fade. The transition class is added to the
 * root element for the duration of the color transition, so element-level
 * hover transitions stay snappy outside of the toggle moment.
 */
export function setTheme(next: Theme): void {
  if (next === theme.value) return;
  document.documentElement.classList.add('theme-transitioning');
  theme.value = next;
  window.setTimeout(() => {
    document.documentElement.classList.remove('theme-transitioning');
  }, 320);
}

// --- language ---
export type Lang = 'en' | 'zh';
const initialLang: Lang = (() => {
  const stored = localStorage.getItem('flyo.lang') as Lang | null;
  if (stored === 'en' || stored === 'zh') return stored;
  const browser = (navigator.language || 'en').toLowerCase();
  return browser.startsWith('zh') ? 'zh' : 'en';
})();
export const lang = signal<Lang>(initialLang);

// --- upload tracking ---
export type Upload = {
  id: string;
  name: string;
  size: number;
  loaded: number;
  state: 'pending' | 'uploading' | 'done' | 'failed';
  error?: string;
};
export const uploads = signal<Upload[]>([]);

// --- toasts ---
export type Toast = {
  id: number;
  kind: 'info' | 'success' | 'danger';
  message: string;
};
export const toasts = signal<Toast[]>([]);

let toastSeq = 0;
export function toast(message: string, kind: Toast['kind'] = 'info', ttl = 3500): void {
  const id = ++toastSeq;
  toasts.value = [...toasts.value, { id, kind, message }];
  setTimeout(() => {
    toasts.value = toasts.value.filter((t) => t.id !== id);
  }, ttl);
}

// --- helpers ---

function parseHashPath(): string {
  const h = window.location.hash.replace(/^#/, '');
  return normalize(decodeURIComponent(h) || '/');
}

export function navigate(path: string): void {
  const p = normalize(path);
  if (p === currentPath.value) return;
  window.location.hash = encodeURI(p);
}

window.addEventListener('hashchange', () => {
  currentPath.value = parseHashPath();
});

// persist sort + theme + language
import { effect } from '@preact/signals';
effect(() => { localStorage.setItem('flyo.sort', sortKey.value); });
effect(() => { localStorage.setItem('flyo.sortAsc', String(sortAsc.value)); });
effect(() => {
  localStorage.setItem('flyo.theme', theme.value);
  document.documentElement.setAttribute('data-theme', theme.value);
});
effect(() => {
  localStorage.setItem('flyo.lang', lang.value);
  // Update <html lang> for screen readers + Lighthouse a11y audit.
  document.documentElement.setAttribute('lang', lang.value === 'zh' ? 'zh-CN' : 'en');
});
