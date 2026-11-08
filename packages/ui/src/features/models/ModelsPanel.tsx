import type {
  CapabilityStatus,
  ModelCapabilities,
  ProbeReport,
  ProviderId,
  ProviderProfile,
  Role,
  RoleAssignment,
} from "@sunset/client";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type ButtonHTMLAttributes, type FormEvent, type ReactNode, useEffect, useState } from "react";
import { useClient } from "../../client-context";
import { Panel } from "../../components/Panel";
import { keys } from "../../query-keys";

const ROLE_TEXT: Record<Role, { title: string; hint: string }> = {
  tutor: { title: "Tutor", hint: "Answers you. Use the strongest model you can." },
  curator: {
    title: "Curator",
    hint: "Proposes wiki updates in the background. Needs reliable JSON output.",
  },
  stt: { title: "Speech recognition", hint: "Turns a recording into an editable question." },
};

const PRESETS = [
  { name: "Ollama", baseUrl: "http://localhost:11434/v1" },
  { name: "LM Studio", baseUrl: "http://localhost:1234/v1" },
  { name: "OpenAI", baseUrl: "https://api.openai.com/v1" },
  { name: "OpenRouter", baseUrl: "https://openrouter.ai/api/v1" },
];

/** Header button: shows what the tutor runs on and whether everything stays local. */
export function ModelsButton({ onClick, expanded }: { onClick: () => void; expanded?: boolean }) {
  const client = useClient();
  const { data } = useQuery({ queryKey: keys.models, queryFn: () => client.modelSettings() });
  const tutor = data?.roles.find((r) => r.role === "tutor");
  const provider = data?.providers.find((p) => p.id === tutor?.providerId);
  return (
    <button
      onClick={onClick}
      aria-expanded={expanded}
      className="flex items-center gap-2 rounded-full border border-stone-300 bg-white px-3 py-1 text-sm"
    >
      <span>Models</span>
      {tutor && provider && (
        <span className="max-w-40 truncate text-stone-500">
          {provider.kind === "demo" ? "Demo" : tutor.model}
        </span>
      )}
      {data && <LocalBadge local={data.allLocal} label={data.allLocal ? "local" : "cloud"} />}
    </button>
  );
}

export function ModelsPanel({ onClose }: { onClose: () => void }) {
  const client = useClient();
  const { data, error, isPending, refetch } = useQuery({ queryKey: keys.models, queryFn: () => client.modelSettings() });
  const [editing, setEditing] = useState<ProviderProfile | "new" | null>(null);

  return (
    <Panel title="Models" onClose={onClose}>
        <p className="panel-intro">Choose who teaches, listens and takes notes.</p>
        <p className="text-sm text-stone-600">Add a provider, assign a model to each role, then test its capabilities. Connection tests send small requests to the selected provider.</p>
        {isPending && <p role="status">Loading model settings…</p>}
        {error && <p role="alert" className="text-red-700">{String(error)} <button onClick={() => void refetch()}>Retry</button></p>}
        {data && (
          <>
            <p className={data.allLocal ? "text-green-800" : "text-amber-800"}>
              {data.allLocal
                ? "Every role runs on this Mac or your own network."
                : "At least one role uses a cloud service."}
            </p>
            <p className="privacy-note">Your library is stored on this device. Local providers may run on your network; “local” does not mean offline. Chat roles receive conversation context; speech recognition receives recordings you choose to transcribe. There is no automatic fallback to another provider.</p>
            <section className="space-y-3">
              <h3 className="text-xs font-semibold uppercase text-stone-500">Roles</h3>
              {data.roles.filter((r) => r.role !== "stt").map((r) => (
                <RoleEditor key={r.role} assignment={r} providers={data.providers} />
              ))}
              <SpeechRoleEditor assignment={data.roles.find((r) => r.role === "stt")} providers={data.providers} />
            </section>
            <section className="space-y-2">
              <div className="flex items-center justify-between">
                <h3 className="text-xs font-semibold uppercase text-stone-500">Providers</h3>
                <button className="text-orange-700 underline" onClick={() => setEditing("new")}>
                  Add provider
                </button>
              </div>
              {editing === "new" && <ProviderForm onDone={() => setEditing(null)} />}
              {data.providers.map((p) =>
                editing !== "new" && editing?.id === p.id ? (
                  <ProviderForm key={p.id} provider={p} onDone={() => setEditing(null)} />
                ) : (
                  <ProviderRow key={p.id} provider={p} onEdit={() => setEditing(p)} />
                ),
              )}
            </section>
          </>
        )}
    </Panel>
  );
}

