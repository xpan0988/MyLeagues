# LaneScore Architecture v0

**Status: ARCHITECTURE FROZEN FOR FACT-EXTRACTION IMPLEMENTATION**

This is the canonical design for future MyLeague top-lane evaluation work. It
freezes the ownership boundaries, model decomposition, correctness obligations,
and versioning rules before implementation. It does **not** implement LaneScore,
new Timeline ingestion, opponent mapping, CombatClusters, migrations, UI,
calibration, weights, or category thresholds.

## 1. Purpose and scope

LaneScore will answer two distinct questions for a reliably identified top-lane
pair:

- **Lane Advantage:** did player A establish an advantage over player B during
  lane phase?
- **Lane Crushing:** did player A establish an unusually large and sustained
  advantage?

They are not interchangeable. A lane can be won without being crushed.

The future model produces a continuous, signed result `S` in `[-1, 1]` from
the perspective of lane player A against lane player B. Positive means A had
the advantage; negative means B had it. Presentation may eventually map a
calibrated score to `CRUSHED`, `WON`, `EVEN`, `LOST`, or `GOT_CRUSHED`, but
those thresholds are intentionally not defined here.

This architecture excludes final win/loss, final KDA, final damage, vision,
bot-side objectives, post-lane teamfights, opponent rank, external ratings,
machine learning, and AI/LLM judgement as LaneScore inputs.

## 2. Current implementation versus future architecture

### Current implementation

MyLeague currently persists normalized Match-V5 facts for the configured player
and a compact Timeline V1 fact at nominal minute 10:

- lane minions, neutral minions, gold, XP, and level;
- the selected frame's actual timestamp;
- a persistent and resumable timeline enrichment queue;
- Champion Profile LANING @10 averages and coverage.

`match_laning_snapshots` is authoritative for those existing facts. `CS @10`
means lane-minion CS only; jungle monsters are separate. If an exact 600000 ms
frame is absent, the current enrichment selects the first later frame, so a
nominal `@10` fact can be slightly after 10:00.

Current MyLeague does **not** collect or retain all participants' role/team
facts, an opponent mapping, multi-frame opponent states, Timeline events,
plates, turret facts, objective facts, combat clusters, lane-phase boundaries,
or LaneScore. The current UI must not claim lane-win, crushed-lane, or
LaneScore results.

### Future architecture

The future implementation must add factual coverage before computing a score.
It must retain source facts and make every derived layer rebuildable:

```text
AUTHORITATIVE NORMALIZED FACTS
  participant roster and role fields
  Timeline participant frames
  Timeline kill/building/objective events
                 |
                 v
VERSIONED DERIVATIONS
  lane-opponent mapping, lane phase, event checkpoints
  lane-pair CombatClusters, pressure attribution
                 |
                 v
MODEL FEATURES AND DIMENSIONS
  EXP, Combat, Farm, Pressure, Objective Conversion
                 |
                 v
VERSIONED LANESCORE CACHE (rebuildable)
                 |
                 v
PRESENTATION
  score, category, coverage, confidence, explanation
```

Normalized facts are authoritative. A score cache must never become the only
stored representation of a match's lane evidence.

## 3. Frozen model hierarchy

```text
                         LaneScore
                             |
              +--------------+--------------+
              |                             |
          CORE EVIDENCE                MODIFIERS
              |                             |
       +------+------+               +------+------+
       |      |      |               |             |
      EXP   COMBAT  FARM          PRESSURE      CONVERSION
       |      |      |               |             |
      XP   LanePair  CS abs       Plates        Grubs
    Level  Clusters  CS relative  Top tower     Herald
       |
Raw superlinear level evidence
       |
Odd bounded saturation
```

The frozen conceptual composition is:

```text
Z_core = CoreComposition(EXP, Combat, Farm)
Z      = Z_core + PressureModifier + ObjectiveModifier
S      = OddBoundedTransform(Z)
```

`S` must be in `[-1, 1]`. The exact functions, coefficients, scales, and
thresholds are not frozen.

Core evidence is EXP, Combat, and Farm. Pressure and Objective Conversion are
secondary modifiers. Gold is a consistency/confidence signal, not a sixth
full-strength dimension.

## 4. EXP architecture

At state frame `t`, use signed lane-pair differences:

