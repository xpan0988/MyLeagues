# LaneScore Architecture v0

**Status: ARCHITECTURE FROZEN WITH APPROVED PRODUCT/ELIGIBILITY AMENDMENT**

This is the canonical design for MyLeague top-lane evaluation work. It freezes
the ownership boundaries, model decomposition, correctness obligations,
eligibility rules, product semantics, and versioning rules. The normalized fact
pipeline and experimental per-match score are implemented; calibration and
category thresholds remain deliberately unfinished.

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

## 2. Current implementation and ownership

### Current implementation

MyLeague persists normalized Match-V5 facts for the configured player and the
legacy compact Timeline V1 fact at nominal minute 10:

- lane minions, neutral minions, gold, XP, and level;
- the selected frame's actual timestamp;
- a persistent and resumable timeline enrichment queue;
- Champion Profile LANING @10 averages and coverage.

`match_laning_snapshots` is authoritative for those existing facts. `CS @10`
means lane-minion CS only; jungle monsters are separate. If an exact 600000 ms
frame is absent, the current enrichment selects the first later frame, so a
nominal `@10` fact can be slightly after 10:00.

The LaneScore fact revision additionally retains all participants' role/team
facts, multi-frame states, selected normalized Timeline events, versioned
opponent mappings, cutoff/checkpoint derivations, CombatClusters, and a
versioned rebuildable experimental score cache. Category labels and calibrated
lane-win/crush statistics are not implemented.

### Implemented architecture boundary

The implementation retains source facts and makes every derived layer
rebuildable:

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

### 5.3 Direct kill participant attribution

Within an otherwise qualifying atomic cluster, direct lane-pair Combat credit
comes only from the normalized kill contributors: the valid killer plus unique
valid assists, excluding the victim. If the lane opponent is a contributor to
the local player's death, their direct share is `1 / contributor_count`; if
they are absent, their direct lane-pair share is zero. The same share is
applied with the opposite sign for the other lane perspective, preserving exact
antisymmetry. This does not discard the downstream EXP, Farm, or Pressure
consequences of a gank, and it does not weaken a valid solo reinforcement
reversal merely because multiple enemies were defeated.

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

### 8.1 Swiftplay objective policy

Swiftplay is a supported Summoner's Rift queue in the same Career and Champion
TOP population, but neutral-objective conversion is deliberately outside its
LaneScore contract. Its result is explicit rather than inferred from absent
facts:

```text
ObjectiveConversionStatus::NotApplicableByQueue
ObjectiveModifier = 0
Z = Z_core + PressureModifier + 0
```

Void Grubs, Rift Herald, and other neutral-objective conversion evidence do
not contribute for Swiftplay. This is neither `Missing`, `Unavailable`, nor
`ObservedZero`; it must not reduce coverage, renormalize EXP/Combat/Farm
weights, or make Pressure stronger. Valid lane-pair `CHAMPION_KILL` evidence
near a neutral objective remains eligible for its independently satisfied
CombatCluster rule.

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

`@10` is a standardized factual benchmark only. It does not define lane end,
and it does not truncate the trajectory consumed by LaneScore.

Event-relative checkpoints are also required:

- `PRE_GRUBS`
- `PRE_HERALD`
- `PRE_TOP_OUTER_TURRET`
- `LANE_PHASE_END`

Every pre-event checkpoint selects a frame strictly before its anchor event.
No post-event state may be used to establish pre-existing priority.

Later lane-phase frames may eventually receive different integration weight,
but the schedule is an **INITIAL HYPOTHESIS — REQUIRES CALIBRATION**.

## 11. Lane-analysis cutoff

For an otherwise eligible traditional supported match, the cutoff is
ruleset-aware:

```text
candidate = first valid Rift Herald kill timestamp, if present
          | 14:00 fallback, otherwise
lane_cutoff = min(candidate, ruleset_lane_cap)
```

The current experimental 16.x ruleset uses a centralized 17:00 maximum cap.
That cap is **INITIAL_HYPOTHESIS_REQUIRES_CALIBRATION**. The 14:00 fallback and
Herald-anchor behavior are frozen product semantics; neither value may be
scattered through services or UI code.

When Herald is the anchor, final state and `PRE_HERALD` use the latest paired
participant frame strictly before the event. A frame at or after Herald death
cannot establish pre-existing priority. CombatCluster and Objective Conversion
may still use qualifying Herald-fight events under their existing rules. If a
Herald kill is later than the cap, the cap wins and its exact frame may be used;
if no Herald is killed, analysis stops at 14:00 rather than waiting indefinitely.
Top outer turret facts remain Pressure evidence and an event-relative
checkpoint; they no longer define the lane cutoff.

