import { render } from "@testing-library/react";
import { axe } from "vitest-axe";
import { expect, it } from "vitest";

it("fails closed when axe reports a serious or critical violation", async () => {
  const { container } = render(<img src="fixture.png" />);
  const results = await axe(container);
  const releaseBlocking = results.violations.filter((violation) =>
    ["serious", "critical"].includes(violation.impact ?? ""),
  );
  expect(releaseBlocking.map((violation) => violation.id)).toContain("image-alt");
});
