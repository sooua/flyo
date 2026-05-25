import { useComputed, useSignal, useSignalEffect } from '@preact/signals';
import { useEffect, useRef } from 'preact/hooks';

import * as api from './api';
import { formatSize, formatTime, iconFor } from './format';
import { Icon } from './icons';
import { Logo } from './logo';
import {
  currentPath,
  entries,
  lang,
  loadError,
  loading,
  navigate,
  selectedNames,
  setTheme,
  sortAsc,
  sortKey,
  theme,
  toast,
  toasts,
  uploads,
  user,
  type SortKey,
} from './state';
import { t } from './strings';

// =============================================================
// Top-level App: switches between Login screen and Browser shell
// =============================================================
export default function App() {
  // Initial whoami probe — sets user and decides login vs browser.
  useEffect(() => {
    api.whoami()
      .then((w) => { user.value = w; })
      .catch(() => { user.value = { authenticated: false, user: null, perms: defaultGuestPerms() }; });
  }, []);

  if (user.value === null) {
    return <div class="login-shell"><div class="login-card"><p>Loading…</p></div></div>;
  }

  // If guest cannot list and there are no users at all → still let them try.
  // The browser view itself surfaces the permission error gracefully.
  return <Browser />;
}

function defaultGuestPerms(): api.Perms {
  return {
    access: false, list: false, upload: false, modify: false,
    show_hidden: false, play_media: false, force_download: false,
  };
}