Swiftplay has a queue-specific derivation policy instead:

```text
Swiftplay lane_cutoff = 14:00
```

It still requires a game duration of at least 14:00, but never uses Rift
Herald death, `PRE_HERALD`, or a Herald-derived extension to choose its lane
window. This does not disable valid lane CombatClusters or top-lane structural
Pressure before 14:00.

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

Only HIGH-confidence mappings enter LaneScore V0 statistics. MEDIUM, LOW, and
UNAVAILABLE remain diagnostic-only and produce an explicit
`OPPONENT_UNAVAILABLE` exclusion rather than a guessed score.

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
Swiftplay's queue-policy Objective Conversion N/A is the explicit exception:
it contributes policy zero without reducing coverage or altering `Z_core`.

A missing core dimension is more serious. Whether all three core dimensions
are strictly required for a final score remains a calibration/validation
decision. Until that policy is frozen, a result contract must be able to return
`insufficient_evidence` rather than manufacture a score.

### 13.1 Match eligibility and exclusion

LaneScore V0 statistics contain only score-ready supported Summoner's Rift TOP
matches under a supported ruleset, with HIGH opponent confidence and complete
required core facts. The supported Career population is Ranked Solo,
traditional Normal/Draft, historically supported Blind Pick, Quickplay, and
Swiftplay. Swiftplay is combined with the other supported queues; it has no
separate user-facing LaneScore tier, Career statistic, or Champion statistic.
Matches ending before the 14:00 fallback horizon are unavailable
and excluded from aggregate denominators. This conservatively excludes remakes,
abnormal early surrender, and other very short terminations. Unsupported
queues/modes, unsupported roles/rulesets, missing opponents, and incomplete
facts are also excluded.

Exclusion is explicit and queryable through reason codes such as
`UNSUPPORTED_QUEUE`, `UNSUPPORTED_ROLE`, `GAME_TOO_SHORT`,
`OPPONENT_UNAVAILABLE`, `FACTS_INCOMPLETE`, and `RULESET_UNSUPPORTED`.
Excluded or missing scores are never treated as `EVEN`.

A normal surrender after the 14:00 horizon is not automatically excluded. If
the match reached a valid cutoff and all other facts are complete, it may be
scored regardless of the later team result. Final win/loss remains outside the
model.

## 14. Final composition and explainability contract

The exact mathematical form is intentionally unfrozen, but the architecture
requires an antisymmetric, monotone, odd bounded final transform:

```text
Z_core = CoreComposition(EXP, Combat, Farm)
Z      = Z_core + bounded PressureModifier + bounded ObjectiveModifier
S      = OddBoundedTransform(Z)
```

The implemented result contract follows this shape:

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

Presentation must explain dimensions and facts, not merely show a scalar. The
expanded diagnostic can name sustained level/XP state, lane CS, an atomic
anti-gank cluster, team-side structural pressure, and pre-objective conversion
with their confidence and provenance.

### 14.1 Product presentation

The internal score remains `S in [-1, 1]`. Product surfaces display the signed
percentage `S * 100`, for example `Lane Score +78%` or `Lane Score -41%`.
This is signed lane dominance, not probability, percentile, win chance, or
confidence; `(S + 1) / 2` is forbidden.

Career and Champion Profile consume backend aggregates over persisted scores
matching one exact model/feature/derivation/ruleset/parameter identity. Their
rate denominator is score-ready eligible TOP matches, never all tracked games.
Until category thresholds exist, Average Lane Score, scored/excluded coverage,
and model identity are shown while category-derived rates/counts remain
unavailable. Champion Profile retains the separate factual `LANING @10`
section.

Matches may show opponent champion and signed LaneScore in the collapsed row,
then checkpoints, cutoff reason, atomic combat, pressure/objective evidence,
dimensions, Gold consistency, coverage, and version identity in expanded
detail. Opponent Riot ID is shown only when already present in authoritative
stored facts; presentation must not trigger an extra Riot request.

Swiftplay uses the same normal match presentation (`vs opponent`, `Lane Score
+XX%`). Expanded diagnostics may state `Objective Conversion: Not applicable
for Swiftplay`; this is not an error or incomplete coverage.

## 15. Formal invariants and proof obligations

The implementation must prove or test:

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

Career and Champion LANING PERFORMANCE use an explicit compatibility set, not
only the newest ruleset. The implemented historical product window begins at
the earliest tracked compatible traditional Summoner's Rift match and currently
contains:

