import { render, screen, waitFor } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { axe } from "vitest-axe";
import { expect, it } from "vitest";
import App from "./App";
import { createFakeArkClient, type ArkClient } from "./lib/ArkClient";
import { ArkClientProvider } from "./lib/ArkClientContext";
import { createConversationOrganizationFixtureClient } from "./lib/developmentArkClient";
import { createArkStores } from "./state/arkStores";
import { ArkStateContext } from "./state/arkStateContext";

function renderApp(client: ArkClient) {
  const stores = createArkStores();
  const wrapper = ({ children }: PropsWithChildren) => (
    <ArkClientProvider client={client}>
      <ArkStateContext.Provider value={stores}>{children}</ArkStateContext.Provider>
    </ArkClientProvider>
  );
  return { ...render(<App />, { wrapper }), stores };
}

it("total bootstrap failure replaces startup with an actionable recovery surface", async () => {
  const client = createFakeArkClient({
    getAppBootstrap: async () => {
      throw { code: "database_corrupt", message: "The workspace database could not be read." };
    },
    getBuiltInRuntimeStatus: async () => ({
      running: false,
      binaryInstalled: false,
      binaryVerified: false,
      state: "unavailable_binary",
      failure: null,
    }),
  });
  const view = renderApp(client);

  const alert = await screen.findByRole("alert");
  expect(alert).toHaveTextContent("Ark couldn't start up");
  expect(screen.getByRole("button", { name: "Retry" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Open Settings" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Copy diagnostics" })).toBeVisible();
  expect(screen.queryByRole("status", { name: "Starting Ark" })).not.toBeInTheDocument();
  expect((await axe(view.container, { rules: { "color-contrast": { enabled: false } } })).violations).toEqual([]);
});

it("workspace recovery pre-empts startup while preserving the usable shell", async () => {
  const fixture = createConversationOrganizationFixtureClient();
  const bootstrap = await fixture.getAppBootstrap();
  const client = createFakeArkClient({
    ...fixture,
    getAppBootstrap: async () => ({
      ...bootstrap,
      workspaceOpenError: { code: "database_locked", message: "Workspace is locked." },
    }),
  });
  renderApp(client);

  const alert = await screen.findByRole("alert");
  expect(alert).toHaveTextContent("Workspace database unavailable");
  expect(screen.queryByRole("status", { name: "Starting Ark" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Ark Chat" })).toBeVisible();
});

it("a pending background provider refresh never holds the startup surface", async () => {
  const fixture = createConversationOrganizationFixtureClient();
  const client = createFakeArkClient({
    ...fixture,
    refreshModels: () => new Promise(() => undefined),
  });
  renderApp(client);

  await waitFor(() => expect(screen.queryByRole("status", { name: "Starting Ark" })).not.toBeInTheDocument());
  expect(screen.getByRole("button", { name: "Ark Chat" })).toBeVisible();
});
