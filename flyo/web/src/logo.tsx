/**
 * Flyo wordmark — three stacked rounded rectangles, smallest to largest,
 * suggesting a tidy stack of files. Uses `currentColor` with stacked opacity
 * so the same SVG works in light and dark themes (the colour is inherited
 * from the parent's `color` CSS).
 */

import type { JSX } from 'preact';

type Props = { size?: number; class?: string };

export function Logo({ size = 24, class: cls }: Props): JSX.Element {
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
      <rect x="4" y="3"  width="16" height="4" rx="1.5" opacity="0.35" />
      <rect x="2" y="8"  width="20" height="5" rx="2"   opacity="0.6"  />
      <rect x="0" y="14" width="24" height="9" rx="2.5" />
    </svg>
  );
}
