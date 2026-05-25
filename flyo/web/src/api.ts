/**
 * Typed wrappers around the flyo HTTP API.
 *
 * All calls are credentialed (cookies sent / received) so the session
 * established by /api/login carries through automatically.
 */

export type Perms = {
  access: boolean;
  list: boolean;
  upload: boolean;
  modify: boolean;
  show_hidden: boolean;
  play_media: boolean;
  force_download: boolean;
};

export type WhoAmI = {
  authenticated: boolean;
  user: string | null;
  perms: Perms;
};

export type Entry = {
  name: string;
  is_dir: boolean;
  size: number;
  mtime: number;
};

export type ListResponse = {
  path: string;
  entries: Entry[];
};

/** Thrown for any non-2xx response so callers can switch on `status`. */
export class ApiError extends Error {
  constructor(public status: number, public body: string) {
    super(`HTTP ${status}: ${body || '(no body)'}`);
  }
}

async function request(
  path: string,
  init: RequestInit = {},
): Promise<Response> {
  const res = await fetch(path, {
    credentials: 'same-origin',
    ...init,
  });
  if (!res.ok) {
    let body = '';
    try { body = await res.text(); } catch { /* ignore */ }
    throw new ApiError(res.status, body);
  }
  return res;
}

export async function whoami(): Promise<WhoAmI> {
  return (await request('/api/whoami')).json();
}

export async function login(user: string, pass: string): Promise<WhoAmI> {
  return (await request('/api/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ user, pass }),
  })).json();
}

export async function logout(): Promise<void> {
  await request('/api/logout', { method: 'POST' });
}

export async function listDir(path: string): Promise<ListResponse> {
  return (await request(`/api/list?path=${encodeURIComponent(path)}`)).json();
}

export async function mkdir(path: string): Promise<void> {
  await request(`/api/mkdir?path=${encodeURIComponent(path)}`, { method: 'POST' });
}

export async function renameEntry(from: string, to: string): Promise<void> {
  await request(
    `/api/rename?from=${encodeURIComponent(from)}&to=${encodeURIComponent(to)}`,
    { method: 'POST' },
  );
}

export async function deleteEntry(path: string): Promise<void> {
  await request(`/api/delete?path=${encodeURIComponent(path)}`, { method: 'POST' });
}

/**
 * Stream-upload a single file/blob, reporting progress through `onProgress`.
 * Uses XHR because fetch() still lacks usable upload-side streaming on most
 * browsers (writableStream support is gated).
 */
export function uploadFile(
  path: string,
  body: Blob,
  onProgress?: (loaded: number, total: number) => void,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open('POST', `/api/upload?path=${encodeURIComponent(path)}`, true);
    xhr.withCredentials = true;
    if (onProgress) {
      xhr.upload.onprogress = (e) => {
        if (e.lengthComputable) onProgress(e.loaded, e.total);
      };
    }
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) resolve();
      else reject(new ApiError(xhr.status, xhr.responseText));
    };
    xhr.onerror = () => reject(new ApiError(0, 'network error'));
    xhr.send(body);
  });
}

/** URL for direct download (used as `<a href>` for media play / save-as). */
export function fileUrl(path: string): string {
  return `/api/file?path=${encodeURIComponent(path)}`;
}

/** Join virtual paths the way URLs do. Always starts with "/". */
export function joinPath(base: string, name: string): string {
  const b = base.endsWith('/') ? base : `${base}/`;
  return `${b}${name}`.replace(/\/+/g, '/');
}

/** Drop trailing slash unless it's the root. */
export function normalize(p: string): string {
  if (!p || p === '/') return '/';
  return p.replace(/\/+$/g, '') || '/';
}
