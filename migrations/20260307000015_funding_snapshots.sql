CREATE TABLE funding_snapshots (
    id              SERIAL PRIMARY KEY,
    project_id      VARCHAR(42) NOT NULL REFERENCES projects(project_id),
    cumulative_usdc NUMERIC NOT NULL,
    investor_count  INT NOT NULL,
    snapshot_date   BIGINT NOT NULL,
    UNIQUE(project_id, snapshot_date)
);
CREATE INDEX idx_funding_snapshots_project ON funding_snapshots(project_id);
