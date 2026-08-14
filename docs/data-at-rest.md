# Local data-at-rest protection

Ark's workspace — conversations, messages, and provider configuration — lives in a single SQLite
file on disk. This document states plainly what protects that file today, what an optional
encrypted workspace adds, and which threats each level does and does not defend against.

## Default: plaintext, permission-hardened

By default the workspace database is **plaintext SQLite**. Ark does not encrypt it unless you
turn on workspace encryption in Settings → Storage. Plaintext is not a bug to be hidden: it is
disclosed here and in Settings before you choose whether to enable encryption.

Every workspace file and directory Ark creates is hardened to the current OS user only: `0700`
directories and `0600` files on Unix-like systems, and a protected DACL with a single
full-control ACE for the current process user on Windows (see `file_permissions.rs`). Unsupported
platforms keep their default permissions and are documented as such rather than silently claiming
protection they don't provide.

## Optional: SQLCipher-encrypted workspace

Settings → Storage lets you turn workspace encryption on, rotate its key, or turn it back off.
When enabled:

- The database is re-opened as a SQLCipher-encrypted file. The passphrase is a randomly generated
  key, never a user-chosen password.
- The key itself is stored only as an opaque reference in workspace metadata; the actual key value
  lives in the operating system's credential store (Windows Credential Manager, macOS Keychain, or
  a Secret Service–backed Linux store) — see
  [Credential storage, export, and restore behavior](secrets-and-backups.md) for how that store
  behaves when locked or unavailable.
- Turning encryption on, rotating the key, or turning it off is **copy-based and verified**: Ark
  writes a full copy of the database in the new mode, verifies that copy can actually be opened
  and read back correctly, and only then atomically swaps it into place. Your original file is
  never modified or deleted until the new copy has proven itself.
- A transition journal records which mode change was in progress. If Ark crashes mid-transition,
  the next launch reads that journal and either finalizes or rolls back the change — it does not
  guess, and it never leaves you with a database that silently reverted to plaintext or silently
  lost the encrypted copy.
- Enabling or rotating encryption shows a **recovery key** once, on screen. It is the only way to
  restore access if the operating-system credential store entry is ever lost (a fresh OS install,
  a moved profile, a corrupted credential store). Ark does not display it again and does not store
  it anywhere itself — write it down.
- **A forgotten key cannot be reset.** Ark does not implement a backdoor, and SQLCipher's
  authenticated encryption makes "recover without the key" cryptographically impossible by design.
  If both the OS credential store entry and the recovery key are lost, the encrypted workspace is
  unrecoverable; you would need to restore from an external backup or start a new workspace.

## Threat model

Encryption at rest defends against specific threats and not others. Being explicit about which is
what makes the "encrypted" claim honest rather than a vague sense of safety.

| Scenario | Plaintext workspace | Encrypted workspace |
|---|---|---|
| **Disk theft / lost device**, powered off, no OS-level full-disk encryption | Data is fully readable by whoever has the disk. | Data is unreadable without the OS-credential-store key or the recovery key. |
| **Disk theft / lost device**, OS full-disk encryption enabled (BitLocker, FileVault, LUKS) | Already protected by the OS while powered off; this is the primary defense most users should rely on first. | Adds a second, independent layer — relevant if the OS disk encryption is later disabled, misconfigured, or bypassed. |
| **Another account on the same machine** | Blocked only by OS file permissions (see hardening above) — a privileged/admin account on the same OS can still read the file. | The file itself is unreadable without the key even if OS permissions are somehow bypassed, but an admin account can typically still read the OS credential store too. Encryption here mainly raises the bar, it does not make the data inaccessible to a fully privileged local attacker. |
| **Malware running in your own OS session** | No protection — malware running as you can read anything you can read, including the plaintext database and (if it can reach the OS credential store the same way Ark does) the encryption key. | **Not defended against.** If code runs with your privileges, it can ask the same OS credential store Ark uses and unlock the database the same way Ark does. Workspace encryption is not a substitute for keeping your own session free of malware. |
| **Cloud-synced folders** (Dropbox, OneDrive, iCloud Drive, etc.) pointed at the workspace directory | The plaintext database is uploaded and stored, unencrypted, by the sync provider — readable by anyone with access to that cloud account or a breach of it. | The database uploaded to the sync provider is SQLCipher-encrypted, so the sync provider (and anyone who breaches that account) sees ciphertext. The *recovery key*, if you saved it into the same synced folder, would not be — save it somewhere else. |

The short version: OS-level full-disk encryption is your primary defense against a lost or stolen
device and should be enabled regardless of anything Ark does. Ark's optional workspace encryption
adds a second layer that specifically matters for cloud-synced workspace folders and for a
powered-off device without OS disk encryption. Neither layer defends against malware or another
privileged account on an already-unlocked machine — nothing running at the OS-user level can.

## See also

- [Credential storage, export, and restore behavior](secrets-and-backups.md) — how the OS
  credential store itself is used and what happens when it is locked or unavailable.