```text
DeltaXP(t)    = XP_A(t) - XP_B(t)
DeltaLevel(t) = Level_A(t) - Level_B(t)
```

XP is continuous evidence. Level is discrete threshold and persistence
evidence. Same-level but materially different within-level XP must remain
observable.

### 4.1 Raw superlinear level evidence

Raw level evidence is explicitly distinct from the normalized contribution:

```text
q(0) = 0
q(1) = 1
q(2) > 2
q(3) > 3
q(4) > 4
```

Equivalently, the eventual implementation must test:

```text
q(2) > 2 * q(1)
q(3) > 3 * q(1)
q(4) > 4 * q(1)
```

`q` is raw evidence and is deliberately not bounded to `[0, 1]`. A sustained
multi-level gap is more informative than repeated independent one-level gaps:
defensive play, jungle intervention, catch-up XP, roaming, and map movement
normally create opportunities for the gap to contract.

Possible families are a constrained monotone table, a power law, or a mild
exponential. A constrained table is preferred for eventual calibration because
its semantics and the required inequalities are directly inspectable. Any
table values, powers, exponents, cap, or scale are **INITIAL HYPOTHESES —
REQUIRES CALIBRATION**.

### 4.2 Bounded level contribution

Raw evidence passes through a bounded odd saturation stage:

```text
L(d) = sign(d) * sat(q(abs(d)) / lambda)
```

where `sat` is an odd monotone bounded function. This preserves odd symmetry,
monotonicity, raw superlinearity before saturation, and bounded output after
saturation. `lambda` and the saturation family are **INITIAL HYPOTHESES —
REQUIRES CALIBRATION**.

Raw superlinear evidence is therefore not the same thing as the normalized
level score.

### 4.3 XP complement and residual

XP must not blindly duplicate the level signal. The intended derivation is a
continuous XP residual relative to the observed level state and time:

```text
XPResidual(t) = DeltaXP(t)
                - ExpectedXPGap(DeltaLevel(t), t)
XPContribution(t) = XPResidualTransform(XPResidual(t), t)
EXP(t) = ExpComposition(LevelContribution(t), XPContribution(t))
```

`ExpectedXPGap` is a future calibrated, antisymmetric reference function, not
a fixed game constant. Thus two level-nine players with different progression
within level can still have distinct EXP evidence, while a multi-level gap is
not counted twice at full strength.

## 5. Lane-pair CombatCluster architecture

Combat is derived around the lane pair, not around unrelated local-player
rules. A cluster describes one continuous combat sequence between A, B, and
their reinforcements:

```text
LaneCombatCluster {
  match_id
  lane_pair: (A, B)
  start_ms, end_ms
  winning_side, losing_side, surviving_side
  lane_opponent_killed
  jungler_reinforcement_involved, jungler_reinforcement_killed
  mid_reinforcement_involved, mid_reinforcement_killed
  participants, victims, assists
  event_positions, region_confidence
  classification, derivation_confidence
  source_event_ids
}
```

Examples:

- A kills B: one `LANE_SOLO_KILL` for the pair, positive for A and negative
  for B.
- A kills B, then kills B's jungler in the same valid combat context: one
  `ANTI_GANK` or `REINFORCEMENT_REVERSAL`.
- A kills B, B's jungler, and B's mid in one valid continuous context: one
  stronger `REINFORCEMENT_TRIPLE` or
  `LARGE_REINFORCEMENT_REVERSAL`.

The same cluster may provide several explanation labels, but it receives one
atomic signed strength. It must never be scored as TOP kill + jungle kill +
mid kill + multi-kill bonus + anti-gank bonus.

Each source event belongs to at most one cluster. Distinct clusters can
accumulate through a bounded diminishing-return combiner. The clustering
window, map-region rule, and cluster-strength values are **INITIAL HYPOTHESES
— REQUIRES CALIBRATION**.

### 5.1 Context and anti-teamfight rules

A qualifying cluster requires lane-pair participation, lane-phase timing, and
supporting role/context evidence. Timeline event positions may strengthen a
top-lane or top-river classification but are event locations, not continuous
player tracking. A large Herald-area teamfight is not a lane reinforcement
reversal merely because both top laners participated.

The derivation must use time, role, position/context, participant set, and
objective context to classify uncertain sequences as ambiguous or non-lane
teamfights rather than fabricate precision.

### 5.2 Antisymmetry