// =============================================================
// Browser shell: appbar + toolbar + listing + statusbar
// =============================================================
function Browser() {
  const dragging = useSignal<boolean>(false);
  const fileInput = useRef<HTMLInputElement>(null);
  const newFolderOpen = useSignal<boolean>(false);
  const newFileOpen = useSignal<boolean>(false);
  const renameTarget = useSignal<string | null>(null);
  const loginOpen = useSignal<boolean>(false);
  const confirmDelete = useSignal<boolean>(false);

  // Refetch listing whenever currentPath changes.
  useSignalEffect(() => {
    const p = currentPath.value;
    refreshListing(p);
  });

  // Bind global drag/drop on the document, so dropping anywhere works.
  useEffect(() => {
    const onDragOver = (e: DragEvent) => {
      if (!user.value?.perms.upload) return;
      e.preventDefault();
      dragging.value = true;
    };
    const onDragLeave = (e: DragEvent) => {
      if ((e as any).fromElement) return;
      if (e.relatedTarget === null) dragging.value = false;
    };
    const onDrop = (e: DragEvent) => {
      e.preventDefault();
      dragging.value = false;
      if (!user.value?.perms.upload) return;
      const files = collectFiles(e.dataTransfer);
      void uploadAll(files, currentPath.value);
    };
    document.addEventListener('dragover', onDragOver);
    document.addEventListener('dragleave', onDragLeave);
    document.addEventListener('drop', onDrop);
    return () => {
      document.removeEventListener('dragover', onDragOver);
      document.removeEventListener('dragleave', onDragLeave);
      document.removeEventListener('drop', onDrop);
    };
  }, []);

  // Keyboard: Delete selected, Esc clears selection, F2 rename, Backspace = up.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement) return;
      if (e.key === 'Escape') { selectedNames.value = new Set(); return; }
      if (e.key === 'Delete' && user.value?.perms.modify && selectedNames.value.size > 0) {
        confirmDelete.value = true;
        return;
      }
      if (e.key === 'F2' && user.value?.perms.modify && selectedNames.value.size === 1) {
        const only = Array.from(selectedNames.value)[0];
        renameTarget.value = only;
        return;
      }
      if (e.key === 'Backspace' && currentPath.value !== '/') {
        navigate(parentPath(currentPath.value));
        return;
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const canUpload = useComputed(() => !!user.value?.perms.upload);
  const canModify = useComputed(() => !!user.value?.perms.modify);

  return (
    <div class="app">
      <header class="appbar">
        <div class="brand">
          <span class="brand-logo"><Logo size={24} /></span>
          <span class="brand-name">Flyo</span>
        </div>
        <Breadcrumbs />
        <div class="spacer" />
        <LangToggle />
        <ThemeToggle />
        {user.value?.authenticated
          ? (
            <UserMenu
              onSignOut={async () => {
                await api.logout();
                user.value = await api.whoami();
                toast(t('appbar.signedout.toast'));
              }}
            />
          )
          : (
            <>
              <span class="user-pill guest">
                <span class="avatar">G</span>
                {t('common.guest')}
              </span>
              <button class="btn primary" onClick={() => { loginOpen.value = true; }}>{t('common.signin')}</button>
            </>
          )
        }
      </header>

      <div class="toolbar">
        <button
          class="btn primary"
          disabled={!canUpload.value}
          onClick={() => fileInput.current?.click()}
          title={t('toolbar.upload')}
        >
          <Icon name="upload" size={14} /> {t('toolbar.upload')}
        </button>
        <button
          class="btn"
          disabled={!canUpload.value}
          onClick={() => { newFolderOpen.value = true; }}
        >
          <Icon name="plus" size={14} /> {t('toolbar.newfolder')}
        </button>
        <button
          class="btn"
          disabled={!canUpload.value}
          onClick={() => { newFileOpen.value = true; }}
          title={t('toolbar.newfile')}
        >
          <Icon name="file-plus" size={14} /> {t('toolbar.newfile')}
        </button>
        <button class="btn" onClick={() => refreshListing(currentPath.value)} title={t('toolbar.refresh')}>
          <Icon name="refresh" size={14} /> {t('toolbar.refresh')}
        </button>
        {selectedNames.value.size > 0 && (
          <>
            <span class="tb-div" aria-hidden="true" />
            <span class="chip">{t('toolbar.selected', { n: selectedNames.value.size })}</span>
            <button class="btn ghost" onClick={() => { selectedNames.value = new Set(); }}>
              <Icon name="x" size={14} /> {t('toolbar.clear')}
            </button>
            <button
              class="btn danger"
              disabled={!canModify.value}
              onClick={() => { confirmDelete.value = true; }}
              title={`${t('toolbar.delete')} (Del)`}
            >
              <Icon name="trash" size={14} /> {t('toolbar.delete')}
            </button>
          </>
        )}
        <span class="spacer" />
        <span class="item-count">
          {entries.value.length === 1
            ? t('status.item', { n: 1 })
            : t('status.items', { n: entries.value.length })}
        </span>
        <input
          ref={fileInput}
          type="file"
          multiple
          style={{ display: 'none' }}
          onChange={(e) => {
            const files = Array.from((e.target as HTMLInputElement).files || []);
            (e.target as HTMLInputElement).value = '';
            void uploadAll(files, currentPath.value);
          }}
        />
      </div>

      <div class="listing-wrap">
        <div class={`dropzone ${dragging.value ? 'active' : ''}`}>
          {t('listing.dropzone.label', { path: currentPath.value })}
        </div>

        {loading.value
          ? <EmptyState title={t('common.loading')} />
          : loadError.value
            ? <EmptyState title={t('error.couldNotLoad')} detail={loadError.value} />
            : <Listing
                onRename={(n) => { renameTarget.value = n; }}
                onDeleteOne={(n) => {
                  selectedNames.value = new Set([n]);
                  confirmDelete.value = true;
                }}
              />}
      </div>

      <footer class="statusbar">
        {selectedNames.value.size > 0
          ? <span>{t('status.selectedOf', { n: selectedNames.value.size, total: entries.value.length })}</span>
          : <span>{entries.value.length === 1
              ? t('status.item', { n: 1 })
              : t('status.items', { n: entries.value.length })}</span>}
        <span class="spacer" />
        {uploads.value.some((u) => u.state === 'uploading') && (
          <span class="upload-indicator">
            <Icon name="cloud-upload" size={13} />
            {t('status.uploading', { n: uploads.value.filter((u) => u.state === 'uploading').length })}
          </span>
        )}
      </footer>

      {newFolderOpen.value && (
        <PromptModal
          title={t('modal.newfolder.title')}
          placeholder={t('modal.newfolder.placeholder')}
          onCancel={() => { newFolderOpen.value = false; }}
          onSubmit={async (name) => {
            newFolderOpen.value = false;
            try {
              await api.mkdir(api.joinPath(currentPath.value, name));
              toast(t('toast.folderCreated', { name }), 'success');
              refreshListing(currentPath.value);
            } catch (e) { toast(String((e as Error).message), 'danger'); }
          }}
        />
      )}

      {newFileOpen.value && (
        <PromptModal
          title={t('modal.newfile.title')}
          placeholder={t('modal.newfile.placeholder')}
          onCancel={() => { newFileOpen.value = false; }}
          onSubmit={async (name) => {
            newFileOpen.value = false;
            try {
              // Empty body → atomic write of zero-byte file via the upload endpoint.
              await api.uploadFile(api.joinPath(currentPath.value, name), new Blob([]));
              toast(t('toast.fileCreated', { name }), 'success');
              refreshListing(currentPath.value);
            } catch (e) { toast(String((e as Error).message), 'danger'); }
          }}
        />
      )}

      {renameTarget.value && (
        <PromptModal
          title={t('modal.rename.title', { name: renameTarget.value })}
          placeholder={t('modal.rename.placeholder')}
          initial={renameTarget.value}
          onCancel={() => { renameTarget.value = null; }}
          onSubmit={async (name) => {
            const old = renameTarget.value!;
            renameTarget.value = null;
            try {
              await api.renameEntry(
                api.joinPath(currentPath.value, old),
                api.joinPath(currentPath.value, name),
              );
              toast(t('toast.renamed'), 'success');
              refreshListing(currentPath.value);
            } catch (e) { toast(String((e as Error).message), 'danger'); }
          }}
        />
      )}

      {loginOpen.value && <LoginModal onClose={() => { loginOpen.value = false; }} />}

      {confirmDelete.value && (
        <ConfirmModal
          title={selectedNames.value.size === 1
            ? t('modal.confirmDelete.title.one')
            : t('modal.confirmDelete.title.many', { n: selectedNames.value.size })}
          message={t('modal.confirmDelete.message')}
          confirmLabel={t('modal.confirmDelete.confirm')}
          destructive
          onCancel={() => { confirmDelete.value = false; }}
          onConfirm={() => { confirmDelete.value = false; void deleteSelected(); }}
        />
      )}

      <Toasts />
    </div>
  );
}

// =============================================================
// Subcomponents
// =============================================================

function Breadcrumbs() {
  const parts = currentPath.value === '/' ? [] : currentPath.value.split('/').filter(Boolean);
  let cumulative = '';
  return (
    <nav class="crumbs" aria-label="breadcrumb">
      <a class="crumb" href="#/" onClick={(e) => { e.preventDefault(); navigate('/'); }}>
        / {t('appbar.breadcrumb.root')}
      </a>
      {parts.map((p, i) => {
        cumulative += `/${p}`;
        const isLast = i === parts.length - 1;
        const link = cumulative;
        return (
          <>
            <span class="crumb-sep">/</span>
            <a
              class={`crumb${isLast ? ' current' : ''}`}
              href={`#${encodeURI(link)}`}
              onClick={(e) => { e.preventDefault(); navigate(link); }}
            >
              {p}
            </a>
          </>
        );
      })}
    </nav>
  );
}

function ThemeToggle() {
  const isDark = theme.value === 'dark';
  const toggle = () => setTheme(isDark ? 'light' : 'dark');
  // Show the icon for the CURRENT theme; clicking flips to the other.
  return (
    <button class="btn ghost" onClick={toggle}
            title={t(isDark ? 'theme.light' : 'theme.dark')}>
      <Icon name={isDark ? 'moon' : 'sun'} size={14} /> {t(isDark ? 'theme.dark' : 'theme.light')}
    </button>
  );
}

function LangToggle() {
  const toggle = () => {
    lang.value = lang.value === 'en' ? 'zh' : 'en';
  };
  // Display the OTHER language as the affordance — "switch to that"
  const label = lang.value === 'en' ? '中文' : 'EN';
  return (
    <button class="btn ghost" onClick={toggle} title={t('lang.toggle.title')}>
      <Icon name="languages" size={14} /> {label}
    </button>
  );
}

/**
 * User pill with a dropdown menu — clicking the pill opens a small popover
 * containing the signed-in name and a Sign-out item. Closes on outside
 * click or Esc.
 */
function UserMenu(props: { onSignOut: () => void }) {
  const open = useSignal(false);
  const anchor = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open.value) return;
    const onDoc = (e: MouseEvent) => {
      if (!anchor.current?.contains(e.target as Node)) open.value = false;
    };
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') open.value = false; };
    document.addEventListener('mousedown', onDoc);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDoc);
      document.removeEventListener('keydown', onKey);
    };
  }, [open.value]);

  const name = user.value?.user || '?';
  const initial = name.charAt(0).toUpperCase();

  return (
    <div class="menu-anchor" ref={anchor}>
      <button
        class="user-pill"
        onClick={() => { open.value = !open.value; }}
        aria-haspopup="menu"
        aria-expanded={open.value}
      >
        <span class="avatar">{initial}</span>
        {name}
        <Icon name={open.value ? 'chevron-up' : 'chevron-down'} size={12} />
      </button>
      {open.value && (
        <div class="menu" role="menu">
          <div class="menu-section">
            <div class="name">{name}</div>
            <div class="sub">{user.value?.perms.modify ? 'Full access' : 'Read only'}</div>
          </div>
          <button class="menu-item danger" role="menuitem"
                  onClick={() => { open.value = false; props.onSignOut(); }}>
            <Icon name="logout" size={16} class="ic" /> {t('common.signout')}
          </button>
        </div>
      )}
    </div>
  );
}

