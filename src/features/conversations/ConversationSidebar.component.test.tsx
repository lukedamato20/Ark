import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { expect, it, vi } from "vitest";
import type { Conversation, Project } from "../../types/ark";
import { ConversationSidebar } from "./ConversationSidebar";

it("renders the cohesive navigation hierarchy and expandable search", async () => {
  const user = userEvent.setup();
  const pinned = {
    id: "pinned-1",
    title: "Pinned architecture notes",
    pinnedAt: "2026-08-17T18:00:00Z",
    updatedAt: "2026-08-17T18:30:00Z",
    projectId: "project-1",
    archived: false,
  } as Conversation;
  const project = { id: "project-1", name: "Ark", archivedAt: null } as Project;
  const onProjectFilter = vi.fn();
  const { container } = render(
    <ConversationSidebar
      conversations={[pinned]}
      pinnedConversations={[pinned]}
      projects={[project]}
      activeMode="chat"
      collapsed={false}
      focusSearchSignal={0}
      hasMore={false}
      isLoading={false}
      searchSnippets={{}}
      showArchived={false}
      onToggleCollapsed={vi.fn()}
      onCreate={vi.fn()}
      onCreateProject={vi.fn()}
      onSelect={vi.fn()}
      onSearch={vi.fn()}
      onProjectFilter={onProjectFilter}
      onLoadMore={vi.fn()}
      onOpenSettings={vi.fn()}
      onModeChange={vi.fn()}
      onShowArchivedChange={vi.fn()}
      onArchive={vi.fn()}
      onPin={vi.fn()}
    />,
  );

  expect(screen.getByRole("button", { name: "Ark Chat" })).toBePressed();
  expect(screen.getByText("Pinned")).toBeVisible();
  expect(screen.getByText("Projects")).toBeVisible();
  expect(screen.getByText("Chats")).toBeVisible();
  expect(screen.getAllByText("Pinned architecture notes")).toHaveLength(1);
  expect(screen.queryByText("2026-08-17T18:30:00Z")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Pinned architecture notes" })).toHaveAttribute(
    "title",
    "2026-08-17T18:30:00Z",
  );
  await user.click(screen.getByRole("button", { name: "Ark" }));
  expect(onProjectFilter).toHaveBeenCalledWith("project-1");
  expect(screen.queryByRole("button", { name: /keyboard shortcuts/i })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Search conversations" }));
  const search = screen.getByPlaceholderText("Search conversations");
  await waitFor(() => expect(search).toHaveFocus());
  await user.type(search, "private query");
  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.getByRole("button", { name: "Search conversations" })).toHaveFocus());
  await user.click(screen.getByRole("button", { name: "Search conversations" }));
  expect(screen.getByPlaceholderText("Search conversations")).toHaveValue("private query");

  await user.click(screen.getByRole("button", { name: "Pinned" }));
  expect(screen.getByRole("button", { name: "Pinned" })).toHaveAttribute("aria-expanded", "false");
  await waitFor(() =>
    expect(JSON.parse(localStorage.getItem("ark.sidebar.sections") ?? "{}")).toMatchObject({ pinned: false }),
  );

  const results = await axe(container, { rules: { "color-contrast": { enabled: false } } });
  expect(results.violations).toEqual([]);
});
