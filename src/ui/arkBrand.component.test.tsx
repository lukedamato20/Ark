import { render, screen } from "@testing-library/react";
import { axe } from "vitest-axe";
import { expect, it } from "vitest";
import { ArkBrand } from "./arkBrand";

it("renders accessible wordmark and compact variants from one canonical asset", async () => {
  const wordmark = render(<ArkBrand />);
  expect(screen.getByText("Ark")).toBeVisible();
  expect(wordmark.container.querySelector("img")?.getAttribute("src")).toMatch(/^data:image\/svg\+xml,/);
  expect((await axe(wordmark.container, { rules: { "color-contrast": { enabled: false } } })).violations).toEqual([]);
  expect(wordmark.container).toMatchSnapshot();
  wordmark.unmount();

  const compact = render(<ArkBrand compact />);
  expect(screen.getByRole("img", { name: "Ark" })).not.toHaveTextContent("Ark");
  expect(compact.container.querySelectorAll("img")).toHaveLength(1);
  expect((await axe(compact.container, { rules: { "color-contrast": { enabled: false } } })).violations).toEqual([]);
  expect(compact.container).toMatchSnapshot();
});
