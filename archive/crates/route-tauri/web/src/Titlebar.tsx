import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Logo from "./Logo";
import { t as i18nT, type Locale } from "./i18n";

// Guard for non-Tauri (plain browser preview) environments where
// __TAURI_INTERNALS__ is absent. Window-control calls become no-ops.
const hasTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function safeWindow() {
  return hasTauri ? getCurrentWindow() : null;
}

// ---------------------------------------------------------------------------
// Theme persistence
// ---------------------------------------------------------------------------

const THEME_KEY = "route:theme";

export function getInitialTheme(): "dark" | "light" {
  const saved = localStorage.getItem(THEME_KEY);
  if (saved === "light" || saved === "dark") return saved;
  return "dark";
}

export function applyTheme(theme: "dark" | "light") {
  document.documentElement.setAttribute("data-theme", theme);
  localStorage.setItem(THEME_KEY, theme);
}

// ---------------------------------------------------------------------------
// Icons (inline SVG, monochrome)
// ---------------------------------------------------------------------------

function MinimizeIcon() {
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round">
      <path d="M2 6h8" />
    </svg>
  );
}

function MaximizeIcon() {
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.3">
      <rect x="2.2" y="2.2" width="7.6" height="7.6" rx="1" />
    </svg>
  );
}

function RestoreIcon() {
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.3">
      <rect x="2.2" y="3.6" width="6.2" height="6.2" rx="1" />
      <path d="M4.2 3.6V2.8a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-.8" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round">
      <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Drag-region guard
// ---------------------------------------------------------------------------
// In Tauri 2, an element with `data-tauri-drag-region` (no value) marks the
// whole element as a window drag region. A child marked with
// `data-tauri-drag-region="false"` should opt out — but in practice some
// versions / webview builds still swallow the mousedown for the drag, so
// we also call `e.stopPropagation()` on pointerdown/mousedown for every
// interactive button. This is a no-op in a regular browser preview and
// belt-and-suspenders in the Tauri webview.

function stopDrag(e: React.SyntheticEvent) {
  // React passes a SyntheticEvent whose nativeEvent has stopPropagation.
  // Both pointer and mouse events need to be stopped so the OS drag never
  // starts before our click handler fires.
  const ne = e.nativeEvent as Event;
  if (ne && typeof ne.stopPropagation === "function") {
    ne.stopPropagation();
  }
}

// ---------------------------------------------------------------------------
// MenuPanel — the unified standard software menu. Anchored to a corner.
// Generic: takes anchor, items, separator positions.
// ---------------------------------------------------------------------------

export type MenuItem =
  | { kind: "item"; label: string; onClick: () => void; disabled?: boolean; hint?: string }
  | { kind: "sep" };

export function MenuPanel({
  open,
  anchor = "right",
  items,
  onClose,
}: {
  open: boolean;
  anchor?: "left" | "right";
  items: MenuItem[];
  onClose: () => void;
}) {
  // (Empty implementation — kept exported in case other surfaces need it.
  // The titlebar itself no longer hosts a menu, the role is taken by the
  // sidebar page switcher.)
  if (!open) return null;
  return <div className={`menu-panel menu-anchor-${anchor}`} />;
}

// ---------------------------------------------------------------------------
// Titlebar — minimal: brand mark on the left, window controls on the
// right. Page navigation lives in the sidebar bottom (Workspace /
// Timeline / Settings) so the titlebar stays a quiet carrier instead
// of a 2nd-level menu.
// ---------------------------------------------------------------------------

export default function Titlebar({ locale }: { locale: Locale }) {
  const [maximized, setMaximized] = useState(false);
  const tr = i18nT(locale);

  useEffect(() => {
    const win = safeWindow();
    if (!win) return;
    let unlisten: (() => void) | undefined;
    win
      .isMaximized()
      .then(setMaximized)
      .catch(() => {});
    win
      .onResized(() => {
        win.isMaximized().then(setMaximized).catch(() => {});
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, []);

  const handleMinimize = () => safeWindow()?.minimize().catch(() => {});
  const handleMaximize = () => safeWindow()?.toggleMaximize().catch(() => {});
  const handleClose = () => safeWindow()?.close().catch(() => {});

  return (
    <div className="titlebar" data-tauri-drag-region>
      {/* Top-left brand mark — the titlebar is the carrier for the logo
          in the "inside the app" state. The welcome page also shows it,
          but that's the entry point, not a redundant repeat. */}
      <div className="titlebar-left" data-tauri-drag-region>
        <Logo size={16} />
      </div>

      {/* Right side: just the OS window-control trio. Page switching
          moved to the sidebar bottom. */}
      <div className="titlebar-right" data-tauri-drag-region="false">
        <button
          className="tb-icon-btn win-ctrl"
          onClick={handleMinimize}
          data-tauri-drag-region="false"
          onPointerDown={stopDrag}
          onMouseDown={stopDrag}
          title={tr.minimize}
          aria-label={tr.minimize}
        >
          <MinimizeIcon />
        </button>
        <button
          className="tb-icon-btn win-ctrl"
          onClick={handleMaximize}
          data-tauri-drag-region="false"
          onPointerDown={stopDrag}
          onMouseDown={stopDrag}
          title={maximized ? tr.restore : tr.maximize}
          aria-label={maximized ? tr.restore : tr.maximize}
        >
          {maximized ? <RestoreIcon /> : <MaximizeIcon />}
        </button>
        <button
          className="tb-icon-btn win-ctrl close"
          onClick={handleClose}
          data-tauri-drag-region="false"
          onPointerDown={stopDrag}
          onMouseDown={stopDrag}
          title={tr.close}
          aria-label={tr.close}
        >
          <CloseIcon />
        </button>
      </div>
    </div>
  );
}
