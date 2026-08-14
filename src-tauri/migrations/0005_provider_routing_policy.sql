-- SEC-001: records the explicit development-mode exception for non-loopback HTTP.
-- `providers.is_local` is the persisted local-only/remote provider class.
ALTER TABLE providers ADD COLUMN allow_insecure_remote INTEGER NOT NULL DEFAULT 0;
