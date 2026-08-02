// Read current file, find broken replacement chars + ask, then rewrite via .NET after they exist already in this file.
// Strategy: just write the full new components block as a UTF-8 string using fs.
const fs = require("fs");
const path = "C:\\Users\\longq\\Desktop\\route (1)\\crates\\route-tauri\\web\\src\\App.tsx";
let content = fs.readFileSync(path, "utf8");

// Detect line 481 broken char: "工作\ufffd?" (U+FFFD + '?') => replace with correct utf8 of 台
// Actually simpler: read the file as bytes, scan for U+FFFD, log
const broken = (content.match(/\uFFFD/g) || []).length;
console.log("Found " + broken + " U+FFFD broken chars");

// Replace '台' that got mangled to "\uFFFD?":
// pattern: "工作" + U+FFFD + "?"  → "工作台"
content = content.replace(/工作\uFFFD\?/g, "工作台");
const after = (content.match(/\uFFFD/g) || []).length;
console.log("After replace: " + after + " U+FFFD remaining");

// Also "设" replacement? Search for "?置" pattern
content = content.replace(/\uFFFD\?置/g, "设置");
content = content.replace(/设置\uFFFD\?/g, "设置");

// Also 倒序, 顺序
content = content.replace(/\uFFFD\?序/g, "倒序");
content = content.replace(/倒\uFFFD\?/g, "倒序");
content = content.replace(/顺\uFFFD\?/g, "顺序");

// settings-page '设置' h2
content = content.replace(/设置\uFFFD?\s*·/g, "设置 ·");

// Inspect specific problems: at line ~480 around page rail.
const lines = content.split("\n");
console.log("Lines 478-492:");
for (let i = 477; i < 495 && i < lines.length; i++) {
  console.log((i+1) + ": " + lines[i]);
}

fs.writeFileSync(path, content, "utf8");
console.log("Saved. Length: " + content.length);
