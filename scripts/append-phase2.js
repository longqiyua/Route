// Append the new styles for Checkpoint modal, Mermaid view, AI badge,
// Undo/Redo, StandardMode header, etc.
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
   StandardMode — workspace header, action bar, summary, watch dot
   ========================================================================= */

.standard-header {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-bottom: 2px;
}

.standard-header h2 {
  font-family: var(--font-display);
  font-weight: 500;
  font-size: 19px;
  margin: 0;
  color: var(--fg);
  letter-spacing: -0.015em;
}

.mode-pill {
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  padding: 2px 9px;
  border-radius: 999px;
  background: var(--accent-soft);
  border: 1px solid var(--accent-line);
  color: var(--accent-bright);
}

.mode-desc {
  margin: 0 0 14px 0;
  font-size: 12px;
  color: var(--fg-dim);
  line-height: 1.6;
}

.workspace-summary {
  display: flex;
  flex-wrap: wrap;
  gap: 18px;
  padding: 12px 0 4px;
  border-top: 1px solid var(--hairline);
  margin-top: 4px;
}

.workspace-summary > div {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: 12px;
}

.ws-summary-label {
  font-family: var(--font-mono);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: var(--fg-dim);
}

.ws-summary-val {
  color: var(--fg);
  font-weight: 500;
}

/* Watch dot on the auto-tracking button */
.workspace-actions .primary .watch-dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--fg-faint);
  margin-right: 4px;
  vertical-align: middle;
  transition: background 0.18s var(--ease), box-shadow 0.18s var(--ease);
}

.workspace-actions .primary.is-on .watch-dot {
  background: var(--accent);
  box-shadow: 0 0 6px var(--accent-soft);
}

/* Undo / Redo buttons get an arrow glyph */
.workspace-actions button[title*="撤销"],
.workspace-actions button[title*="Undo"] {
  min-width: 78px;
}

.workspace-actions button[title*="重做"],
.workspace-actions button[title*="Redo"] {
  min-width: 78px;
}

.checkpoint-btn {
  font-weight: 600;
}

/* =========================================================================
   Checkpoint modal
   ========================================================================= */

.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(8, 6, 14, 0.62);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 50;
  animation: fade-in 0.18s var(--ease);
}

@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

.modal-card {
  width: min(480px, 92vw);
  background: linear-gradient(180deg, rgba(28, 25, 40, 0.92), rgba(18, 16, 28, 0.92));
  border: 1px solid var(--hairline-2);
  border-radius: 14px;
  padding: 22px 22px 18px;
  box-shadow: 0 18px 60px rgba(0, 0, 0, 0.5);
  animation: modal-rise 0.22s var(--ease);
}

@keyframes modal-rise {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

.modal-card h3 {
  margin: 0 0 6px 0;
  font-family: var(--font-display);
  font-weight: 500;
  font-size: 16px;
  color: var(--fg);
  letter-spacing: -0.01em;
}

.modal-hint {
  margin: 0 0 14px 0;
  font-size: 12px;
  line-height: 1.55;
  color: var(--fg-dim);
}

.modal-label {
  display: flex;
  flex-direction: column;
  gap: 5px;
  margin-bottom: 12px;
}

.modal-label > span {
  font-family: var(--font-mono);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.14em;
  color: var(--fg-dim);
}

.modal-textarea {
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.55;
  min-height: 110px;
  resize: vertical;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}

.modal-btn {
  padding: 7px 14px;
  font-family: var(--font-ui);
  font-size: 12px;
  font-weight: 500;
  background: var(--glass);
  border: 1px solid var(--hairline);
  color: var(--fg-muted);
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.16s var(--ease), border-color 0.16s var(--ease),
    color 0.16s var(--ease);
}

.modal-btn:hover {
  background: var(--glass-2);
  border-color: var(--hairline-3);
  color: var(--fg);
}

.modal-btn.primary {
  background: var(--accent);
  color: #0a0a0b;
  border-color: var(--accent);
}

.modal-btn.primary:hover:not(:disabled) {
  background: var(--accent-bright);
  border-color: var(--accent-bright);
}

.modal-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

/* =========================================================================
   Timeline — AI badge, checkpoint star, rollback button
   ========================================================================= */

.timeline-item.op-ai .timeline-dot {
  background: var(--lavender);
  box-shadow: 0 0 0 4px rgba(179, 166, 207, 0.18);
}

.timeline-item.op-checkpoint .timeline-dot {
  background: var(--accent);
  box-shadow: 0 0 0 4px var(--accent-soft);
}

.timeline-item.op-checkpoint .timeline-card {
  border-color: var(--accent-line);
  background: var(--accent-soft);
}

.timeline-ai-pill {
  display: inline-block;
  font-family: var(--font-mono);
  font-size: 9.5px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--lavender);
  color: #0a0a0b;
  margin-right: 6px;
  vertical-align: middle;
}

