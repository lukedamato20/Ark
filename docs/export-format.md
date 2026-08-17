# Ark JSON export format

Ark's portable JSON is user-owned data, not an internal database dump. It excludes credentials,
credential references, caches, runtime binaries, model files, and diagnostics. JSON exports are
plaintext and may contain sensitive conversations and full attachment content; the desktop asks
for confirmation before writing one.

## Supported versions

- Conversation export: versions 1–2; Ark currently writes version 2.
- Workspace export: versions 1–2; Ark currently writes version 2.
- Attachment export: version 1, introduced by conversation/workspace version 2.

Unknown object fields are ignored within these supported versions. Higher versions are rejected
before database mutation. Version 1 remains importable and has no `attachments`,
`attachmentCount`, or `entityVersions` fields.

## Workspace bundle shape (version 2)

```json
{
  "manifest": {
    "schemaVersion": 2,
    "exportedAt": "ISO-8601 timestamp",
    "scope": "workspace or project:<stable-id>",
    "entityVersions": {
      "conversation": 2,
      "message": 1,
      "provider": 1,
      "attachment": 1
    },
    "entries": [
      {
        "conversationId": "stable source ID",
        "title": "Conversation title",
        "messageCount": 2,
        "attachmentCount": 1,
        "sha256": "content fingerprint"
      }
    ]
  },
  "conversations": ["conversation export objects described below"]
}
```

Every manifest entry must have exactly one conversation object. Counts and hashes are verified
before preview/import. Duplicate manifest IDs, missing entity versions, altered attachment
metadata/content, and mismatched hashes are rejected.

## Conversation shape (version 2)

```json
{
  "schemaVersion": 2,
  "exportedAt": "ISO-8601 timestamp",
  "conversation": "the public Conversation object",
  "messages": ["public Message objects, including branch/provenance fields"],
  "provider": "the public ProviderConfig object or null, always with apiKeyRef null",
  "attachments": [
    {
      "schemaVersion": 1,
      "attachment": {
        "id": "source attachment ID",
        "conversationId": "source conversation ID",
        "messageId": "source message ID or null",
        "fileName": "evidence.txt",
        "byteSize": 12,
        "sha256": "SHA-256 of UTF-8 content",
        "createdAt": "ISO-8601 timestamp"
      },
      "content": "full validated plain-text content"
    }
  ]
}
```

On import Ark assigns new conversation, message, and attachment IDs. Parent/revision links,
current branch selection, and attachment-to-message links are remapped atomically. Original
message IDs remain in import provenance metadata. Unknown providers map to the default local
provider; exported provider configuration never creates a remote endpoint.

## Content fingerprint

The version-2 manifest SHA-256 is deterministic across ID/timestamp remapping. Ark hashes the
existing message fingerprint (role, content, and path index in order), followed by each
attachment's file name and verified content SHA-256 in stable creation order. Version 1 hashes
messages only. This supports duplicate detection and tamper/corruption checks; it is not a digital
signature and does not prove who created a bundle.

## Markdown exports

Markdown is intentionally human-readable without Ark. It includes active-path chat content,
provider/model labels, a branch-presence notice, and an Attachments section containing file name,
size, digest, source-message reference, and indented plain-text content. Credentials are never
included.