function SpeechRoleEditor({ assignment, providers }: { assignment?: RoleAssignment; providers: ProviderProfile[] }) {
  const client = useClient();
  const queryClient = useQueryClient();
  const available = providers.filter((p) => p.kind !== "demo");
  const [providerId, setProviderId] = useState<ProviderId>(assignment?.providerId ?? "");
  const [model, setModel] = useState(assignment?.model ?? "");
  const provider = available.find((p) => p.id === providerId);
  const changed = providerId !== assignment?.providerId || model.trim() !== assignment?.model;
  useEffect(() => {
    setProviderId(assignment?.providerId ?? "");
    setModel(assignment?.model ?? "");
  }, [assignment?.providerId, assignment?.model]);
  const models = useQuery({
    queryKey: keys.providerModels(providerId),
    queryFn: () => client.listModels(providerId),
    enabled: !!provider,
    retry: false,
    staleTime: 60_000,
  });
  const save = useMutation({
    mutationFn: () => client.setRole("stt", providerId, model.trim()),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: keys.models }),
  });
  const disable = useMutation({
    mutationFn: () => client.clearSpeechModel(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: keys.models }),
  });
  const pending = save.isPending || disable.isPending;

  return (
    <section aria-label="Speech recognition" className="space-y-2 rounded-lg border border-orange-200 bg-orange-50/40 p-3">
      <div className="flex items-baseline justify-between gap-2"><h4 className="font-medium">Speech recognition</h4><span className="text-xs text-stone-500">Optional</span></div>
      <p className="text-xs text-stone-600">Record up to two minutes, then review and edit the transcript before sending. Choose a model that supports audio transcription.</p>
      {!available.length && <p className="text-sm text-stone-600">Add a speech provider below to enable the microphone.</p>}
      <div className="flex gap-2">
        <select aria-label="Speech recognition provider" value={providerId} disabled={pending || !available.length}
          onChange={(e) => { setProviderId(e.target.value); setModel(""); }} className="w-40 rounded border border-stone-300 bg-white px-1.5 py-1">
          <option value="">Choose a provider…</option>
          {available.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
        </select>
        <input aria-label="Speech recognition model" list="models-stt" value={model} disabled={pending || !provider}
          onChange={(e) => setModel(e.target.value)} placeholder="e.g. qwen3-asr" className="min-w-0 flex-1 rounded border border-stone-300 bg-white px-2 py-1" />
        <datalist id="models-stt">{models.data?.map((m) => <option key={m} value={m} />)}</datalist>
      </div>
      {models.isError && <p className="text-xs text-stone-500">Could not list models. You can type the model id.</p>}
      {provider && <p className="text-xs text-stone-600">Recordings will be sent to {provider.name} ({provider.isLocal ? "your network" : "cloud"}) at {provider.baseUrl}. Recordings are held in memory and discarded after transcription or cancellation.</p>}
      {assignment && !changed && <p className="text-xs text-green-800">Selected. Try “Record a question” in the conversation to check recognition.</p>}
      {(save.error || disable.error) && <p role="alert" className="text-sm text-red-700">{String(save.error ?? disable.error)}</p>}
      <div className="flex gap-2"><Button primary disabled={!provider || !model.trim() || !changed || pending} onClick={() => save.mutate()}>
        {save.isPending ? "Saving…" : "Use for speech"}
      </Button>
      {assignment && <Button disabled={pending} onClick={() => disable.mutate()}>{disable.isPending ? "Disabling…" : "Disable speech"}</Button>}</div>
    </section>
  );
}

