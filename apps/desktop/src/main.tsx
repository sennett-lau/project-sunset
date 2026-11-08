import "@sunset/ui/styles.css";
import { App } from "@sunset/ui";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { appIdentity } from "./identity";
import { tauriClient } from "./tauri-client";

document.title = appIdentity.productName;

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing from index.html");

createRoot(root).render(
  <StrictMode>
    <App client={tauriClient} productName={appIdentity.productName} />
  </StrictMode>,
);
