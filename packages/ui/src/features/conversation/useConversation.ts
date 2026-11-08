import type { AppEvent, SessionId, Speaker, TurnId } from "@sunset/client";
import { useCallback, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { keys } from "../../query-keys";
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

/** Buffer before acknowledgement: only the returned session may consume early events. */
export function useConversation() {
  const client = useClient();
  const queries = useQueryClient();
  const [state, setState] = useState<State>({ sessionId: null, messages: [] });
  const current = useRef(state);
  const lock = useRef(false);
  const buffer = useRef<AppEvent[] | null>(null);
  const [working, setWorking] = useState<"sending" | "loading" | null>(null);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const replace = useCallback((next: State) => { current.current = next; setState(next); }, []);
  const dispatch = useCallback((action: Action) => replace(reducer(current.current, action)), [replace]);
  const apply = useCallback((event: AppEvent) => {
    const sid = current.current.sessionId;
    if (!sid) return;
    if (event.type === "tutor_delta" && event.sessionId === sid)
      dispatch({ type: "upsert", id: event.turnId, speaker: "tutor", text: event.text, pending: true, append: true });
    if (event.type === "turn_finalized" && event.turn.sessionId === sid)
      dispatch({ type: "upsert", id: event.turn.id, speaker: event.turn.speaker, text: event.turn.text, pending: false });
    if (event.type === "tutor_failed" && event.sessionId === sid)
      dispatch({ type: "failed", id: event.turnId, message: event.message });
  }, [dispatch]);
  const onEvent = useCallback((event: AppEvent) => {
    if (event.type !== "tutor_delta" && event.type !== "turn_finalized" && event.type !== "tutor_failed") return;
    if (buffer.current) buffer.current.push(event);
    else apply(event);
  }, [apply]);
  const isBusy = () => lock.current || current.current.messages.some((m) => m.pending);

  const send = async (text: string) => {
    if (isBusy()) return;
    lock.current = true;
    setWorking("sending");
    buffer.current = [];
    try {
      const result = await client.sendMessage(current.current.sessionId, text);
      dispatch({ type: "session", sessionId: result.sessionId });
      dispatch({ type: "upsert", id: result.userTurn.id, speaker: "user", text: result.userTurn.text, pending: false });
      dispatch({ type: "upsert", id: result.tutorTurnId, speaker: "tutor", text: "", pending: true, append: true });
      for (const event of buffer.current) apply(event);
      void queries.invalidateQueries({ queryKey: keys.sessions });
    } catch (error) {
      // Preserve updates to an already known session even if acknowledgement fails.
      for (const event of buffer.current) apply(event);
      throw error;
    } finally {
      buffer.current = null;
      lock.current = false;
      setWorking(null);
    }
  };

  const resume = async (sessionId: SessionId) => {
    if (isBusy() || sessionId === current.current.sessionId) return;
    lock.current = true;
    setWorking("loading");
    setHistoryError(null);
    buffer.current = [];
    try {
      const turns = await client.listTurns(sessionId);
      replace({ sessionId, messages: turns.map((t) => ({ id: t.id, speaker: t.speaker, text: t.text, pending: false })) });
      for (const event of buffer.current) apply(event);
    } catch (error) {
      setHistoryError(String(error));
      // A failed load leaves the previous transcript intact.
      for (const event of buffer.current) apply(event);
    } finally {
      buffer.current = null;
      lock.current = false;
      setWorking(null);
    }
  };

  const newConversation = () => {
    if (isBusy()) return;
    replace({ sessionId: null, messages: [] });
    setHistoryError(null);
  };
  return { ...state, send, onEvent, resume, newConversation, historyError,
    loadingHistory: working === "loading",
    busy: Boolean(working) || state.messages.some((m) => m.pending) };
}

export type ConversationState = ReturnType<typeof useConversation>;
