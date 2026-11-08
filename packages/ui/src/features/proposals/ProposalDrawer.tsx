import type { ConceptId, Proposal } from "@sunset/client";
import { useMutation, useQuery } from "@tanstack/react-query";
import type { ButtonHTMLAttributes } from "react";
import { useClient } from "../../client-context";
import { Panel } from "../../components/Panel";
import { keys } from "../../query-keys";
import { Markdown } from "../wiki/Markdown";
import { Visual } from "../visuals/Visual";
import { describeChange } from "./describe";

/** Quiet badge: counts pending proposals without taking focus (initial spec Part I §6). */
export function ProposalBadge({ onClick, expanded }: { onClick: () => void; expanded?: boolean }) {
  const client = useClient();
  const { data, error } = useQuery({ queryKey: keys.pending, queryFn: () => client.pendingProposals() });
  const count = data?.length ?? 0;
  const latest = data?.[0]?.summary;
  return (
    <button
      onClick={onClick}
      aria-expanded={expanded}
      title={latest}
      className="flex items-center gap-2 rounded-full border border-stone-300 bg-white px-3 py-1 text-sm"
    >
      <span>Proposed updates</span>
      <span
        className={`min-w-6 rounded-full px-1.5 text-center font-medium ${
          count ? "bg-orange-500 text-white" : "bg-stone-200 text-stone-600"
        }`}
      >
        {error ? "!" : data ? count : "…"}
      </span>
    </button>
  );
}

export function ProposalDrawer({
  onClose,
  onOpenConcept,
}: {
  onClose: () => void;
  onOpenConcept: (id: ConceptId) => void;
}) {
  const client = useClient();
  const pending = useQuery({ queryKey: keys.pending, queryFn: () => client.pendingProposals() });
  const recent = useQuery({ queryKey: keys.recent, queryFn: () => client.recentProposals(20) });
  const history = (recent.data ?? []).filter((p) => p.status !== "awaiting_approval");

  return (
    <Panel title="Proposed updates" onClose={onClose}>
        <p className="panel-intro">Keep what helps you learn.</p>
        <p className="text-sm text-stone-600">Review the changes below before approving. Saved updates can be undone; private learning notes stay separate from articles.</p>
        {(pending.isPending || recent.isPending) && <p role="status">Loading updates…</p>}
        {(pending.error || recent.error) && <p role="alert" className="text-red-700">Could not load updates. <button onClick={() => { void pending.refetch(); void recent.refetch(); }}>Retry</button></p>}
        {pending.data?.length === 0 && <p className="text-sm text-stone-500">Nothing waiting for review.</p>}
        {pending.data?.map((p) => <ProposalCard key={p.id} proposal={p} onOpenConcept={onOpenConcept} />)}
        {history.length > 0 && <h3 className="pt-3 text-xs font-semibold uppercase text-stone-500">History</h3>}
        {history.map((p) => (
          <ProposalCard key={p.id} proposal={p} onOpenConcept={onOpenConcept} />
        ))}
    </Panel>
  );
}

function ProposalCard({ proposal: p, onOpenConcept }: { proposal: Proposal; onOpenConcept: (id: ConceptId) => void }) {
  const client = useClient();
  const act = useMutation({
    mutationFn: (action: "approve" | "reject" | "undo") =>
      action === "approve"
        ? client.approveProposal(p.id)
        : action === "reject"
          ? client.rejectProposal(p.id)
          : client.undoProposal(p.id),
  });
  const open = useMutation({ mutationFn: async () => {
    const slug = p.changes.find((c) => c.op === "upsert_concept")?.slug;
    const match = (await client.listConcepts()).find((c) => c.slug === slug);
    if (match) onOpenConcept(match.id);
    else throw new Error("This update has no available article.");
  } });

  return (
    <div className="rounded-lg border border-stone-200 bg-white p-3 text-sm shadow-sm">
      <div className="mb-2 flex items-start justify-between gap-2">
        <p className="font-medium">{p.summary}</p>
        <StatusChip status={p.status} />
      </div>
      <p className="mb-2 text-xs text-stone-500">{p.evidenceTurnIds.length} conversation turns referenced · {new Date(p.createdAt).toLocaleDateString()}</p>
      <ul className="mb-3 space-y-1">
        {p.changes.map((c, i) => {
          const d = describeChange(c);
          return (
            <li key={i} className={d.private ? "text-violet-800" : "text-stone-700"}>
              <span className="mr-1.5 text-xs font-semibold uppercase text-stone-400">{d.label}</span>
              {d.text}
              {c.op === "revise_article" && <details className="mt-2 rounded border border-stone-200 p-2"><summary className="cursor-pointer text-xs">Read proposed article</summary><Markdown source={c.bodyMd} className="prose-sm mt-2" /></details>}
              {c.op === "add_visual" && <details className="mt-2 rounded border border-stone-200 p-2"><summary className="cursor-pointer text-xs">Preview teaching visual</summary><Visual payload={c.payload} /></details>}
            </li>
          );
        })}
      </ul>
      {(act.error || open.error) && <p role="alert" className="mb-2 text-red-700">{String(act.error ?? open.error)}</p>}
      <div className="flex gap-2">
        {p.status === "awaiting_approval" && (
          <>
            <Button primary disabled={act.isPending} onClick={() => act.mutate("approve")}>
              Approve
            </Button>
            <Button disabled={act.isPending} onClick={() => act.mutate("reject")}>
              Reject
            </Button>
          </>
        )}
        {p.status === "committed" && (
          <>
            {p.changes.some((c) => c.op === "upsert_concept") && <Button disabled={open.isPending} onClick={() => open.mutate()}>Open</Button>}
            <Button disabled={act.isPending} onClick={() => act.mutate("undo")}>
              Undo
            </Button>
          </>
        )}
      </div>
    </div>
  );
}

function StatusChip({ status }: { status: Proposal["status"] }) {
  const style = {
    awaiting_approval: "bg-orange-100 text-orange-800",
    committed: "bg-green-100 text-green-800",
    rejected: "bg-stone-200 text-stone-600",
    undone: "bg-stone-200 text-stone-600",
  }[status];
  return (
    <span className={`shrink-0 rounded px-1.5 py-0.5 text-xs ${style}`}>{status.replace("_", " ")}</span>
  );
}

function Button({
  primary,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { primary?: boolean }) {
  return (
    <button
      {...props}
      className={`rounded-md px-3 py-1 font-medium disabled:opacity-40 ${
        primary ? "bg-orange-500 text-white" : "border border-stone-300 bg-white"
      }`}
    />
  );
}
