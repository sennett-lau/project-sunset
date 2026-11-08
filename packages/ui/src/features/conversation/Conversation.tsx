import { type FormEvent, useEffect, useRef, useState } from "react";
import { Markdown } from "../wiki/Markdown";
import type { ConversationState } from "./useConversation";

export function Conversation({ state }: { state: ConversationState }) {
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const bottom = useRef<HTMLDivElement>(null);
  const busy = state.messages.some((m) => m.pending);

  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" });
  }, [state.messages]);

  async function submit(e: FormEvent) {
    e.preventDefault();
    const text = draft.trim();
    if (!text) return;
    setError(null);
    try {
      await state.send(text);
      setDraft("");
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <>
      <div className="min-h-0 flex-1 space-y-4 overflow-auto px-5 py-4">
        {state.messages.length === 0 && (
          <p className="mt-10 text-center text-stone-500">
            Ask anything to start learning. In Phase 0, try “What is a VAE?”
          </p>
        )}
        {state.messages.map((m) => (
          <div key={m.id} className={m.speaker === "user" ? "flex justify-end" : ""}>
            <div
              className={`max-w-[85%] rounded-2xl px-4 py-2.5 leading-relaxed ${
                m.speaker === "user" ? "whitespace-pre-wrap bg-orange-100" : "bg-white shadow-sm"
              }`}
            >
              {m.speaker === "user" ? m.text : <Markdown source={m.text} className="prose-sm" />}
              {m.pending && <span className="ml-0.5 animate-pulse">▍</span>}
              {m.error && <p className="mt-1 text-sm text-red-700">Could not answer: {m.error}</p>}
            </div>
          </div>
        ))}
        <div ref={bottom} />
      </div>
      <form onSubmit={submit} className="border-t border-stone-200 p-3">
        {error && <p className="mb-2 text-sm text-red-700">{error}</p>}
        <div className="flex gap-2">
          <input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder="Type a question…"
            className="flex-1 rounded-lg border border-stone-300 bg-white px-3 py-2 outline-none focus:border-orange-400"
          />
          <button
            type="submit"
            disabled={busy || !draft.trim()}
            className="rounded-lg bg-orange-500 px-4 py-2 font-medium text-white disabled:opacity-40"
          >
            Send
          </button>
        </div>
      </form>
    </>
  );
}
