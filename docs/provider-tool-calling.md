# Provider tool-calling contract

Ark Code selects tool behavior from each refreshed model's `toolCallingMode`; it does not infer
support from a provider name.

- `native`: the provider/model metadata reports structured tool support. Ark sends JSON-schema
  functions through the provider's native protocol and parses structured calls.
- `prompted`: metadata reports a chat/completion model but no native tool support. Ark uses the
  fallback protocol below.
- `unsupported`: metadata does not establish either capability. Ark fails the step before sending
  tools.

`supportsTools` remains `true` only for `native`. This preserves its original meaning while the
three-state field makes fallback explicit.

## Prompted fallback protocol v1

The model receives bounded, JSON-serialized tool definitions and must return exactly one JSON
object, without Markdown or surrounding text:

```json
{"type":"tool_call","name":"tool_name","arguments":{}}
```

If no tool is needed, it may return:

```json
{"type":"text","content":"response"}
```

Ark accepts one tool call per fallback turn. It rejects unknown tool names, non-object arguments,
extra protocol fields, invalid JSON, empty text, and oversized output. After a completed but
malformed response, Ark includes that response as assistant history and sends one fixed repair
instruction. If the repaired response is still malformed, the step fails. Ark does not retry
transport, authentication, timeout, or provider failures in this layer because doing so could
duplicate billable work.

This contract only requests a call. It does not authorize or execute one. Tool permission,
side-effect previews, argument validation at the concrete tool boundary, audit logging, and durable
agent-run state remain authoritative in Ark's tool/security layers.

## Metadata sources

- Ollama: `/api/show` `capabilities` and architecture-specific `model_info.*.context_length`.
- llama.cpp/local OpenAI-compatible runtime: `/props`
  `default_generation_settings.n_ctx`, `chat_template`, and
  `chat_template_caps.supports_tools/supports_tool_calls`.
- Other OpenAI-compatible inventories: explicitly reported `context_window`/`context_length` and
  capability fields when present. Missing metadata stays unknown/unsupported.