function RoleEditor({ assignment, providers }: { assignment: RoleAssignment; providers: ProviderProfile[] }) {
  const client = useClient();
  const queryClient = useQueryClient();
  const [providerId, setProviderId] = useState<ProviderId>(assignment.providerId);
  const [model, setModel] = useState(assignment.model);
  const [report, setReport] = useState<ProbeReport | null>(null);
  const provider = providers.find((p) => p.id === providerId);
  const changed = providerId !== assignment.providerId || model !== assignment.model;

  useEffect(() => {
    setProviderId(assignment.providerId);
    setModel(assignment.model);
  }, [assignment.providerId, assignment.model]);

  const models = useQuery({
    queryKey: keys.providerModels(providerId),
    queryFn: () => client.listModels(providerId),
    retry: false,
    staleTime: 60_000,
  });
  const save = useMutation({
    mutationFn: () => client.setRole(assignment.role, providerId, model.trim()),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: keys.models }),
  });
  const test = useMutation({
    mutationFn: () => client.testConnection(providerId, model.trim()),
    onSuccess: (r) => {
      setReport(r);
      void queryClient.invalidateQueries({ queryKey: keys.models });
    },
  });
  const listId = `models-${assignment.role}`;

  return (
    <div className="space-y-2 rounded-lg border border-stone-200 bg-white p-3">
      <div>
        <p className="font-medium">{ROLE_TEXT[assignment.role].title}</p>
        <p className="text-xs text-stone-500">{ROLE_TEXT[assignment.role].hint}</p>
      </div>
      <div className="flex gap-2">
        <select
          aria-label={`${ROLE_TEXT[assignment.role].title} provider`}
          disabled={test.isPending || save.isPending}
          value={providerId}
          onChange={(e) => {
            const next = providers.find((p) => p.id === e.target.value);
            setProviderId(e.target.value);
            setModel(next?.kind === "demo" ? "scripted" : "");
            setReport(null);
          }}
          className="w-40 rounded border border-stone-300 px-1.5 py-1"
        >
          {providers.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
        <input
          aria-label={`${ROLE_TEXT[assignment.role].title} model`}
          disabled={test.isPending || save.isPending}
          list={listId}
          value={model}
          onChange={(e) => {
            setModel(e.target.value);
            setReport(null);
          }}
          placeholder="model id, e.g. qwen3:8b"
          className="min-w-0 flex-1 rounded border border-stone-300 px-2 py-1"
        />
        <datalist id={listId}>
          {models.data?.map((m) => <option key={m} value={m} />)}
        </datalist>
      </div>
      {models.isError && provider?.kind !== "demo" && (
        <p className="text-xs text-stone-500">Could not list models ({String(models.error)}). Type the model id.</p>
      )}
      {!changed && <Capabilities caps={assignment.capabilities} />}
      {report && <ProbeNotes report={report} />}
      {(save.error || test.error) && <p role="alert" className="text-red-700">{String(save.error ?? test.error)}</p>}
      <div className="flex gap-2">
        <Button disabled={!model.trim() || test.isPending || save.isPending} onClick={() => test.mutate()}>
          {test.isPending ? "Testing…" : "Test connection"}
        </Button>
        <Button primary disabled={!changed || !model.trim() || save.isPending || test.isPending} onClick={() => save.mutate()}>
          {save.isPending ? "Saving…" : "Use this model"}
        </Button>
      </div>
    </div>
  );
}

function Capabilities({ caps }: { caps: ModelCapabilities }) {
  return (
    <div className="flex flex-wrap gap-1.5 text-xs">
      <Chip label="Streaming" status={caps.streaming} />
      <Chip label="JSON schema" status={caps.jsonSchema} />
      <Chip label="Tool calls" status={caps.toolCalls} />
      {!caps.probedAt && <span className="text-stone-500">Not tested yet; safest settings in use.</span>}
    </div>
  );
}

function Chip({ label, status }: { label: string; status: CapabilityStatus }) {
  const style = {
    supported: "bg-green-100 text-green-800",
    unsupported: "bg-stone-200 text-stone-600",
    needs_test: "bg-amber-50 text-amber-800",
  }[status];
  const mark = { supported: "✓", unsupported: "✕", needs_test: "?" }[status];
  return (
    <span className={`rounded px-1.5 py-0.5 ${style}`}>
      {mark} {label}
    </span>
  );
}

function ProbeNotes({ report }: { report: ProbeReport }) {
  return (
    <div role="status" className={`rounded p-2 text-xs ${report.reachable ? "bg-green-50" : "bg-red-50 text-red-800"}`}>
      {report.reachable && <Capabilities caps={report.capabilities} />}
      <ul className="mt-1 space-y-0.5">
        {report.notes.map((n, i) => (
          <li key={i}>{n}</li>
        ))}
      </ul>
    </div>
  );
}

