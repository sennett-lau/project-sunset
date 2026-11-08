// SunsetClient over Tauri IPC. The only file in the app that touches Tauri APIs.
import { commands, events } from "@sunset/bindings";
import type { SunsetClient } from "@sunset/client";

export const tauriClient: SunsetClient = {
  searchKnowledge: commands.searchKnowledge,
  listSessions: commands.listSessions,
  exportPublicToFolder: commands.exportPublicToFolder,
  exportPublic: commands.exportPublic,
  vaultStatus: commands.vaultStatus,
  syncVault: commands.syncVault,
  sendMessage: commands.sendMessage,
  listTurns: commands.listTurns,
  pendingProposals: commands.pendingProposals,
  recentProposals: commands.recentProposals,
  editProposal: commands.editProposal,
  proposalVersions: commands.proposalVersions,
  approveProposal: commands.approveProposal,
  rejectProposal: commands.rejectProposal,
  undoProposal: commands.undoProposal,
  listConcepts: commands.listConcepts,
  conceptPage: commands.conceptPage,
  graph: commands.graph,
  saveMode: commands.saveMode,
  setSaveMode: async (mode) => {
    await commands.setSaveMode(mode);
  },
  modelSettings: commands.modelSettings,
  saveProvider: commands.saveProvider,
  deleteProvider: async (id) => {
    await commands.deleteProvider(id);
  },
  listModels: commands.listModels,
  testConnection: commands.testConnection,
  setRole: async (role, providerId, model) => {
    await commands.setRole(role, providerId, model);
  },
  onEvent(listener) {
    const unlisten = events.coreEvent.listen((e) => listener(e.payload));
    return () => void unlisten.then((stop) => stop());
  },
};
