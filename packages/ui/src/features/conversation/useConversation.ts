import type { AppEvent, SessionId, Speaker, TurnId } from "@sunset/client";
import { useCallback, useReducer } from "react";
import { useClient } from "../../client-context";

export interface Message {
  id: TurnId;
  speaker: Speaker;
  text: string;
  /** Still streaming. */
  pending: boolean;
  error?: string;
}

interface State {
  sessionId: SessionId | null;
  messages: Message[];
}

type Action =
  | { type: "session"; sessionId: SessionId }
  | { type: "upsert"; id: TurnId; speaker: Speaker; text: string; pending: boolean; append?: boolean }
  | { type: "failed"; id: TurnId; message: string };

/** Events can arrive before `sendMessage` resolves, so every update is an upsert by turn id. */
function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "session":
      return { ...state, sessionId: action.sessionId };
    case "upsert": {
      const i = state.messages.findIndex((m) => m.id === action.id);
      if (i === -1) {
        const m: Message = {
          id: action.id,
          speaker: action.speaker,
          text: action.text,
          pending: action.pending,
        };
        return { ...state, messages: [...state.messages, m] };
      }
      const old = state.messages[i]!;
      if (!old.pending && action.append) return state; // late delta after the final text
      const m: Message = {
        ...old,
        text: action.append ? old.text + action.text : action.text,
        pending: action.pending,
      };
      return { ...state, messages: state.messages.with(i, m) };
    }
    case "failed": {
      const i = state.messages.findIndex((m) => m.id === action.id);
      const base: Message = state.messages[i] ?? {
        id: action.id,
        speaker: "tutor",
        text: "",
        pending: false,
      };
      const m: Message = { ...base, pending: false, error: action.message };
      return {
        ...state,
        messages: i === -1 ? [...state.messages, m] : state.messages.with(i, m),
      };
    }
  }
}

export function useConversation() {
  const client = useClient();
  const [state, dispatch] = useReducer(reducer, { sessionId: null, messages: [] });

  const onEvent = useCallback(
    (event: AppEvent) => {
      // Until the first reply we do not know our session id, so accept any session.
      const mine = (id: SessionId) => state.sessionId === null || id === state.sessionId;
      switch (event.type) {
        case "tutor_delta":
          if (mine(event.sessionId)) {
            dispatch({ type: "upsert", id: event.turnId, speaker: "tutor", text: event.text, pending: true, append: true });
          }
          break;
        case "turn_finalized": {
          const t = event.turn;
          if (mine(t.sessionId)) {
            dispatch({ type: "upsert", id: t.id, speaker: t.speaker, text: t.text, pending: false });
          }
          break;
        }
        case "tutor_failed":
          if (mine(event.sessionId)) dispatch({ type: "failed", id: event.turnId, message: event.message });
          break;
      }
    },
    [state.sessionId],
  );

  const send = useCallback(
    async (text: string) => {
      const result = await client.sendMessage(state.sessionId, text);
      dispatch({ type: "session", sessionId: result.sessionId });
      const t = result.userTurn;
      dispatch({ type: "upsert", id: t.id, speaker: t.speaker, text: t.text, pending: false });
      dispatch({ type: "upsert", id: result.tutorTurnId, speaker: "tutor", text: "", pending: true, append: true });
    },
    [client, state.sessionId],
  );

  return { messages: state.messages, send, onEvent };
}

export type ConversationState = ReturnType<typeof useConversation>;
