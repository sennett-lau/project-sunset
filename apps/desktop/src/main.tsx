import { App } from "@sunset/ui";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { appIdentity } from "./identity";

document.title = appIdentity.productName;

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing from index.html");

createRoot(root).render(
  <StrictMode>
    <App productName={appIdentity.productName} />
  </StrictMode>,
);