function Listing(props: { onRename: (name: string) => void; onDeleteOne: (name: string) => void }) {
  const sorted = useComputed(() => sortEntries(entries.value, sortKey.value, sortAsc.value));

  if (sorted.value.length === 0) {
    const canUpload = !!user.value?.perms.upload;
    return (
      <EmptyState
        title={t(canUpload ? 'listing.empty.title.canUpload' : 'listing.empty.title.cannotUpload')}
        detail={canUpload ? t('listing.empty.detail') : undefined}
        showCta={canUpload}
        onUpload={canUpload
          ? () => (document.querySelector('input[type=file]') as HTMLInputElement | null)?.click()
          : undefined}
      />
    );
  }
  return (
    <table class="listing">
      <thead>
        <tr>
          <Th col="name">{t('listing.header.name')}</Th>
          <Th col="size">{t('listing.header.size')}</Th>
          <Th col="mtime">{t('listing.header.mtime')}</Th>
        </tr>
      </thead>
      <tbody>
        {sorted.value.map((e) => (
          <Row key={e.name} entry={e} onRename={props.onRename} onDeleteOne={props.onDeleteOne} />
        ))}
      </tbody>
    </table>
  );
}

function Th(props: { col: SortKey; children: any }) {
  const active = sortKey.value === props.col;
  return (
    <th
      data-col={props.col}
      class={active ? '' : 'sort-inactive'}
      onClick={() => {
        if (sortKey.value === props.col) sortAsc.value = !sortAsc.value;
        else { sortKey.value = props.col; sortAsc.value = true; }
      }}
    >
      <span class="th-label">
        {props.children}
        {active && (
          <Icon name={sortAsc.value ? 'chevron-up' : 'chevron-down'} size={11} class="th-sort" />
        )}
      </span>
    </th>
  );
}

