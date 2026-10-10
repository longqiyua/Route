// Route logo — Reuleaux triangle (with circular hole, evenodd) + "route" wordmark.
// Mirrors route-logo.html: glyph uses the single accent color #B4AEEA.

function reuleauxPath(cx: number, cy: number, R: number): string {
  // Equilateral triangle vertices, apex up (270°, 30°, 150°).
  const angles = [270, 30, 150].map((d) => (d * Math.PI) / 180);
  const pts = angles.map((a) => ({ x: cx + R * Math.cos(a), y: cy + R * Math.sin(a) }));
  const side = R * Math.sqrt(3);
  return (
    `M ${pts[0].x} ${pts[0].y} ` +
    `A ${side} ${side} 0 0 1 ${pts[1].x} ${pts[1].y} ` +
    `A ${side} ${side} 0 0 1 ${pts[2].x} ${pts[2].y} ` +
    `A ${side} ${side} 0 0 1 ${pts[0].x} ${pts[0].y} Z`
  );
}

function circlePath(cx: number, cy: number, r: number): string {
  return (
    `M ${cx + r} ${cy} ` +
    `A ${r} ${r} 0 1 0 ${cx - r} ${cy} ` +
    `A ${r} ${r} 0 1 0 ${cx + r} ${cy} Z`
  );
}

export default function Logo({
  size = 16,
  withWordmark = true,
  variant = "filled",
}: {
  size?: number;
  withWordmark?: boolean;
  variant?: "filled" | "outline";
}) {
  const S = size;
  const cx = S / 2;
  // Visual center correction: reuleaux apex-up shifts perceived center upward.
  const cy = S / 2 + (size >= 16 ? 0.4 : 0.2);
  const R = S * 0.44;
  const holeR = R * 0.3;

  const outer = reuleauxPath(cx, cy, R);
  const hole = circlePath(cx, cy, holeR);

  const isOutline = variant === "outline";
  const strokeWidth = Math.max(0.6, S * 0.018);

  return (
    <span className="route-logo">
      <svg width={S} height={S} viewBox={`0 0 ${S} ${S}`} style={{ overflow: "visible" }}>
        <path
          d={`${outer} ${hole}`}
          fill={isOutline ? "none" : "var(--accent)"}
          stroke={isOutline ? "var(--accent)" : "none"}
          strokeWidth={isOutline ? strokeWidth : 0}
          fillRule="evenodd"
          style={{ transform: `translateY(${size >= 16 ? 0.5 : 0}px)` }}
        />
      </svg>
      {withWordmark && (
        <span className="wordmark" style={{ marginLeft: size >= 16 ? 9 : 7 }}>
          route
        </span>
      )}
    </span>
  );
}
