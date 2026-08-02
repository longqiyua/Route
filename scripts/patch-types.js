const fs = require("fs");
const path = "C:\\Users\\longq\\Desktop\\route (1)\\crates\\route-tauri\\web\\src\\App.tsx";
let c = fs.readFileSync(path, "utf8");

// 1. CommitDto uses created_at, not timestamp
c = c.replace(/new Date\(a\.timestamp\)\.getTime\(\)/g, "new Date((a as any).created_at).getTime()");
c = c.replace(/new Date\(b\.timestamp\)\.getTime\(\)/g, "new Date((b as any).created_at).getTime()");
c = c.replace(/formatTs\(c\.timestamp\)/g, "formatTs((c as any).created_at)");

// 2. StatusDto: no 'pending' field, but has current_branch. We can also
//    derive "pending" from workingDirStatus if needed — for now use
//    info.current_branch as one of the stats and add a derived pending
//    via workingDirStatus call... but keep it simple: just show
//    current_branch + branch count + latest snapshot present.
c = c.replace(/info\?\.pending/g, "0");
c = c.replace(/<span className="stat-label">待处理<\/span>/, "<span className=\"stat-label\">分支数</span>");
c = c.replace(/<span className="stat-value">{info\?\.pending \?\? 0}<\/span>/, "<span className=\"stat-value\">{info?.branches?.length ?? 0}</span>");

// 3. TreeNode.kind has no "file" — it's "root" | "branch" | "snapshot".
//    countFiles: treat every leaf as a file. All nodes are kind root/branch/snapshot
//    (no "file" kind), so just count branches + snapshots. But for display
//    we want a count of "tracked units". Use branches + snapshots in tree.
c = c.replace(/node\.kind === "file" \|\| node\.kind === "snapshot"/, "node.kind === \"snapshot\"");
c = c.replace(/function countFiles\(node: TreeNode \| null \| undefined\): number \{[\s\S]*?return children\.reduce\(\(sum, c\) => sum \+ countFiles\(c\), 0\);\n\}/, `function countFiles(node: TreeNode | null | undefined): number {
  if (!node) return 0;
  if (node.kind === "snapshot") return 1;
  const children = (node as { children?: TreeNode[] }).children ?? [];
  return children.reduce((sum, c) => sum + countFiles(c), 0);
}`);

fs.writeFileSync(path, c, "utf8");
console.log("Patched. New length: " + c.length);
