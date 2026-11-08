import { Fragment, type ReactNode } from "react";

/**
 * Small, safe Markdown subset for Phase 0: `## headings`, paragraphs, `**bold**` and
 * `[[slug]]` links. Builds React elements only; never injects HTML.
 */
export function Markdown({ source, onLink }: { source: string; onLink: (slug: string) => void }) {
  const blocks = source.split(/\n{2,}/).filter((b) => b.trim());
  return (
    <div className="space-y-3 leading-relaxed">
      {blocks.map((block, i) => {
        const heading = /^(#{1,3})\s+(.*)$/.exec(block.trim());
        if (heading) {
          return (
            <h3 key={i} className="pt-2 font-semibold">
              {inline(heading[2] ?? "", onLink)}
            </h3>
          );
        }
        return <p key={i}>{inline(block, onLink)}</p>;
      })}
    </div>
  );
}

function inline(text: string, onLink: (slug: string) => void): ReactNode[] {
  return text.split(/(\*\*[^*]+\*\*|\[\[[^\]]+\]\])/g).map((part, i) => {
    if (part.startsWith("**") && part.endsWith("**")) return <strong key={i}>{part.slice(2, -2)}</strong>;
    if (part.startsWith("[[") && part.endsWith("]]")) {
      const slug = part.slice(2, -2);
      return (
        <button key={i} onClick={() => onLink(slug)} className="text-orange-700 underline decoration-dotted">
          {slug.replaceAll("-", " ")}
        </button>
      );
    }
    return <Fragment key={i}>{part}</Fragment>;
  });
}
