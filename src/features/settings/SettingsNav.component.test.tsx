import * as React from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { expect, it } from "vitest";
import type { SettingsSectionId } from "../../lib/settingsSections";
import { SettingsNav } from "./SettingsView";

function Harness({ orientation }: { orientation: "vertical" | "horizontal" }) {
  const [active, setActive] = React.useState<SettingsSectionId>("ai-behavior");
  return (
    <>
      <SettingsNav active={active} onSelect={setActive} orientation={orientation} />
      <section id="settings-panel" role="tabpanel" aria-labelledby={`settings-tab-${active}`} />
    </>
  );
}

it.each(["vertical", "horizontal"] as const)(
  "exposes one keyboard-operable Tools tab in the %s navigation",
  async (orientation) => {
    const user = userEvent.setup();
    const { container } = render(<Harness orientation={orientation} />);
    const tablist = screen.getByRole("tablist", { name: "Settings sections" });
    expect(tablist).toHaveAttribute("aria-orientation", orientation);
    expect(screen.getAllByRole("tab", { name: "Tools" })).toHaveLength(1);

    const first = screen.getByRole("tab", { name: "AI & Behavior" });
    first.focus();
    await user.keyboard(orientation === "vertical" ? "{ArrowDown}" : "{ArrowRight}");
    expect(screen.getByRole("tab", { name: "Providers" })).toHaveFocus();
    expect(screen.getByRole("tab", { name: "Providers" })).toHaveAttribute("aria-selected", "true");

    await user.keyboard("{End}");
    expect(screen.getByRole("tab", { name: "Advanced" })).toHaveFocus();
    await user.keyboard(orientation === "vertical" ? "{ArrowDown}" : "{ArrowRight}");
    expect(first).toHaveFocus();
    expect((await axe(container, { rules: { "color-contrast": { enabled: false } } })).violations).toEqual([]);
  },
);
