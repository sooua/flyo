/** Display helpers — keep small, no deps. */

import type { IconName } from './icons';

export function formatSize(bytes: number): string {
  if (bytes === 0) return '';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0;
  let n = bytes;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }
  const decimals = n >= 100 || i === 0 ? 0 : n >= 10 ? 1 : 2;
  return `${n.toFixed(decimals)} ${units[i]}`;
}

export function formatTime(unixSeconds: number): string {
  if (!unixSeconds) return '';
  const d = new Date(unixSeconds * 1000);
  const now = new Date();
  const sameYear = d.getFullYear() === now.getFullYear();
  return new Intl.DateTimeFormat(undefined, {
    year: sameYear ? undefined : '2-digit',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(d);
}

/** Extract the file extension (lowercase) for icon selection / mime hint. */
export function ext(name: string): string {
  const i = name.lastIndexOf('.');
  if (i < 0 || i === name.length - 1) return '';
  return name.slice(i + 1).toLowerCase();
}

/**
 * Pick a monochrome icon name for a row. Returned name maps to a glyph in
 * `icons.tsx`; the calling component renders `<Icon name={...} />`.
 */
export function iconFor(name: string, isDir: boolean): IconName {
  if (isDir) return 'folder';
  const e = ext(name);
  if (!e) return 'file';
  if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'ico', 'svg', 'avif', 'heic'].includes(e)) return 'image';
  if (['mp4', 'mkv', 'mov', 'avi', 'webm', 'flv', 'wmv', 'mpeg', 'mpg', 'm4v'].includes(e)) return 'video';
  if (['mp3', 'wav', 'flac', 'ogg', 'm4a', 'aac', 'opus', 'wma'].includes(e)) return 'audio';
  if (['pdf'].includes(e)) return 'pdf';
  if (['doc', 'docx', 'rtf', 'odt', 'pages'].includes(e)) return 'doc';
  if (['xls', 'xlsx', 'csv', 'tsv', 'numbers', 'ods'].includes(e)) return 'sheet';
  if (['ppt', 'pptx', 'keynote', 'odp'].includes(e)) return 'slides';
  if (['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz', 'iso', 'dmg', 'cab'].includes(e)) return 'archive';
  if (['exe', 'msi', 'app', 'apk', 'deb', 'rpm', 'appimage'].includes(e)) return 'executable';
  if (['html', 'htm', 'xml', 'svg'].includes(e)) return 'markup';
  if (['js', 'mjs', 'ts', 'tsx', 'jsx', 'rs', 'go', 'py', 'java', 'c', 'h', 'cpp', 'rb', 'sh', 'php', 'sql', 'css', 'scss', 'json', 'yaml', 'yml', 'toml'].includes(e)) return 'code';
  if (['txt', 'log', 'ini', 'conf', 'cfg', 'env', 'md', 'markdown'].includes(e)) return 'text';
  if (['ttf', 'otf', 'woff', 'woff2'].includes(e)) return 'font';
  return 'file';
}
