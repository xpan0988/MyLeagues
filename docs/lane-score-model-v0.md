# LaneScore V0 model manifest

`lane-score-v0-experimental` is an **EXPERIMENTAL / INITIAL HYPOTHESIS**.
It is executable to exercise the fact and derivation pipeline, but it is not
calibrated and has no validated score categories or aggregate lane statistics.

All empirical values live in `ExperimentalManifest::initial()` in
`src-tauri/src/domain/lane_score.rs`; its canonical, explicitly field-ordered
parameter serialization is hashed with FNV-1a and stored with each cached
result. The hash is a deterministic historical model identity, not a security
primitive. The initial values cover the level
table and saturation, XP residual, EXP/Farm/Core weights, CS scales, combat
window and strengths, map/teamfight boundaries, ruleset lane cap, and modifier caps.
They must only change through a new immutable manifest identity.

The current manifest uses `lane-derivation-v6-combat-contributor-sharing` across the
explicit `riot-sr-2024-late-through-2026-compatible-v0` set. Its members are
`riot-2024-late-sr-lane-v0`, `riot-2025-s1-sr-lane-v0`,
`riot-2025-s2-sr-lane-v0`, and `riot-2026-sr-lane-v0`. Raw Match-V5 versions
`14.x`, `15.x`, and `16.x` are stored separately from those semantic ruleset
identities; raw `16.x` is Riot's public 2026 `26.x` patch line.

Traditional supported Summoner's Rift queues use the approved 14:00 no-Herald
fallback and centralized 17:00 maximum cap. Swiftplay (queue 480) instead uses
a fixed 14:00 lane cutoff: Herald never shortens or extends its lane window,
and no `PRE_HERALD` state is used for its LaneScore derivation. The maximum cap is
`INITIAL_HYPOTHESIS_REQUIRES_CALIBRATION`; it is not evidence of empirical
validation. The rulesets separately identify one-versus-two Void Grub encounter
semantics and pre-2026 expiring versus 2026 permanent turret plates. Cached
scores from older derivation identities do not enter current summary queries.

Career and Champion historical summaries require the exact model, feature
schema, derivation, and parameter hash plus one of the explicit compatible
ruleset identities. Per-match provenance is never replaced by the aggregate-set
identity. Ranked Solo, traditional Normal/Draft, historically supported Blind
Pick, Quickplay, and Swiftplay queue 480 share the one supported Summoner's
Rift TOP population. Swiftplay is not a separate Career, Champion, or match
presentation tier.

Swiftplay retains EXP, Combat, Farm, and valid structural Pressure evidence
under the same `S in [-1, 1]` model and unchanged parameter hash. Objective
Conversion is explicitly `NotApplicableByQueue`: Void Grubs, Herald, and other
neutral-objective conversion evidence contribute exactly zero by policy. This
is neither missing, unavailable, nor observed zero; it does not lower
coverage, renormalize the core weights, or strengthen Pressure.

The manifest records an explicit, unvalidated patch window in
`lane_score_model_manifests` rather than implying cross-patch calibration.
Before it is used for categories or aggregate claims, it requires the blind
label calibration and ruleset compatibility workflow specified by
`docs/lane-score-architecture.md`.

Combat uses an explicit, deterministic equal-share attribution policy for each
normalized `CHAMPION_KILL`: valid killer plus unique valid assists, excluding
the victim. A lane-pair participant receives `1 / contributor_count` only when
they are a contributor to the opposing lane player's death; the opposing
perspective receives the exact negative value. A solo lane kill therefore
remains stronger direct evidence than a two- or three-player gank. This applies
only to the direct Combat evidence: it neither changes the atomic
CombatCluster taxonomy nor suppresses resulting EXP, Farm, or Pressure facts.
Reinforcement reversals retain their existing upgraded cluster semantics when
the lane player independently defeats the opponent and reinforcements.

Live normalized Match-V5 Timeline evidence uses `monsterType = "HORDE"` for
Void Grub kills across the sampled raw `14.x`, `15.x`, and `16.x` families.
The derivation recognizes that Riot value explicitly (alongside conservative
legacy aliases); Rift Herald remains `RIFTHERALD`. This event-shape correction
is part of the derivation identity above, not a scoring-parameter change.

Presentation multiplies the internal signed score by 100. `+0.78` is displayed
as `Lane Score +78%`; this is signed lane dominance, not a probability,
percentile, confidence value, or `(S + 1) / 2`. Category thresholds remain
unavailable, so Lane Advantage Rate, Crush Rate, and category counts are not
fabricated.
