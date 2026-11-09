import type { ProposalEdit, ProposedChange, RelationKind } from "@sunset/client";
import type { ReactNode } from "react";

const relations: RelationKind[] = ["IS_A", "PART_OF", "USES", "DEPENDS_ON", "EXPLAINS", "RELATED_TO", "CONTRASTS_WITH", "APPEARS_IN_SOURCE"];

/** Edit content without exposing or replacing core-owned evidence and lifecycle fields. */
export function ProposalEditor({ draft, onChange, disabled, articleOnly = false }: {
  draft: ProposalEdit;
  onChange: (draft: ProposalEdit) => void;
  disabled: boolean;
  articleOnly?: boolean;
}) {
  const changeAt = (index: number, change: ProposedChange) => onChange({ ...draft, changes: draft.changes.map((c, i) => i === index ? change : c) });
  return <fieldset disabled={disabled} className="mb-3 space-y-4 disabled:opacity-60">
    <legend className="mb-2 font-medium">Edit proposed content</legend>
    <p className="text-xs text-stone-600">Save your edits, review the updated preview, then approve. Original conversation references and earlier versions are kept.</p>
    <Field label="Update summary" value={draft.summary} set={(summary) => onChange({ ...draft, summary })} />
    {draft.changes.map((c, index) => <section key={index} className="space-y-2 rounded border border-stone-200 p-3" aria-label={`Change ${index + 1}`}>
      <div className="flex items-center justify-between gap-2"><p className="font-medium">{index + 1}. {c.op.replaceAll("_", " ")}</p>
        {!articleOnly && <button type="button" className="text-xs text-red-700 underline" onClick={() => onChange({ ...draft, changes: draft.changes.filter((_, i) => i !== index) })}>Remove change {index + 1}</button>}
      </div>
      {c.op === "upsert_concept" && <>
        <Field label="Concept address" value={c.slug} set={(slug) => changeAt(index, { ...c, slug })} />
        <Field label="Title" value={c.title} set={(title) => changeAt(index, { ...c, title })} />
        <Field label="Aliases (one per line)" value={c.aliases.join("\n")} multiline set={(value) => changeAt(index, { ...c, aliases: value.split("\n") })} />
      </>}
      {"concept" in c && <Field disabled={articleOnly} label="Concept address" value={c.concept} set={(concept) => changeAt(index, { ...c, concept })} />}
      {c.op === "revise_article" && <>
        <Field label="Article summary" value={c.summary} set={(summary) => changeAt(index, { ...c, summary })} />
        <Field label="Article text (Markdown)" value={c.bodyMd} multiline rows={12} set={(bodyMd) => changeAt(index, { ...c, bodyMd })} />
      </>}
      {c.op === "add_relation" && <>
        <Field label="From concept" value={c.from} set={(from) => changeAt(index, { ...c, from })} />
        <Field label="To concept" value={c.to} set={(to) => changeAt(index, { ...c, to })} />
        <Label name="Relationship"><select value={c.kind} onChange={(e) => changeAt(index, { ...c, kind: e.target.value as RelationKind })}>{relations.map((kind) => <option key={kind} value={kind}>{kind.toLowerCase().replaceAll("_", " ")}</option>)}</select></Label>
        <Field label="Relationship label (optional)" value={c.label ?? ""} set={(label) => changeAt(index, { ...c, label: label || null })} />
      </>}
      {c.op === "add_observation" && <>
        <p className="text-xs text-violet-800">Private learning note · never included in public exports</p>
        <Field label="Private note" value={c.note} multiline set={(note) => changeAt(index, { ...c, note })} />
        <Label name="Note type"><select value={c.kind} onChange={(e) => changeAt(index, { ...c, kind: e.target.value as typeof c.kind })}>{["question", "confusion", "familiarity", "preferred_explanation"].map((kind) => <option key={kind} value={kind}>{kind.replaceAll("_", " ")}</option>)}</select></Label>
        <Label name="Note basis"><select value={c.source} onChange={(e) => changeAt(index, { ...c, source: e.target.value as typeof c.source })}>{["self_reported", "observed", "model_inferred"].map((source) => <option key={source} value={source}>{source.replaceAll("_", " ")}</option>)}</select></Label>
      </>}
      {c.op === "add_visual" && <>
        <Field label="Visual title" value={c.title} set={(title) => changeAt(index, { ...c, title })} />
        {c.payload.kind === "concept_flow" && <>
          {c.payload.nodes.map((node, ni) => <Field key={node.id} label={`Box ${ni + 1} label`} value={node.label} set={(label) => { if (c.payload.kind === "concept_flow") changeAt(index, { ...c, payload: { ...c.payload, nodes: c.payload.nodes.map((n, i) => i === ni ? { ...n, label } : n) } }); }} />)}
          {c.payload.edges.map((edge, ei) => <Field key={ei} label={`Arrow ${ei + 1} label (optional)`} value={edge.label ?? ""} set={(label) => { if (c.payload.kind === "concept_flow") changeAt(index, { ...c, payload: { ...c.payload, edges: c.payload.edges.map((e, i) => i === ei ? { ...e, label: label || null } : e) } }); }} />)}
        </>}
        {c.payload.kind === "mermaid" && <Field label="Diagram text" value={c.payload.diagram} multiline set={(diagram) => changeAt(index, { ...c, payload: { kind: "mermaid", diagram } })} />}
      </>}
    </section>)}
    {draft.changes.length === 0 && <p role="alert" className="text-red-700">Keep at least one change, or cancel editing and reject this proposal.</p>}
  </fieldset>;
}

function Label({ name, children }: { name: string; children: ReactNode }) {
  return <label className="block space-y-1 text-xs"><span className="font-medium text-stone-600">{name}</span>{children}</label>;
}
function Field({ label, value, set, multiline, rows = 3, disabled = false }: { label: string; value: string; set: (value: string) => void; multiline?: boolean; rows?: number; disabled?: boolean }) {
  const className = "block w-full rounded border border-stone-300 bg-white px-2 py-1.5 text-sm";
  return <Label name={label}>{multiline
    ? <textarea className={className} value={value} rows={rows} onChange={(e) => set(e.target.value)} />
    : <input disabled={disabled} className={className} value={value} onChange={(e) => set(e.target.value)} />}</Label>;
}
