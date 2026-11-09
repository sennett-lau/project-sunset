// In-memory SunsetClient for UI tests. Mimics the Phase 0 mock loop of sunset-core.
import type {
  AppEvent,
  ArticleRevision,
  Concept,
  ConceptPage,
  ModelCapabilities,
  Proposal,
  ProposalId,
  ProposalVersion,
  ProviderProfile,
  RoleAssignment,
  SaveMode,
  Session,
  Turn,
  SunsetClient,
} from "@sunset/client";

let n = 0;
const id = () => `00000000-0000-7000-8000-${String(++n).padStart(12, "0")}`;
const now = () => new Date().toISOString();

export function createFakeClient(): SunsetClient {
  const listeners = new Set<(e: AppEvent) => void>();
  const emit = (e: AppEvent) => listeners.forEach((l) => l(e));
  const proposals: Proposal[] = [];
  const articles = new Map<string, ArticleRevision>();
  const previousArticles = new Map<string, ArticleRevision>();
  const versions = new Map<ProposalId, ProposalVersion[]>();
  const sessions: Session[] = [];
  const turns: Turn[] = [];
  let concepts: Concept[] = [];
  let mode: SaveMode = "approval";
  const untested: ModelCapabilities = { streaming: "needs_test", jsonSchema: "needs_test", toolCalls: "needs_test", probedAt: null };
  const demo: ProviderProfile = { id: id(), name: "Demo (offline, scripted)", kind: "demo", baseUrl: "", hasApiKey: false, isLocal: true, createdAt: now() };
  const providers: ProviderProfile[] = [demo];
  const roles: RoleAssignment[] = (["tutor", "curator"] as const).map((role) => ({
    role,
    providerId: demo.id,
    model: "scripted",
    capabilities: untested,
  }));
  /** Keys the UI sent; exposed so tests can check they are never read back. */
  const keychain = new Map<string, string>();

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
    async searchKnowledge(query, limit) {
      return concepts.filter((c) => [c.title, ...c.aliases].some((text) => text.toLocaleLowerCase().includes(query.toLocaleLowerCase()))).slice(0, limit).map(({ id, slug, title }) => ({ id, slug, title }));
    },
    async listSessions(limit) { return sessions.slice(0, limit).map((s) => ({ ...s })); },
    async exportPublic() { return []; },
    async exportPublicToFolder() { return "/test/exports/public-" + id(); },
    async vaultStatus() { return { path: "/test/vault", conflicts: [], error: null }; },
    async syncVault() { return this.vaultStatus(); },
    async sendMessage(sessionId, text) {
      const sid = sessionId ?? id();
      if (!sessions.some((s) => s.id === sid)) sessions.unshift({ id: sid, createdAt: now() });
      const userTurn = { id: id(), sessionId: sid, speaker: "user" as const, text, createdAt: now() };
      const tutorTurnId = id();
      turns.push(userTurn);
      emit({ type: "turn_finalized", turn: userTurn });
      queueMicrotask(() => {
        const answer = "A VAE encodes data as a distribution.";
        for (const w of answer.split(/(?<= )/)) emit({ type: "tutor_delta", sessionId: sid, turnId: tutorTurnId, text: w });
        const tutorTurn: Turn = { id: tutorTurnId, sessionId: sid, speaker: "tutor", text: answer, createdAt: now() };
        turns.push(tutorTurn);
        emit({ type: "turn_finalized", turn: tutorTurn });
        const proposal: Proposal = {
          id: id(),
          sessionId: sid,
          evidenceTurnIds: [userTurn.id, tutorTurnId],
          summary: "Add Variational Autoencoder",
          changes: [
            { op: "upsert_concept", slug: "variational-autoencoder", title: "Variational Autoencoder", aliases: ["VAE"] },
            { op: "add_observation", concept: "variational-autoencoder", kind: "question", source: "observed", note: `Asked: "${text}"` },
          ],
          revision: 0,
          articleBase: null,
          status: "awaiting_approval",
          commitId: null,
          createdAt: now(),
        };
        proposals.unshift(proposal);
        versions.set(proposal.id, [{ revision: 0, summary: proposal.summary, changes: structuredClone(proposal.changes), createdAt: proposal.createdAt }]);
        emit({ type: "proposal_created", proposal: { ...proposal } });
      });
      return { sessionId: sid, userTurn, tutorTurnId };
    },
    async listTurns(sessionId) {
      return turns.filter((t) => t.sessionId === sessionId).map((t) => ({ ...t }));
    },
    async proposeArticleEdit(conceptId, expectedRevisionId, summary, bodyMd) {
      const page = await this.conceptPage(conceptId);
      if (page.article?.id !== expectedRevisionId) throw new Error("Article changed; reopen it before editing.");
      const source = proposals.find((p) => p.commitId === page.article?.commitId);
      if (!source) throw new Error("source proposal not found");
      const proposal: Proposal = {
        id: id(), sessionId: source.sessionId, evidenceTurnIds: [], summary: `Edit article: ${page.concept.title}`,
        articleBase: { conceptId, revisionId: expectedRevisionId }, revision: 0,
        changes: [{ op: "revise_article", concept: page.concept.slug, summary, bodyMd }],
        status: "awaiting_approval", commitId: null, createdAt: now(),
      };
      proposals.unshift(proposal);
      versions.set(proposal.id, [{ revision: 0, summary: proposal.summary, changes: structuredClone(proposal.changes), createdAt: proposal.createdAt }]);
      emit({ type: "proposal_created", proposal: { ...proposal } });
      return proposal;
    },
    async pendingProposals() {
      return proposals.filter((p) => p.status === "awaiting_approval").map((p) => ({ ...p }));
    },
    async recentProposals(limit) {
      return proposals.slice(0, limit).map((p) => ({ ...p }));
    },
    async editProposal(pid, edit) {
      const p = find(pid);
      if (p.status !== "awaiting_approval" || p.revision !== edit.expectedRevision) throw new Error("Proposal changed; review the latest version.");
      if (!edit.summary.trim() || !edit.changes.length) throw new Error("A summary and at least one change are required.");
      const patch = { summary: edit.summary, changes: structuredClone(edit.changes), revision: p.revision + 1 };
      versions.get(pid)!.unshift({ ...patch, createdAt: now() });
      return update(p, patch);
    },
    async proposalVersions(pid) { return structuredClone(versions.get(pid) ?? []); },
    async approveProposal(pid, revision) {
      const p = find(pid);
      if (p.status !== "awaiting_approval" || p.revision !== revision) throw new Error("Proposal changed; review the latest version.");
      if (p.articleBase && articles.get(p.articleBase.conceptId)?.id !== p.articleBase.revisionId) throw new Error("Article changed; reopen it before editing.");
      if (!p.articleBase) concepts = [{ id: id(), slug: "variational-autoencoder", title: "Variational Autoencoder", aliases: ["VAE"], createdAt: now() }];
      const commitId = id();
      const concept = concepts.find((c) => c.id === p.articleBase?.conceptId) ?? concepts[0]!;
      const previous = articles.get(concept.id);
      if (previous) previousArticles.set(pid, previous);
      const change = p.changes.find((c) => c.op === "revise_article");
      articles.set(concept.id, { id: id(), conceptId: concept.id, version: (previous?.version ?? 0) + 1, summary: change?.summary ?? "A VAE learns a latent distribution.", bodyMd: change?.bodyMd ?? "A VAE encodes and reconstructs data.", commitId, createdAt: now() });
      const out = update(p, { status: "committed", commitId });
      emit({ type: "knowledge_changed", commitId });
      return out;
    },
    async rejectProposal(pid, revision) {
      const p = find(pid);
      if (p.status !== "awaiting_approval" || p.revision !== revision) throw new Error("Proposal changed; review the latest version.");
      return update(p, { status: "rejected" });
    },
    async undoProposal(pid) {
      const proposal = find(pid);
      if (proposal.articleBase) {
        const previous = previousArticles.get(pid);
        if (previous) articles.set(proposal.articleBase.conceptId, previous);
      } else { concepts = []; articles.clear(); }
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
        article: articles.get(cid) ?? null,
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
    async modelSettings() {
      return {
        providers: providers.map((p) => ({ ...p })),
        roles: roles.map((r) => ({ ...r })),
        allLocal: roles.every((r) => providers.find((p) => p.id === r.providerId)?.isLocal),
      };
    },
    async saveProvider(pid, input) {
      let p = providers.find((x) => x.id === pid);
      if (!p) {
        p = { id: id(), name: "", kind: "openai_compatible", baseUrl: "", hasApiKey: false, isLocal: false, createdAt: now() };
        providers.push(p);
      }
      p.name = input.name;
      p.baseUrl = input.baseUrl;
      p.isLocal = /localhost|127\.0\.0\.1/.test(input.baseUrl);
      if (input.apiKey !== null) {
        if (input.apiKey) keychain.set(p.id, input.apiKey);
        else keychain.delete(p.id);
        p.hasApiKey = Boolean(input.apiKey);
      }
      return { ...p };
    },
    async deleteProvider(pid) {
      if (roles.some((r) => r.providerId === pid)) throw new Error("still used by a role");
      providers.splice(providers.findIndex((p) => p.id === pid), 1);
    },
    async listModels(pid) {
      return pid === demo.id ? ["scripted"] : ["qwen3:8b", "llama3.2"];
    },
    async testConnection(pid, model) {
      const capabilities: ModelCapabilities = { streaming: "supported", jsonSchema: "supported", toolCalls: "unsupported", probedAt: now() };
      return { providerId: pid, model, reachable: true, capabilities, notes: ["Basic request: OK"] };
    },
    async setRole(role, providerId, model) {
      const r = roles.find((x) => x.role === role);
      if (r) Object.assign(r, { providerId, model, capabilities: untested });
      else roles.push({ role, providerId, model, capabilities: untested });
    },
    async startRecording() {},
    async clearSpeechModel() { const index = roles.findIndex((r) => r.role === "stt"); if (index >= 0) roles.splice(index, 1); },
    async stopRecording() { return "What is a VAE?"; },
    async cancelRecording() {},
    async recordingStatus() { return { recording: false, elapsedMs: 0, autoStopped: false, error: null }; },
    onEvent(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
