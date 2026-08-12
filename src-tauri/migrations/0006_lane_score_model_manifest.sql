-- Immutable manifest rows make a cached score reproducible without treating an
-- experimental parameter set as a calibrated claim.
CREATE TABLE lane_score_model_manifests (
    model_version TEXT NOT NULL,
    feature_schema_version TEXT NOT NULL,
    derivation_version TEXT NOT NULL,
    ruleset_version TEXT NOT NULL,
    parameter_hash TEXT NOT NULL,
    valid_patch_from TEXT NOT NULL,
    valid_patch_to TEXT,
    calibration_dataset_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('EXPERIMENTAL_INITIAL_HYPOTHESIS', 'CALIBRATED')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (model_version, feature_schema_version, derivation_version, ruleset_version, parameter_hash)
);
