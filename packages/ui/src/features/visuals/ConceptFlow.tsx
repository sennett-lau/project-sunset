import type { FlowEdge, FlowNode } from "@sunset/client";

const BOX_W = 112;
const BOX_H = 44;
const GAP = 28;

/**
 * Boxes left to right in dependency order, with arrows. Good for short pipelines such as
 * Input → Encoder → Latent → Decoder → Reconstruction. Plain SVG text: labels cannot inject markup.
 */
export function ConceptFlow({ nodes, edges }: { nodes: FlowNode[]; edges: FlowEdge[] }) {
  const order = topoOrder(nodes, edges);
  const x = new Map(order.map((n, i) => [n.id, i * (BOX_W + GAP)]));
  const width = order.length * (BOX_W + GAP) - GAP;
  const y = 8;

  return (
    <svg viewBox={`0 0 ${width} ${BOX_H + 16}`} className="w-full" role="img">
      <defs>
        <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">
          <path d="M0,0 L10,5 L0,10 z" fill="#a8a29e" />
        </marker>
      </defs>
      {edges.map((e, i) => {
        const sx = x.get(e.source);
        const tx = x.get(e.target);
        if (sx === undefined || tx === undefined) return null;
        const forward = tx > sx;
        const x1 = forward ? sx + BOX_W : sx;
        const x2 = forward ? tx - 2 : tx + BOX_W + 2;
        return (
          <line key={i} x1={x1} y1={y + BOX_H / 2} x2={x2} y2={y + BOX_H / 2} stroke="#a8a29e" strokeWidth="1.5" markerEnd="url(#arrow)" />
        );
      })}
      {order.map((n) => (
        <g key={n.id} transform={`translate(${x.get(n.id)}, ${y})`}>
          <rect width={BOX_W} height={BOX_H} rx="8" fill="#fff7ed" stroke="#fdba74" />
          <foreignObject width={BOX_W} height={BOX_H}>
            <div className="flex h-full items-center justify-center px-1.5 text-center text-xs leading-tight text-stone-800">
              {n.label}
            </div>
          </foreignObject>
        </g>
      ))}
    </svg>
  );
}

/** Nodes ordered so edges point forward where possible; cycles keep input order. */
function topoOrder(nodes: FlowNode[], edges: FlowEdge[]): FlowNode[] {
  const indegree = new Map(nodes.map((n) => [n.id, 0]));
  for (const e of edges) indegree.set(e.target, (indegree.get(e.target) ?? 0) + 1);
  const queue = nodes.filter((n) => indegree.get(n.id) === 0);
  const out: FlowNode[] = [];
  const seen = new Set<string>();
  while (queue.length) {
    const n = queue.shift()!;
    if (seen.has(n.id)) continue;
    seen.add(n.id);
    out.push(n);
    for (const e of edges.filter((e) => e.source === n.id)) {
      const d = (indegree.get(e.target) ?? 1) - 1;
      indegree.set(e.target, d);
      const target = nodes.find((m) => m.id === e.target);
      if (d === 0 && target) queue.push(target);
    }
  }
  return [...out, ...nodes.filter((n) => !seen.has(n.id))];
}
