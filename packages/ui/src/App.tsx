import type { AppEvent, ConceptId, SunsetClient } from "@sunset/client";
import { QueryClient, QueryClientProvider, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { ClientProvider, useClient } from "./client-context";
import { Conversation } from "./features/conversation/Conversation";
import { useConversation } from "./features/conversation/useConversation";
import { Graph } from "./features/graph/Graph";
import { ProposalDrawer, ProposalBadge } from "./features/proposals/ProposalDrawer";
import { SaveModeToggle } from "./features/proposals/SaveModeToggle";
import { Wiki } from "./features/wiki/Wiki";
import { keys } from "./query-keys";

export interface AppProps {
  client: SunsetClient;
  /** From app-identity.json. Never hard-code the product name in the UI. */
  productName: string;
}

export function App({ client, productName }: AppProps) {
  const [queryClient] = useState(() => new QueryClient());
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
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [tab, setTab] = useState<Tab>("wiki");
  const [conceptId, setConceptId] = useState<ConceptId | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useCoreEvents((event) => {
    conversation.onEvent(event);
    if (event.type === "job_failed") setNotice(`Background capture failed: ${event.message}`);
  });

  const openConcept = (id: ConceptId) => {
    setConceptId(id);
    setTab("wiki");
  };

  return (
    <div className="flex h-full flex-col bg-stone-50 text-stone-900">
      <header className="flex items-center justify-between border-b border-stone-200 px-5 py-3">
        <h1 className="text-lg font-semibold">{productName}</h1>
        <div className="flex items-center gap-3">
          <SaveModeToggle />
          <ProposalBadge onClick={() => setDrawerOpen((o) => !o)} />
        </div>
      </header>
      {notice && (
        <div className="flex justify-between bg-amber-50 px-5 py-2 text-sm text-amber-900">
          <span>{notice}</span>
          <button className="underline" onClick={() => setNotice(null)}>
            Dismiss
          </button>
        </div>
      )}
      <div className="flex min-h-0 flex-1">
        <section className="flex w-1/2 min-w-0 flex-col border-r border-stone-200">
          <Conversation state={conversation} />
        </section>
        <section className="flex w-1/2 min-w-0 flex-col">
          <nav className="flex gap-1 border-b border-stone-200 px-4 pt-2">
            {(["wiki", "graph"] as const).map((t) => (
              <button
                key={t}
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
              <Graph onSelect={openConcept} />
            )}
          </div>
        </section>
        {drawerOpen && (
          <ProposalDrawer onClose={() => setDrawerOpen(false)} onOpenConcept={openConcept} />
        )}
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
        case "proposal_created":
        case "proposal_updated":
          refresh(["proposals"]);
          break;
        case "knowledge_changed":
          refresh(keys.concepts);
          refresh(["concept"]);
          refresh(keys.graph);
          break;
      }
    });
  }, [client, queryClient, latest]);
}
