import type { ProposedChange } from "@sunset/client";

const RELATION_WORDS: Record<string, string> = {
  IS_A: "is a",
  PART_OF: "is part of",
  USES: "uses",
  DEPENDS_ON: "depends on",
  EXPLAINS: "explains",
  RELATED_TO: "is related to",
  CONTRASTS_WITH: "contrasts with",
  APPEARS_IN_SOURCE: "appears in",
};

/** One line per change, for review. */
export function describeChange(c: ProposedChange): { label: string; text: string; private?: boolean } {
  switch (c.op) {
    case "upsert_concept":
      return { label: "Concept", text: c.aliases.length ? `${c.title} (${c.aliases.join(", ")})` : c.title };
    case "revise_article":
      return { label: "Article", text: `${c.concept}: ${c.summary}` };
    case "add_relation":
      return { label: "Link", text: `${c.from} ${RELATION_WORDS[c.kind] ?? c.kind} ${c.to}` };
    case "add_observation":
      return { label: "Private note", text: c.note, private: true };
    case "add_visual":
      return { label: "Visual", text: `${c.title} (${c.payload.kind.replace("_", " ")})` };
  }
}