Clusters are stored and evaluated for the pair. Reversing A and B reverses the
winning/losing side and the signed evidence:

```text
Combat(A, B) = -Combat(B, A)
```

when the same source facts are available.

## 6. Farm architecture

Farm uses lane minions only. Neutral/jungle minions are never silently merged
into CS.

```text
DeltaCS(t) = LaneCS_A(t) - LaneCS_B(t)
RelativeCS(t) = DeltaCS(t) / symmetric_lane_CS_baseline(t)
Farm(t) = FarmComposition(absolute DeltaCS, RelativeCS, their interaction)
```

The symmetric baseline must not change merely because the lane sides are
swapped. Small relative gaps at very low CS should remain weak; large absolute
gaps with modest relative gaps can be moderate; large absolute and relative
gaps together are strong before saturation.

An approximately 20% relative CS lead is an **INITIAL HYPOTHESIS — REQUIRES
CALIBRATION**, not a production breakpoint.

## 7. Lane Pressure architecture

Potential pressure facts are top-lane plate differential and top outer turret
state. They are separate from combat because a player can create substantial
structural lane pressure without kills.

Riot Timeline data may identify lane, team side, building, and event position,
but personal plate attribution is not always reliable. In particular, plate
events may lack a usable individual killer. V0 must therefore distinguish:

```text
TopLaneTeamPressure     factual team-side structural state
PersonalPlateCredit     unavailable unless the data establishes it
```

Team-side top pressure may be used conservatively with provenance and reduced
confidence. The UI must never claim personal plate counts without sufficient
attribution evidence. Pressure is a bounded secondary modifier; its cap is an
**INITIAL HYPOTHESIS — REQUIRES CALIBRATION**.

## 8. Objective Conversion architecture

Only top-side, lane-phase objectives are candidates: Void Grubs and Rift
Herald. They represent conversion of pre-existing lane priority, not the
primary definition of lane advantage.

For an objective event, choose the latest valid state frame **strictly before**
the event. Define non-negative quantities from each side's own prior state:

```text
LocalConversion(A) = Participation(A)
                     * PositivePreObjectivePriority(A)

OpponentConversion(B) = Participation(B)
                        * PositivePreObjectivePriority(B)

Objective(A, B) = LocalConversion(A) - OpponentConversion(B)
```

Consequently:

```text
Objective(A, B) = -Objective(B, A)
```

The modifier is small and bounded. It must not turn an otherwise even lane
into a crushed lane. Objective classification, participation semantics, and
modifier cap are **INITIAL HYPOTHESES — REQUIRES CALIBRATION**.

## 9. Gold consistency and confidence

Gold is a state verification, confidence, and missing-fact detector. It is not
a sixth full-strength score component in V0.

Future derivations may compare observed gold difference with a calibrated
explanation from EXP, Farm, Combat, and Pressure. Disagreement can emit an
explanation such as “gold lead is not explained by captured lane facts” and
lower confidence. It must not directly reduce a valid monotone LaneScore merely
because gold and currently captured evidence disagree.

## 10. Trajectory and checkpoints

Lane evaluation is a trajectory, not an endpoint. State dimensions are derived
at multiple retained participant frames through lane phase and integrated using
a deterministic weighted discrete integral. Actual frame timestamps are part of
the facts.

Standard diagnostics remain useful:

- @6, @8, @10, @12, and @14 when valid frames exist;
- the exact selected timestamp is always retained with the nominal label.

Event-relative checkpoints are also required:

- `PRE_GRUBS`
- `PRE_HERALD`
- `PRE_TOP_OUTER_TURRET`
- `LANE_PHASE_END`

Every pre-event checkpoint selects a frame strictly before its anchor event.
No post-event state may be used to establish pre-existing priority.

Later lane-phase frames may eventually receive different integration weight,
but the schedule is an **INITIAL HYPOTHESIS — REQUIRES CALIBRATION**.

## 11. Conservative V0 lane phase

V0 ends lane phase at the earliest of:

- top outer turret destruction;
- a calibrated global lane-phase cap;
- game end.

The cap is an **INITIAL HYPOTHESIS — REQUIRES CALIBRATION**. V0 does not infer
continuous roaming or lane abandonment because the required reliable movement
facts are not currently available. Rift Herald timing alone does not end lane
phase.

## 12. Lane opponent mapping and confidence

