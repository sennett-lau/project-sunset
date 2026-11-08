import type { SaveMode } from "@sunset/client";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useClient } from "../../client-context";
import { keys } from "../../query-keys";

export function SaveModeToggle() {
  const client = useClient();
  const queryClient = useQueryClient();
  const { data: mode, error } = useQuery({ queryKey: keys.saveMode, queryFn: () => client.saveMode() });
  const set = useMutation({
    mutationFn: (m: SaveMode) => client.setSaveMode(m),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: keys.saveMode }),
  });
  if (!mode) return <span role="status" className="text-xs">{error ? "Save mode unavailable" : "Loading save mode…"}</span>;
  return (
    <label className="flex items-center gap-2 text-sm text-stone-600">
      Save
      <select
        disabled={set.isPending}
        value={mode}
        onChange={(e) => set.mutate(e.target.value as SaveMode)}
        className="rounded border border-stone-300 bg-white px-1.5 py-0.5"
      >
        <option value="approval">after I approve</option>
        <option value="auto">automatically</option>
      </select>
      {set.error && <span role="alert" className="text-red-700">Could not change save mode.</span>}
    </label>
  );
}
