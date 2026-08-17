-- CODE-001: distinguish provider-native tool calling from Ark's prompted fallback. The legacy
-- supports_tools flag remains the wire-compatible native-support signal; this column records the
-- complete capability decision used by Ark Code.
ALTER TABLE models ADD COLUMN tool_calling_mode TEXT NOT NULL DEFAULT 'unsupported'
    CHECK (tool_calling_mode IN ('native', 'prompted', 'unsupported'));

UPDATE models
SET tool_calling_mode = 'native'
WHERE supports_tools = 1;
