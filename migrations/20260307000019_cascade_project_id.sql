-- Allow project_id updates to cascade to referencing tables
ALTER TABLE milestones DROP CONSTRAINT milestones_project_id_fkey;
ALTER TABLE milestones ADD CONSTRAINT milestones_project_id_fkey
    FOREIGN KEY (project_id) REFERENCES projects(project_id) ON UPDATE CASCADE;

ALTER TABLE investments DROP CONSTRAINT investments_project_id_fkey;
ALTER TABLE investments ADD CONSTRAINT investments_project_id_fkey
    FOREIGN KEY (project_id) REFERENCES projects(project_id) ON UPDATE CASCADE;

ALTER TABLE refunds DROP CONSTRAINT refunds_project_id_fkey;
ALTER TABLE refunds ADD CONSTRAINT refunds_project_id_fkey
    FOREIGN KEY (project_id) REFERENCES projects(project_id) ON UPDATE CASCADE;

ALTER TABLE funding_snapshots DROP CONSTRAINT funding_snapshots_project_id_fkey;
ALTER TABLE funding_snapshots ADD CONSTRAINT funding_snapshots_project_id_fkey
    FOREIGN KEY (project_id) REFERENCES projects(project_id) ON UPDATE CASCADE;
