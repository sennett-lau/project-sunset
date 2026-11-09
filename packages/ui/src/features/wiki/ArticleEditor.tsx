import type { ArticleRevision, ConceptId, SunsetClient } from "@sunset/client";
import { useSyncExternalStore } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useClient } from "../../client-context";
import { keys } from "../../query-keys";
import { Markdown } from "./Markdown";

type Draft = { base: string; summary: string; body: string; pending: boolean; error: string | null };
type DraftStore = { drafts: Map<ConceptId, Draft>; listeners: Set<() => void> };
const stores = new WeakMap<SunsetClient, DraftStore>();
function storeFor(client: SunsetClient) {
  let store = stores.get(client);
  if (!store) { store = { drafts: new Map(), listeners: new Set() }; stores.set(client, store); }
  return store;
}
export function useArticleDraft(id: ConceptId) {
  const client = useClient();
  const store = storeFor(client);
  const draft = useSyncExternalStore(
    (listener) => { store.listeners.add(listener); return () => { store.listeners.delete(listener); }; },
    () => store.drafts.get(id) ?? null,
  );
  const setDraft = (next: Draft | null) => {
    if (next) store.drafts.set(id, next); else store.drafts.delete(id);
    store.listeners.forEach((listener) => listener());
  };
  return { draft, setDraft, begin: (article: ArticleRevision) => setDraft({ base: article.id, summary: article.summary, body: article.bodyMd, pending: false, error: null }) };
}

export function ArticleEditor({ id, article, draft, setDraft, onSaved }: {
  id: ConceptId; article: ArticleRevision; draft: Draft; setDraft: (draft: Draft | null) => void; onSaved: () => void;
}) {
  const client = useClient();
  const cache = useQueryClient();
  const stale = article.id !== draft.base;
  const unchanged = article.summary === draft.summary && article.bodyMd === draft.body;
  async function submit() {
    if (storeFor(client).drafts.get(id)?.pending || stale || unchanged || !draft.summary.trim()) return;
    setDraft({ ...draft, pending: true, error: null });
    try {
      await client.proposeArticleEdit(id, draft.base, draft.summary, draft.body);
      setDraft(null);
      void cache.invalidateQueries({ queryKey: keys.pending });
      void cache.invalidateQueries({ queryKey: keys.recent });
      onSaved();
    } catch (error) {
      setDraft({ ...draft, pending: false, error: String(error) });
      void cache.invalidateQueries({ queryKey: keys.concept(id) });
    }
  }
  return <section aria-label="Edit saved article" className="space-y-3 rounded-lg border border-orange-200 bg-orange-50/40 p-4">
    <h3 className="font-semibold">Edit article</h3>
    <p className="text-sm text-stone-600">Your changes become a proposal for review. The saved article stays unchanged until approval, even with auto-save enabled. This draft stays available while the app is open.</p>
    {stale && <p role="alert">This article changed while you were editing. Your draft is preserved; copy any wording you need, then discard this draft and reopen the current article.</p>}
    {draft.error && <p role="alert">Could not propose this edit: {draft.error}</p>}
    <label className="block text-sm">Article summary<textarea aria-label="Article summary" className="mt-1 w-full rounded border border-stone-300 bg-white p-2" rows={2} value={draft.summary} disabled={draft.pending} onChange={(event) => setDraft({ ...draft, summary: event.target.value })} /></label>
    <label className="block text-sm">Article body (Markdown)<textarea aria-label="Article body (Markdown)" className="mt-1 w-full rounded border border-stone-300 bg-white p-2 font-mono" rows={10} value={draft.body} disabled={draft.pending} onChange={(event) => setDraft({ ...draft, body: event.target.value })} /></label>
    <details><summary className="cursor-pointer text-sm">Preview article</summary><p className="my-2 font-medium">{draft.summary}</p><Markdown source={draft.body} /></details>
    <div className="flex gap-3"><button disabled={draft.pending || stale || unchanged || !draft.summary.trim()} onClick={() => void submit()} className="rounded bg-orange-600 px-3 py-2 text-sm text-white disabled:opacity-40">{draft.pending ? "Creating proposal…" : "Review article edit"}</button>
    <button disabled={draft.pending} onClick={() => setDraft(null)} className="text-sm underline">Discard article draft</button></div>
  </section>;
}
