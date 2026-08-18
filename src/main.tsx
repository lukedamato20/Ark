import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { createTauriArkClient } from "./lib/ArkClient";
import { ArkClientProvider } from "./lib/ArkClientContext";
import { ArkStateProvider } from "./state/ArkStateProvider";
import "@fontsource-variable/inter/wght.css";
import "./styles.css";

const fixture = import.meta.env.DEV ? new URLSearchParams(window.location.search).get("fixture") : null;
const developmentClients = fixture ? await import("./lib/developmentArkClient") : null;
const arkClient =
  fixture === "runtime-provenance"
    ? developmentClients!.createRuntimeProvenanceFixtureClient()
    : fixture === "secret-store"
      ? developmentClients!.createSecretStoreFixtureClient()
      : fixture === "workspace-protection"
        ? developmentClients!.createWorkspaceProtectionFixtureClient()
        : fixture === "long-conversation"
          ? developmentClients!.createLongConversationFixtureClient()
          : fixture === "conversation-organization"
            ? developmentClients!.createConversationOrganizationFixtureClient()
            : fixture === "ollama-models"
              ? developmentClients!.createOllamaModelsFixtureClient()
              : fixture === "delayed-bootstrap"
                ? developmentClients!.createDelayedBootstrapFixtureClient()
                : fixture === "bootstrap-failure"
                  ? developmentClients!.createBootstrapFailureFixtureClient()
                  : fixture === "code-edit"
                    ? developmentClients!.createCodeEditFixtureClient()
                    : createTauriArkClient();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ArkClientProvider client={arkClient}>
      <ArkStateProvider>
        <App />
      </ArkStateProvider>
    </ArkClientProvider>
  </React.StrictMode>,
);
