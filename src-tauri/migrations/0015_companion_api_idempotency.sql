-- FTR-010: persisted idempotency results for mutating companion API operations. Keeping the
-- request fingerprint and serialized response in the workspace database lets retries after a
-- lost HTTP response or application restart return the original result without repeating the
-- mutation. The bearer token itself remains in the OS credential store and never enters here.
CREATE TABLE IF NOT EXISTS companion_api_idempotency (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    idempotency_key TEXT NOT NULL UNIQUE
        CHECK(length(idempotency_key) BETWEEN 1 AND 128),
    method TEXT NOT NULL,
    path TEXT NOT NULL,
    request_hash TEXT NOT NULL
        CHECK(length(request_hash) = 64),
    response_status INTEGER NOT NULL
        CHECK(response_status BETWEEN 200 AND 299),
    response_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);
