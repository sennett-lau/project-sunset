import type { ConceptId, GraphView } from "@sunset/client";
import { useQuery } from "@tanstack/react-query";
import cytoscape, { type Core, type ElementDefinition } from "cytoscape";
import { useEffect, useMemo, useRef, useState } from "react";
import { useClient } from "../../client-context";
import { keys } from "../../query-keys";

/** How far from the focused concept the focused view reaches. */
const HOPS = 2;

/**
 * Focused concept graph (initial spec Part I §7.3). Shows the selected concept and its
 * neighbourhood, or everything. The layout only re-runs when the set of visible nodes changes,
 * so the view stays still while you read.
 */
export function Graph({ focus, onSelect }: { focus: ConceptId | null; onSelect: (id: ConceptId) => void }) {
  const client = useClient();
  const { data, error, isPending, refetch } = useQuery({ queryKey: keys.graph, queryFn: () => client.graph() });
  const [showAll, setShowAll] = useState(false);
  const container = useRef<HTMLDivElement>(null);
  const cy = useRef<Core | null>(null);
  const shownKey = useRef("");
  const select = useRef(onSelect);
  select.current = onSelect;

  const elements = useMemo(() => (data ? visible(data, showAll ? null : focus) : []), [data, focus, showAll]);

  useEffect(() => {
    if (!container.current) return;
    const instance = cytoscape({
      container: container.current,
      wheelSensitivity: 0.3,
      style: [
        {
          selector: "node",
          style: {
            label: "data(label)",
            "background-color": "#fb923c",
            width: 18,
            height: 18,
            "font-size": 11,
            color: "#292524",
            "text-valign": "bottom",
            "text-margin-y": 4,
          },
        },
        { selector: "node.focus", style: { "background-color": "#c2410c", width: 26, height: 26, "font-weight": "bold" } },
        {
          selector: "edge",
          style: {
            label: "data(label)",
            width: 1.5,
            "line-color": "#d6d3d1",
            "target-arrow-color": "#a8a29e",
            "target-arrow-shape": "triangle",
            "curve-style": "bezier",
            "font-size": 9,
            color: "#78716c",
            "text-rotation": "autorotate",
            "text-background-color": "#ffffff",
            "text-background-opacity": 1,
          },
        },
      ],
    });
    instance.on("tap", "node", (e) => select.current(e.target.id()));
    cy.current = instance;
    const resize = new ResizeObserver(() => instance.resize());
    resize.observe(container.current);
    return () => {
      resize.disconnect();
      instance.destroy();
      cy.current = null;
      shownKey.current = "";
    };
  }, []);

  useEffect(() => {
    const c = cy.current;
    if (!c) return;
    const key = elements
      .map((e) => e.data.id)
      .sort()
      .join(",");
    if (key !== shownKey.current) {
      c.elements().remove();
      c.add(elements);
      c.layout({ name: "cose", animate: false, fit: true, padding: 40 }).run();
      shownKey.current = key;
    } else {
      for (const e of elements) c.getElementById(e.data.id!).data("label", e.data.label);
    }
    c.nodes().removeClass("focus");
    if (focus) c.getElementById(focus).addClass("focus");
  }, [elements, focus]);

  const empty = data && data.nodes.length === 0;
  return (
    <div className="relative flex h-full flex-col">
      {isPending && <p role="status" className="p-6">Loading concept graph…</p>}
      {error && <p role="alert" className="p-6">Could not load graph. <button onClick={() => void refetch()}>Retry</button></p>}
      {empty && <p className="absolute inset-x-0 top-0 p-6 text-stone-500">No concepts yet. Saved proposals will connect your ideas here.</p>}
      {data && !empty && (
        <label className="absolute right-3 top-3 z-10 flex items-center gap-1.5 rounded bg-white/90 px-2 py-1 text-xs text-stone-600 shadow-sm">
          <input type="checkbox" checked={showAll || !focus} disabled={!focus} onChange={(e) => setShowAll(e.target.checked)} />
          Show all concepts
        </label>
      )}
      <div ref={container} className="min-h-0 w-full flex-1" aria-hidden="true" />
      {data && !empty && <div className="border-t border-stone-200 p-3 text-xs">
        <div className="mb-2 flex items-center justify-between"><p>Open a concept to read its article.</p><button className="underline" onClick={() => cy.current?.fit(undefined, 40)}>Fit graph</button></div>
        <ul aria-label="Visible concepts" className="flex max-h-24 flex-wrap gap-2 overflow-auto">{elements.filter((e) => !e.data.source).map((e) => <li key={e.data.id}><button className="rounded border border-stone-200 px-2 py-1" onClick={() => onSelect(e.data.id!)}>{e.data.label}</button></li>)}</ul>
      </div>}
    </div>
  );
}

/** Nodes within `HOPS` of `focus` (all nodes when `focus` is null), and the edges between them. */
export function visible(graph: GraphView, focus: ConceptId | null): ElementDefinition[] {
  let keep = new Set(graph.nodes.map((n) => n.id));
  if (focus && keep.has(focus)) {
    keep = new Set([focus]);
    for (let i = 0; i < HOPS; i++) {
      const frontier = new Set(keep);
      for (const e of graph.edges) {
        if (frontier.has(e.from)) keep.add(e.to);
        if (frontier.has(e.to)) keep.add(e.from);
      }
    }
  }
  const nodes: ElementDefinition[] = graph.nodes
    .filter((n) => keep.has(n.id))
    .map((n) => ({ data: { id: n.id, label: n.title } }));
  const edges: ElementDefinition[] = graph.edges
    .filter((e) => keep.has(e.from) && keep.has(e.to))
    .map((e) => ({
      data: {
        id: e.id,
        source: e.from,
        target: e.to,
        label: e.label ?? e.kind.toLowerCase().replaceAll("_", " "),
      },
    }));
  return [...nodes, ...edges];
}