function Row({ entry, onRename, onDeleteOne }: { entry: api.Entry; onRename: (n: string) => void; onDeleteOne: (n: string) => void }) {
  const selected = selectedNames.value.has(entry.name);
  const upload = uploads.value.find((u) => u.name === entry.name && u.state === 'uploading');
  const failed = uploads.value.find((u) => u.name === entry.name && u.state === 'failed');

  const rowClass = ['']
    .concat(selected ? ['selected'] : [])
    .concat(upload ? ['uploading'] : [])
    .concat(failed ? ['failed'] : [])
    .join(' ').trim();

  const fullPath = api.joinPath(currentPath.value, entry.name);

  return (
    <tr
      class={rowClass}
      onClick={(e) => {
        if (e.shiftKey || e.metaKey || e.ctrlKey) {
          const next = new Set(selectedNames.value);
          if (next.has(entry.name)) next.delete(entry.name);
          else next.add(entry.name);
          selectedNames.value = next;
        } else {
          selectedNames.value = new Set([entry.name]);
        }
      }}
      onDblClick={() => {
        if (entry.is_dir) navigate(fullPath);
      }}
    >
      <td>
        <div class={`row-name ${entry.is_dir ? 'is-dir' : ''}`}>
          <span class="row-icon" aria-hidden="true">
            <Icon name={iconFor(entry.name, entry.is_dir)} size={18} />
          </span>
          {entry.is_dir
            ? (
              <a class="row-link" href={`#${encodeURI(fullPath)}`}
                onClick={(e) => { e.preventDefault(); navigate(fullPath); }}>
                {entry.name}
              </a>
            )
            : (
              <a class="row-link" href={api.fileUrl(fullPath)} target="_blank"
                onClick={(e) => e.stopPropagation()}>
                {entry.name}
              </a>
            )}
          {upload && (
            <div class="upload-progress" style={{ marginLeft: 'auto', width: 100 }}>
              <div class="bar" style={{ width: `${(upload.loaded / upload.size) * 100}%` }} />
            </div>
          )}
          {user.value?.perms.modify && (
            <div class="row-actions" onClick={(e) => e.stopPropagation()}>
              <button
                class="row-action-btn"
                title={t('row.action.rename')}
                aria-label={t('row.action.rename')}
                onClick={(e) => { e.stopPropagation(); onRename(entry.name); }}
              >
                <Icon name="pencil" size={14} />
              </button>
              <button
                class="row-action-btn danger"
                title={t('row.action.delete')}
                aria-label={t('row.action.delete')}
                onClick={(e) => { e.stopPropagation(); onDeleteOne(entry.name); }}
              >
                <Icon name="trash" size={14} />
              </button>
            </div>
          )}
        </div>
      </td>
      <td class="size">{formatSize(entry.size)}</td>
      <td class="mtime">{formatTime(entry.mtime)}</td>
    </tr>
  );
}