.timeline-checkmark {
  display: inline-block;
  color: var(--accent-bright);
  font-size: 13px;
  margin-right: 4px;
  vertical-align: middle;
}

.timeline-rollback {
  margin-left: auto;
  padding: 2px 9px;
  font-family: var(--font-mono);
  font-size: 10.5px;
  background: transparent;
  border: 1px solid var(--hairline-2);
  border-radius: 999px;
  color: var(--fg-muted);
  cursor: pointer;
  transition: background 0.16s var(--ease), border-color 0.16s var(--ease),
    color 0.16s var(--ease);
}

.timeline-rollback:hover {
  background: var(--accent-soft);
  border-color: var(--accent-line);
  color: var(--accent-bright);
}

/* =========================================================================
   Mermaid view
   ========================================================================= */

.mermaid-pane {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.mermaid-toolbar {
  display: flex;
  gap: 8px;
  align-items: center;
}

.mermaid-toolbar .settings-input-pick {
  padding: 5px 12px;
  font-family: var(--font-mono);
  font-size: 11px;
}

.mermaid-frame {
  min-height: 360px;
  padding: 24px 16px;
  background: linear-gradient(180deg, rgba(244, 241, 251, 0.92), rgba(238, 234, 248, 0.92));
  border: 1px solid var(--hairline);
  border-radius: 12px;
  overflow: auto;
  display: flex;
  align-items: flex-start;
  justify-content: center;
}

.mermaid-frame svg {
  max-width: 100%;
  height: auto;
}

:root[data-theme="light"] .mermaid-frame {
  background: linear-gradient(180deg, rgba(244, 241, 251, 0.92), rgba(238, 234, 248, 0.92));
}

:root[data-theme="dark"] .mermaid-frame {
  background: linear-gradient(180deg, rgba(28, 25, 40, 0.5), rgba(20, 18, 32, 0.5));
}

.mermaid-source {
  border: 1px solid var(--hairline);
  border-radius: 10px;
  padding: 8px 12px;
  background: var(--glass);
}

.mermaid-source summary {
  cursor: pointer;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--fg-muted);
  letter-spacing: 0.08em;
  text-transform: uppercase;
  user-select: none;
}

.mermaid-source-pre {
  margin: 8px 0 0 0;
  max-height: 220px;
  overflow: auto;
  padding: 10px 12px;
  background: rgba(0, 0, 0, 0.32);
  border-radius: 8px;
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.55;
  color: var(--fg-muted);
  white-space: pre;
}

.mermaid-error {
  margin: 0;
  padding: 12px;
  background: var(--danger-soft);
  border: 1px solid rgba(217, 138, 130, 0.32);
  border-radius: 8px;
  color: var(--danger);
  font-family: var(--font-mono);
  font-size: 11px;
  white-space: pre-wrap;
  word-break: break-word;
}
`;

let c = fs.readFileSync(filePath, "utf8");
c = c.trimEnd() + "\n" + append;
fs.writeFileSync(filePath, c, "utf8");
console.log("CSS appended. File length:", c.length);
