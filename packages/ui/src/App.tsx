import type { AppEvent, ConceptId, SunsetClient } from "@sunset/client";
import { QueryClient, QueryClientProvider, useQueryClient } from "@tanstack/react-query";
import { lazy, Suspense, useEffect, useState } from "react";
import { ClientProvider, useClient } from "./client-context";
import { Conversation } from "./features/conversation/Conversation";
import { useConversation } from "./features/conversation/useConversation";
import { ModelsButton, ModelsPanel } from "./features/models/ModelsPanel";
import { ProposalDrawer, ProposalBadge } from "./features/proposals/ProposalDrawer";
import { SaveModeToggle } from "./features/proposals/SaveModeToggle";
import { LibraryPanel, usePublicExport } from "./features/library/LibraryPanel";
import { Wiki } from "./features/wiki/Wiki";
import { keys } from "./query-keys";

// Cytoscape is large; load it when the Graph tab first opens.
const Graph = lazy(() => import("./features/graph/Graph").then((m) => ({ default: m.Graph })));

export interface AppProps {
  client: SunsetClient;
  /** From app-identity.json. Never hard-code the product name in the UI. */
  productName: string;
}

export function App({ client, productName }: AppProps) {
  const [queryClient] = useState(() => new QueryClient({ defaultOptions: { queries: { retry: 1 } } }));
  return (
    <ClientProvider value={client}>
      <QueryClientProvider client={queryClient}>
        <Shell productName={productName} />
      </QueryClientProvider>
    </ClientProvider>
  );
}

type Tab = "wiki" | "graph";

function Shell({ productName }: { productName: string }) {
  const conversation = useConversation();
  const publicExport = usePublicExport();
  // One side panel at a time.
  const [panel, setPanel] = useState<"proposals" | "models" | "library" | null>(null);
  const toggle = (p: "proposals" | "models" | "library") => setPanel((cur) => (cur === p ? null : p));
  const [mobilePane, setMobilePane] = useState<"conversation" | "knowledge">("conversation");
  const [tab, setTab] = useState<Tab>("wiki");
  const [conceptId, setConceptId] = useState<ConceptId | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useCoreEvents((event) => {
    conversation.onEvent(event);
    if (event.type === "job_failed") setNotice(`Background capture failed: ${event.message}`);
    if (event.type === "tutor_failed" && event.sessionId === conversation.sessionId) setNotice(`The tutor model failed: ${event.message}. Check Models.`);
  });

  const openConcept = (id: ConceptId) => {
    setConceptId(id);
    setMobilePane("knowledge");
    setTab("wiki");
  };

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand"><span className="brand-mark" aria-hidden="true">◒</span><div><h1>{productName}</h1><p>A little more understanding, every day.</p></div></div>
        <div className="header-actions">
          <SaveModeToggle />
          <button className="rounded-full border border-stone-300 bg-white px-3 py-1 text-sm" aria-expanded={panel === "library"} onClick={() => toggle("library")}>Library</button>
          <ModelsButton expanded={panel === "models"} onClick={() => toggle("models")} />
          <ProposalBadge expanded={panel === "proposals"} onClick={() => toggle("proposals")} />
        </div>
      </header>
      {notice && (
        <div role="alert" className="flex justify-between bg-amber-50 px-5 py-2 text-sm text-amber-900">
          <span>{notice}</span>
          <button className="underline" onClick={() => setNotice(null)}>
            Dismiss
          </button>
        </div>
      )}
      <nav className="mobile-navigation" aria-label="Workspace">
        <button aria-pressed={mobilePane === "conversation"} onClick={() => setMobilePane("conversation")}>Conversation</button>
        <button aria-pressed={mobilePane === "knowledge"} onClick={() => setMobilePane("knowledge")}>Knowledge</button>
      </nav>
      <div className="workspace">
        <section aria-label="Conversation" className={`conversation-pane ${mobilePane === "conversation" ? "mobile-active" : ""}`}>
          <div className="pane-heading"><span className="eyebrow">01 / Conversation</span><span className="quiet-label">Follow your curiosity</span></div>
          <Conversation state={conversation} onOpenModels={() => setPanel("models")} />
        </section>
        <section aria-label="Knowledge" className={`knowledge-pane ${mobilePane === "knowledge" ? "mobile-active" : ""}`}>
          <nav aria-label="Knowledge views" className="knowledge-navigation"><span className="eyebrow">02 / Knowledge</span>
            {(["wiki", "graph"] as const).map((t) => (
              <button
                key={t}
                aria-pressed={tab === t}
                onClick={() => setTab(t)}
                className={`rounded-t px-3 py-1.5 text-sm capitalize ${
                  tab === t ? "bg-white font-medium shadow-sm" : "text-stone-500"
                }`}
              >
                {t}
              </button>
            ))}
          </nav>
          <div className="min-h-0 flex-1 overflow-auto bg-white">
            {tab === "wiki" ? (
              <Wiki selected={conceptId} onSelect={openConcept} />
            ) : (
              <Suspense fallback={<p role="status" className="p-6">Loading concept graph…</p>}>
                <Graph focus={conceptId} onSelect={openConcept} />
              </Suspense>
            )}
          </div>
        </section>
        {panel === "proposals" && <ProposalDrawer onClose={() => setPanel(null)} onOpenConcept={(id) => { openConcept(id); setPanel(null); }} />}
        {panel === "library" && <LibraryPanel exported={publicExport} onClose={() => setPanel(null)} />}
        {panel === "models" && <ModelsPanel onClose={() => setPanel(null)} />}
      </div>
    </div>
  );
}

/** Keep cached data in step with the core, and pass events to `handler`. */
function useCoreEvents(handler: (event: AppEvent) => void) {
  const client = useClient();
  const queryClient = useQueryClient();
  const [latest] = useState(() => ({ handler }));
  latest.handler = handler;

  useEffect(() => {
    // Cancel first: a fetch already in flight may have read the data before this event, and
    // invalidating alone would reuse that stale fetch.
    const refresh = (queryKey: readonly unknown[]) =>
      void queryClient
        .cancelQueries({ queryKey })
        .then(() => queryClient.invalidateQueries({ queryKey }));

    return client.onEvent((event) => {
      latest.handler(event);
      switch (event.type) {
        case "turn_finalized":
          refresh(keys.sessions);
          break;
        case "proposal_created":
        case "proposal_updated":
          refresh(["proposals"]);
          break;
        case "knowledge_changed":
          refresh(keys.concepts);
          refresh(["concept"]);
          refresh(keys.graph);
          refresh(["knowledge-search"]);
          refresh(keys.vault);
          break;
      }
    });
  }, [client, queryClient, latest]);
}