function EmptyState(props: { title: string; detail?: string; showCta?: boolean; onUpload?: () => void; onNewFolder?: () => void }) {
  return (
    <div class="empty">
      <div class="icon-box" aria-hidden="true">
        <Icon name="cloud-upload" size={30} />
      </div>
      <div class="copy">
        <h2>{props.title}</h2>
        {props.detail && <p>{props.detail}</p>}
      </div>
      {props.showCta && (
        <div class="cta">
          {props.onUpload && (
            <button class="btn primary" onClick={props.onUpload}>
              <Icon name="upload" size={15} /> {t('listing.empty.cta.upload')}
            </button>
          )}
          {props.onNewFolder && (
            <button class="btn outline" onClick={props.onNewFolder}>
              <Icon name="plus" size={15} /> {t('listing.empty.cta.newfolder')}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function PromptModal(props: {
  title: string;
  placeholder?: string;
  initial?: string;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') props.onCancel(); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
  return (
    <div class="scrim" onClick={props.onCancel}>
      <form
        class="modal"
        onClick={(e) => e.stopPropagation()}
        onSubmit={(e) => {
          e.preventDefault();
          const v = inputRef.current!.value.trim();
          if (v) props.onSubmit(v);
        }}
      >
        <h2>{props.title}</h2>
        <input
          ref={inputRef}
          class="input"
          placeholder={props.placeholder}
          defaultValue={props.initial}
        />
        <div class="modal-actions">
          <button type="button" class="btn ghost" onClick={props.onCancel}>{t('common.cancel')}</button>
          <button type="submit" class="btn primary">{t('common.ok')}</button>
        </div>
      </form>
    </div>
  );
}

function ConfirmModal(props: {
  title: string;
  message: string;
  confirmLabel?: string;
  destructive?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') props.onCancel();
      if (e.key === 'Enter')  props.onConfirm();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
  return (
    <div class="scrim" onClick={props.onCancel}>
      <div class="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{props.title}</h2>
        <p style={{ margin: 0, color: 'var(--ink-muted)', fontSize: 14, lineHeight: 1.5 }}>
          {props.message}
        </p>
        <div class="modal-actions">
          <button class="btn ghost" onClick={props.onCancel}>{t('common.cancel')}</button>
          <button
            class={props.destructive ? 'btn primary' : 'btn primary'}
            style={props.destructive ? { background: 'var(--danger)' } : undefined}
            onClick={props.onConfirm}
          >
            {props.confirmLabel || t('common.confirm')}
          </button>
        </div>
      </div>
    </div>
  );
}

function LoginModal(props: { onClose: () => void }) {
  const u = useSignal('');
  const p = useSignal('');
  const err = useSignal<string | null>(null);
  const busy = useSignal(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') props.onClose(); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const submit = async (e: Event) => {
    e.preventDefault();
    busy.value = true;
    err.value = null;
    try {
      const w = await api.login(u.value, p.value);
      user.value = w;
      props.onClose();
      toast(t('appbar.welcome', { user: w.user || '' }), 'success');
      refreshListing(currentPath.value);
    } catch (e: any) {
      err.value = e?.status === 401 ? t('error.invalidCreds') : String(e?.message || e);
    } finally {
      busy.value = false;
    }
  };

  return (
    <div class="scrim" onClick={props.onClose}>
      <form class="modal login-card" onClick={(e) => e.stopPropagation()} onSubmit={submit}
            style={{ padding: 'var(--s-7)', gap: 'var(--s-5)' }}>
        <div class="login-header">
          <div class="login-title-row">
            <span class="brand-logo" style={{ width: 32, height: 32 }}><Logo size={32} /></span>
            <h1>{t('modal.signin.title')}</h1>
          </div>
          <p class="sub">{t('modal.signin.sub')}</p>
        </div>
        <label>{t('modal.signin.user')}
          <input class="input" autocomplete="username" required placeholder="admin"
            value={u.value} onInput={(e) => { u.value = (e.target as HTMLInputElement).value; }} />
        </label>
        <label>{t('modal.signin.pass')}
          <input class="input" type="password" autocomplete="current-password" required
            value={p.value} onInput={(e) => { p.value = (e.target as HTMLInputElement).value; }} />
        </label>
        {err.value && <p class="err">{err.value}</p>}
        <div class="modal-actions">
          <button type="button" class="btn ghost" onClick={props.onClose}>{t('common.cancel')}</button>
          <button type="submit" class="btn primary" disabled={busy.value}>
            {busy.value ? t('modal.signin.submitting') : t('modal.signin.submit')}
          </button>
        </div>
      </form>
    </div>
  );
}

function Toasts() {
  return (
    <div class="toasts" aria-live="polite">
      {toasts.value.map((t) => (
        <div key={t.id} class={`toast ${t.kind}`}>{t.message}</div>
      ))}
    </div>
  );
}

// =============================================================
// Actions
// =============================================================

async function refreshListing(path: string): Promise<void> {
  loading.value = true;
  loadError.value = null;
  try {
    const res = await api.listDir(path);
    entries.value = res.entries;
    selectedNames.value = new Set();
  } catch (e: any) {
    loadError.value = e?.status === 403 ? t('error.listForbidden') : String(e?.message || e);
    entries.value = [];
  } finally {
    loading.value = false;
  }
}

async function deleteSelected(): Promise<void> {
  const targets = Array.from(selectedNames.value);
  if (targets.length === 0) return;
  const results = await Promise.all(targets.map(async (name) => {
    try {
      await api.deleteEntry(api.joinPath(currentPath.value, name));
      return { name, ok: true };
    } catch (e: any) {
      return { name, ok: false, error: String(e?.message || e) };
    }
  }));
  const failed = results.filter((r) => !r.ok);
  if (failed.length === 0) toast(t('toast.movedToTrash', { n: results.length }), 'success');
  else toast(t('error.deleteSummary', { failed: failed.length, total: results.length }), 'danger');
  refreshListing(currentPath.value);
}

function collectFiles(dt: DataTransfer | null): File[] {
  if (!dt) return [];
  const out: File[] = [];
  for (const item of dt.items) {
    if (item.kind === 'file') {
      const f = item.getAsFile();
      if (f) out.push(f);
    }
  }
  return out;
}

let uploadSeq = 0;
async function uploadAll(files: File[], dir: string): Promise<void> {
  if (files.length === 0) return;
  const items = files.map((f) => ({
    id: `u${++uploadSeq}`,
    name: f.name,
    size: f.size,
    loaded: 0,
    state: 'pending' as const,
    file: f,
  }));
  uploads.value = [...uploads.value, ...items.map(({ file: _f, ...rest }) => rest)];

  for (const item of items) {
    setUpload(item.id, { state: 'uploading' });
    try {
      await api.uploadFile(api.joinPath(dir, item.name), item.file, (loaded) => {
        setUpload(item.id, { loaded });
      });
      setUpload(item.id, { state: 'done', loaded: item.size });
    } catch (e: any) {
      setUpload(item.id, { state: 'failed', error: String(e?.message || e) });
      toast(t('error.uploadFailed', { name: item.name }), 'danger');
    }
  }
  // Strip completed uploads after a beat so the bar can fade.
  setTimeout(() => {
    uploads.value = uploads.value.filter((u) => u.state !== 'done');
  }, 800);
  refreshListing(dir);
}

function setUpload(id: string, patch: Partial<{ loaded: number; state: 'pending' | 'uploading' | 'done' | 'failed'; error: string }>) {
  uploads.value = uploads.value.map((u) => (u.id === id ? { ...u, ...patch } : u));
}

function sortEntries(rows: api.Entry[], key: SortKey, asc: boolean): api.Entry[] {
  const sign = asc ? 1 : -1;
  return [...rows].sort((a, b) => {
    // Directories always first regardless of sort, mirroring Finder/Explorer.
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    let cmp = 0;
    if (key === 'name') cmp = a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' });
    else if (key === 'size') cmp = a.size - b.size;
    else cmp = a.mtime - b.mtime;
    return cmp * sign;
  });
}

function parentPath(p: string): string {
  if (p === '/') return '/';
  const i = p.lastIndexOf('/');
  if (i <= 0) return '/';
  return p.slice(0, i);
}
