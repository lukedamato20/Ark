# Credential storage, exports, and restore

Ark stores provider credentials through a platform `SecretStore` adapter:

- Windows Credential Manager on Windows;
- Keychain on macOS;
- Secret Service on supported Linux desktops.

The workspace database stores only a versioned opaque identifier (`secret:v1:<UUID>`). The raw
credential enters the Rust command once, is moved to the operating-system store, and is never
returned through IPC. Public responses contain only the opaque identifier, a fixed mask, and an
availability flag. The password field is cleared as soon as submission begins, is excluded from
browser credential completion, and Ark never copies it to the clipboard.

## Export and backup behavior

Conversation Markdown and JSON exports never contain credentials. JSON export also removes the
device-local opaque identifier because it is not portable. A future full-workspace backup may
copy the SQLite identifier as part of the database, but it must not copy the operating-system
credential-store entry. Secret values are likewise excluded from diagnostics, runtime logs,
crash-report payloads, and support artifacts.

After restoring on another machine or OS account, Ark reports the credential as unavailable and
asks the user to reconnect the affected provider. Restoring on the same OS account can continue
to use an existing entry when the opaque identifier still resolves. Deleting a provider
credential removes the OS entry before clearing the database reference; a missing entry is
treated as an idempotent delete.

## Locked or unavailable stores

Settings reports credential-store availability independently of any provider. If the platform
store is locked, denied, or unavailable, Ark disables credential submission, explains the
platform action needed, and provides Retry. It does not fall back to SQLite, localStorage, a
plaintext file, or custom encryption.