- `riot-2024-late-sr-lane-v0` for raw Match-V5 `14.22`–`14.23`;
- `riot-2025-s1-sr-lane-v0` for raw `15.4`–`15.8`;
- `riot-2025-s2-sr-lane-v0` for raw `15.9`–`15.23`;
- `riot-2026-sr-lane-v0` for raw `16.1` and later ordinary `16.x` minors while
  that semantic ruleset is active (the tracked archive begins at `16.4`). A
  mechanic change closes this open range and adds a newer explicit ruleset;
  raw Match-V5 provenance remains unchanged.

Raw Match-V5 build families are not Riot's public year-prefixed patch names:
for example raw `16.x` corresponds to public 2026 `26.x` patches. Each score
retains its exact ruleset version, while the aggregate also exposes the bounded
compatibility-set identity and member rulesets.

Traditional Draft, Ranked Solo, historically supported Blind Pick, Quickplay,
and Swiftplay queue 480 are compatible for this historical V0 scope. Swiftplay
is deliberately included in the same supported Summoner's Rift Career and
Champion TOP population, not silently treated as missing enrichment and not
exposed as a separate user-facing tier. Its queue-specific derivation contract
uses the same core evidence and structural Pressure, a fixed 14:00 cutoff, and
`ObjectiveConversionStatus::NotApplicableByQueue` with a policy-zero modifier.

The historical manifests share the experimental LaneScore mathematics and
parameter hash, but record mechanic differences. Late 2024 and early 2025 have
up to two Void Grub encounters; public patch 25.09 moves to one encounter and a
15:00 Herald spawn. The 2026 ruleset keeps the one-encounter objective shape and
introduces permanent plates on all lane turrets plus top-lane role-quest XP.
These differences require distribution review before any calibration claim;
this compatibility set enables descriptive experimental coverage only.

## 19. Implemented fact and derivation schema

The implemented normalized additions include:

- full `match_participants` roster facts: participant, team, champion, and role
  fields;
- generalized Timeline participant snapshots for both lane sides and multiple
  timestamps, alongside the unchanged V1 local @10 projection;
- selected normalized Timeline events plus event-participant relations;
- versioned lane-opponent mapping facts;
- versioned lane-cutoff and checkpoint derivations;
- rebuildable CombatCluster and score caches, each linked to source facts and
  derivation/model versions.

Facts remain durable. Clusters, checkpoints, model features, and scores are
rebuildable. Historical archive enrichment uses a persistent fact queue
independent from V1 and a separate derivation queue. A derivation-version change
reuses complete normalized facts locally and refetches Riot payloads only when
authoritative facts are missing.

## 20. Known Riot-data limitations

The implementation must verify current real payloads before relying on event
fields. Known design limitations include possible incomplete personal plate
attribution, event positions that are not continuous player locations, role
ambiguity in unusual games, potentially absent frames, and patch-dependent
objective semantics. These limitations lower confidence or make a match
unscorable; they must not be hidden with inferred neutral values.

## 21. Implementation status

Participant rosters, multi-frame states, selected events, opponent mapping,
cutoffs/checkpoints, CombatClusters, dimensions, eligibility, experimental score
cache, local re-derivation, and product diagnostics are implemented. Real-data
coverage still depends on the persistent Riot fact backfill. Blind-label
calibration, validated category thresholds, and calibrated aggregate category
rates remain future work.

## 22. Explicitly unfrozen parameters

The following remain unfrozen and every numeric proposal for them is an
**INITIAL HYPOTHESIS — REQUIRES CALIBRATION**:

- raw level table values, exponents, and saturation constants;
- XP residual and normalization parameters;
- core composition weights and final saturation shape;
- CombatCluster time window, map-region boundaries, and cluster strengths;
- CS absolute/relative scales and breakpoints;
- the 17:00 ruleset maximum cap and trajectory weights;
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
- traditional objective conversion is explicitly antisymmetric and pre-event
  only; Swiftplay objective conversion is explicit queue-policy N/A/zero;
- EXP, Combat, and Farm are core; Pressure and Conversion are bounded optional
  modifiers without missing-data weight renormalization;
- Gold is consistency/confidence only;
- trajectory and actual frame timestamps matter;
- opponent ambiguity yields unavailable rather than fake precision;
- all scores are patch/ruleset-aware, versioned, and reproducible;
- @10 remains a benchmark while traditional Herald/fallback/cap or the
  Swiftplay fixed-14:00 policy resolves lane cutoff;
- traditional pre-Herald state is strictly before the Herald event; Swiftplay
  has no Herald-derived lane-end state;
- short/remake-like games are excluded while normal post-horizon surrenders may
  remain eligible;
- product display uses signed `S * 100`, never probability semantics;
- calibration determines semantics and categories after—not before—fact
  extraction and validation.