Opponent mapping is a versioned derivation, not an assumption that enemy TOP
is automatically the direct opponent. It uses full Match-V5 participant roster
facts, `teamPosition`, `individualPosition`, team identity, and—where useful—
supporting Timeline event context.

```text
HIGH        exactly one compatible TOP per side; strong role agreement
MEDIUM      exactly one plausible TOP per side; incomplete or mild conflict
LOW         partial/conflicting role evidence or unusual composition
UNAVAILABLE no unique reliable opponent
```

Only HIGH and, after future validation, possibly MEDIUM matches may enter
official LaneScore statistics. LOW and UNAVAILABLE produce no official score.
The final inclusion policy is not frozen.

## 13. Missing-data semantics and confidence

Missing does not mean neutral. The result must retain per-dimension availability
and coverage.

```text
CORE:       EXP, Combat, Farm
OPTIONAL:   Pressure, Objective Conversion
```

A missing optional modifier is unavailable, not zero evidence. It must not
cause core weights to be renormalized or become stronger merely because a
modifier is absent:

```text
Z = Z_core + PressureModifier + ObjectiveModifier
```

If Pressure is unavailable, omit its modifier and lower coverage/confidence as
appropriate; do not convert it to zero and do not alter `Z_core`'s scale.

A missing core dimension is more serious. Whether all three core dimensions
are strictly required for a final score remains a calibration/validation
decision. Until that policy is frozen, a result contract must be able to return
`insufficient_evidence` rather than manufacture a score.

## 14. Final composition and explainability contract

The exact mathematical form is intentionally unfrozen, but the architecture
requires an antisymmetric, monotone, odd bounded final transform:

```text
Z_core = CoreComposition(EXP, Combat, Farm)
Z      = Z_core + bounded PressureModifier + bounded ObjectiveModifier
S      = OddBoundedTransform(Z)
```

An eventual result is conceptually:

```text
LaneScoreResult {
  score | unavailable
  category | unavailable
  status: ready | backfilling | insufficient_evidence | unsupported
  confidence: high | medium | low | unavailable
  coverage by facts and dimensions
  model_version, feature_schema_version, derivation_version, ruleset_version
  evidence lines linked to factual sources and checkpoints
  gold consistency state
}
```

Presentation must explain dimensions and facts, not merely show a scalar. A
future explanation can name sustained level/XP state, lane CS, an atomic
anti-gank cluster, team-side structural pressure, and pre-objective conversion
with their confidence and provenance.

## 15. Formal invariants and proof obligations

The eventual implementation must prove or test:

1. **Boundedness:** `-1 <= S <= 1`.
2. **Antisymmetry:** `S(A, B) = -S(B, A)` when symmetric evidence is
   available.
3. **Neutrality:** equivalent lane facts yield `S = 0`.
4. **Monotonicity:** holding all other valid evidence fixed, improving a signed
   advantage cannot lower the score.
5. **Raw level superlinearity:** `q(1)=1`, `q(2)>2`, `q(3)>3`, and `q(4)>4`
   before saturation.
6. **Saturation:** extreme evidence cannot make a dimension or final score
   unbounded.
7. **Determinism:** identical normalized facts, derivation versions, and model
   manifest produce exactly the same result.
8. **No future leakage:** a state at time `t` uses only allowed facts at or
   before `t`; pre-event checkpoints use facts strictly before the event.
9. **Anti-double-counting:** one source event or causal cluster cannot receive
   multiple full-strength rewards.
10. **Missing-data safety:** missing facts remain explicit and optional absence
    never inflates core weights.
11. **Reproducibility:** historical scores can be reconstructed from facts,
    derivation versions, model manifest, parameter hash, and ruleset version.

Proof sketches follow directly from the architecture: use an odd bounded final
transform; make every signed lane-pair feature antisymmetric; use positive,
monotone component transforms; retain source event identity and assign each
event to at most one cluster; and make all time selection deterministic.

## 16. Formal correctness versus semantic validity

Formal correctness can establish the invariants above. It cannot prove that a
particular score objectively means a lane was crushed. That is semantic
validity, requiring blind human labels and held-out empirical validation.

## 17. Calibration and validation architecture

The future workflow is:

```text
facts
  -> blind human labels
  -> calibration set
  -> validation set
  -> untouched test set
  -> constrained parameter search
  -> immutable model manifest
  -> final evaluation
```

