const fs = require("fs");
const path = "C:\\Users\\longq\\Desktop\\route (1)\\crates\\route-tauri\\web\\src\\styles.css";
let c = fs.readFileSync(path, "utf8");

const append = `

/* =========================================================================
   Sidebar — multi-project, master switch, add-project bar
   ========================================================================= */

.sidebar {
  display: flex;
  flex-direction: column;
  width: 248px;
  background: var(--glass-2);
  border-right: 1px solid var(--hairline-1);
  padding: 18px 14px 16px;
  gap: 14px;
  overflow-y: auto;
}

.brand-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 4px 4px;
  border-bottom: 1px solid var(--hairline-1);
}

/* Master switch (running/off) */
.sidebar-master {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  background: var(--glass-1);
  border: 1px solid var(--hairline-1);
  border-radius: 10px;
  font-size: 12px;
  letter-spacing: 0.05em;
}
.sidebar-master-label {
  color: var(--fg-muted);
  text-transform: uppercase;
  font-size: 10.5px;
  letter-spacing: 0.18em;
}
.sidebar-master-switch {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 4px 10px 4px 8px;
  border: 1px solid var(--hairline-2);
  background: var(--glass-2);
  border-radius: 999px;
  cursor: pointer;
  font-size: 12px;
  color: var(--fg-muted);
  transition: all 0.22s var(--ease);
}
.sidebar-master-switch:hover { border-color: var(--hairline-3); }
.sidebar-master-switch .master-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--fg-faint);
  transition: all 0.4s var(--ease);
}
.sidebar-master-switch.is-on { color: var(--accent-bright); border-color: rgba(180, 174, 234, 0.45); }
.sidebar-master-switch.is-on .master-dot {
  background: var(--accent);
  box-shadow: 0 0 6px rgba(180, 174, 234, 0.5);
}

/* Add-project wide bar */
.sidebar-add {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  width: 100%;
  height: 36px;
  background: var(--glass-2);
  border: 1px dashed var(--hairline-2);
  border-radius: 8px;
  color: var(--fg-muted);
  font-size: 12px;
  letter-spacing: 0.08em;
  cursor: pointer;
  transition: all 0.22s var(--ease);
}
.sidebar-add:hover {
  background: var(--glass-3);
  border-color: var(--hairline-3);
  color: var(--fg);
}
.sidebar-add-icon { display: inline-flex; }
.sidebar-add-icon svg { width: 13px; height: 13px; }
.sidebar-add-text { font-family: var(--font-ui); }

/* Project cards */
.project-cards {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  min-height: 80px;
}
.empty-projects {
  text-align: center;
  color: var(--fg-faint);
  font-size: 12px;
  padding: 24px 0;
  letter-spacing: 0.08em;
}
.project-card {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 12px;
  background: var(--glass-1);
  border: 1px solid var(--hairline-1);
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.22s var(--ease);
  position: relative;
}
.project-card:hover {
  background: var(--glass-2);
  border-color: var(--hairline-2);
}
.project-card.active {
  background: var(--glass-3);
  border-color: rgba(180, 174, 234, 0.4);
  box-shadow: 0 0 0 1px rgba(180, 174, 234, 0.08) inset;
}
.project-card-row1 {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.project-card-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--fg);
  font-family: var(--font-ui);
  letter-spacing: 0.01em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.project-card-remove {
  width: 18px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: var(--fg-faint);
  cursor: pointer;
  font-size: 16px;
  line-height: 1;
  border-radius: 4px;
  transition: all 0.18s var(--ease);
}
.project-card-remove:hover { color: var(--fg); background: var(--glass-3); }
.project-card-row2 {
  display: flex;
  gap: 6px;
  align-items: center;
}
.ep-pill {
  display: inline-block;
  padding: 1px 8px;
  border-radius: 999px;
  font-size: 10px;
  letter-spacing: 0.08em;
  font-family: var(--font-ui);
  text-transform: uppercase;
}
.ep-pill.ep-local { background: rgba(180, 174, 234, 0.16); color: var(--accent-bright); }
.ep-pill.ep-cloud { background: rgba(225, 195, 248, 0.16); color: #d3b9ed; }
.routea-pill {
  display: inline-block;
  padding: 1px 8px;
  border-radius: 999px;
  font-size: 10px;
  letter-spacing: 0.08em;
  background: rgba(155, 232, 200, 0.16);
  color: #95d8b8;
  text-transform: uppercase;
  font-family: var(--font-ui);
}
.project-card-path {
  font-size: 10.5px;
  color: var(--fg-faint);
  font-family: var(--font-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;
  text-align: left;
}

/* =========================================================================
   Page rail (top-right page switcher)
   ========================================================================= */

.page-rail {
  display: inline-flex;
  align-self: flex-end;
  margin: 0 0 16px;
  gap: 2px;
  padding: 3px;
  background: var(--glass-1);
  border: 1px solid var(--hairline-1);
  border-radius: 999px;
}
.page-rail button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  height: 28px;
  background: transparent;
  border: none;
  border-radius: 999px;
  color: var(--fg-muted);
  font-size: 12px;
  font-family: var(--font-ui);
  letter-spacing: 0.06em;
  cursor: pointer;
  transition: all 0.22s var(--ease);
}
.page-rail button:hover { color: var(--fg); }
.page-rail button.active {
  background: var(--accent-soft);
  color: var(--accent-bright);
}
.page-rail .page-glyph { display: inline-flex; }
.page-rail .page-glyph svg { width: 14px; height: 14px; }

/* =========================================================================
   Settings page
   ========================================================================= */

.settings-page h2 {
  font-family: var(--font-display);
  font-size: 18px;
  font-weight: 600;
  letter-spacing: 0.02em;
  margin: 0 0 18px;
  color: var(--fg);
}

.settings-row {
  display: grid;
  grid-template-columns: 220px 1fr;
  gap: 24px;
  padding: 14px 0;
  border-top: 1px solid var(--hairline-1);
  align-items: center;
}
.settings-row:first-of-type { border-top: none; }
.settings-row-label { display: flex; flex-direction: column; gap: 2px; }
.settings-row-label > span:first-child {
  font-size: 12.5px;
  color: var(--fg);
  font-weight: 500;
}
.settings-row-hint {
  font-size: 10.5px;
  color: var(--fg-faint);
  letter-spacing: 0.04em;
}
.settings-row-control { display: flex; align-items: center; gap: 12px; }
.settings-input {
  flex: 1;
  height: 32px;
  background: var(--glass-1);
  border: 1px solid var(--hairline-1);
  border-radius: 8px;
  color: var(--fg);
  font-size: 12.5px;
  padding: 0 12px;
  font-family: var(--font-mono);
  letter-spacing: 0.01em;
  outline: none;
  transition: all 0.2s var(--ease);
}
.settings-input:focus { border-color: rgba(180, 174, 234, 0.45); }
.settings-input::placeholder { color: var(--fg-faint); }

.seg-control {
  display: inline-flex;
  background: var(--glass-1);
  border: 1px solid var(--hairline-1);
  border-radius: 999px;
  padding: 2px;
  gap: 2px;
}
.seg-control button {
  padding: 5px 14px;
  border: none;
  background: transparent;
  border-radius: 999px;
  font-size: 11.5px;
  color: var(--fg-muted);
  cursor: pointer;
  font-family: var(--font-ui);
  letter-spacing: 0.04em;
  transition: all 0.22s var(--ease);
}
.seg-control button:hover { color: var(--fg); }
.seg-control button.active {
  background: var(--accent-soft);
  color: var(--accent-bright);
}

.settings-cheatsheet {
  margin-top: 28px;
  padding: 16px 18px;
  background: var(--glass-1);
  border: 1px solid var(--hairline-1);
  border-radius: 10px;
}
.settings-cheatsheet h3 {
  font-size: 12px;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: var(--fg-muted);
  margin: 0 0 10px;
  font-family: var(--font-ui);
  font-weight: 500;
}
.cheat-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12.5px;
}
.cheat-table th {
  text-align: left;
  font-weight: 500;
  font-size: 10.5px;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--fg-faint);
  padding: 6px 12px 6px 0;
  border-bottom: 1px solid var(--hairline-1);
}
.cheat-table td {
  padding: 8px 12px 8px 0;
  vertical-align: top;
  color: var(--fg);
}
.cheat-table td:first-child { width: 50%; }
.cheat-table code {
  background: var(--glass-2);
  padding: 1px 6px;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--accent-bright);
}
.cheat-note {
  margin: 12px 0 0;
  font-size: 11.5px;
  color: var(--fg-faint);
  letter-spacing: 0.04em;
}
.cheat-note code {
  background: var(--glass-2);
  padding: 1px 6px;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 11px;
}

.settings-privacy {
  margin-top: 24px;
  padding: 16px 18px;
  background: var(--glass-1);
  border: 1px solid var(--hairline-1);
  border-left: 2px solid var(--accent);
  border-radius: 10px;
}
.settings-privacy h3 {
  font-size: 12px;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: var(--accent-bright);
  margin: 0 0 10px;
  font-family: var(--font-ui);
  font-weight: 500;
}
.settings-privacy p {
  font-size: 12.5px;
  color: var(--fg-muted);
  margin: 0 0 8px;
  line-height: 1.6;
  letter-spacing: 0.02em;
}
.settings-privacy p:last-child { margin-bottom: 0; }
.settings-privacy strong { color: var(--fg); font-weight: 600; }

/* =========================================================================
   History page (timeline)
   ========================================================================= */

.history-page { display: flex; flex-direction: column; gap: 18px; }
.history-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.history-header h2 {
  font-family: var(--font-display);
  font-size: 18px;
  font-weight: 600;
  margin: 0;
  color: var(--fg);
  letter-spacing: 0.02em;
}
.history-controls { display: flex; align-items: center; gap: 10px; }
.history-search { width: 220px; }

.timeline {
  list-style: none;
  margin: 0;
  padding: 0;
  position: relative;
}
.timeline::before {
  content: "";
  position: absolute;
  left: 11px;
  top: 8px;
  bottom: 8px;
  width: 1px;
  background: var(--hairline-1);
}
.timeline-item {
  position: relative;
  padding: 4px 0 14px 36px;
}
.timeline-dot {
  position: absolute;
  left: 6px;
  top: 10px;
  width: 11px;
  height: 11px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 0 3px var(--bg-1);
}
.timeline-card {
  background: var(--glass-1);
  border: 1px solid var(--hairline-1);
  border-radius: 10px;
  padding: 10px 14px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.timeline-row1 {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 12px;
}
.timeline-msg {
  font-size: 13px;
  color: var(--fg);
  font-family: var(--font-ui);
}
.timeline-time {
  font-size: 10.5px;
  color: var(--fg-faint);
  font-family: var(--font-mono);
  letter-spacing: 0.02em;
  white-space: nowrap;
}
.timeline-row2 {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 12px;
}
.timeline-author { font-size: 11px; color: var(--fg-muted); }
.timeline-id { font-size: 10.5px; color: var(--fg-faint); font-family: var(--font-mono); }

/* =========================================================================
   Workspace (BasicMode stub) — stats + actions
   ========================================================================= */

.workspace-stats {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
  margin: 16px 0 24px;
}
.stat-card {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 14px 16px;
  background: var(--glass-1);
  border: 1px solid var(--hairline-1);
  border-radius: 10px;
}
.stat-label {
  font-size: 10.5px;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: var(--fg-faint);
  font-family: var(--font-ui);
}
.stat-value {
  font-size: 18px;
  color: var(--fg);
  font-family: var(--font-display);
  font-weight: 600;
  letter-spacing: 0.02em;
}
.workspace-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}
.workspace-actions .primary {
  background: var(--accent-soft);
  border-color: rgba(180, 174, 234, 0.4);
  color: var(--accent-bright);
}
`;

c = c.trimEnd() + "\n" + append;
fs.writeFileSync(path, c, "utf8");
console.log("CSS appended. New length: " + c.length);
