import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { describe, expect, it, vi } from "vitest";
import { Button } from "./button";

describe("Button", () => {
  it("is keyboard operable and has no serious accessibility violations", async () => {
    const onClick = vi.fn();
    const user = userEvent.setup();
    const { container } = render(<Button onClick={onClick}>Create chat</Button>);

    await user.tab();
    expect(screen.getByRole("button", { name: "Create chat" })).toHaveFocus();
    await user.keyboard("{Enter}");

    expect(onClick).toHaveBeenCalledOnce();
    const results = await axe(container, {
      rules: { "color-contrast": { enabled: false } },
    });
    expect(results.violations).toEqual([]);
  });
});
