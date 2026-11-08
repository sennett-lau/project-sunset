import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useClient } from "../../client-context";
import { Panel } from "../../components/Panel";
import { keys } from "../../query-keys";

// Keep export progress and its destination alive when the inspector is closed.
export function usePublicExport() {
  const client = useClient();
  return useMutation({ mutationFn: () => client.exportPublicToFolder() });
}

export function LibraryPanel({ onClose, exported }: { onClose: () => void; exported: ReturnType<typeof usePublicExport> }) {
  const client = useClient();
  const queries = useQueryClient();
  const status = useQuery({ queryKey: keys.vault, queryFn: () => client.vaultStatus() });
  const sync = useMutation({
    mutationFn: () => client.syncVault(),
    onSuccess: (value) => queries.setQueryData(keys.vault, value),
  });
  return <Panel title="Library" onClose={onClose}>
    <p className="panel-intro">Your knowledge, close at hand.</p>
    <section className="library-section">
      <h3>Markdown vault</h3>
      <p>A one-way mirror of approved public knowledge. Your database remains the source of truth.</p>
      <p className="privacy-note">The vault also contains private learning notes in its .private_overlay folder. Use the public export below when sharing.</p>
      {status.isPending && <p role="status">Reading vault status…</p>}
      {status.error && <p role="alert">Could not read vault status: {String(status.error)} <button onClick={() => void status.refetch()}>Retry status</button></p>}
      {status.data && <>
        <p className="eyebrow">Vault location</p>
        <p className="library-path">{status.data.path}</p>
        {status.data.error
          ? <p role="alert">Vault needs attention: {status.data.error}</p>
          : <p role="status">{status.data.conflicts.length ? "Some files need attention." : "Vault is ready. No conflicts reported."}</p>}
        {status.data.conflicts.length > 0 && <div>
          <h4>Conflicts · {status.data.conflicts.length}</h4>
          <p>These edited or unmanaged paths were left untouched. Review them in your vault before retrying.</p>
          <ul className="library-path">{status.data.conflicts.map((path) => <li key={path}>{path}</li>)}</ul>
        </div>}
      </>}
      <button className="library-action" disabled={sync.isPending} onClick={() => sync.mutate()}>{sync.isPending ? "Syncing vault…" : "Retry vault sync"}</button>
      {sync.error && <p role="alert">Could not sync vault: {String(sync.error)}</p>}
    </section>
    <section className="library-section">
      <h3>Take your knowledge with you</h3>
      <p className="privacy-note">Public knowledge only. Conversations, private learning notes, and credentials are never included.</p>
      <p>Save a snapshot to a fresh folder on this Mac. Existing files are left untouched.</p>
      <button className="library-action" disabled={exported.isPending} onClick={() => exported.mutate()}>{exported.isPending ? "Exporting…" : "Export public knowledge"}</button>
      {exported.error && <p role="alert">Could not export public knowledge: {String(exported.error)}. Try exporting again.</p>}
      {exported.isSuccess && <div role="status"><p>Public export saved to:</p><p className="library-path">{exported.data}</p></div>}
    </section>
  </Panel>;
}
