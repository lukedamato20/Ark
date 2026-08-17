/** CODE-005: renders an `edit_file` preview's line-oriented diff text (produced by
 * `code_write_tools::render_hunk` — lines prefixed `"- "`/`"+ "`/`"  "`). Plain React text
 * children are used throughout, so content is escaped by React itself; no raw-HTML injection. */
export function DiffView({ diff }: { diff: string }) {
  const lines =
    diff.length > 0 ? diff.split("\n").filter((_, index, all) => index < all.length - 1 || all[index] !== "") : [];

  return (
    <pre className="max-h-96 overflow-auto rounded-md border border-border bg-muted/30 p-2 text-xs leading-5">
      {lines.map((line, index) => {
        const isAdded = line.startsWith("+ ");
        const isRemoved = line.startsWith("- ");
        const tone = isAdded
          ? "block bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
          : isRemoved
            ? "block bg-destructive/10 text-destructive"
            : "block text-muted-foreground";
        return (
          <span key={index} className={tone}>
            {line.length > 0 ? line : " "}
          </span>
        );
      })}
    </pre>
  );
}
