export interface AppProps {
  /** From app-identity.json. Never hard-code the product name in the UI. */
  productName: string;
}

export function App({ productName }: AppProps) {
  return (
    <main style={{ fontFamily: "system-ui, sans-serif", padding: "2rem" }}>
      <h1>{productName}</h1>
      <p>Phase 0 skeleton. Nothing to learn here yet.</p>
    </main>
  );
}
