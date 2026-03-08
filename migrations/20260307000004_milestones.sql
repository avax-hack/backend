CREATE TABLE milestones (
    id              SERIAL PRIMARY KEY,
    project_id      VARCHAR(42) NOT NULL REFERENCES projects(project_id),
    milestone_index INT NOT NULL,
    title           VARCHAR(200) NOT NULL,
    description     TEXT NOT NULL,
    allocation_bps  INT NOT NULL,
    status          VARCHAR(20) NOT NULL DEFAULT 'pending',
    funds_released  BOOLEAN NOT NULL DEFAULT FALSE,
    release_amount  NUMERIC,
    evidence_uri    TEXT,
    evidence_text   TEXT,
    submitted_at    BIGINT,
    verified_at     BIGINT,
    tx_hash         VARCHAR(66),
    UNIQUE(project_id, milestone_index)
);
CREATE INDEX idx_milestones_project ON milestones(project_id);
