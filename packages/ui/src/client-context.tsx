import type { SunsetClient } from "@sunset/client";
import { createContext, useContext } from "react";

const ClientContext = createContext<SunsetClient | null>(null);

export const ClientProvider = ClientContext.Provider;

/** The backend. Components use this instead of any platform API. */
export function useClient(): SunsetClient {
  const client = useContext(ClientContext);
  if (!client) throw new Error("useClient must be used inside <ClientProvider>");
  return client;
}
