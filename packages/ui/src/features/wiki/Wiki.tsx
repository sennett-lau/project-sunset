import type { ConceptId, ConceptPage, RelationView } from "@sunset/client";
import { useQuery } from "@tanstack/react-query";
import { useClient } from "../../client-context";
import { keys } from "../../query-keys";
import { Visual } from "../visuals/Visual";
import { Markdown } from "./Markdown";

export function Wiki({ selected, onSelect }: { selected: ConceptId | null; onSelect: (id: ConceptId) => void }) {
  const client = useClient();
  const concepts = useQuery({ queryKey: keys.concepts, queryFn: () => client.listConcepts() });
  const list = concepts.data ?? [];
  const current = list.find((c) => c.id === selected) ?? list[0];

  if (concepts.isSuccess && list.length === 0) {
    return (
      <p className="p-6 text-stone-500">
        Your wiki is empty. Approved proposals from your conversations appear here.
      </p>
    );
  }
  return (
    <div className="flex h-full">
      <ul className="w-48 shrink-0 space-y-0.5 overflow-auto border-r border-stone-100 p-3 text-sm">
        {list.map((c) => (
          <li key={c.id}>
            <button
              onClick={() => onSelect(c.id)}
              className={`w-full rounded px-2 py-1 text-left ${c.id === current?.id ? "bg-orange-50 font-medium" : ""}`}
            >
              {c.title}
            </button>
          </li>
        ))}
      </ul>
      <div className="min-w-0 flex-1 overflow-auto p-6">
        {current && <ConceptView id={current.id} onSelect={onSelect} />}
      </div>
    </div>
  );
}

function ConceptView({ id, onSelect }: { id: ConceptId; onSelect: (id: ConceptId) => void }) {
  const client = useClient();
  const page = useQuery({ queryKey: keys.concept(id), queryFn: () => client.conceptPage(id) });
  const concepts = useQuery({ queryKey: keys.concepts, queryFn: () => client.listConcepts() });
  if (!page.data) return null;
  const { concept, article, outgoing, incoming, overlay, visuals }: ConceptPage = page.data;
  const openSlug = (slug: string) => {
    const match = concepts.data?.find((c) => c.slug === slug);
    if (match) onSelect(match.id);
  };

  return (
    <article className="max-w-2xl space-y-6">
      <header>
        <h2 className="text-2xl font-semibold">{concept.title}</h2>
        {concept.aliases.length > 0 && <p className="text-sm text-stone-500">Also: {concept.aliases.join(", ")}</p>}
        {article && <p className="mt-3 text-lg text-stone-700">{article.summary}</p>}
      </header>

      {article?.bodyMd && <Markdown source={article.bodyMd} onLink={openSlug} />}

      {visuals.map((v) => (
        <figure key={v.id} className="rounded-lg border border-stone-200 p-4">
          <figcaption className="mb-3 text-sm font-medium text-stone-600">{v.title}</figcaption>
          <Visual payload={v.payload} />
        </figure>
      ))}

      {(outgoing.length > 0 || incoming.length > 0) && (
        <section>
          <h3 className="mb-2 font-semibold">Related</h3>
          <ul className="space-y-1 text-sm">
            {outgoing.map((r) => (
              <RelationLine key={r.relation.id} r={r} direction="out" title={concept.title} onSelect={onSelect} />
            ))}
            {incoming.map((r) => (
              <RelationLine key={r.relation.id} r={r} direction="in" title={concept.title} onSelect={onSelect} />
            ))}
          </ul>
        </section>
      )}

      {overlay.length > 0 && (
        <section className="rounded-lg border border-violet-200 bg-violet-50 p-4">
          <h3 className="mb-1 font-semibold text-violet-900">Your learning notes</h3>
          <p className="mb-2 text-xs text-violet-700">Private. Never part of the article or an export.</p>
          <ul className="space-y-1 text-sm text-violet-950">
            {overlay.map((o) => (
              <li key={o.id}>
                <span className="mr-1.5 text-xs uppercase text-violet-500">{o.kind.replace("_", " ")}</span>
                {o.note}
              </li>
            ))}
          </ul>
        </section>
      )}

      {article && (
        <p className="text-xs text-stone-400">
          Revision {article.version} · {new Date(article.createdAt).toLocaleString()}
        </p>
      )}
    </article>
  );
}

function RelationLine({
  r,
  direction,
  title,
  onSelect,
}: {
  r: RelationView;
  direction: "in" | "out";
  title: string;
  onSelect: (id: ConceptId) => void;
}) {
  const kind = r.relation.label ?? r.relation.kind.toLowerCase().replaceAll("_", " ");
  const other = (
    <button onClick={() => onSelect(r.other.id)} className="text-orange-700 underline decoration-dotted">
      {r.other.title}
    </button>
  );
  return (
    <li>
      {direction === "out" ? (
        <>
          {title} <em className="text-stone-500">{kind}</em> {other}
        </>
      ) : (
        <>
          {other} <em className="text-stone-500">{kind}</em> {title}
        </>
      )}
    </li>
  );
}
