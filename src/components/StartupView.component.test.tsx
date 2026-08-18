import { render, screen } from "@testing-library/react";
import { axe } from "vitest-axe";
import { expect, it } from "vitest";
import { StartupView } from "./StartupView";

it("shows one branded, accessible actual-state startup surface", async () => {
  const { container } = render(<StartupView />);
  expect(screen.getByRole("status", { name: "Starting Ark" })).toBeVisible();
  expect(screen.getByText("Preparing")).toBeVisible();
  const results = await axe(container, { rules: { "color-contrast": { enabled: false } } });
  expect(results.violations).toEqual([]);
});
