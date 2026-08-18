export function needsNewChatConfirmation(draft: string, discardDraft: boolean): boolean {
  return !discardDraft && draft.trim().length > 0;
}
