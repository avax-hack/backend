CREATE TABLE accounts (
    account_id      VARCHAR(42) PRIMARY KEY,
    nickname        VARCHAR(50) NOT NULL DEFAULT '',
    bio             TEXT NOT NULL DEFAULT '',
    image_uri       TEXT NOT NULL DEFAULT '',
    created_at      BIGINT NOT NULL,
    updated_at      BIGINT NOT NULL
);
