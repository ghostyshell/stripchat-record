# Stripchat Recorder design system

The "midnight console" family look, with this app's own accent. Near-black
ink, one accent spectrum, brass as the single warm note, hairline glass
surfaces, monospaced data, staggered entrances, and a hard reduced-motion kill
switch.

Everything is implemented in `app/src/styles.css`. Tokens live on `:root`;
raw hex in components is banned.

## Tokens

```css
--ink-0: #04060c;  --ink-1: #070a12;  --ink-2: #0b0f1a;
--surface: rgba(16, 20, 34, 0.72);
--text: #eef2fb;  --text-dim: #a5b0c7;  --text-faint: #7b869f;

/* this app's spectrum */
--laser: #fb7185;  --laser-2: #ff9aa9;  --brass: #f2c078;
--good: #4ae3a5;   --warn: #ffc46b;        --bad: #ff8484;

--radius-xl: 22px; --radius-lg: 16px; --radius: 12px; --radius-sm: 9px;
--ease: cubic-bezier(0.22, 0.8, 0.24, 1);
--ease-out: cubic-bezier(0.16, 1, 0.3, 1);
```

## Fonts

- Chakra Petch for UI copy.
- JetBrains Mono for every identifier, path, size and timestamp: if a number
  changes on screen it is mono.

## Shell layout

```
topbar      62px (84px on macOS for the overlay title bar), drag region
rail        236px nav, 4 sections, collapses to icons under 880px
workspace   one scrolling page, content max-width 980px
inspector   fixed right column 300px: output card + job queue, hidden under 1240px
statusbar   34px: ffmpeg state, version
```

The macOS overlay inset is applied from `app/src/main.tsx`
(`document.documentElement.dataset.os = "mac"`), never by padding the brand
sideways.

## Motion

Entrances: `.page` fades and rises 10px (500ms), direct children stagger.
Only transform and opacity animate. Hard kill switch:

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.001ms !important;
    transition-duration: 0.001ms !important;
  }
}
```

## Accessibility

- `:focus-visible` rings in `--laser-2`, offset 2px.
- `aria-current="page"` on the rail, `role="status"` on toasts, skip link to
  `#workspace`, Alt+1..4 section jumps.
- State chips pair a dot with a word; colour is never the only signal.
