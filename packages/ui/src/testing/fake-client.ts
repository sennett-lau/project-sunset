// In-memory SunsetClient for UI tests. Mimics the Phase 0 mock loop of sunset-core.
import type {
  AppEvent,
  Concept,
  ConceptPage,
  Proposal,
  ProposalId,
  SaveMode,
  SunsetClient,
} from "@sunset/client";

let n = 0;
const id = () => `00000000-0000-7000-8000-${String(++n).padStart(12, "0")}`;
const now = () => new Date().toISOString();

export function createFakeClient(): SunsetClient {
  const listeners = new Set<(e: AppEvent) => void>();
  const emit = (e: AppEvent) => listeners.forEach((l) => l(e));
  const proposals: Proposal[] = [];
  let concepts: Concept[] = [];
  let mode: SaveMode = "approval";

  const find = (pid: ProposalId) => {
    const p = proposals.find((x) => x.id === pid);
    if (!p) throw new Error("proposal not found");
    return p;
  };
  const update = (p: Proposal, patch: Partial<Proposal>) => {
    Object.assign(p, patch);
    emit({ type: "proposal_updated", proposal: { ...p } });
    return { ...p };
  };

  return {
    async sendMessage(sessionId, text) {
      const sid = sessionId ?? id();
      const userTurn = { id: id(), sessionId: sid, speaker: "user" as const, text, createdAt: now() };
      const tutorTurnId = id();
      emit({ type: "turn_finalized", turn: userTurn });
      queueMicrotask(() => {
        const answer = "A VAE encodes data as a distribution.";
        for (const w of answer.split(/(?<= )/)) emit({ type: "tutor_delta", sessionId: sid, turnId: tutorTurnId, text: w });
        emit({ type: "turn_finalized", turn: { id: tutorTurnId, sessionId: sid, speaker: "tutor", text: answer, createdAt: now() } });
        const proposal: Proposal = {
          id: id(),
          sessionId: sid,
          evidenceTurnIds: [userTurn.id, tutorTurnId],
          summary: "Add Variational Autoencoder",
          changes: [
            { op: "upsert_concept", slug: "variational-autoencoder", title: "Variational Autoencoder", aliases: ["VAE"] },
            { op: "add_observation", concept: "variational-autoencoder", kind: "question", source: "observed", note: `Asked: "${text}"` },
          ],
          status: "awaiting_approval",
          commitId: null,
          createdAt: now(),
        };
        proposals.unshift(proposal);
        emit({ type: "proposal_created", proposal: { ...proposal } });
      });
      return { sessionId: sid, userTurn, tutorTurnId };
    },
    async listTurns() {
      return [];
    },
    async pendingProposals() {
      return proposals.filter((p) => p.status === "awaiting_approval").map((p) => ({ ...p }));
    },
    async recentProposals(limit) {
      return proposals.slice(0, limit).map((p) => ({ ...p }));
    },
    async approveProposal(pid) {
      const p = find(pid);
      concepts = [{ id: id(), slug: "variational-autoencoder", title: "Variational Autoencoder", aliases: ["VAE"], createdAt: now() }];
      const commitId = id();
      const out = update(p, { status: "committed", commitId });
      emit({ type: "knowledge_changed", commitId });
      return out;
    },
    async rejectProposal(pid) {
      return update(find(pid), { status: "rejected" });
    },
    async undoProposal(pid) {
      concepts = [];
      const out = update(find(pid), { status: "undone" });
      emit({ type: "knowledge_changed", commitId: id() });
      return out;
    },
    async listConcepts() {
      return concepts.map(({ id, slug, title }) => ({ id, slug, title }));
    },
    async conceptPage(cid) {
      const concept = concepts.find((c) => c.id === cid);
      if (!concept) throw new Error("concept not found");
      const page: ConceptPage = {
        concept,
        article: null,
        outgoing: [],
        incoming: [],
        overlay: [
          { id: id(), conceptId: cid, kind: "question", source: "observed", note: 'Asked: "What is a VAE?"', evidenceTurnIds: [], createdAt: now() },
        ],
        visuals: [
          {
            id: id(),
            conceptId: cid,
            revisionId: null,
            title: "Encoder and decoder",
            rendererVersion: 1,
            createdAt: now(),
            payload: {
              kind: "concept_flow",
              nodes: [
                { id: "enc", label: "Encoder" },
                { id: "dec", label: "Decoder" },
              ],
              edges: [{ source: "enc", target: "dec", label: null }],
            },
          },
        ],
      };
      return page;
    },
    async graph() {
      return { nodes: await this.listConcepts(), edges: [] };
    },
    async saveMode() {
      return mode;
    },
    async setSaveMode(m) {
      mode = m;
    },
    onEvent(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
