// ---------------------------------------------------------------------------
// Welcome — the first-run / no-project surface.
//
// Layout: three vertical bands on a grid (title / middle line / CTA).
//
// Background art — "seismic wave" metaphor, all synced to one cycle:
//   1. 线条划过中心 — a horizontal line sweeps left → right. At 50%
//      it passes through dead center.
//   2. 圆圈震动 — the moment the line hits center, a circular ripple
//      emanates outward and vibrates.
//   3. 碰到勒洛三角形 变形 — when the ripple reaches the Reuleaux
//      triangle boundary, it deforms: the circle cross-fades into
//      the Reuleaux shape.
//   4. 继续往外延展 — the Reuleaux wave continues expanding outward,
//      covering most of the screen, then fades.
//
// No center bright spot — the source dot has been removed. The wave
// originates from the line crossing center, not from a visible point.
// ---------------------------------------------------------------------------

import Titlebar from "./Titlebar";
import type { Locale, Dict } from "./i18n";

// Reuleaux triangle SVG path — three circular arcs, not straight lines.
// Vertices: top (50,8), bottom-right (86,70), bottom-left (14,70).
// Each arc has radius ≈ 71.7 (= side length) and bulges outward.
const REULEAUX_PATH =
  "M50,8 A71.7,71.7 0 0,1 86,70 A71.7,71.7 0 0,1 14,70 A71.7,71.7 0 0,1 50,8 Z";

interface WelcomeProps {
  tr: Dict;
  locale: Locale;
  onPickFolder: () => void;
  error?: string | null;
}

export default function Welcome({ tr, locale, onPickFolder, error }: WelcomeProps) {
  return (
    <div className="app-shell">
      <Titlebar locale={locale} />
      <div className="empty-state">
        {/* Background art — line sweep → one ripple → fuse into Reuleaux → extend. */}
        <div className="welcome-aura">
          {/* 波纹 — a single ripple emanates when the line hits center.
              As it reaches the Reuleaux boundary it slowly fades and
              fuses into the Reuleaux shape (gradual cross-fade). */}
          <div className="welcome-aura-ripple" />
          {/* 勒洛三角形 (morph) — fades in as the ripple reaches the
              boundary and fuses, then extends outward to cover most
              of the screen. */}
          <div className="welcome-aura-glyph-wave">
            <svg viewBox="0 0 100 100" aria-hidden="true">
              <path d={REULEAUX_PATH} />
            </svg>
          </div>
          {/* 勒洛三角形 (static) — faint outline, the logo shape,
              always visible as a quiet boundary marker. */}
          <div className="welcome-aura-glyph">
            <svg viewBox="0 0 100 100" aria-hidden="true">
              <path d={REULEAUX_PATH} />
            </svg>
          </div>
        </div>

        {/* Band 1 — title. */}
        <div className="welcome-title-band">
          <h1 className="welcome-title">
            <span>{tr.welcome}</span>
            <span className="title-space" />
            <span className="title-brand">{tr.brand}</span>
          </h1>
        </div>

        {/* Band 2 — sweeping line + tagline floating above it. */}
        <div className="welcome-middle">
          <div className="welcome-line">
            <div className="welcome-line-track" />
            <div className="welcome-line-glow" />
          </div>
          <p className="welcome-line-tagline">{tr.tagline}</p>
        </div>

        {/* Band 3 — CTA (folder pick only, no arrow). */}
        <div className="welcome-cta-band">
          <div className="cta-slot">
            <div className="cta-shape cta-shape-folder is-active">
              <button
                type="button"
                className="welcome-cta welcome-cta-primary"
                onClick={onPickFolder}
                aria-label={tr.cta}
              >
                <span className="cta-label">{tr.cta}</span>
                <span className="cta-sub">{tr.pickFolderHint}</span>
              </button>
            </div>
          </div>
        </div>

        {/* Error toast (if folder pick / init fails). */}
        {error && <p className="welcome-error">{error}</p>}
      </div>
    </div>
  );
}
