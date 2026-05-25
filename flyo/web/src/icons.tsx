/**
 * Icon component backed by Material Design Icons (Templarian / MDI).
 *
 * Each entry in `paths` is imported by name from `@mdi/js`; Vite tree-shakes
 * unused icons out of the production bundle, so this dictionary is the
 * single source of truth for what's actually shipped (~30 SVG paths,
 * ≈3 KB total compressed).
 *
 * MDI icons use the standard 24×24 viewBox with filled paths designed for
 * `currentColor` fill (not stroke). That makes them slightly heavier
 * visually than line icons — which fits "this should look polished" better
 * than my earlier hand-drawn outlines.
 */

import type { JSX } from 'preact';

import {
  // File types — outline variants where available, to keep the listing airy
  mdiFolderOutline,
  mdiFileOutline,
  mdiImageOutline,
  mdiPlayCircleOutline,
  mdiMusicNoteOutline,
  mdiFilePdfBox,
  mdiFileDocumentOutline,
  mdiFileTableOutline,
  mdiFilePresentationBox,
  mdiCodeTags,
  mdiTextBoxOutline,
  mdiPackageVariantClosed,
  mdiApplicationBracesOutline,
  mdiFormatFont,
  mdiLanguageHtml5,
  // UI actions
  mdiRefresh,
  mdiUpload,
  mdiDownload,
  mdiPlus,
  mdiClose,
  mdiCheck,
  mdiTrashCanOutline,
  mdiPencilOutline,
  mdiFilePlusOutline,
  mdiChevronDown,
  mdiChevronUp,
  mdiChevronRight,
  mdiDotsHorizontal,
  // Status / theme / misc
  mdiWeatherSunny,
  mdiWeatherNight,
  mdiMonitor,
  mdiCloudUploadOutline,
  mdiLockOutline,
  mdiTranslate,
  mdiLogout,
} from '@mdi/js';

export type IconName =
  // File types
  | 'folder' | 'file' | 'image' | 'video' | 'audio' | 'pdf'
  | 'doc' | 'sheet' | 'slides' | 'code' | 'text' | 'archive'
  | 'executable' | 'font' | 'markup'
  // UI actions
  | 'refresh' | 'upload' | 'download' | 'plus' | 'x' | 'check'
  | 'trash' | 'pencil' | 'file-plus'
  | 'chevron-down' | 'chevron-up' | 'chevron-right' | 'more'
  // Status / theme
  | 'sun' | 'moon' | 'monitor' | 'cloud-upload' | 'lock' | 'languages' | 'logout';

const paths: Record<IconName, string> = {
  // file types
  folder:     mdiFolderOutline,
  file:       mdiFileOutline,
  image:      mdiImageOutline,
  video:      mdiPlayCircleOutline,
  audio:      mdiMusicNoteOutline,
  pdf:        mdiFilePdfBox,
  doc:        mdiFileDocumentOutline,
  sheet:      mdiFileTableOutline,
  slides:     mdiFilePresentationBox,
  code:       mdiCodeTags,
  text:       mdiTextBoxOutline,
  archive:    mdiPackageVariantClosed,
  executable: mdiApplicationBracesOutline,
  font:       mdiFormatFont,
  markup:     mdiLanguageHtml5,
  // ui actions
  refresh:        mdiRefresh,
  upload:         mdiUpload,
  download:       mdiDownload,
  plus:           mdiPlus,
  x:              mdiClose,
  check:          mdiCheck,
  trash:          mdiTrashCanOutline,
  pencil:         mdiPencilOutline,
  'file-plus':    mdiFilePlusOutline,
  'chevron-down': mdiChevronDown,
  'chevron-up':   mdiChevronUp,
  'chevron-right':mdiChevronRight,
  more:           mdiDotsHorizontal,
  // status / theme
  sun:            mdiWeatherSunny,
  moon:           mdiWeatherNight,
  monitor:        mdiMonitor,
  'cloud-upload': mdiCloudUploadOutline,
  lock:           mdiLockOutline,
  languages:      mdiTranslate,
  logout:         mdiLogout,
};

type Props = { name: IconName; size?: number; class?: string };

export function Icon({ name, size = 18, class: cls }: Props): JSX.Element {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="currentColor"
      class={cls}
      aria-hidden="true"
      focusable="false"
    >
      <path d={paths[name]} />
    </svg>
  );
}
