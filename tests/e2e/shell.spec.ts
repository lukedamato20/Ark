import { type Page, expect, test } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

async function openSidebarIfNeeded(page: Page) {
  const toggle = page.getByRole("button", { name: "Open conversations" });
  if (await toggle.isVisible()) await toggle.click();
}

test("startup reflects bootstrap readiness and failure without an artificial hold", async ({ page }) => {
  await page.goto("/?fixture=delayed-bootstrap");
  await expect(page.getByRole("status", { name: "Starting Ark" })).toBeVisible();
  await expect(page.getByRole("status", { name: "Starting Ark" })).toBeHidden();
  await openSidebarIfNeeded(page);
  await expect(page.getByRole("button", { name: /new chat/i })).toBeVisible();

  await page.goto("/?fixture=bootstrap-failure");
  await expect(page.getByRole("alert")).toContainText("Ark couldn't start up");
  await expect(page.getByRole("button", { name: "Retry" })).toBeVisible();
});

test("conversation fixture is keyboard reachable and has no serious accessibility violations", async ({ page }) => {
  await page.goto("/?fixture=conversation-organization");
  await openSidebarIfNeeded(page);
  await expect(page.getByRole("button", { name: /new chat/i })).toBeVisible();

  await page.keyboard.press("Tab");
  await expect(page.locator(":focus-visible")).toBeVisible();

  const results = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa"]).analyze();
  expect(results.violations.filter((violation) => ["serious", "critical"].includes(violation.impact ?? ""))).toEqual(
    [],
  );
});

test("New Chat protects an unsent draft and creates after confirmation", async ({ page }) => {
  await page.goto("/?fixture=conversation-organization");
  const composer = page.getByPlaceholder(/Ask Ark/i);
  await composer.fill("unsent private draft");
  await openSidebarIfNeeded(page);
  const newChat = page.getByRole("button", { name: "New Chat" });
  await newChat.click();

  const dialog = page.getByRole("alertdialog", { name: "Discard unsent message?" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(composer).toHaveValue("unsent private draft");

  await openSidebarIfNeeded(page);
  await newChat.click();
  await dialog.getByRole("button", { name: "Discard and create chat" }).click();
  await expect(composer).toHaveValue("");
});

test("responsive shell has no page overflow and preserves Chat state across Code switching", async ({ page }) => {
  await page.goto("/?fixture=conversation-organization");
  const composer = page.getByPlaceholder(/Ask Ark/i);
  await composer.fill("draft survives mode switching");

  await openSidebarIfNeeded(page);
  const codeMode = page.getByRole("button", { name: "Ark Code", exact: true });
  await codeMode.click();
  await expect(page.getByText("Repository investigation and approved edits")).toBeVisible();
  await openSidebarIfNeeded(page);
  const chatMode = page.getByRole("button", { name: "Ark Chat", exact: true });
  await chatMode.click();
  await expect(composer).toHaveValue("draft survives mode switching");

  const overflow = await page.evaluate(() => ({
    document: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    body: document.body.scrollWidth - document.body.clientWidth,
  }));
  expect(overflow.document).toBeLessThanOrEqual(1);
  expect(overflow.body).toBeLessThanOrEqual(1);
});

test("Models presents installed cards before the curated library with honest lifecycle guidance", async ({ page }) => {
  await page.goto("/?fixture=ollama-models");
  await openSidebarIfNeeded(page);
  const settings = page.getByRole("button", { name: "Settings", exact: true });
  await settings.click();
  await page.getByRole("tab", { name: "Models" }).click();

  const installed = page.getByRole("heading", { name: "Your Models" });
  const curated = page.getByRole("heading", { name: "Curated Ollama Library" });
  await expect(installed).toBeVisible();
  await expect(curated).toBeVisible();
  expect(
    await installed.evaluate((installedNode) => {
      const curatedNode = [...document.querySelectorAll("h3")].find(
        (heading) => heading.textContent?.trim() === "Curated Ollama Library",
      );
      return Boolean(
        curatedNode && installedNode.compareDocumentPosition(curatedNode) & Node.DOCUMENT_POSITION_FOLLOWING,
      );
    }),
  ).toBe(true);
  await expect(page.getByText("Metadata confidence: reported").first()).toBeVisible();
  await expect(page.getByText(/Hardware fit:/).first()).toContainText("confidence");

  const largeModel = page.getByRole("listitem").filter({ has: page.getByRole("heading", { name: "Llama 3.1 8B" }) });
  await largeModel.getByRole("button", { name: "Pull" }).click();
  await expect(page.getByRole("alert")).toContainText("may fail partway through if space runs out");
  await page.getByRole("button", { name: "Cancel" }).click();

  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  expect(overflow).toBeLessThanOrEqual(1);
});
