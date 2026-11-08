import type { ReactNode } from "react";
import ReactMarkdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";

const WIKI = "wiki:";

/**
 * Markdown for articles and tutor answers. Raw HTML is never rendered (react-markdown
 * default). `[[slug]]` and `[[slug|label]]` become wiki links. External links are shown but do
 * not navigate the app window.
 */
export function Markdown({
  source,
  onLink,
  className = "",
}: {
  source: string;
  onLink?: (slug: string) => void;
  className?: string;
}) {
  return (
    <div className={`prose prose-stone max-w-none prose-headings:font-semibold ${className}`}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        urlTransform={(url) => (url.startsWith(WIKI) ? url : defaultUrlTransform(url))}
        components={{
          a: ({ href, children }) => <Link href={href} onLink={onLink}>{children}</Link>,
          img: ({ alt }) => <span className="text-stone-500">[image: {alt}]</span>,
        }}
      >
        {wikiLinks(source)}
      </ReactMarkdown>
    </div>
  );
}

function Link({ href, onLink, children }: { href?: string; onLink?: (slug: string) => void; children: ReactNode }) {
  if (href?.startsWith(WIKI)) {
    const slug = decodeURIComponent(href.slice(WIKI.length));
    return (
      <button
        type="button"
        onClick={() => onLink?.(slug)}
        className="text-orange-700 underline decoration-dotted"
      >
        {children}
      </button>
    );
  }
  return (
    <span className="underline decoration-stone-400" title={href}>
      {children}
    </span>
  );
}

/** `[[slug]]` → `[slug words](wiki:slug)`; `[[slug|label]]` → `[label](wiki:slug)`. */
export function wikiLinks(source: string): string {
  return source.replace(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g, (_, slug: string, label?: string) => {
    const s = slug.trim();
    const text = (label ?? s.replaceAll("-", " ")).replace(/[[\]]/g, "");
    return `[${text}](${WIKI}${encodeURIComponent(s)})`;
  });
}
