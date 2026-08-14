-- FTR-004: `providers.streaming_enabled` is stored and has a real writer (`update_provider`),
-- but nothing in `generation.rs` has ever read it — every generation call streams
-- unconditionally, gated only by `ProviderCapabilities::for_provider_type`'s fixed per-type
-- `streaming: true` flag, not this per-provider-instance setting. No adapter implements a
-- non-streaming code path (`Provider` trait only defines `stream_chat`), so there was never a
-- working way to actually turn this toggle "off" and get a response. Resolving this task's own
-- "remove or implement the hard-coded streaming toggle" acceptance criterion via removal: this
-- is not a live, working, currently-editable setting to preserve.
ALTER TABLE providers DROP COLUMN streaming_enabled;
