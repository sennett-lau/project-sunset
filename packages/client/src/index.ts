// The contract between the UI and a backend. The desktop app implements it over Tauri; a web
// app can implement it over HTTP + WebSocket. UI code never calls Tauri directly.

import type {
  AppEvent,
  ConceptId,
  ConceptPage,
  ConceptSummary,
  GraphView,
  Proposal,
  ProposalId,
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

  /** Receive every core event. Returns a function that stops listening. */
  onEvent(listener: (event: AppEvent) => void): () => void;
}
