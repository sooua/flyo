/**
 * Translation dictionaries.
 *
 * Two languages: English and Simplified Chinese. Keys are flat dot-paths so
 * we can spot mismatches at a glance, and the runtime helper is a single
 * cheap object lookup — no library needed for ~80 strings.
 *
 * Pattern:
 *   t('toolbar.upload')           → "Upload" / "上传"
 *   t('items', { n: 3 })          → "3 items" / "3 个项目"
 *   t('confirm.delete.title', { n: 2 }) → "Delete 2 items?" / "删除 2 个项目？"
 */

import { lang } from './state';

export type Lang = 'en' | 'zh';

type Dict = Record<string, string>;

const en: Dict = {
  // Brand / generic
  'brand.name': 'Flyo',
  'common.cancel': 'Cancel',
  'common.ok': 'OK',
  'common.confirm': 'Confirm',
  'common.loading': 'Loading…',
  'common.signin': 'Sign in',
  'common.signout': 'Sign out',
  'common.guest': 'guest',
  'common.share': 'Share',

  // Theme toggle labels
  'theme.auto': 'Auto',
  'theme.light': 'Light',
  'theme.dark': 'Dark',

  // Language toggle labels
  'lang.toggle.title': 'Switch language',

  // App bar
  'appbar.breadcrumb.root': 'root',
  'appbar.signedout.toast': 'Signed out',
  'appbar.welcome': 'Welcome, {user}',

  // Toolbar
  'toolbar.upload': 'Upload',
  'toolbar.newfolder': 'New folder',
  'toolbar.refresh': 'Refresh',
  'toolbar.clear': 'Clear',
  'toolbar.delete': 'Delete',
  'toolbar.selected': '{n} selected',
  'toolbar.rename': 'Rename',
  'toolbar.newfile': 'New file',
  'modal.newfile.title': 'New file',
  'modal.newfile.placeholder': 'untitled.txt',
  'toast.fileCreated': 'File {name} created',
  'row.action.open': 'Open',
  'row.action.rename': 'Rename',
  'row.action.delete': 'Delete',

  // Listing
  'listing.header.name': 'Name',
  'listing.header.size': 'Size',
  'listing.header.mtime': 'Modified',
  'listing.empty.title.cannotUpload': 'This folder is empty',
  'listing.empty.title.canUpload': 'Drop files here to upload',
  'listing.empty.detail': 'Or click Upload above. Folders are supported.',
  'listing.empty.cta.upload': 'Upload files',
  'listing.empty.cta.newfolder': 'New folder',
  'listing.dropzone.label': 'Drop files to upload into {path}',

  // Status bar
  'status.items': '{n} items',
  'status.item': '{n} item',
  'status.selectedOf': '{n} selected of {total} items',
  'status.uploading': 'Uploading {n} file…',

  // Errors
  'error.listForbidden': 'You do not have permission to list this folder.',
  'error.couldNotLoad': 'Could not load',
  'error.uploadFailed': 'Upload failed: {name}',
  'error.invalidCreds': 'Invalid username or password.',
  'error.streamError': 'stream error',
  'error.deleteSummary': '{failed}/{total} failed to delete',

  // Toasts
  'toast.folderCreated': 'Folder {name} created',
  'toast.renamed': 'Renamed',
  'toast.movedToTrash': 'Moved {n} to .Trash',

  // Modals
  'modal.signin.title': 'Sign in to Flyo',
  'modal.signin.sub': 'Enter your credentials. Guest access is read-only.',
  'modal.signin.user': 'Username',
  'modal.signin.pass': 'Password',
  'modal.signin.submit': 'Sign in',
  'modal.signin.submitting': 'Signing in…',

  'modal.newfolder.title': 'New folder',
  'modal.newfolder.placeholder': 'folder name',

  'modal.rename.title': 'Rename {name}',
  'modal.rename.placeholder': 'new name',

  'modal.confirmDelete.title.one': 'Delete this item?',
  'modal.confirmDelete.title.many': 'Delete {n} items?',
  'modal.confirmDelete.message': 'They will be moved to .Trash and can be restored from the file system.',
  'modal.confirmDelete.confirm': 'Move to Trash',
};

