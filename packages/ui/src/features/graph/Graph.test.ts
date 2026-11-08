import { expect, test } from "vitest";
import type { GraphView } from "@sunset/client";
import { visible } from "./Graph";

test("focused graph is bounded to two hops regardless of edge order", () => {
  const graph: GraphView = {
    nodes: ["a", "b", "c", "d", "e"].map((id) => ({ id, title: id, slug: id })),
    edges: [["a", "b"], ["b", "c"], ["c", "d"], ["d", "e"]].map(([from, to], i) => ({ id: String(i), from: from!, to: to!, kind: "RELATED_TO", label: null, evidenceTurnIds: [] })),
  };
  const ids = (g: GraphView) => visible(g, "a").filter((e) => !e.data.source).map((e) => e.data.id);
  expect(ids(graph)).toEqual(["a", "b", "c"]);
  expect(ids({ ...graph, edges: [...graph.edges].reverse() })).toEqual(["a", "b", "c"]);
  expect(visible(graph, null)).toHaveLength(9);
});
