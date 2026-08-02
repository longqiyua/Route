// Append the new component styles to styles.css.
// Run from project root: node scripts/append-newcss.js
const fs = require("fs");
const path = require("path");

const filePath = path.join(
  __dirname,
  "..",
  "crates",
  "route-tauri",
  "web",
  "src",
  "styles.css"
);

const append = `
/* =========================================================================
   Multi-project sidebar — top add bar, master switch, project cards.
   Replaces the old single-project brand-row / branch-list layout.
   ========================================================================= */

.app {
  display: grid;
  grid-template-columns: 248px 1fr;
  height: 100%;
  min-height: 0;
}

.sidebar {
  display: flex;
  flex-direction: column;
  width: 248px;
  background: linear-gradient(180deg, rgba(20, 24, 30, 0.72), rgba(13, 15, 19, 0.7));
  backdrop-filter: blur(22px) saturate(135%);
  -webkit-backdrop-filter: blur(22px) saturate(135%);
  border-right: 1px solid var(--hairline);
  padding: 18px 14px 16px;
  gap: 14px;
  overflow-y: auto;
}

.sidebar .brand-row {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 0 4px 4px;
}

.sidebar .brand-row .brand {
  font-family: var(--font-display);
  font-weight: 600;
  font-size: 15px;
  color: var(--fg);
  letter-spacing: -0.02em;
}

.sidebar-master {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 9px 12px;
  background: var(--glass);
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
}

.sidebar-master-label {
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--fg-dim);
}

.sidebar-master-switch {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 4px 10px 4px 8px;
  height: 24px;
  border-radius: 12px;
  border: 1px solid var(--hairline-2);
  background: var(--glass-2);
  cursor: pointer;
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--fg-muted);
  transition: border-color 0.18s var(--ease), background 0.18s var(--ease),
    color 0.18s var(--ease);
}

.sidebar-master-switch:hover {
  border-color: var(--hairline-3);
}

.sidebar-master-switch.is-on {
  border-color: var(--accent-line);
  background: var(--accent-soft);
  color: var(--accent-bright);
}

.sidebar-master-switch .master-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--fg-faint);
  transition: background 0.22s var(--ease), box-shadow 0.22s var(--ease);
}

.sidebar-master-switch.is-on .master-dot {
  background: var(--accent);
  box-shadow: 0 0 6px rgba(180, 174, 234, 0.4);
}

.sidebar-add {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 9px 12px;
  background: var(--glass);
  border: 1px solid var(--hairline);
  border-radius: 999px;
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.08em;
  color: var(--fg-muted);
  cursor: pointer;
  transition: border-color 0.16s var(--ease), background 0.16s var(--ease),
    color 0.16s var(--ease);
}

.sidebar-add:hover {
  border-color: var(--accent-line);
  background: var(--accent-soft);
  color: var(--accent-bright);
  transform: none;
}

.sidebar-add-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
}

.sidebar-add-icon svg {
  width: 14px;
  height: 14px;
}

.sidebar-add-text {
  font-family: var(--font-ui);
  font-size: 12px;
  letter-spacing: 0;
  text-transform: none;
  font-weight: 500;
}

.project-cards {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 2px;
}

.empty-projects {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--fg-dim);
  padding: 14px 4px;
  text-align: center;
  border: 1px dashed var(--hairline);
  border-radius: var(--radius);
  background: rgba(0, 0, 0, 0.18);
}

.project-card {
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 10px 12px;
  background: var(--glass);
  border: 1px solid var(--hairline);
  border-radius: 10px;
  cursor: pointer;
  transition: border-color 0.22s var(--ease), background 0.22s var(--ease),
    transform 0.22s var(--ease);
  position: relative;
}

.project-card:hover {
  border-color: var(--hairline-3);
  background: var(--glass-2);
  transform: translateY(-1px);
}

.project-card.active {
  border-color: var(--accent-line);
  background: var(--accent-soft);
  box-shadow: 0 0 0 1px var(--accent-line);
}

.project-card-row1 {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}

.project-card-name {
  font-family: var(--font-display);
  font-weight: 500;
  font-size: 13px;
  color: var(--fg);
  letter-spacing: -0.01em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}

.project-card-remove {
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  border: 1px solid transparent;
  background: transparent;
  color: var(--fg-dim);
  font-size: 14px;
  line-height: 1;
  cursor: pointer;
  transition: background 0.15s var(--ease), color 0.15s var(--ease),
    border-color 0.15s var(--ease);
}

.project-card-remove:hover {
  color: var(--danger);
  background: var(--danger-soft);
  border-color: rgba(217, 138, 130, 0.32);
}

.project-card-row2 {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  align-items: center;
}

.ep-pill,
.bk-pill,
.routea-pill {
  font-family: var(--font-mono);
  font-size: 9.5px;
  padding: 1px 7px;
  border-radius: 10px;
  border: 1px solid var(--hairline);
  background: var(--glass-2);
  color: var(--fg-muted);
  letter-spacing: 0.06em;
  text-transform: lowercase;
}

.ep-pill.ep-local {
  color: var(--accent-bright);
  border-color: var(--accent-line);
  background: var(--accent-soft);
}

.ep-pill.ep-cloud {
  color: var(--lavender);
  border-color: rgba(179, 166, 207, 0.35);
  background: rgba(179, 166, 207, 0.12);
}

.bk-pill.bk-local {
  color: var(--green);
  border-color: rgba(143, 191, 154, 0.32);
  background: var(--green-soft);
}

.bk-pill.bk-cloud {
  color: var(--ochre);
  border-color: rgba(200, 168, 122, 0.32);
  background: rgba(200, 168, 122, 0.12);
}

.routea-pill {
  color: var(--accent-bright);
  border-color: var(--accent-line);
  background: var(--accent-soft);
}

.project-card-backup {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--fg-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: 2px 0 0;
}

.project-card-path {
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--fg-faint);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: 0;
}

/* =========================================================================
   Top-right page rail — workspace / settings / history switcher.
   ========================================================================= */

.page-rail {
  display: inline-flex;
  align-self: flex-end;
  margin: 0 0 16px;
  gap: 2px;
  padding: 3px;
  background: var(--glass);
  border: 1px solid var(--hairline);
  border-radius: 999px;
}

.page-rail button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 12px;
  font-family: var(--font-ui);
  font-size: 11.5px;
  color: var(--fg-muted);
  background: transparent;
  border: 1px solid transparent;
  border-radius: 999px;
  cursor: pointer;
  transition: background 0.18s var(--ease), color 0.18s var(--ease),
    border-color 0.18s var(--ease);
}

.page-rail button:hover:not(.active) {
  background: var(--glass-2);
  color: var(--fg);
}

.page-rail button.active {
  background: var(--accent-soft);
  border-color: var(--accent-line);
  color: var(--accent-bright);
}

.page-rail .page-glyph {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.page-rail .page-glyph svg {
  width: 13px;
  height: 13px;
}

/* =========================================================================
   Settings page — per-project language, endpoint, backup, route A.
   ========================================================================= */

.settings-page {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.settings-row {
  display: grid;
  grid-template-columns: 200px 1fr;
  gap: 18px;
  align-items: start;
  padding: 12px 0;
  border-bottom: 1px solid var(--hairline);
}

.settings-row:last-of-type {
  border-bottom: none;
}

.settings-row-label {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.settings-row-label > span:first-child {
  font-family: var(--font-display);
  font-weight: 500;
  font-size: 13px;
  color: var(--fg);
  letter-spacing: -0.005em;
}

.settings-row-hint {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--fg-dim);
  line-height: 1.55;
}

.settings-row-control {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.settings-row-control-input {
  display: flex;
  gap: 6px;
  align-items: stretch;
}

.settings-input {
  flex: 1;
  min-width: 220px;
  padding: 7px 11px;
  font-size: 12px;
  font-family: var(--font-mono);
  background: rgba(0, 0, 0, 0.22);
  border: 1px solid var(--hairline);
  border-radius: var(--radius-sm);
  color: var(--fg);
  transition: border-color 0.16s var(--ease), background 0.16s var(--ease);
}

.settings-input::placeholder {
  color: var(--fg-faint);
}

.settings-input:focus {
  outline: none;
  border-color: var(--accent-line);
  background: var(--glass-2);
}

.settings-input-pick {
  padding: 0 14px;
  font-family: var(--font-ui);
  font-size: 12px;
  font-weight: 500;
  background: var(--accent-soft);
  border: 1px solid var(--accent-line);
  color: var(--accent-bright);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 0.16s var(--ease), border-color 0.16s var(--ease);
}

.settings-input-pick:hover {
  background: var(--accent);
  color: #0a0a0b;
  border-color: var(--accent);
}

.seg-control {
  display: inline-flex;
  gap: 2px;
  padding: 3px;
  background: rgba(0, 0, 0, 0.22);
  border: 1px solid var(--hairline);
  border-radius: 999px;
}

.seg-control button {
  padding: 4px 13px;
  font-size: 11.5px;
  font-family: var(--font-ui);
  border: 1px solid transparent;
  background: transparent;
  backdrop-filter: none;
  border-radius: 999px;
  color: var(--fg-muted);
  cursor: pointer;
  transition: background 0.15s var(--ease), color 0.15s var(--ease),
    border-color 0.15s var(--ease);
}

.seg-control button:hover:not(.active) {
  background: var(--glass-2);
  color: var(--fg);
}

.seg-control button.active {
  background: var(--accent-soft);
  border-color: var(--accent-line);
  color: var(--accent-bright);
}

/* Route A toggle label status text */
.switch + .status,
.settings-row-control .status {
  font-family: var(--font-mono);
  font-size: 10.5px;
  letter-spacing: 0.05em;
  color: var(--fg-dim);
}

.settings-row-control .status.on {
  color: var(--accent-bright);
}

/* Route A cheatsheet */
.settings-cheatsheet {
  margin-top: 6px;
  padding: 14px 16px;
  background: var(--glass);
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
}

.settings-cheatsheet h3 {
  font-family: var(--font-mono);
  font-size: 10.5px;
  text-transform: uppercase;
  letter-spacing: 0.16em;
  color: var(--fg-dim);
  margin: 0 0 10px 0;
}

.cheat-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.cheat-table th,
.cheat-table td {
  text-align: left;
  padding: 6px 8px;
  border-bottom: 1px dashed var(--hairline);
  color: var(--fg-muted);
}

.cheat-table th {
  font-family: var(--font-mono);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: var(--fg-dim);
  font-weight: 500;
}

.cheat-table code {
  font-family: var(--font-mono);
  font-size: 11px;
  background: var(--accent-soft);
  border: 1px solid var(--accent-line);
  color: var(--accent-bright);
  padding: 1px 7px;
  border-radius: 4px;
}

.cheat-note {
  margin: 9px 0 0 0;
  font-size: 11px;
  color: var(--fg-dim);
  line-height: 1.6;
}

.cheat-note code {
  font-family: var(--font-mono);
  font-size: 10.5px;
  background: rgba(0, 0, 0, 0.28);
  border: 1px solid var(--hairline);
  padding: 0 5px;
  border-radius: 3px;
  color: var(--fg-muted);
}

/* Privacy notice */
.settings-privacy {
  margin-top: 6px;
  padding: 14px 16px;
  background: var(--danger-soft);
  border: 1px solid rgba(217, 138, 130, 0.25);
  border-radius: var(--radius);
}

.settings-privacy h3 {
  font-family: var(--font-mono);
  font-size: 10.5px;
  text-transform: uppercase;
  letter-spacing: 0.16em;
  color: var(--danger);
  margin: 0 0 8px 0;
}

.settings-privacy p {
  margin: 0 0 7px 0;
  font-size: 11.5px;
  line-height: 1.7;
  color: var(--fg-muted);
}

.settings-privacy p:last-child {
  margin-bottom: 0;
}

.settings-privacy strong {
  color: var(--fg);
  font-weight: 600;
}

/* =========================================================================
   History page — timeline view of commits with sort and search.
   ========================================================================= */

.history-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.history-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.history-header h2 {
  font-family: var(--font-display);
  font-weight: 500;
  font-size: 19px;
  margin: 0;
  color: var(--fg);
  letter-spacing: -0.015em;
}

.history-controls {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.history-search {
  width: 200px;
  min-width: 0;
}

.timeline {
  list-style: none;
  margin: 0;
  padding: 4px 0 0 18px;
  border-left: 1px dashed var(--hairline-2);
  display: flex;
  flex-direction: column;
  gap: 12px;
  position: relative;
}

.timeline-item {
  position: relative;
  padding: 0 0 0 8px;
}

.timeline-dot {
  position: absolute;
  top: 14px;
  left: -23px;
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 0 4px var(--accent-soft);
}

.timeline-card {
  padding: 12px 14px;
  background: var(--glass);
  border: 1px solid var(--hairline);
  border-radius: var(--radius);
  display: flex;
  flex-direction: column;
  gap: 5px;
  transition: border-color 0.16s var(--ease), background 0.16s var(--ease);
}

.timeline-card:hover {
  border-color: var(--hairline-3);
  background: var(--glass-2);
}

.timeline-row1 {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.timeline-msg {
  font-size: 13px;
  color: var(--fg);
  line-height: 1.5;
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.timeline-time {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--fg-dim);
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
}

.timeline-row2 {
  display: flex;
  align-items: center;
  gap: 12px;
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--fg-dim);
}

.timeline-author {
  color: var(--fg-muted);
}

.timeline-id {
  color: var(--accent-bright);
}

.mono {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}

/* =========================================================================
   Workspace page — header cards and primary actions.
   ========================================================================= */

.workspace-stats {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 12px;
  margin: 14px 0 6px;
}

.workspace-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 14px;
}

.workspace-actions .primary {
  background: var(--accent);
  color: #0a0a0b;
  border-color: var(--accent);
  font-weight: 600;
}

.workspace-actions .primary:hover:not(:disabled) {
  background: var(--accent-bright);
  border-color: var(--accent-bright);
  color: #0a0a0b;
}
`;

let c = fs.readFileSync(filePath, "utf8");
c = c.trimEnd() + "\n" + append;
fs.writeFileSync(filePath, c, "utf8");
console.log("CSS appended. File length:", c.length);
