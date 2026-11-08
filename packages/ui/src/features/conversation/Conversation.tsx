import { type FormEvent, useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useClient } from "../../client-context";
import { keys } from "../../query-keys";
import { Markdown } from "../wiki/Markdown";
import type { ConversationState } from "./useConversation";
import { useVoiceInput } from "./useVoiceInput";

export function Conversation({ state, onOpenModels }: { state: ConversationState; onOpenModels?: () => void }) {
  const client = useClient();
  const settings = useQuery({ queryKey: keys.models, queryFn: () => client.modelSettings() });
  const tutor = settings.data?.roles.find((r) => r.role === "tutor");
  const demo = settings.data?.providers.find((p) => p.id === tutor?.providerId)?.kind === "demo";
  const [sending, setSending] = useState(false);
  const inFlight = useRef(false);
  const input = useRef<HTMLTextAreaElement>(null);
  const following = useRef(true);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const bottom = useRef<HTMLDivElement>(null);
  const busy = state.busy;
  const stt = settings.data?.roles.find((r) => r.role === "stt");
  const speechProvider = settings.data?.providers.find((p) => p.id === stt?.providerId);
  const speechReady = !!stt && !!speechProvider && speechProvider.kind !== "demo";
  const voice = useVoiceInput(client, JSON.stringify([state.sessionId, stt?.providerId, stt?.model, speechProvider?.baseUrl]), (text) => {
    setDraft((previous) => previous.trim() ? `${previous.trimEnd()}\n${text}` : text);
    input.current?.focus();
  });
  const sessions = useQuery({ queryKey: keys.sessions, queryFn: () => client.listSessions(100) });
  useEffect(() => { setDraft(""); setError(null); following.current = true; }, [state.sessionId]);

  useEffect(() => {
    if (following.current) bottom.current?.scrollIntoView({ block: "end" });
  }, [state.messages]);

  async function submit(e: FormEvent) {
    e.preventDefault();
    const text = draft.trim();
    if (!text || busy || inFlight.current || voice.isBusy()) return;
    inFlight.current = true;
    setSending(true);
    following.current = true;
    setError(null);
    try {
      await state.send(text);
      setDraft("");
    } catch (err) {
      setError(String(err));
    } finally {
      inFlight.current = false;
      setSending(false);
      input.current?.focus();
    }
  }

  return (
    <>
      <div className="conversation-history">
        <label className="eyebrow" htmlFor="conversation-history">Return to an idea</label>
        <div>
          <select id="conversation-history" aria-label="Conversation history" value={state.sessionId ?? ""} disabled={busy || sending || voice.busy}
            onChange={(e) => { if (e.target.value) void state.resume(e.target.value); }}>
            <option value="">Choose a conversation…</option>
            {state.sessionId && !sessions.data?.some((s) => s.id === state.sessionId) && <option value={state.sessionId}>Current conversation</option>}
            {sessions.data?.map((s, i) => <option key={s.id} value={s.id}>{new Date(s.createdAt).toLocaleString()} · {sessions.data.length - i}</option>)}
          </select>
          <button disabled={busy || sending || voice.busy} onClick={state.newConversation}>New conversation</button>
        </div>
        {sessions.isPending && <p role="status">Loading conversations…</p>}
        {sessions.error && <p role="alert">Could not load conversations. <button onClick={() => void sessions.refetch()}>Retry history</button></p>}
        {state.historyError && <p role="alert">Could not open conversation: {state.historyError}. Choose it again to retry.</p>}
      </div>
      <div className="conversation-scroll" onScroll={(e) => { const el = e.currentTarget; following.current = el.scrollHeight - el.scrollTop - el.clientHeight < 100; }}>
        {demo && <div className="demo-notice"><span className="eyebrow">Offline demo</span><p>A scripted introduction to VAEs. Connect a model for open-ended learning.</p>{onOpenModels && <button onClick={onOpenModels}>Connect a model ↗</button>}</div>}
        {state.messages.length === 0 && (
          <div className="welcome">
            <p className="eyebrow">Start with a question</p>
            <h2>Make room for<br /><em>an idea.</em></h2>
            <p>Talk it through with your tutor. Useful ideas become proposed articles and connections for your personal wiki.</p>
            <button className="starter-question" onClick={() => { setDraft("What is a VAE?"); input.current?.focus(); }}>What is a VAE? <span aria-hidden="true">↗</span></button>
            <ol className="onboarding-steps"><li><strong>Ask</strong> a question in your own words.</li><li><strong>Review</strong> proposed updates when you’re ready.</li><li><strong>Revisit</strong> your wiki as understanding grows.</li></ol>
          </div>
        )}
        {state.messages.map((m) => (
          <div key={m.id} className={m.speaker === "user" ? "flex justify-end" : ""}>
            <div
              className={`message max-w-[95%] rounded-2xl px-4 py-2.5 leading-relaxed ${
                m.speaker === "user" ? "whitespace-pre-wrap bg-orange-100" : "bg-white shadow-sm"
              }`}
            >
              <p className="message-speaker">{m.speaker === "user" ? "You" : "Tutor"}</p>
              {m.speaker === "user" ? m.text : <Markdown source={m.text} className="prose-sm" />}
              {m.pending && <span className="ml-0.5 animate-pulse">▍</span>}
              {m.error && <p role="alert" className="mt-1 text-sm text-red-700">Could not answer: {m.error}</p>}
            </div>
          </div>
        ))}
        <div ref={bottom} />
      </div>
      <form onSubmit={submit} className="composer">
        <label htmlFor="question" className="eyebrow">Your next question</label>
        {error && <p role="alert" className="mb-2 text-sm text-red-700">{error}</p>}
        {voice.error && <p role="alert" className="mb-2 text-sm text-red-700">{voice.error}</p>}
        <div className="flex gap-2">
          <textarea
            id="question"
            ref={input}
            rows={2}
            readOnly={sending}
            aria-busy={sending}
            onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); void submit(e); } }}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder="Type a question…"
            className="flex-1 rounded-lg border border-stone-300 bg-white px-3 py-2 outline-none focus:border-orange-400"
          />
          <button
            type="submit"
            disabled={sending || busy || voice.busy || !draft.trim()}
            className="rounded-lg bg-orange-500 px-4 py-2 font-medium text-white disabled:opacity-40"
          >
            {sending ? "Sending…" : "Send"}
          </button>
        </div>
        <div className="mt-3 flex flex-wrap items-center gap-3">
          {voice.phase === "idle" ? (
            <button type="button" disabled={busy || sending || !speechReady} onClick={() => void voice.start()}
              className="flex items-center gap-2 rounded-full border border-stone-300 bg-white px-3 py-1.5 text-sm disabled:opacity-40">
              <span aria-hidden="true">◉</span> Record a question
            </button>
          ) : (
            <>
              <span role="status" className="flex items-center gap-2 text-sm text-orange-900">
                <span aria-hidden="true" className="inline-block size-2 rounded-full bg-orange-600" />
                {voice.phase === "recording" ? `Recording ${Math.floor(voice.elapsedMs / 60000)}:${String(Math.floor(voice.elapsedMs / 1000) % 60).padStart(2, "0")} / 2:00`
                  : voice.phase === "starting" ? "Opening microphone…" : voice.phase === "transcribing" ? "Transcribing your question…" : voice.phase === "stop_failed" ? "Microphone stop unconfirmed" : "Cancelling…"}
              </span>
              {voice.phase === "recording" && <button type="button" onClick={() => void voice.finish()} className="rounded-full bg-orange-600 px-3 py-1.5 text-sm text-white">Stop & transcribe</button>}
              <button type="button" disabled={voice.phase === "cancelling"} onClick={() => void voice.cancel()} className="text-sm underline disabled:opacity-40">Cancel recording</button>
            </>
          )}
          {!speechReady && onOpenModels && <button type="button" className="text-sm text-orange-800 underline" onClick={onOpenModels}>Set up speech recognition</button>}
        </div>
        {speechReady && <p className="mt-2 text-xs text-stone-600">Audio goes to {speechProvider.name} ({speechProvider.isLocal ? "your network" : "cloud"}) after you stop. Review the transcript here before sending. Audio is not saved.</p>}
        <p className="composer-hint" role="status">{state.loadingHistory ? "Opening conversation…" : busy ? "Your tutor is responding…" : "Enter to send · Shift + Enter for a new line"}</p>
      </form>
    </>
  );
}