Labels are ordinal:

```text
+2  CRUSHED
+1  WON
 0  EVEN
-1  LOST
-2  GOT CRUSHED
```

Required evaluation includes a 5x5 confusion matrix, exact accuracy,
adjacent-category accuracy, mean absolute ordinal error, squared ordinal error,
signed bias, and metrics by confidence/coverage tier. Parameters and category
thresholds are not selected in this document.

## 18. Patch/ruleset-aware versioning and reproducibility

Model version alone is insufficient because XP, minion economy, plates,
objectives, and top-lane systems can change. Every immutable model manifest
must conceptually include:

```text
model_version
feature_schema_version
derivation_version
ruleset_version
valid_patch_from
valid_patch_to
parameter_hash
calibration_dataset_id
created_at
```

A patch needs only metadata coverage when it does not materially alter a fact's
meaning, derivation, or calibrated relationship. It requires recalibration when
mechanics materially affect a calibrated transform or distribution. It requires
a new model version when model structure, feature meaning, derivation contract,
or parameter set changes.

Historical results retain their manifest and ruleset identity. Cross-ruleset
aggregates must either group by compatible model/ruleset or label the mixture;
they must never silently present incomparable scores as one homogeneous metric.

## 19. Proposed future fact schema

Future schema work must be driven by this document, but is deliberately not
implemented here. The smallest normalized additions are expected to include:

- full `match_participants` roster facts: participant, team, champion, and role
  fields;
- generalized Timeline participant snapshots for both lane sides and multiple
  timestamps, replacing indefinite duplication of the V1 local @10 projection;
- selected normalized Timeline events plus event-participant relations;
- versioned lane-opponent mapping facts;
- versioned lane-phase and checkpoint derivations;
- rebuildable CombatCluster and score caches, each linked to source facts and
  derivation/model versions.

Facts remain durable. Clusters, checkpoints, model features, and scores are
rebuildable. Historical archive enrichment will require versioned persistent
backfill work because current V1 completion only means V1 @10 facts were
captured, not that all future lane facts exist.

## 20. Known Riot-data limitations

The implementation must verify current real payloads before relying on event
fields. Known design limitations include possible incomplete personal plate
attribution, event positions that are not continuous player locations, role
ambiguity in unusual games, potentially absent frames, and patch-dependent
objective semantics. These limitations lower confidence or make a match
unscorable; they must not be hidden with inferred neutral values.

## 21. Implementation phases

The next implementation phase is factual coverage, not scoring:

1. Persist full participant roster/team/role facts and backfill them.
2. Persist multi-frame lane-pair snapshots and selected Timeline events.
3. Implement versioned opponent mapping, lane phase, and checkpoints with
   fixtures and coverage reporting.
4. Implement rebuildable lane-pair CombatClusters and pressure/objective facts.
5. Build a blind-label calibration dataset and validation harness.
6. Only then implement and freeze a calibrated model manifest and score cache.

## 22. Explicitly unfrozen parameters

The following remain unfrozen and every numeric proposal for them is an
**INITIAL HYPOTHESIS — REQUIRES CALIBRATION**:

- raw level table values, exponents, and saturation constants;
- XP residual and normalization parameters;
- core composition weights and final saturation shape;
- CombatCluster time window, map-region boundaries, and cluster strengths;
- CS absolute/relative scales and breakpoints;
- lane-phase cap and trajectory weights;
- pressure and objective modifier caps;
- high-versus-medium confidence inclusion policy;
- category thresholds and final calibrated parameter values.

## 23. Architecture decision summary

Frozen decisions are:

- facts, derivations, model, score cache, and presentation have distinct
  ownership;
- normalized facts are authoritative and scores are rebuildable;
- level evidence is raw-superlinear before separate bounded saturation;
- combat is lane-pair-centric, atomic, and antisymmetric;
- objective conversion is explicitly antisymmetric and pre-event only;
- EXP, Combat, and Farm are core; Pressure and Conversion are bounded optional
  modifiers without missing-data weight renormalization;
- Gold is consistency/confidence only;
- trajectory and actual frame timestamps matter;
- opponent ambiguity yields unavailable rather than fake precision;
- all scores are patch/ruleset-aware, versioned, and reproducible;
- calibration determines semantics and categories after—not before—fact
  extraction and validation.
