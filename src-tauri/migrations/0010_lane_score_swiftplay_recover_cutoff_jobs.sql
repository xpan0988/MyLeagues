-- Recover only V5 derivations interrupted before migration 9 allowed its
-- explicit Swiftplay cutoff reason. Facts and all prior provenance stay intact.
UPDATE lane_derivation_queue
SET status = 'pending', attempts = 0, last_error = NULL, updated_at = CURRENT_TIMESTAMP
WHERE derivation_version = 'lane-derivation-v5-swiftplay-queue-policy'
  AND status = 'error'
  AND last_error LIKE '%cutoff_reason IS NULL OR cutoff_reason IN%';
