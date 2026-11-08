import type { ConceptId } from "@sunset/client";
import { useQuery } from "@tanstack/react-query";
import { useClient } from "../../client-context";
import { keys } from "../../query-keys";

const SIZE = 480;
const R = 170;

/**
 * Phase 0 graph: concepts on a circle, typed edges between them. The focused, interactive
 * Cytoscape.js graph replaces this in Phase 1.
 */
export function Graph({ onSelect }: { onSelect: (id: ConceptId) => void }) {
  const client = useClient();
  const { data } = useQuery({ queryKey: keys.graph, queryFn: () => client.graph() });
  if (!data) return null;
  if (data.nodes.length === 0) {
    return <p className="p-6 text-stone-500">No concepts yet.</p>;
  }
  const c = SIZE / 2;
  const pos = new Map(
    data.nodes.map((n, i) => {
      const a = (2 * Math.PI * i) / data.nodes.length - Math.PI / 2;
      return [n.id, { x: c + R * Math.cos(a), y: c + R * Math.sin(a) }];
    }),
  );

  return (
    <svg viewBox={`0 0 ${SIZE} ${SIZE}`} className="mx-auto h-full max-h-[600px] w-full">
      {data.edges.map((e) => {
        const a = pos.get(e.from);
        const b = pos.get(e.to);
        if (!a || !b) return null;
        return (
          <g key={e.id}>
            <line x1={a.x} y1={a.y} x2={b.x} y2={b.y} stroke="#d6d3d1" strokeWidth="1.5" />
            <text x={(a.x + b.x) / 2} y={(a.y + b.y) / 2 - 4} textAnchor="middle" className="fill-stone-500 text-[10px]">
              {e.label ?? e.kind.toLowerCase().replaceAll("_", " ")}
            </text>
          </g>
        );
      })}
      {data.nodes.map((n) => {
        const p = pos.get(n.id)!;
        return (
          <g key={n.id} onClick={() => onSelect(n.id)} className="cursor-pointer">
            <circle cx={p.x} cy={p.y} r="8" className="fill-orange-400" />
            <text x={p.x} y={p.y + 22} textAnchor="middle" className="fill-stone-800 text-xs">
              {n.title}
            </text>
          </g>
        );
      })}
    </svg>
  );
}
