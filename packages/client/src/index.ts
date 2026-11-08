// The contract between the UI and a backend. The desktop app implements it over Tauri; a web
// app can implement it over HTTP + WebSocket. UI code never calls Tauri directly.

import type {
  AppEvent,
  ConceptId,
  ConceptPage,
  ConceptSummary,
  GraphView,
  ModelSettings,
  ProbeReport,
  Proposal,
  ProposalId,
  ProviderId,
  ProviderInput,
  ProviderProfile,
  Role,
  SaveMode,
  SendMessageResult,
  SessionId,
  Turn,
} from "@sunset/bindings";

export type * from "@sunset/bindings";

export interface SunsetClient {
  /** Returns at once. The answer arrives as `tutor_delta` events, then `turn_finalized`. */
  sendMessage(sessionId: SessionId | null, text: string): Promise<SendMessageResult>;
  listTurns(sessionId: SessionId): Promise<Turn[]>;

  pendingProposals(): Promise<Proposal[]>;
  /** Newest first, any status. */
  recentProposals(limit: number): Promise<Proposal[]>;
  approveProposal(id: ProposalId): Promise<Proposal>;
  rejectProposal(id: ProposalId): Promise<Proposal>;
  /** Undo a committed proposal. History is kept. */
  undoProposal(id: ProposalId): Promise<Proposal>;

  listConcepts(): Promise<ConceptSummary[]>;
  conceptPage(id: ConceptId): Promise<ConceptPage>;
  graph(): Promise<GraphView>;

  saveMode(): Promise<SaveMode>;
  setSaveMode(mode: SaveMode): Promise<void>;

  /** Providers, which model each role uses, and what each model was found to support. */
  modelSettings(): Promise<ModelSettings>;
  /** `id` null creates a provider. `input.apiKey`: null keeps the stored key, "" removes it. */
  saveProvider(id: ProviderId | null, input: ProviderInput): Promise<ProviderProfile>;
  deleteProvider(id: ProviderId): Promise<void>;
  /** Model ids the server lists. Not every server supports this. */
  listModels(id: ProviderId): Promise<string[]>;
  /** Probe what a model supports. The result is remembered. */
  testConnection(id: ProviderId, model: string): Promise<ProbeReport>;
  setRole(role: Role, providerId: ProviderId, model: string): Promise<void>;

  /** Receive every core event. Returns a function that stops listening. */
  onEvent(listener: (event: AppEvent) => void): () => void;
}
