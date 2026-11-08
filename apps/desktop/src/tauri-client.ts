// SunsetClient over Tauri IPC. The only file in the app that touches Tauri APIs.
import { commands, events } from "@sunset/bindings";
import type { SunsetClient } from "@sunset/client";

export const tauriClient: SunsetClient = {
  sendMessage: commands.sendMessage,
  listTurns: commands.listTurns,
  pendingProposals: commands.pendingProposals,
  recentProposals: commands.recentProposals,
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
  onEvent(listener) {
    const unlisten = events.coreEvent.listen((e) => listener(e.payload));
    return () => void unlisten.then((stop) => stop());
  },
};
