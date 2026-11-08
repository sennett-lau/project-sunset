import type { VisualPayload } from "@sunset/client";
import { ConceptFlow } from "./ConceptFlow";

/** Picks the trusted renderer for each payload kind. Model output is data, never code. */
export function Visual({ payload }: { payload: VisualPayload }) {
  switch (payload.kind) {
    case "concept_flow":
      return <ConceptFlow nodes={payload.nodes} edges={payload.edges} />;
    case "mermaid":
    case "plot":
    case "parameter_explorer":
    case "equation_steps":
      return (
        <p className="text-sm text-stone-500">
          A {payload.kind.replaceAll("_", " ")} renderer arrives in a later phase.
        </p>
      );
  }
}