const zh: Dict = {
  'brand.name': 'Flyo',
  'common.cancel': '取消',
  'common.ok': '确定',
  'common.confirm': '确认',
  'common.loading': '加载中…',
  'common.signin': '登录',
  'common.signout': '退出登录',
  'common.guest': '访客',
  'common.share': '分享',

  'theme.auto': '自动',
  'theme.light': '浅色',
  'theme.dark': '深色',

  'lang.toggle.title': '切换语言',

  'appbar.breadcrumb.root': '根目录',
  'appbar.signedout.toast': '已退出登录',
  'appbar.welcome': '欢迎，{user}',

  'toolbar.upload': '上传',
  'toolbar.newfolder': '新建文件夹',
  'toolbar.refresh': '刷新',
  'toolbar.clear': '清空',
  'toolbar.delete': '删除',
  'toolbar.selected': '已选 {n} 项',
  'toolbar.rename': '重命名',
  'toolbar.newfile': '新建文件',
  'modal.newfile.title': '新建文件',
  'modal.newfile.placeholder': 'untitled.txt',
  'toast.fileCreated': '已创建文件 {name}',
  'row.action.open': '打开',
  'row.action.rename': '重命名',
  'row.action.delete': '删除',

  'listing.header.name': '名称',
  'listing.header.size': '大小',
  'listing.header.mtime': '修改时间',
  'listing.empty.title.cannotUpload': '此文件夹为空',
  'listing.empty.title.canUpload': '拖文件到此处上传',
  'listing.empty.detail': '或点击上方"上传"。支持上传文件夹。',
  'listing.empty.cta.upload': '上传文件',
  'listing.empty.cta.newfolder': '新建文件夹',
  'listing.dropzone.label': '放入文件以上传到 {path}',

  'status.items': '共 {n} 项',
  'status.item': '1 项',
  'status.selectedOf': '已选 {n} / {total} 项',
  'status.uploading': '正在上传 {n} 个文件…',

  'error.listForbidden': '你没有列出此文件夹的权限。',
  'error.couldNotLoad': '加载失败',
  'error.uploadFailed': '上传失败：{name}',
  'error.invalidCreds': '用户名或密码错误。',
  'error.streamError': '传输错误',
  'error.deleteSummary': '{total} 项中 {failed} 项删除失败',

  'toast.folderCreated': '已创建文件夹 {name}',
  'toast.renamed': '已重命名',
  'toast.movedToTrash': '已将 {n} 项移入回收站',

  'modal.signin.title': '登录 Flyo',
  'modal.signin.sub': '输入用户名和密码。访客只能查看。',
  'modal.signin.user': '用户名',
  'modal.signin.pass': '密码',
  'modal.signin.submit': '登录',
  'modal.signin.submitting': '登录中…',

  'modal.newfolder.title': '新建文件夹',
  'modal.newfolder.placeholder': '文件夹名',

  'modal.rename.title': '重命名 {name}',
  'modal.rename.placeholder': '新名称',

  'modal.confirmDelete.title.one': '删除此项？',
  'modal.confirmDelete.title.many': '删除 {n} 项？',
  'modal.confirmDelete.message': '它们将被移入 .Trash，可从文件系统中恢复。',
  'modal.confirmDelete.confirm': '移入回收站',
};

const dicts: Record<Lang, Dict> = { en, zh };

/**
 * Look up a translated string for the active language. Falls back to the
 * English value if a key isn't translated, and to the key itself if neither
 * dictionary has it (loud failure in dev).
 *
 * Placeholders use `{name}` syntax.
 */
export function t(key: string, vars?: Record<string, string | number>): string {
  const dict = dicts[lang.value] || en;
  let str = dict[key] ?? en[key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      str = str.replaceAll(`{${k}}`, String(v));
    }
  }
  return str;
}

/** Detect the language a new visitor would prefer. */
export function detectLang(): Lang {
  if (typeof navigator === 'undefined') return 'en';
  const tag = (navigator.language || 'en').toLowerCase();
  return tag.startsWith('zh') ? 'zh' : 'en';
}
