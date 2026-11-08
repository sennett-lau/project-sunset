import type { ConceptId } from "@sunset/client";

export const keys = {
  sessions: ["sessions"] as const,
  vault: ["vault"] as const,
  search: (query: string) => ["knowledge-search", query] as const,
  pending: ["proposals", "pending"] as const,
  recent: ["proposals", "recent"] as const,
  concepts: ["concepts"] as const,
  concept: (id: ConceptId) => ["concept", id] as const,
  graph: ["graph"] as const,
  saveMode: ["saveMode"] as const,
  models: ["models"] as const,
  providerModels: (id: string) => ["providerModels", id] as const,
};
