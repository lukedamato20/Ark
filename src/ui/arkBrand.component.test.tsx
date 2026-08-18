import { render, screen } from "@testing-library/react";
import { axe } from "vitest-axe";
import { expect, it } from "vitest";
import { ArkBrand } from "./arkBrand";

it("renders icon-only brand mark with accessible label", async () => {
  const { container, unmount } = render(<ArkBrand />);
  const brand = screen.getByRole("img", { name: "Ark" });
  expect(brand).toBeVisible();
  expect(brand).not.toHaveTextContent("Ark");
  expect(container.querySelector("img")?.getAttribute("src")).toMatch(/^data:image\/svg\+xml,/);
  expect((await axe(container, { rules: { "color-contrast": { enabled: false } } })).violations).toEqual([]);
  expect(container).toMatchSnapshot();
  unmount();
});

it("compact prop is accepted without error (icon-only in both variants)", async () => {
  const { container } = render(<ArkBrand compact />);
  expect(screen.getByRole("img", { name: "Ark" })).toBeVisible();
  expect(container.querySelectorAll("img")).toHaveLength(1);
  expect((await axe(container, { rules: { "color-contrast": { enabled: false } } })).violations).toEqual([]);
});