function ProviderRow({ provider: p, onEdit }: { provider: ProviderProfile; onEdit: () => void }) {
  const client = useClient();
  const queryClient = useQueryClient();
  const remove = useMutation({
    mutationFn: () => client.deleteProvider(p.id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: keys.models }),
  });
  return (
    <div className="rounded-lg border border-stone-200 bg-white p-3">
      <div className="flex items-center justify-between gap-2">
        <p className="font-medium">{p.name}</p>
        <LocalBadge local={p.isLocal} label={p.isLocal ? "local" : "cloud"} />
      </div>
      {p.kind !== "demo" && (
        <>
          <p className="truncate text-xs text-stone-500">{p.baseUrl}</p>
          <p className="text-xs text-stone-500">{p.hasApiKey ? "API key saved in Keychain" : "No API key"}</p>
          {remove.error && <p role="alert" className="text-red-700">{String(remove.error)}</p>}
          <div className="mt-2 flex gap-2">
            <Button onClick={onEdit}>Edit</Button>
            <Button disabled={remove.isPending} onClick={() => remove.mutate()}>
              Remove
            </Button>
          </div>
        </>
      )}
    </div>
  );
}

function ProviderForm({ provider, onDone }: { provider?: ProviderProfile; onDone: () => void }) {
  const client = useClient();
  const queryClient = useQueryClient();
  const [name, setName] = useState(provider?.name ?? "");
  const [baseUrl, setBaseUrl] = useState(provider?.baseUrl ?? "");
  const [apiKey, setApiKey] = useState("");
  const [removeKey, setRemoveKey] = useState(false);
  const save = useMutation({
    mutationFn: () =>
      client.saveProvider(provider?.id ?? null, {
        name: name.trim(),
        baseUrl: baseUrl.trim(),
        // Empty field on edit keeps the stored key; the key is never read back.
        apiKey: removeKey ? "" : apiKey.trim() ? apiKey.trim() : null,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: keys.models });
      void queryClient.invalidateQueries({ queryKey: ["providerModels"] });
      onDone();
    },
  });
  function submit(e: FormEvent) {
    e.preventDefault();
    save.mutate();
  }
  return (
    <form onSubmit={submit} className="space-y-2 rounded-lg border border-orange-200 bg-white p-3">
      {!provider && (
        <div className="flex flex-wrap gap-1.5">
          {PRESETS.map((p) => (
            <button
              type="button"
              key={p.name}
              className="rounded border border-stone-300 px-2 py-0.5 text-xs"
              onClick={() => {
                setName(p.name);
                setBaseUrl(p.baseUrl);
              }}
            >
              {p.name}
            </button>
          ))}
        </div>
      )}
      <Field label="Name">
        <input value={name} onChange={(e) => setName(e.target.value)} className="w-full rounded border border-stone-300 px-2 py-1" />
      </Field>
      <Field label="Base URL (OpenAI-compatible, usually ends in /v1)">
        <input
          type="url"
          required
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.target.value)}
          placeholder="http://localhost:11434/v1"
          className="w-full rounded border border-stone-300 px-2 py-1"
        />
      </Field>
      <Field label={provider?.hasApiKey ? "API key (leave empty to keep the saved one)" : "API key (optional for local servers)"}>
        <input
          type="password"
          autoComplete="off"
          value={apiKey}
          disabled={removeKey}
          onChange={(e) => setApiKey(e.target.value)}
          className="w-full rounded border border-stone-300 px-2 py-1"
        />
      </Field>
      {provider?.hasApiKey && (
        <label className="flex items-center gap-1.5 text-xs">
          <input type="checkbox" checked={removeKey} onChange={(e) => setRemoveKey(e.target.checked)} />
          Remove the saved key
        </label>
      )}
      <p className="text-xs text-stone-500">Keys are stored in the macOS Keychain and never shown again.</p>
      {save.error && <p role="alert" className="text-red-700">{String(save.error)}</p>}
      <div className="flex gap-2">
        <Button primary type="submit" disabled={!name.trim() || !baseUrl.trim() || save.isPending}>
          {save.isPending ? "Saving…" : "Save"}
        </Button>
        <Button type="button" disabled={save.isPending} onClick={onDone}>
          Cancel
        </Button>
      </div>
    </form>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block space-y-0.5">
      <span className="text-xs text-stone-600">{label}</span>
      {children}
    </label>
  );
}

function LocalBadge({ local, label }: { local: boolean; label: string }) {
  return (
    <span className={`rounded px-1.5 py-0.5 text-xs ${local ? "bg-green-100 text-green-800" : "bg-amber-100 text-amber-800"}`}>
      {label}
    </span>
  );
}

function Button({ primary, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { primary?: boolean }) {
  return (
    <button
      type="button"
      {...props}
      className={`rounded-md px-3 py-1 font-medium disabled:opacity-40 ${
        primary ? "bg-orange-500 text-white" : "border border-stone-300 bg-white"
      }`}
    />
  );
}
