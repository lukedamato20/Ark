import { render, screen } from "@testing-library/react";
import { axe } from "vitest-axe";
import { expect, it } from "vitest";
import { ActivityIndicator } from "./activityIndicator";

it("distinguishes active work from user wait without exposing untrusted tool text", async () => {
  const { container, rerender } = render(<ActivityIndicator state="generating" />);
  expect(screen.getByRole("status")).toHaveTextContent("Generating");

  rerender(<ActivityIndicator state="approval" />);
  expect(screen.getByRole("status")).toHaveTextContent("Waiting for approval");

  rerender(<ActivityIndicator state="tool" toolName={'<img src=x onerror="alert(1)">'} />);
  expect(screen.getByRole("status")).toHaveTextContent("Using tool");
  expect(container.querySelector("img")).toBeNull();
  expect(container.querySelectorAll(".motion-reduce\\:animate-none")).toHaveLength(3);
  expect((await axe(container, { rules: { "color-contrast": { enabled: false } } })).violations).toEqual([]);
});

it("can defer announcements to an enclosing status region", () => {
  render(<ActivityIndicator state="preparing" announce={false} />);
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
  expect(screen.getByText("Preparing")).toBeVisible();
});
