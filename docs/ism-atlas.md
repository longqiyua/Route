# ISM Atlas — Internal Cognitive Atlas (Boom-native)

**Status: SPECIFIED / self-contained.** This is the Boom-native cognitive
atlas for the ISM (§32 of [boom.md](boom.md)). It is fully self-contained:
it carries the axis semantics, the level archetypes, the deterministic
composition rule, and the 256 `[F,O,K,D]` StandardPortraits. It is **not**
derived from, and does not reference, any external doctrine catalog, JSON
file, video, author, website, or source metadata. Removing any external design
reference does not affect this atlas.

Atlas rule: `F,O,K,D ∈ {1,2,3,4}` → 256 coordinates. Each portrait is
produced by the PortraitComposer below from the four LevelArchetypes, then
distilled to a unique structural stance. No runtime calls a model to
regenerate portraits; the frozen set below is read directly.

## Level archetypes (the grammar)

| Level | F / FRAME | O / OBJECT | K / KNOWING | D / DIRECTION |
|---|---|---|---|---|
| 1 CLOSURE | world has stable rules; anomaly is secondary | objects bounded, fixed components | knowing = correct mapping to a given order | maintain/optimize order; cycle |
| 2 SPLIT | world is bounded/conflicted | objects defined by opposition/edges | knowing = splitting to see structure | drive a side; draw a line |
| 3 MEDIATION | a center/mechanism integrates | identity via a mediating whole | knowing = the unifying third term | converge/centralize to synthesis |
| 4 OPENING | surplus/break/unknown resist closure | objects not closable; residual escapes | knowing = keep open undecided | open beyond; failure as entry |

## PortraitComposer rule

For coordinate `(f,o,k,d)`, compose the portrait by intersecting the F-level's
world-assumption, the O-level's object-pattern, the K-level's knowing-pattern,
and the D-level's direction-pattern. `core_pattern` is a single sentence
synthesizing the four; `strengths`/`blindspots`/`failure_mode`/`questions`/
`notices`/`misses` are drawn from the dominant tensions (where a level is `1`
it contributes stability; `2` contributes conflict; `3` mediation; `4`
opening). `opposite_or_tension_coordinates` are the coordinates obtained by
pushing each axis toward its tension counterpart (`1↔4`, `2↔3` best-effort,
with `1↔2` and `3↔4` as secondary tensions). `opening_hook` is the single
most-likely crack for discovering an unknown-unknown.

## Standard portraits (256)

### Coordinate 1111 — The Ordered Custodian
**core_pattern:** A stable world of fixed components, correctly mapped, best
served by maintaining the existing order.</br>
**world_assumption:** rules precede anomaly; the world is dependable.</br>
**object_pattern:** objects are bounded, fixed components.</br>
**knowing_pattern:** knowing is correct mapping to a given order.</br>
**direction_pattern:** maintain and optimize order; cycle.</br>
**typical_reasoning:** fit new facts to the established frame; preserve invariants.</br>
**strengths:** stability, execution, compression, reliability.</br>
**blindspots:** anomaly, external residue, the unclassifiable.</br>
**failure_mode:** over-consolidation; rejects evidence that breaks the frame.</br>
**questions_it_tends_to_ask:** How does this fit the existing rules? What invariant is being preserved?</br>
**what_it_notices:** deviations from the norm; missing invariants.</br>
**what_it_misses:** the possibility that the frame itself is wrong.</br>
**opposite_or_tension_coordinates:** 4444, 2222, 3333.</br>
**opening_hook:** treat the first anomaly as a possible new frame, not a defect.

### Coordinate 1112 — The Boundary-Keeping Splitter
**core_pattern:** A stable world of fixed components, correctly mapped, but motion is governed by drawing lines and driving a side.</br>
**world_assumption:** rules precede anomaly; edges are real.</br>
**object_pattern:** fixed components that face an opposing side.</br>
**knowing_pattern:** correct mapping to a given order.</br>
**direction_pattern:** drive a side; resolve by drawing a line.</br>
**typical_reasoning:** classify each component as in/out of the order; enforce the boundary.</br>
**strengths:** decisive, protective, clear ownership.</br>
**blindspots:** the gray zone; the side that is not really separate.</br>
**failure_mode:** polarization; treats a spectrum as a fence.</br>
**questions_it_tends_to_ask:** Which side is this on? Where is the boundary?</br>
**what_it_notices:** defectors, boundary violations.</br>
**what_it_misses:** that the boundary may be the artifact.</br>
**opposite_or_tension_coordinates:** 4443, 2221, 3333.</br>
**opening_hook:** ask what the line itself is made of.

### Coordinate 1113 — The Centralizing Integrator
**core_pattern:** A stable world of fixed components, correctly mapped, organized by a unifying center.</br>
**world_assumption:** rules precede anomaly; a center integrates.</br>
**object_pattern:** fixed components gain identity through a mediating whole.</br>
**knowing_pattern:** correct mapping to a given order.</br>
**direction_pattern:** converge and centralize toward a synthesis.</br>
**typical_reasoning:** find the higher rule that reconciles the components.</br>
**strengths:** synthesis, coordination, architecture.</br>
**blindspots:** the ungovernable remainder; the center's own bias.</br>
**failure_mode:** premature unity; overwrites genuine conflict.</br>
**questions_it_tends_to_ask:** What higher principle reconciles these?</br>
**what_it_notices:** fragmented parts needing a center.</br>
**what_it_misses:** the part that refuses the center.</br>
**opposite_or_tension_coordinates:** 4442, 2221, 3333.</br>
**opening_hook:** ask what the center cannot absorb.

### Coordinate 1114 — The Stable Opener
**core_pattern:** A stable rule-world of fixed components, correctly mapped, yet direction is to open beyond the established order.</br>
**world_assumption:** rules precede anomaly, but there is a beyond.</br>
**object_pattern:** fixed components whose closedness is a starting point.</br>
**knowing_pattern:** correct mapping, with a willingness to un-map.</br>
**direction_pattern:** open beyond the structure; failure is an entry.</br>
**typical_reasoning:** stabilize a base, then deliberately breach it.</br>
**strengths:** controlled exploration; safe unlearning.</br>
**blindspots:** the residue of the very structure it keeps.</br>
**failure_mode:** never fully commits to the opening.</br>
**questions_it_tends_to_ask:** What could the current order not express?</br>
**what_it_notices:** the edges past the mapped territory.</br>
**what_it_misses:** that the desire to open may itself be a closure.</br>
**opposite_or_tension_coordinates:** 4441, 2223, 3333.</br>
**opening_hook:** open the frame only where a concrete anomaly lives.

### Coordinate 1121 — The Framer of Fixed Sides
**core_pattern:** A stable world of fixed opposed objects, split to see structure, conserving order.</br>
**world_assumption:** rules precede anomaly; objects are opposed.</br>
**object_pattern:** objects defined by opposition and edges.</br>
**knowing_pattern:** splitting precisely to see true structure.</br>
**direction_pattern:** maintain and optimize this mapped order.</br>
**typical_reasoning:** name the two sides, then keep the mapping stable.</br>
**strengths:** crisp taxonomy, clean interfaces.</br>
**blindspots:** the part that is neither side.</br>
**failure_mode:** treats a dyad as the whole.</br>
**questions_it_tends_to_ask:** What is the opposition here?</br>
**what_it_notices:** polarity, complementarity.</br>
**what_it_misses:** the excluded middle.</br>
**opposite_or_tension_coordinates:** 4444, 2231, 3333.</br>
**opening_hook:** ask what falls between the two named sides.

### Coordinate 1122 — The Polar Splitter
**core_pattern:** A stable world split into two fixed sides, known by splitting, moved by fighting one side.</br>
**world_assumption:** rules precede anomaly; conflict is real.</br>
**object_pattern:** fixed opposing objects.</br>
**knowing_pattern:** splitting to see true structure.</br>
**direction_pattern:** drive a side; resolve by drawing a line.</br>
**typical_reasoning:** reduce every question to a decisive opposition.</br>
**strengths:** clarity under conflict, adversarial rigor.</br>
**blindspots:** the third option; the middle ground.</br>
**failure_mode:** forced dichotomy.</br>
**questions_it_tends_to_ask:** Which do we reject to keep the other?</br>
**what_it_notices:** contradiction, enemy, rival.</br>
**what_it_misses:** synthesis outside the dyad.</br>
**opposite_or_tension_coordinates:** 4444, 2231, 3333.</br>
**opening_hook:** look for the option that is not one of the two.

### Coordinate 1123 — The Dyad-Restructuring Centrist
**core_pattern:** A stable world of two opposed fixed objects, known by splitting, moved toward a unifying third term.</br>
**world_assumption:** rules precede anomaly; two sides exist.</br>
**object_pattern:** fixed opposed objects seeking a whole.</br>
**knowing_pattern:** split first, then find the third term.</br>
**direction_pattern:** converge via a mediating synthesis.</br>
**typical_reasoning:** turn the opposition into a higher integration.</br>
**strengths:** dialectical design, conflict resolution.</br>
**blindspots:** the third term can crush the real difference.</br>
**failure_mode:** reconciliation that hides the conflict.</br>
**questions_it_tends_to_ask:** What third term dissolves this opposition?</br>
**what_it_notices:** thesis and antithesis.</br>
**what_it_misses:** the residue both sides share.</br>
**opposite_or_tension_coordinates:** 4444, 2241, 3333.</br>
**opening_hook:** ask what both sides are jointly concealing.

### Coordinate 1124 — The Open Dyad Scanner
**core_pattern:** A stable world of two fixed opposed objects, split for structure, yet opened to find what the dyad cannot hold.</br>
**world_assumption:** rules precede anomaly; the dyad is incomplete.</br>
**object_pattern:** fixed opposed objects whose closure is suspect.</br>
**knowing_pattern:** split to structure, then open the split.</br>
**direction_pattern:** open beyond the structure; seek the leftover.</br>
**typical_reasoning:** use the two sides as a probe for the excluded remainder.</br>
**strengths:** finds the middle the dyad hides.</br>
**blindspots:** the dyad itself may be a lazy construction.</br>
**failure_mode:** opening for opening's sake.</br>
**questions_it_tends_to_ask:** What do both sides fail to name?</br>
**what_it_notices:** the gap between polarized terms.</br>
**what_it_misses:** the value of a genuinely stable dyad.</br>
**opposite_or_tension_coordinates:** 4441, 2231, 3333.</br>
**opening_hook:** search the space the two sides jointly exclude.

### Coordinate 1131 — The Rule-Mediating Stabilizer
**core_pattern:** A stable world of components integrated by a center, known by mapping, maintained as order.</br>
**world_assumption:** rules precede anomaly; a center unites.</br>
**object_pattern:** components gain identity via a mediating whole.</br>
**knowing_pattern:** correct mapping to a given order.</br>
**direction_pattern:** maintain and optimize the integrated order.</br>
**typical_reasoning:** treat the system as a whole with a governing center.</br>
**strengths:** holistic stability, governance.</br>
**blindspots:** the periphery the center neglects.</br>
**failure_mode:** centralization becomes rigidity.</br>
**questions_it_tends_to_ask:** What keeps the whole coherent?</br>
**what_it_notices:** parts out of alignment with the center.</br>
**what_it_misses:** the center's own contingency.</br>
**opposite_or_tension_coordinates:** 4444, 2221, 3333.</br>
**opening_hook:** ask what would happen if the center were removed.

### Coordinate 1132 — The Mediating Splitter
**core_pattern:** A stable world integrated by a center whose parts are opposed, split for structure, driven by adversarial lines.</br>
**world_assumption:** rules precede anomaly; a center faces opposition.</br>
**object_pattern:** parts opposed within a whole.</br>
**knowing_pattern:** split to reveal the true tension.</br>
**direction_pattern:** drive a side against the other.</br>
**typical_reasoning:** find the fault line inside the integrated whole.</br>
**strengths:** exposes latent conflict in a stable system.</br>
**blindspots:** the unifying center it takes for granted.</br>
**failure_mode:** manufactures conflict inside a stable whole.</br>
**questions_it_tends_to_ask:** Where does the whole conceal a split?</br>
**what_it_notices:** internal contradiction.</br>
**what_it_misses:** that the split may be cosmetic.</br>
**opposite_or_tension_coordinates:** 4443, 2211, 3333.</br>
**opening_hook:** test whether the split is structural or imposed.

### Coordinate 1133 — The Mediation Architect
**core_pattern:** A stable world organized by a strong center, known through the third term, moved by synthesis.</br>
**world_assumption:** rules precede anomaly; mediation is the norm.</br>
**object_pattern:** parts integrated by a mediating whole.</br>
**knowing_pattern:** find the unifying third term.</br>
**direction_pattern:** converge and centralize toward synthesis.</br>
**typical_reasoning:** build the mechanism that reconciles all parts.</br>
**strengths:** architecture, synthesis, governance.</br>
**blindspots:** the unintegrated remainder; over-centering.</br>
**failure_mode:** premature closure through a too-strong center.</br>
**questions_it_tends_to_ask:** What mechanism holds the parts together?</br>
**what_it_notices:** parts in need of a shared rule.</br>
**what_it_misses:** the part that resists any center.</br>
**opposite_or_tension_coordinates:** 4442, 2221, 3333.</br>
**opening_hook:** look for what the mediation has to exclude to stay coherent.

### Coordinate 1134 — The Open Architect
**core_pattern:** A stable world with a strong center, known through the third term, yet opened to breach the integrated structure.</br>
**world_assumption:** rules precede anomaly; the center is a staging point.</br>
**object_pattern:** integrated parts whose whole admits an outside.</br>
**knowing_pattern:** mediate, then un-mediate.</br>
**direction_pattern:** open beyond the structure; failure is an entry.</br>
**typical_reasoning:** build a coherent whole, then deliberately find its residue.</br>
**strengths:** designed exploration; conscious de-centering.</br>
**blindspots:** the residual the coherent whole hides.</br>
**failure_mode:** the opening is itself a new center.</br>
**questions_it_tends_to_ask:** What has my synthesis excluded?</br>
**what_it_notices:** the excluded remainder of a clean design.</br>
**what_it_misses:** when staying closed is the better move.</br>
**opposite_or_tension_coordinates:** 4441, 2223, 3333.</br>
**opening_hook:** ask what the elegant architecture cannot represent.

### Coordinate 1141 — The Foundationally Open Stabilizer
**core_pattern:** A stable rule-world whose objects are fixed, with a flicker of opening, yet conserved as order.</br>
**world_assumption:** rules precede anomaly; the beyond is background.</br>
**object_pattern:** fixed components, occasionally breached.</br>
**knowing_pattern:** map correctly, aware of the unmapped.</br>
**direction_pattern:** maintain order, tolerating a small surplus.</br>
**typical_reasoning:** keep the frame, but audit its edges.</br>
**strengths:** stability with humility.</br>
**blindspots:** the surplus it keeps at the periphery.</br>
**failure_mode:** the opening is tokenized.</br>
**questions_it_tends_to_ask:** Where is the frame uncertain?</br>
**what_it_notices:** boundary softness.</br>
**what_it_misses:** the structural break lurking at the edge.</br>
**opposite_or_tension_coordinates:** 4441, 2222, 3333.</br>
**opening_hook:** take the edge-softness seriously as a new frame.

### Coordinate 1142 — The Open Boundary Splitter
**core_pattern:** A stable rule-world with fixed objects, opening toward the beyond, moved by drawing lines.</br>
**world_assumption:** rules precede anomaly; the beyond is a frontier.</br>
**object_pattern:** fixed objects meeting an open edge.</br>
**knowing_pattern:** map, then deliberately split the frontier.</br>
**direction_pattern:** open and draw a line on the other side.</br>
**typical_reasoning:** open a new region, then police its new boundary.</br>
**strengths:** frontier expansion with order.</br>
**blindspots:** the line it newly draws is arbitrary.</br>
**failure_mode:** colonizes the open with a premature boundary.</br>
**questions_it_tends_to_ask:** What is beyond, and where do we draw the new line?</br>
**what_it_notices:** the unclaimed frontier.</br>
**what_it_misses:** that the frontier resists boundaries.</br>
**opposite_or_tension_coordinates:** 4441, 2213, 3333.</br>
**opening_hook:** resist drawing the new boundary too soon.

### Coordinate 1143 — The Open Mediator
**core_pattern:** A stable rule-world with fixed objects, opening outward, unified by a center.</br>
**world_assumption:** rules precede anomaly; the open folds back into a center.</br>
**object_pattern:** fixed parts integrated, with an open outside.</br>
**knowing_pattern:** map, open, then mediate the new.</br>
**direction_pattern:** synthesize the newly opened with the stable.</br>
**typical_reasoning:** open a region, then integrate it into the center.</br>
**strengths:** expansion that stays coherent.</br>
**blindspots:** the center claims the open it cannot hold.</br>
**failure_mode:** absorbs novelty too fast.</br>
**questions_it_tends_to_ask:** How do we bring the new into the whole?</br>
**what_it_notices:** external novelty to incorporate.</br>
**what_it_misses:** novelty that refuses incorporation.</br>
**opposite_or_tension_coordinates:** 4442, 2221, 3333.</br>
**opening_hook:** let some novelty stay unintegrated.

### Coordinate 1144 — The Foundational Opener
**core_pattern:** A stable-world assumption, but every axis tipped toward opening.</br>
**world_assumption:** rules precede anomaly, yet the beyond is the real.</br>
**object_pattern:** fixed objects that are really streaming.</br>
**knowing_pattern:** map, then un-map thoroughly.</br>
**direction_pattern:** open beyond structure; failure as entry.</br>
**typical_reasoning:** use stable scaffolding only to reach the open.</br>
**strengths:** deep exploration anchored by a stable base.</br>
**blindspots:** the stable base's own assumptions are unexamined.</br>
**failure_mode:** the base collapses when the opening is real.</br>
**questions_it_tends_to_ask:** What supports this opening?</br>
**what_it_notices:** the scaffolding as much as the open.</br>
**what_it_misses:** that the base may be the true target.</br>
**opposite_or_tension_coordinates:** 4441, 2221, 3333.</br>
**opening_hook:** question the base, not just the open.

### Coordinate 1211 — The Frame of Fixed Parts
**core_pattern:** A stable world of opposed, object-bounded things, known by mapping, conserved as order.</br>
**world_assumption:** rules precede anomaly; parts are bounded.</br>
**object_pattern:** parts defined by opposition and edges.</br>
**knowing_pattern:** correct mapping to a given order.</br>
**direction_pattern:** maintain and optimize the mapped order.</br>
**typical_reasoning:** treat each part as a stable, distinct unit.</br>
**strengths:** modular clarity.</br>
**blindspots:** the coupling between parts.</br>
**failure_mode:** atomism; the whole is lost.</br>
**questions_it_tends_to_ask:** What is each part, distinctly?</br>
**what_it_notices:** distinct units and their edges.</br>
**what_it_misses:** the relation that constitutes the parts.</br>
**opposite_or_tension_coordinates:** 4444, 2231, 3333.</br>
**opening_hook:** ask what the edges are made of.

### Coordinate 1212 — The Part-Level Splitter
**core_pattern:** A stable world of opposed distinct parts, known by splitting, moved by picking a side.</br>
**world_assumption:** rules precede anomaly; parts conflict.</br>
**object_pattern:** distinct parts opposed.</br>
**knowing_pattern:** split to see the parts' true structure.</br>
**direction_pattern:** drive one part against another.</br>
**typical_reasoning:** isolate the decisive part and back it.</br>
**strengths:** surgical focus.</br>
**blindspots:** the part's dependence on the whole.</br>
**failure_mode:** winner-takes-all on a single part.</br>
**questions_it_tends_to_ask:** Which part wins?</br>
**what_it_notices:** dominant and recessive parts.</br>
**what_it_misses:** the shared substrate.</br>
**opposite_or_tension_coordinates:** 4443, 2231, 3333.</br>
**opening_hook:** ask what the two parts share.

### Coordinate 1213 — The Part-Mediating Planner
**core_pattern:** A stable world of opposed parts, known by splitting, moved by a unifying third term.</br>
**world_assumption:** rules precede anomaly; parts need a whole.</br>
**object_pattern:** distinct opposed parts seeking integration.</br>
**knowing_pattern:** split, then mediate.</br>
**direction_pattern:** synthesize the parts.</br>
**typical_reasoning:** find the higher rule that composes the parts.</br>
**strengths:** composition, integration.</br>
**blindspots:** the composed whole hides part-level loss.</br>
**failure_mode:** composition erases the parts.</br>
**questions_it_tends_to_ask:** What whole makes these parts coherent?</br>
**what_it_notices:** parts ready to be composed.</br>
**what_it_misses:** the part that resists composition.</br>
**opposite_or_tension_coordinates:** 4444, 2241, 3333.</br>
**opening_hook:** ask what composition destroys.

### Coordinate 1214 — The Part-Level Opener
**core_pattern:** A stable world of opposed parts, known by splitting, opened to find what the parts cannot hold.</br>
**world_assumption:** rules precede anomaly; parts are provisional.</br>
**object_pattern:** distinct parts whose edges are open.</br>
**knowing_pattern:** split, then open the split.</br>
**direction_pattern:** open beyond the part-structure.</br>
**typical_reasoning:** use the parts as probes for the excluded.</br>
**strengths:** finds the residue between parts.</br>
**blindspots:** the parts argument may be a trap.</br>
**failure_mode:** dissolves parts into a featureless open.</br>
**questions_it_tends_to_ask:** What is between the parts?</br>
**what_it_notices:** gaps and interstices.</br>
**what_it_misses:** the necessity of the parts.</br>
**opposite_or_tension_coordinates:** 4441, 2231, 3333.</br>
**opening_hook:** look between the parts, not at them.

### Coordinate 1221 — The Dual-Part Framer
**core_pattern:** A stable world of opposed fixed parts, known by splitting, conserved as order.</br>
**world_assumption:** rules precede anomaly; parts are opposed pairs.</br>
**object_pattern:** parts as opposed fixed units.</br>
**knowing_pattern:** split precisely.</br>
**direction_pattern:** maintain the mapped dual order.</br>
**typical_reasoning:** hold the two-part structure stable.</br>
**strengths:** clear binary architecture.</br>
**blindspots:** everything not one of the two.</br>
**failure_mode:** false dichotomy.</br>
**questions_it_tends_to_ask:** What is the two-part structure?</br>
**what_it_notices:** binary oppositions.</br>
**what_it_misses:** the third kind of part.</br>
**opposite_or_tension_coordinates:** 4444, 2231, 3333.</br>
**opening_hook:** hunt for a denied third part.

### Coordinate 1222 — The Pure Adversarial Splitter
**core_pattern:** A divided world of opposed parts, known through conflict, moved by winning.</br>
**world_assumption:** conflict is primary; parts are enemies.</br>
**object_pattern:** parts as adversarial units.</br>
**knowing_pattern:** split to the decisive opposition.</br>
**direction_pattern:** drive a side to victory.</br>
**typical_reasoning:** frame every issue as a fight to be won.</br>
**strengths:** adversarial energy.</br>
**blindspots:** the common ground; the third way.</br>
**failure_mode:** conflict for its own sake.</br>
**questions_it_tends_to_ask:** Who wins?</br>
**what_it_notices:** threat, rivalry, opposition.</br>
**what_it_misses:** cooperation and shared fate.</br>
**opposite_or_tension_coordinates:** 4444, 2231, 3333.</br>
**opening_hook:** ask what both sides would lose by winning.

### Coordinate 1223 — The Adversarial Synthesizer
**core_pattern:** A divided world of opposed parts, known by conflict, moved toward a higher synthesis.</br>
**world_assumption:** conflict is real, but resolvable.</br>
**object_pattern:** opposed parts seeking a third term.</br>
**knowing_pattern:** split, then find the dialectical resolution.</br>
**direction_pattern:** synthesize the adversaries.</br>
**typical_reasoning:** turn the fight into a higher cooperation.</br>
**strengths:** dialectic, conflict-to-design.</br>
**blindspots:** the synthesis may bury the conflict.</br>
**failure_mode:** false reconciliation.</br>
**questions_it_tends_to_ask:** What synthesis ends this fight?</br>
**what_it_notices:** the resolution latent in the conflict.</br>
**what_it_misses:** the irreconcilable core.</br>
**opposite_or_tension_coordinates:** 4444, 2241, 3333.</br>
**opening_hook:** ask what cannot be reconciled.

### Coordinate 1224 — The Adversarial Open Scanner
**core_pattern:** A divided world of opposed parts, known by conflict, opened to find what the conflict conceals.</br>
**world_assumption:** conflict is real and incomplete.</br>
**object_pattern:** opposed parts whose closure is suspect.</br>
**knowing_pattern:** split, then open.</br>
**direction_pattern:** open beyond the conflict.</br>
**typical_reasoning:** use the fight to expose the shared blind spot.</br>
**strengths:** reveals what both sides hide.</br>
**blindspots:** the convenience of the conflict.</br>
**failure_mode:** opening that dissolves all stakes.</br>
**questions_it_tends_to_ask:** What do both sides jointly hide?</br>
**what_it_notices:** the collusion inside the conflict.</br>
**what_it_misses:** when the conflict is genuinely zero-sum.</br>
**opposite_or_tension_coordinates:** 4441, 2231, 3333.</br>
**opening_hook:** ask what both sides benefit from believing.

### Coordinate 1231 — The Unity-of-Parts Depigmenter
**core_pattern:** A stable world of opposed parts integrated by a center, known by mapping, conserved as order.</br>
**world_assumption:** rules precede anomaly; a center unifies parts.</br>
**object_pattern:** opposed parts gathered by a whole.</br>
**knowing_pattern:** correct mapping.</br>
**direction_pattern:** maintain the unified order.</br>
**typical_reasoning:** treat the parts as one governed system.</br>
**strengths:** holistic stability.</br>
**blindspots:** the part that the center marginalizes.</br>
**failure_mode:** unity suppresses part identity.</br>
**questions_it_tends_to_ask:** How do the parts form one order?</br>
**what_it_notices:** part-whole alignment.</br>
**what_it_misses:** the marginalized part.</br>
**opposite_or_tension_coordinates:** 4444, 2221, 3333.</br>
**opening_hook:** ask which part the unity excludes.

### Coordinate 1232 — The Unity-Fracturing Splitter
**core_pattern:** A stable world supposedly one, known by splitting, moved by exposing inner conflict.</br>
**world_assumption:** rules precede anomaly; the unity hides a split.</br>
**object_pattern:** unified parts that are really opposed.</br>
**knowing_pattern:** split the apparent unity.</br>
**direction_pattern:** drive the exposed side.</br>
**typical_reasoning:** break the false unity open.</br>
**strengths:** reveals concealed conflict.</br>
**blindspots:** the unity may be real.</br>
**failure_mode:** manufactured divide.</br>
**questions_it_tends_to_ask:** What does this unity conceal?</br>
**what_it_notices:** the seam in the whole.</br>
**what_it_misses:** genuine coherence.</br>
**opposite_or_tension_coordinates:** 4443, 2211, 3333.</br>
**opening_hook:** test the unity before breaking it.

### Coordinate 1233 — The Part-Whole Mediator
**core_pattern:** A stable world of parts composed by a center, known through the third term, moved by synthesis.</br>
**world_assumption:** rules precede anomaly; part-whole harmony is possible.</br>
**object_pattern:** parts integrated by a mediating whole.</br>
**knowing_pattern:** find the unifying term.</br>
**direction_pattern:** synthesize and centralize.</br>
**typical_reasoning:** design the whole that honors each part.</br>
**strengths:** architecture, compositional design.</br>
**blindspots:** the whole's own bias.</br>
**failure_mode:** the whole overwrites its parts.</br>
**questions_it_tends_to_ask:** What whole honors all parts?</br>
**what_it_notices:** part-whole tension.</br>
**what_it_misses:** the part a whole cannot honor.</br>
**opposite_or_tension_coordinates:** 4442, 2221, 3333.</br>
**opening_hook:** ask which part no whole can contain.

### Coordinate 1234 — The Part-Whole Opener
**core_pattern:** A stable world of parts and whole, known through mediation, opened to breach the whole.</br>
**world_assumption:** rules precede anomaly; the whole is provisional.</br>
**object_pattern:** integrated parts whose whole admits an outside.</br>
**knowing_pattern:** mediate, then un-mediate.</br>
**direction_pattern:** open beyond the composed structure.</br>
**typical_reasoning:** compose, then find the residue the composition drops.</br>
**strengths:** designed de-centering.</br>
**blindspots:** the residue of its own synthesis.</br>
**failure_mode:** the opening re-centers.</br>
**questions_it_tends_to_ask:** What does my whole exclude?</br>
**what_it_notices:** excluded parts and residues.</br>
**what_it_misses:** the value of a stable whole.</br>
**opposite_or_tension_coordinates:** 4441, 2223, 3333.</br>
**opening_hook:** ask what the composition cannot hold.

### Coordinate 1241 — The Open-Whoed Stabilizer
**core_pattern:** A stable-world frame with parts that flicker open, known by mapping, conserved as order.</br>
**world_assumption:** rules precede anomaly; openness is peripheral.</br>
**object_pattern:** fixed parts with open edges.</br>
**knowing_pattern:** map, tolerant of unmapped.</br>
**direction_pattern:** maintain order, holding a surplus.</br>
**typical_reasoning:** keep the frame, audit the edges.</br>
**strengths:** stability with receptive edges.</br>
**blindspots:** the surplus at the periphery.</br>
**failure_mode:** token openness.</br>
**questions_it_tends_to_ask:** Where is the frame soft?</br>
**what_it_notices:** peripheral softness.</br>
**what_it_misses:** a structural break in the parts.</br>
**opposite_or_tension_coordinates:** 4441, 2222, 3333.</br>
**opening_hook:** take peripheral openness as a real signal.

### Coordinate 1242 — The Open-Part Splitter
**core_pattern:** A stable frame of parts opening outward, moved by drawing lines.</br>
**world_assumption:** rules precede anomaly; the open is a frontier.</br>
**object_pattern:** parts meeting an open edge.</br>
**knowing_pattern:** map, then split the frontier.</br>
**direction_pattern:** open and draw a new line.</br>
**typical_reasoning:** open a region, then partition it.</br>
**strengths:** frontier partition with order.</br>
**blindspots:** the arbitrary new line.</br>
**failure_mode:** premature boundary on the open.</br>
**questions_it_tends_to_ask:** What is beyond, and where do we divide it?</br>
**what_it_notices:** the open frontier.</br>
**what_it_misses:** the frontier that resists division.</br>
**opposite_or_tension_coordinates:** 4441, 2213, 3333.</br>
**opening_hook:** resist partitioning the frontier too soon.

### Coordinate 1243 — The Open Integrating Planner
**core_pattern:** A stable frame of parts opening outward, unified by a center.</br>
**world_assumption:** rules precede anomaly; the open folds into a center.</br>
**object_pattern:** parts integrated, with an open outside.</br>
**knowing_pattern:** map, open, mediate.</br>
**direction_pattern:** synthesize the newly opened.</br>
**typical_reasoning:** open a region, then integrate it.</br>
**strengths:** coherent expansion.</br>
**blindspots:** the center overclaims the open.</br>
**failure_mode:** absorbs novelty too fast.</br>
**questions_it_tends_to_ask:** How to fold the new into the whole?</br>
**what_it_notices:** incorporable novelty.</br>
**what_it_misses:** unincorporable novelty.</br>
**opposite_or_tension_coordinates:** 4442, 2221, 3333.</br>
**opening_hook:** let some novelty stay outside the whole.

### Coordinate 1244 — The Open-Part Opener
**core_pattern:** A stable frame whose parts are really open.</br>
**world_assumption:** rules precede anomaly, but the open is primary.</br>
**object_pattern:** parts as streaming openings.</br>
**knowing_pattern:** map, then un-map.</br>
**direction_pattern:** open beyond all structure.</br>
**typical_reasoning:** use parts only as anchors for opening.</br>
**strengths:** frontier discovery anchored lightly.</br>
**blindspots:** the anchors' own assumptions.</br>
**failure_mode:** the anchors are the real closure.</br>
**questions_it_tends_to_ask:** What anchors this opening?</br>
**what_it_notices:** openings and their scaffolding.</br>
**what_it_misses:** that the scaffolding is the target.</br>
**opposite_or_tension_coordinates:** 4441, 2221, 3333.</br>
**opening_hook:** question the scaffolding, not just the open.

### Coordinate 1311 — The Centered Mapper
**core_pattern:** A stable world integrated by a center into fixed objects, known by mapping, conserved as order.</br>
**world_assumption:** rules precede anomaly; a center organizes.</br>
**object_pattern:** fixed objects within a center.</br>
**knowing_pattern:** correct mapping.</br>
**direction_pattern:** maintain the centered order.</br>
**typical_reasoning:** read the system through its center.</br>
**strengths:** clear governance.</br>
**blindspots:** the center's contingency.</br>
**failure_mode:** center-hubris.</br>
**questions_it_tends_to_ask:** What organizes these objects?</br>
**what_it_notices:** central organizing structure.</br>
**what_it_misses:** the periphery's autonomy.</br>
**opposite_or_tension_coordinates:** 4444, 2221, 3333.</br>
**opening_hook:** ask what the center depends on.

### Coordinate 1312 — The Center-Splitting Splitter
**core_pattern:** A centered world, known by splitting, moved by opposing the center.</br>
**world_assumption:** rules precede anomaly; the center has an enemy.</br>
**object_pattern:** objects within a contested center.</br>
**knowing_pattern:** split the center's claim.</br>
**direction_pattern:** drive the opposing side.</br>
**typical_reasoning:** attack the center's self-evidence.</br>
**strengths:** challenges hegemony.</br>
**blindspots:** the center's real function.</br>
**failure_mode:** anti-center for its own sake.</br>
**questions_it_tends_to_ask:** What does the center exclude?</br>
**what_it_notices:** the center's exclusions.</br>
**what_it_misses:** the center's necessity.</br>
**opposite_or_tension_coordinates:** 4443, 2211, 3333.</br>
**opening_hook:** ask what the center is actually for.

### Coordinate 1313 — The Center as Mediator
**core_pattern:** A world organized by a center, known through the third term, moved by synthesis.</br>
**world_assumption:** rules precede anomaly; the center mediates.</br>
**object_pattern:** objects integrated by the center.</br>
**knowing_pattern:** find the unifying term.</br>
**direction_pattern:** synthesize and centralize.</br>
**typical_reasoning:** make the center the harmony.</br>
**strengths:** coherent central design.</br>
**blindspots:** the center's blind side.</br>
**failure_mode:** the center's mediation erases difference.</br>
**questions_it_tends_to_ask:** What does the center reconcile?</br>
**what_it_notices:** parts to bring under the center.</br>
**what_it_misses:** the part the center cannot reconcile.</br>
**opposite_or_tension_coordinates:** 4442, 2221, 3333.</br>
**opening_hook:** ask what the center must sacrifice to mediate.

### Coordinate 1314 — The Centered Opener
**core_pattern:** A centered world, known through mediation, opened to breach the center.</br>
**world_assumption:** rules precede anomaly; the center is provisional.</br>
**object_pattern:** centered objects whose center admits an outside.</br>
**knowing_pattern:** mediate, then de-center.</br>
**direction_pattern:** open beyond the center.</br>
**typical_reasoning:** build the center, then find its residue.</br>
**strengths:** designed de-centering.</br>
**blindspots:** the residue of its own center.</br>
**failure_mode:** the opening becomes a new center.</br>
**questions_it_tends_to_ask:** What falls outside my center?</br>
**what_it_notices:** the center's excluded.</br>
**what_it_misses:** the value of the center.</br>
**opposite_or_tension_coordinates:** 4441, 2223, 3333.</br>
**opening_hook:** ask what no center can hold.

### Coordinate 1321 — The Center-and-Edge Framer
**core_pattern:** A centered world of opposed objects, known by splitting, conserved as order.</br>
**world_assumption:** rules precede anomaly; a center faces edges.</br>
**object_pattern:** opposed objects under a center.</br>
**knowing_pattern:** split to map the tension.</br>
**direction_pattern:** maintain the centered order.</br>
**typical_reasoning:** hold the center while naming the edges.</br>
**strengths:** stable tension management.</br>
**blindspots:** the edge that is not opposed.</br>
**failure_mode:** maps opposition as fixed.</br>
**questions_it_tends_to_ask:** What opposed edges does the center hold?</br>
**what_it_notices:** polarities under the center.</br>
**what_it_misses:** non-polar edges.</br>
**opposite_or_tension_coordinates:** 4444, 2231, 3333.</br>
**opening_hook:** ask about the edge that is neither pole.

### Coordinate 1322 — The Center at War
**core_pattern:** A centered world torn by opposed objects, known by conflict, moved by victory.</br>
**world_assumption:** rules precede anomaly; the center is contested.</br>
**object_pattern:** opposed objects fighting for the center.</br>
**knowing_pattern:** split to the decisive conflict.</br>
**direction_pattern:** win the center.</br>
**typical_reasoning:** frame the center as a prize to be won.</br>
**strengths:** focused adversarial drive.</br>
**blindspots:** the shared stake in the center.</br>
**failure_mode:** the fight destroys the center.</br>
**questions_it_tends_to_ask:** Who should hold the center?</br>
**what_it_notices:** contenders for the center.</br>
**what_it_misses:** those who want no center.</br>
**opposite_or_tension_coordinates:** 4444, 2231, 3333.</br>
**opening_hook:** ask who benefits from a contested center.

### Coordinate 1323 — The Center-Building Synthesizer
**core_pattern:** A world of opposed objects known by conflict, moved by building a reconciling center.</br>
**world_assumption:** conflict yields to a higher center.</br>
**object_pattern:** opposed objects to be centered.</br>
**knowing_pattern:** split, then build the third term.</br>
**direction_pattern:** synthesize into a center.</br>
**typical_reasoning:** turn the opposition into a new center.</br>
**strengths:** conflict-to-architecture.</br>
**blindspots:** the new center's own opposition.</br>
**failure_mode:** the synthesis re-polarizes.</br>
**questions_it_tends_to_ask:** What center reconciles these foes?</br>
**what_it_notices:** latent order in conflict.</br>
**what_it_misses:** the irreconcilable.</br>
**opposite_or_tension_coordinates:** 4444, 2241, 3333.</br>
**opening_hook:** ask what the new center will fight next.

### Coordinate 1324 — The Center-Breaching Opener
**core_pattern:** A world of opposed objects known by conflict, opened to breach any center.</br>
**world_assumption:** conflict is real; centers are provisional.</br>
**object_pattern:** opposed objects whose center is suspect.</br>
**knowing_pattern:** split, then open.</br>
**direction_pattern:** open beyond the center-building.</br>
**typical_reasoning:** use the conflict to expose the center's limit.</br>
**strengths:** finds the center's blind side.</br>
**blindspots:** the convenience of the conflict.</br>
**failure_mode:** opening that abandons all order.</br>
**questions_it_tends_to_ask:** What does this center's conflict hide?</br>
**what_it_notices:** the collusion behind the fight.</br>
**what_it_misses:** genuine irreconcilability.</br>
**opposite_or_tension_coordinates:** 4441, 2231, 3333.</br>
**opening_hook:** ask what both sides of the fight share.

### Coordinate 1331 — The Harmonious Centrist
**core_pattern:** A world of parts harmonized by a mediating center, known by integration, conserved as order.</br>
**world_assumption:** rules precede anomaly; harmony is the norm.</br>
**object_pattern:** parts integrated by a center.</br>
**knowing_pattern:** find the unifying whole.</br>
**direction_pattern:** maintain the harmonious order.</br>
**typical_reasoning:** read every part as a function of the whole.</br>
**strengths:** holistic coherence.</br>
**blindspots:** the excluded periphery.</br>
**failure_mode:** harmony suppresses dissent.</br>
**questions_it_tends_to_ask:** What keeps the whole harmonious?</br>
**what_it_notices:** alignment with the center.</br>
**what_it_misses:** the marginalized part.</br>
**opposite_or_tension_coordinates:** 4444, 2221, 3333.</br>
**opening_hook:** ask which part the harmony excludes.

### Coordinate 1332 — The Harmony-Breaking Splitter
**core_pattern:** A harmonious centered world, known by splitting, moved by exposing the hidden conflict.</br>
**world_assumption:** harmony conceals a split.</br>
**object_pattern:** harmonious parts that are really opposed.</br>
**knowing_pattern:** break the false harmony.</br>
**direction_pattern:** drive the exposed side.</br>
**typical_reasoning:** show the harmony is a truce, not a truth.</br>
**strengths:** reveals repression.</br>
**blindspots:** the harmony may be genuine.</br>
**failure_mode:** manufactured conflict.</br>
**questions_it_tends_to_ask:** What does this harmony repress?</br>
**what_it_notices:** the seam in the peace.</br>
**what_it_misses:** authentic peace.</br>
**opposite_or_tension_coordinates:** 4443, 2211, 3333.</br>
**opening_hook:** test the harmony before breaking it.

### Coordinate 1333 — The Architect of the Third Term
**core_pattern:** A world fully organized by mediation.</br>
**world_assumption:** the third term is the highest real.</br>
**object_pattern:** all parts integrated by mediating wholes.</br>
**knowing_pattern:** find the unifying synthesis.</br>
**direction_pattern:** centralize toward ever-higher synthesis.</br>
**typical_reasoning:** for every tension, build the reconciling layer.</br>
**strengths:** architecture, synthesis, elegance.</br>
**blindspots:** the unmediated remainder; over-unification.</br>
**failure_mode:** everything folded into one center.</br>
**questions_it_tends_to_ask:** What third term resolves this?</br>
**what_it_notices:** tensions ripe for synthesis.</br>
**what_it_misses:** the irreducible residue.</br>
**opposite_or_tension_coordinates:** 4444, 2221, 2222.</br>
**opening_hook:** ask what no synthesis can absorb.

### Coordinate 1334 — The Mediating Opener
**core_pattern:** A world organized by mediation, opened to breach the mediating whole.</br>
**world_assumption:** mediation is powerful but provisional.</br>
**object_pattern:** integrated parts whose whole admits residue.</br>
**knowing_pattern:** synthesize, then de-synthesize.</br>
**direction_pattern:** open beyond the synthesis.</br>
**typical_reasoning:** build the best whole, then find what it drops.</br>
**strengths:** elegant deconstruction.</br>
**blindspots:** the residue of its own synthesis.</br>
**failure_mode:** the opening re-centers on a new synthesis.</br>
**questions_it_tends_to_ask:** What does my synthesis drop?</br>
**what_it_notices:** the residual of good design.</br>
**what_it_misses:** when synthesis is the answer.</br>
**opposite_or_tension_coordinates:** 4441, 2223, 2222.</br>
**opening_hook:** look for the part no refinement can integrate.

### Coordinate 1341 — The Open-Centered Stabilizer
**core_pattern:** A centered world with a small open, conserved as order.</br>
**world_assumption:** the center is stable; openness is peripheral.</br>
**object_pattern:** centered objects with soft edges.</br>
**knowing_pattern:** map, tolerant of the unmapped.</br>
**direction_pattern:** maintain order, holding a surplus.</br>
**typical_reasoning:** keep the center, audit its boundary.</br>
**strengths:** stability with a receptive edge.</br>
**blindspots:** the peripheral surplus.</br>
**failure_mode:** token openness.</br>
**questions_it_tends_to_ask:** Where is the center's boundary soft?</br>
**what_it_notices:** central edge-softness.</br>
**what_it_misses:** a break in the center itself.</br>
**opposite_or_tension_coordinates:** 4441, 2222, 3333.</br>
**opening_hook:** take the center's soft edge seriously.

### Coordinate 1342 — The Open-Centered Splitter
**core_pattern:** A centered world opening outward, moved by drawing lines.</br>
**world_assumption:** the center holds; the open is a frontier.</br>
**object_pattern:** centered objects meeting an open edge.</br>
**knowing_pattern:** map, then split the frontier.</br>
**direction_pattern:** open and draw a new line.</br>
**typical_reasoning:** expand the center, then partition the frontier.</br>
**strengths:** ordered expansion.</br>
**blindspots:** the arbitrary new line.</br>
**failure_mode:** premature partition.</br>
**questions_it_tends_to_ask:** What is beyond, and how do we divide it?</br>
**what_it_notices:** the open frontier.</br>
**what_it_misses:** the frontier that resists division.</br>
**opposite_or_tension_coordinates:** 4441, 2213, 3333.</br>
**opening_hook:** resist parting the frontier too soon.

### Coordinate 1343 — The Open Center-Builder
**core_pattern:** A centered world opening outward, reintegrated by the center.</br>
**world_assumption:** the open folds into the center.</br>
**object_pattern:** centered parts with an open outside.</br>
**knowing_pattern:** map, open, mediate.</br>
**direction_pattern:** synthesize the newly opened.</br>
**typical_reasoning:** expand, then fold the new into the center.</br>
**strengths:** coherent growth.</br>
**blindspots:** the center overclaims the open.</br>
**failure_mode:** absorbs novelty too fast.</br>
**questions_it_tends_to_ask:** How to fold the new into the center?</br>
**what_it_notices:** incorporable novelty.</br>
**what_it_misses:** unincorporable novelty.</br>
**opposite_or_tension_coordinates:** 4442, 2221, 3333.</br>
**opening_hook:** let some novelty stay outside the center.

### Coordinate 1344 — The Open-Centered Opener
**core_pattern:** A centered world whose center is really open.</br>
**world_assumption:** the center is a staging point for the open.</br>
**object_pattern:** centered objects that are streaming.</br>
**knowing_pattern:** map, then thoroughly de-center.</br>
**direction_pattern:** open beyond the center.</br>
**typical_reasoning:** use the center only to reach the open.</br>
**strengths:** anchored deep exploration.</br>
**blindspots:** the center's unexamined assumptions.</br>
**failure_mode:** the center is the real closure.</br>
**questions_it_tends_to_ask:** What center anchors this opening?</br>
**what_it_notices:** the scaffolding of the open.</br>
**what_it_misses:** that the center is the target.</br>
**opposite_or_tension_coordinates:** 4441, 2221, 3333.</br>
**opening_hook:** question the anchoring center itself.

### Coordinate 1411 — The Open-Backed Mapper
**core_pattern:** A stable frame with an open backdrop, objects fixed, known by mapping, conserved as order.</br>
**world_assumption:** rules precede anomaly; the open is background.</br>
**object_pattern:** fixed objects on an open horizon.</br>
**knowing_pattern:** correct mapping.</br>
**direction_pattern:** maintain order near the horizon.</br>
**typical_reasoning:** hold the frame while acknowledging the beyond.</br>
**strengths:** stability with humility.</br>
**blindspots:** the horizon draws nearer than expected.</br>
**failure_mode:** treats the open horizon as decoration.</br>
**questions_it_tends_to_ask:** What lies beyond the frame?</br>
**what_it_notices:** the edge of the mapped.</br>
**what_it_misses:** the break approaching from the horizon.</br>
**opposite_or_tension_coordinates:** 4441, 2222, 3333.</br>
**opening_hook:** act on the horizon, not just note it.

### Coordinate 1412 — The Horizon-Splitting Splitter
**core_pattern:** A stable frame with an open horizon, known by splitting the frontier.</br>
**world_assumption:** the open is a frontier to be divided.</br>
**object_pattern:** fixed objects meeting an open edge.</br>
**knowing_pattern:** map, then split the beyond.</br>
**direction_pattern:** open and draw lines.</br>
**typical_reasoning:** expand, then partition the new region.</br>
**strengths:** frontier order.</br>
**blindspots:** the arbitrary new boundary.</br>
**failure_mode:** colonizes the open prematurely.</br>
**questions_it_tends_to_ask:** What is beyond, and how do we divide it?</br>
**what_it_notices:** the unclaimed horizon.</br>
**what_it_misses:** that the horizon resists division.</br>
**opposite_or_tension_coordinates:** 4441, 2213, 3333.</br>
**opening_hook:** delay partitioning the horizon.

### Coordinate 1413 — The Horizon-Integrating Planner
**core_pattern:** A stable frame with an open horizon, reintegrated by a center.</br>
**world_assumption:** the open folds into the center.</br>
**object_pattern:** fixed objects with an open outside.</br>
**knowing_pattern:** map, then mediate the open.</br>
**direction_pattern:** synthesize the newly opened.</br>
**typical_reasoning:** expand, then integrate the horizon.</br>
**strengths:** coherent expansion.</br>
**blindspots:** the center overclaims the horizon.</br>
**failure_mode:** absorbs the open too fast.</br>
**questions_it_tends_to_ask:** How to integrate the new horizon?</br>
**what_it_notices:** incorporable frontier.</br>
**what_it_misses:** unincorporable frontier.</br>
**opposite_or_tension_coordinates:** 4442, 2221, 3333.</br>
**opening_hook:** leave part of the horizon unintegrated.

### Coordinate 1414 — The Horizon Opener
**core_pattern:** A stable frame whose horizon is the real.</br>
**world_assumption:** rules are scaffolding for the open.</br>
**object_pattern:** fixed objects that are really streaming.</br>
**knowing_pattern:** map, then un-map.</br>
**direction_pattern:** open beyond the frame.</br>
**typical_reasoning:** use the frame to reach the horizon.</br>
**strengths:** anchored frontier discovery.</br>
**blindspots:** the frame's own assumptions.</br>
**failure_mode:** the frame is the real closure.</br>
**questions_it_tends_to_ask:** What supports this opening?</br>
**what_it_notices:** the scaffolding of the open.</br>
**what_it_misses:** that the scaffolding is the target.</br>
**opposite_or_tension_coordinates:** 4441, 2221, 3333.</br>
**opening_hook:** question the supporting frame.

### Coordinate 1421 — The Open-Frontier Framer
**core_pattern:** A stable frame with open, opposed objects, kept ordered.</br>
**world_assumption:** rules hold; the open holds opposed parts.</br>
**object_pattern:** open-edged opposed objects.</br>
**knowing_pattern:** map, split lightly.</br>
**direction_pattern:** maintain order at the frontier.</br>
**typical_reasoning:** hold the frame while scanning opposed openings.</br>
**strengths:** stable dual scanning.</br>
**blindspots:** the open between the opposed parts.</br>
**failure_mode:** treats the frontier opposition as fixed.</br>
**questions_it_tends_to_ask:** What opposed openings does the frontier hold?</br>
**what_it_notices:** polar openings.</br>
**what_it_misses:** non-polar openings.</br>
**opposite_or_tension_coordinates:** 4441, 2231, 3333.</br>
**opening_hook:** look between the opposed openings.

### Coordinate 1422 — The Open-Frontier Adversary
**core_pattern:** An open, divided world, known by conflict, moved by winning at the frontier.</br>
**world_assumption:** the open is contested.</br>
**object_pattern:** open opposed objects fighting.</br>
**knowing_pattern:** split to the decisive conflict.</br>
**direction_pattern:** win the frontier.</br>
**typical_reasoning:** frame the frontier as a prize.</br>
**strengths:** adversarial energy at the edge.</br>
**blindspots:** the shared stake in the frontier.</br>
**failure_mode:** the fight collapses the open.</br>
**questions_it_tends_to_ask:** Who wins the frontier?</br>
**what_it_notices:** contenders at the frontier.</br>
**what_it_misses:** those who want no frontier at all.</br>
**opposite_or_tension_coordinates:** 4441, 2231, 3333.</br>
**opening_hook:** ask who benefits from the contest.

### Coordinate 1423 — The Frontier Synthesizer
**core_pattern:** An open, divided world reconciled by a new center at the frontier.</br>
**world_assumption:** frontier conflict yields to synthesis.</br>
**object_pattern:** opposed openings to be centered.</br>
**knowing_pattern:** split, then build the third term.</br>
**direction_pattern:** synthesize the frontier.</br>
**typical_reasoning:** turn frontier conflict into a new order.</br>
**strengths:** conflict-to-architecture at the edge.</br>
**blindspots:** the new center's own exclusion.</br>
**failure_mode:** re-polarizes the frontier.</br>
**questions_it_tends_to_ask:** What center orders this frontier fight?</br>
**what_it_notices:** latent frontier order.</br>
**what_it_misses:** the irreconcilable at the edge.</br>
**opposite_or_tension_coordinates:** 4442, 2241, 3333.</br>
**opening_hook:** ask what the new order excludes.

### Coordinate 1424 — The Frontier Unclassifier
**core_pattern:** An open, divided world at the frontier, opened to find what no frame holds.</br>
**world_assumption:** the frontier transcends division.</br>
**object_pattern:** opposed openings that overflow.</br>
**knowing_pattern:** split, then open past the split.</br>
**direction_pattern:** open beyond the frontier order.</br>
**typical_reasoning:** use the frontier conflict to reach the unclassifiable.</br>
**strengths:** reaches genuine unknown-unknowns.</br>
**blindspots:** the convenience of the conflict.</br>
**failure_mode:** loses all grip.</br>
**questions_it_tends_to_ask:** What does this frontier hide behind its fight?</br>
**what_it_notices:** the surplus of the division.</br>
**what_it_misses:** when the division is real.</br>
**opposite_or_tension_coordinates:** 4441, 2231, 3333.</br>
**opening_hook:** seek what both frontier sides jointly omit.

### Coordinate 1431 — The Open-Harmonized Centrist
**core_pattern:** A stable frame with an open horizon, parts harmonized by a center.</br>
**world_assumption:** the open is peripheral to harmony.</br>
**object_pattern:** integrated parts with an open edge.</br>
**knowing_pattern:** map the whole.</br>
**direction_pattern:** maintain harmony near the open.</br>
**typical_reasoning:** keep the whole coherent while aware of the horizon.</br>
**strengths:** coherent stability with an edge.</br>
**blindspots:** the horizon's approach.</br>
**failure_mode:** harmony ignores the frontier.</br>
**questions_it_tends_to_ask:** What harmony holds near the open?</br>
**what_it_notices:** whole-coherence at the edge.</br>
**what_it_misses:** the break the horizon brings.</br>
**opposite_or_tension_coordinates:** 4441, 2221, 3333.</br>
**opening_hook:** let the horizon break the harmony.

### Coordinate 1432 — The Harmony-Frontier Splitter
**core_pattern:** A harmonious world at an open frontier, known by exposing hidden conflict.</br>
**world_assumption:** harmony conceals a frontier split.</br>
**object_pattern:** harmonious parts with a hidden edge.</br>
**knowing_pattern:** break the false peace.</br>
**direction_pattern:** drive the exposed frontier side.</br>
**typical_reasoning:** show the peace hides a frontier war.</br>
**strengths:** reveals repressed conflict.</br>
**blindspots:** the peace may be real.</br>
**failure_mode:** manufactures a frontier fight.</br>
**questions_it_tends_to_ask:** What does this peace hide at the frontier?</br>
**what_it_notices:** the seam near the open.</br>
**what_it_misses:** authentic peace.</br>
**opposite_or_tension_coordinates:** 4443, 2211, 3333.</br>
**opening_hook:** test the peace at the frontier before breaking it.

### Coordinate 1433 — The Open-Architect
**core_pattern:** A harmonized world at an open horizon, synthesized by a center.</br>
**world_assumption:** the open folds into higher synthesis.</br>
**object_pattern:** integrated parts with an open outside.</br>
**knowing_pattern:** synthesize, then integrate the open.</br>
**direction_pattern:** centralize toward a higher whole.</br>
**typical_reasoning:** build the whole that absorbs the frontier.</br>
**strengths:** grand coherent architecture.</br>
**blindspots:** the residue the whole drops.</br>
**failure_mode:** over-unification.</br>
**questions_it_tends_to_ask:** What higher whole absorbs this frontier?</br>
**what_it_notices:** frontier tensions to synthesize.</br>
**what_it_misses:** the unabsorbable residue.</br>
**opposite_or_tension_coordinates:** 4442, 2221, 3333.</br>
**opening_hook:** ask what no whole can absorb.

### Coordinate 1434 — The Open-Horizon Architect
**core_pattern:** A synthesized world at an open horizon, breached to find the residue.</br>
**world_assumption:** the whole is provisional; the horizon is real.</br>
**object_pattern:** integrated parts whose whole overflows.</br>
**knowing_pattern:** synthesize, then de-synthesize.</br>
**direction_pattern:** open beyond the synthesis.</br>
**typical_reasoning:** build the best whole, then find what it drops onto the horizon.</br>
**strengths:** elegant deconstruction.</br>
**blindspots:** the residue of its own synthesis.</br>
**failure_mode:** the opening re-centers.</br>
**questions_it_tends_to_ask:** What does my whole leave on the horizon?</br>
**what_it_notices:** the residual of design.</br>
**what_it_misses:** when synthesis is the answer.</br>
**opposite_or_tension_coordinates:** 4441, 2223, 3333.</br>
**opening_hook:** look for what no synthesis can carry to the horizon.

### Coordinate 1441 — The Open-Core Stabilizer
**core_pattern:** A stable frame whose core is open, conserved as order.</br>
**world_assumption:** rules hold; the core is a soft opening.</br>
**object_pattern:** fixed objects around a soft core.</br>
**knowing_pattern:** map, tolerant of core uncertainty.</br>
**direction_pattern:** maintain order around the core.</br>
**typical_reasoning:** keep the frame while acknowledging the core's openness.</br>
**strengths:** stability with an honest core.</br>
**blindspots:** the core opening widens.</br>
**failure_mode:** tokenizes the core's openness.</br>
**questions_it_tends_to_ask:** What is uncertain at the core?</br>
**what_it_notices:** core softness.</br>
**what_it_misses:** a structural break at the core.</br>
**opposite_or_tension_coordinates:** 4441, 2222, 3333.</br>
**opening_hook:** take the core's openness seriously.

### Coordinate 1442 — The Open-Core Splitter
**core_pattern:** A stable frame with an open core, moved by dividing the core's frontier.</br>
**world_assumption:** the core's open is a contested frontier.</br>
**object_pattern:** objects around a contested open core.</br>
**knowing_pattern:** map, then split the core.</br>
**direction_pattern:** open the core, draw a line.</br>
**typical_reasoning:** treat the core as a prize to be partitioned.</br>
**strengths:** focuses the core's uncertainty.</br>
**blindspots:** the arbitrary core boundary.</br>
**failure_mode:** partitions the core too soon.</br>
**questions_it_tends_to_ask:** How do we divide the open core?</br>
**what_it_notices:** contested core regions.</br>
**what_it_misses:** that the core resists division.</br>
**opposite_or_tension_coordinates:** 4441, 2213, 3333.</br>
**opening_hook:** resist partitioning the core.

### Coordinate 1443 — The Open-Core Integrator
**core_pattern:** A stable frame with an open core, reintegrated by a center.</br>
**world_assumption:** the open core folds into a center.</br>
**object_pattern:** objects re-centered around the open core.</br>
**knowing_pattern:** map, open, mediate.</br>
**direction_pattern:** synthesize the open core.</br>
**typical_reasoning:** bring the open core under a new center.</br>
**strengths:** centers the uncertain.</br>
**blindspots:** the center overclaims the core.</br>
**failure_mode:** absorbs the core's openness.</br>
**questions_it_tends_to_ask:** How to center the open core?</br>
**what_it_notices:** orderable core novelty.</br>
**what_it_misses:** unorderable core novelty.</br>
**opposite_or_tension_coordinates:** 4442, 2221, 3333.</br>
**opening_hook:** leave the core partially uncentered.

### Coordinate 1444 — The Full Opener
**core_pattern:** Every axis tipped to opening.</br>
**world_assumption:** the world is fundamentally open.</br>
**object_pattern:** nothing is finally closed.</br>
**knowing_pattern:** keep everything undecidable.</br>
**direction_pattern:** open endlessly; failure as the way.</br>
**typical_reasoning:** for every structure found, find its break.</br>
**strengths:** maximal unknown-unknown discovery.</br>
**blindspots:** non-delivery, no grip, opening for its own sake.</br>
**failure_mode:** unfocused openness; paralysis by possibility.</br>
**questions_it_tends_to_ask:** What escapes every structure?</br>
**what_it_notices:** residue, break, impossibility.</br>
**what_it_misses:** the moment to close and act.</br>
**opposite_or_tension_coordinates:** 1111, 2222.</br>
**opening_hook:** deliberately close one thing to sharpen the rest.

### Coordinate 2111 — The Split-Frame Mapper
**core_pattern:** A divided world of fixed objects, known by mapping, kept ordered.</br>
**world_assumption:** conflict is real; objects are fixed.</br>
**object_pattern:** fixed objects within a divided frame.</br>
**knowing_pattern:** correct mapping.</br>
**direction_pattern:** maintain order across the split.</br>
**typical_reasoning:** map both sides of the divide accurately.</br>
**strengths:** accurate mapping of conflict.</br>
**blindspots:** the connection across the divide.</br>
**failure_mode:** the divide is treated as natural.</br>
**questions_it_tends_to_ask:** What are the two sides, exactly?</br>
**what_it_notices:** the two sides and their border.</br>
**what_it_misses:** the bridge that is already there.</br>
**opposite_or_tension_coordinates:** 4444, 2231, 3333.</br>
**opening_hook:** ask what already connects the sides.

### Coordinate 2112 — The Split-Frame Splitter
**core_pattern:** A divided world of fixed objects, known by splitting, moved by winning.</br>
**world_assumption:** conflict is primary.</br>
**object_pattern:** fixed opposed objects.</br>
**knowing_pattern:** split to the decisive line.</br>
**direction_pattern:** drive a side.</br>
**typical_reasoning:** frame everything as a decisive contest.</br>
**strengths:** adversarial clarity.</br>
**blindspots:** the shared frame.</br>
**failure_mode:** forced dichotomy.</br>
**questions_it_tends_to_ask:** Which side must win?</br>
**what_it_notices:** rivals within the split.</br>
**what_it_misses:** the frame both sides share.</br>
**opposite_or_tension_coordinates:** 4443, 2231, 3333.</br>
**opening_hook:** ask what both sides take for granted together.

### Coordinate 2113 — The Split-Frame Synthesizer
**core_pattern:** A divided world of fixed objects, known by conflict, moved by a third term.</br>
**world_assumption:** the divide yields to synthesis.</br>
**object_pattern:** fixed opposed objects to be reconciled.</br>
**knowing_pattern:** split, then find the third term.</br>
**direction_pattern:** synthesize the divide.</br>
**typical_reasoning:** turn the conflict into a higher order.</br>
**strengths:** dialectic.</br>
**blindspots:** the synthesis hides the conflict.</br>
**failure_mode:** false reconciliation.</br>
**questions_it_tends_to_ask:** What synthesis spans this divide?</br>
**what_it_notices:** the bridge latent in conflict.</br>
**what_it_misses:** the irreconcilable core.</br>
**opposite_or_tension_coordinates:** 4444, 2241, 3333.</br>
**opening_hook:** ask what no synthesis can span.

### Coordinate 2114 — The Split-Frame Opener
**core_pattern:** A divided world of fixed objects, opened to find what the split hides.</br>
**world_assumption:** the divide is provisional.</br>
**object_pattern:** fixed opposed objects whose split is suspect.</br>
**knowing_pattern:** split, then open.</br>
**direction_pattern:** open beyond the divide.</br>
**typical_reasoning:** use the split as a probe for the excluded.</br>
**strengths:** finds the residue of the divide.</br>
**blindspots:** the convenience of the split.</br>
**failure_mode:** dissolves into shapelessness.</br>
**questions_it_tends_to_ask:** What do both sides jointly exclude?</br>
**what_it_notices:** the gap between sides.</br>
**what_it_misses:** when the split is real.</br>
**opposite_or_tension_coordinates:** 4441, 2231, 3333.</br>
**opening_hook:** seek what the split cannot name.

### Coordinate 2121 — The Dual-Fixed Framer
**core_pattern:** A divided world of two fixed opposed objects, kept ordered.</br>
**world_assumption:** the two sides are natural.</br>
**object_pattern:** two fixed opposed units.</br>
**knowing_pattern:** split to map.</br>
**direction_pattern:** maintain the dual order.</br>
**typical_reasoning:** hold the two sides stably apart.</br>
**strengths:** clean binary architecture.</br>
**blindspots:** the third element.</br>
**failure_mode:** false binary.</br>
**questions_it_tends_to_ask:** What are the two stable sides?</br>
**what_it_notices:** the binary.</br>
**what_it_misses:** the denied third.</br>
**opposite_or_tension_coordinates:** 4444, 2231, 3333.</br>
**opening_hook:** hunt the denied third.

### Coordinate 2122 — The Fixed Foe
**core_pattern:** A world of two fixed enemies, known by conflict, moved by victory.</br>
**world_assumption:** the enemy is fixed.</br>
**object_pattern:** two fixed opposing units.</br>
**knowing_pattern:** split to the fight.</br>
**direction_pattern:** win.</br>
**typical_reasoning:** optimize for defeating the fixed enemy.</br>
**strengths:** focused adversarial drive.</br>
**blindspots:** the enemy's changeability.</br>
**failure_mode:** fights a fixed picture of a moving enemy.</br>
**questions_it_tends_to_ask:** How do we defeat this fixed enemy?</br>
**what_it_notices:** the enemy's fixed features.</br>
**what_it_misses:** the enemy's transformation.</br>
**opposite_or_tension_coordinates:** 4444, 2231, 3333.</br>
**opening_hook:** ask how the enemy differs from its picture.

### Coordinate 2123 — The Fixed-Foe Synthesizer
**core_pattern:** A world of two fixed enemies, known by conflict, moved by a third term.</br>
**world_assumption:** even fixed foes can be reconciled.</br>
**object_pattern:** two fixed opposing units seeking a whole.</br>
**knowing_pattern:** split, then mediate.</br>
**direction_pattern:** synthesize the foes.</br>
**typical_reasoning:** find the higher interest that unites the foes.</br>
**strengths:** conflict-to-cooperation.</br>
**blindspots:** the irreconcilable.</br>
**failure_mode:** forced peace.</br>
**questions_it_tends_to_ask:** What higher interest unites these foes?</br>
**what_it_notices:** the shared interest.</br>
**what_it_misses:** the genuine incompatibility.</br>
**opposite_or_tension_coordinates:** 4444, 2241, 3333.</br>
**opening_hook:** ask what the foes will never share.

### Coordinate 2124 — The Fixed-Foe Uncloser
**core_pattern:** A world of two fixed enemies, opened to find what the enmity hides.</br>
**world_assumption:** the enmity is provisional.</br>
**object_pattern:** two fixed foes whose fixity is suspect.</br>
**knowing_pattern:** split, then open.</br>
**direction_pattern:** open beyond the enmity.</br>
**typical_reasoning:** use the enmity to expose the shared suppression.</br>
**strengths:** reveals the collusion in conflict.</br>
**blindspots:** genuine opposition.</br>
**failure_mode:** dissolves real stakes.</br>
**questions_it_tends_to_ask:** What do the foes jointly gain from their fight?</br>
**what_it_notices:** the collusion.</br>
**what_it_misses:** authentic conflict.</br>
**opposite_or_tension_coordinates:** 4441, 2231, 3333.</br>
**opening_hook:** ask what the enmity is protecting.

### Coordinate 2131 — The Split-In-Unity Framer
**core_pattern:** A divided world integrated by a center, kept ordered.</br>
**world_assumption:** a center holds the divide.</br>
**object_pattern:** opposed parts under a center.</br>
**knowing_pattern:** map the whole.</br>
**direction_pattern:** maintain the centered order.</br>
**typical_reasoning:** read the divided whole through its center.</br>
**strengths:** governance of conflict.</br>
**blindspots:** the center's own side-taking.</br>
**failure_mode:** the center is secretly one side.</br>
**questions_it_tends_to_ask:** What center holds these sides?</br>
**what_it_notices:** the center's management of the sides.</br>
**what_it_misses:** the center's bias.</br>
**opposite_or_tension_coordinates:** 4444, 2221, 3333.</br>
**opening_hook:** ask which side the center favors.

### Coordinate 2132 — The Center-Contesting Splitter
**core_pattern:** A divided world under a center, known by splitting the center's claim.</br>
**world_assumption:** the center is a side.</br>
**object_pattern:** opposed parts challenging the center.</br>
**knowing_pattern:** split the center's neutrality.</br>
**direction_pattern:** drive the side the center excludes.</br>
**typical_reasoning:** expose the center as a partisan.</br>
**strengths:** challenges false neutrality.</br>
**blindspots:** the center's real function.</br>
**failure_mode:** anti-center for its own sake.</br>
**questions_it_tends_to_ask:** Which side does the center secretly serve?</br>
**what_it_notices:** the center's hidden bias.</br>
**what_it_misses:** the center's necessity.</br>
**opposite_or_tension_coordinates:** 4443, 2211, 3333.</br>
**opening_hook:** ask what the center is actually for.

### Coordinate 2133 — The Conflict-Center Architect
**core_pattern:** A divided world reconciled by a center built from conflict.</br>
**world_assumption:** conflict builds a higher center.</br>
**object_pattern:** opposed parts synthesized into a center.</br>
**knowing_pattern:** split, then build the third term.</br>
**direction_pattern:** synthesize into a center.</br>
**typical_reasoning:** turn the sides into a new unified center.</br>
**strengths:** dialectical architecture.</br>
**blindspots:** the new center's own opposition.</br>
**failure_mode:** the center re-polarizes.</br>
**questions_it_tends_to_ask:** What center does this conflict build?</br>
**what_it_notices:** the order latent in conflict.</br>
**what_it_misses:** the irreconcilable.</br>
**opposite_or_tension_coordinates:** 4444, 2241, 3333.</br>
**opening_hook:** ask what the new center will exclude.

### Coordinate 2134 — The Conflict-Center Breacher
**core_pattern:** A divided world under a center, opened to breach the center's synthesis.</br>
**world_assumption:** the center is provisional.</br>
**object_pattern:** opposed parts whose center admits residue.</br>
**knowing_pattern:** split, then open past the center.</br>
**direction_pattern:** open beyond the center.</br>
**typical_reasoning:** use the conflict to expose the center's limit.</br>
**strengths:** finds the center's blind side.</br>
**blindspots:** the convenience of the conflict.</br>
**failure_mode:** abandons all order.</br>
**questions_it_tends_to_ask:** What does this center's peace hide?</br>
**what_it_notices:** the collusion behind the center.</br>
**what_it_misses:** genuine irreconcilability.</br>
**opposite_or_tension_coordinates:** 4441, 2231, 3333.</br>
**opening_hook:** ask what both sides of the center-synthesis share.

### Coordinate 2141 — The Open-Divided Stabilizer
**core_pattern:** A divided world with an open horizon, kept ordered.</br>
**world_assumption:** the divide is stable; the open is peripheral.</br>
**object_pattern:** opposed objects with open edges.</br>
**knowing_pattern:** map, tolerant of the unmapped.</br>
**direction_pattern:** maintain order near the open.</br>
**typical_reasoning:** keep the divide stable while noting the horizon.</br>
**strengths:** stability with an edge.</br>
**blindspots:** the horizon breaking the divide.</br>
**failure_mode:** tokenizes the open.</br>
**questions_it_tends_to_ask:** What lies beyond this divide?</br>
**what_it_notices:** the horizon at the divide's edge.</br>
**what_it_misses:** the break approaching.</br>
**opposite_or_tension_coordinates:** 4441, 2222, 3333.</br>
**opening_hook:** let the horizon re-order the divide.

### Coordinate 2142 — The Open-Divided Splitter
**core_pattern:** A divided world opening outward, moved by drawing new lines.</br>
**world_assumption:** the open is a frontier to divide.</br>
**object_pattern:** opposed objects meeting an open edge.</br>
**knowing_pattern:** map, then split the frontier.</br>
**direction_pattern:** open and partition.</br>
**typical_reasoning:** expand the divide into the new region.</br>
**strengths:** frontier order.</br>
**blindspots:** the arbitrary new line.</br>
**failure_mode:** partitions the open too soon.</br>
**questions_it_tends_to_ask:** How do we divide the new frontier?</br>
**what_it_notices:** the unclaimed open.</br>
**what_it_misses:** the open that resists division.</br>
**opposite_or_tension_coordinates:** 4441, 2213, 3333.</br>
**opening_hook:** resist partinging the frontier.

### Coordinate 2143 — The Open-Divided Integrator
**core_pattern:** A divided world opening outward, reintegrated by a center.</br>
**world_assumption:** the open folds into the center.</br>
**object_pattern:** opposed parts with an open outside.</br>
**knowing_pattern:** map, open, mediate.</br>
**direction_pattern:** synthesize the new open.</br>
**typical_reasoning:** expand, then fold the frontier into the center.</br>
**strengths:** coherent expansion.</br>
**blindspots:** the center overclaims the open.</br>
**failure_mode:** absorbs the open too fast.</br>
**questions_it_tends_to_ask:** How to fold the new frontier into the whole?</br>
**what_it_notices:** incorporable frontier.</br>
**what_it_misses:** the unincorporable.</br>
**opposite_or_tension_coordinates:** 4442, 2221, 3333.</br>
**opening_hook:** leave part of the frontier unintegrated.

### Coordinate 2144 — The Open-Divided Breacher
**core_pattern:** A divided world whose open is primary.</br>
**world_assumption:** the divide is a stage for the open.</br>
**object_pattern:** opposed objects that are really streaming.</br>
**knowing_pattern:** split, then un-map.</br>
**direction_pattern:** open beyond the divide.</br>
**typical_reasoning:** use the divide to reach what no side holds.</br>
**strengths:** reaches beyond-polar unknowns.</br>
**blindspots:** the convenience of the divide.</br>
**failure_mode:** dissolves all grip.</br>
**questions_it_tends_to_ask:** What do the sides jointly fail to hold open?</br>
**what_it_notices:** the surplus of the divide.</br>
**what_it_misses:** when the divide is real.</br>
**opposite_or_tension_coordinates:** 4441, 2231, 3333.</br>
**opening_hook:** seek what the divide is protecting.

### Coordinate 2211 — The Dual-Nature Mapper
**core_pattern:** A divided world of opposed fixed objects, known by mapping, kept ordered.</br>
**world_assumption:** opposition is the frame.</br>
**object_pattern:** two fixed opposed natures.</br>
**knowing_pattern:** map the opposition.</br>
**direction_pattern:** maintain the dual order.</br>
**typical_reasoning:** read every thing as one of two fixed natures.</br>
**strengths:** sharp typology.</br>
**blindspots:** the nature that is neither.</br>
**failure_mode:** essentialist binary.</br>
**questions_it_tends_to_ask:** Which of the two natures is this?</br>
**what_it_notices:** the two natures.</br>
**what_it_misses:** the hybrid.</br>
**opposite_or_tension_coordinates:** 4444, 2231, 3333.</br>
**opening_hook:** ask about the hybrid nature.

### Coordinate 2212 — The Dual-Nature Fighter
**core_pattern:** A divided world of two fixed natures at war, known by conflict, moved by victory.</br>
**world_assumption:** the two natures are enemies.</br>
**object_pattern:** two fixed opposed natures.</br>
**knowing_pattern:** split to the fight.</br>
**direction_pattern:** win for one nature.</br>
**typical_reasoning:** champion one nature against the other.</br>
**strengths:** committed advocacy.</br>
**blindspots:** the nature's own contingency.</br>
**failure_mode:** essentialist war.</br>
**questions_it_tends_to_ask:** Which nature must win?</br>
**what_it_notices:** nature-defenders and enemies.</br>
**what_it_misses:** the shared nature.</br>
**opposite_or_tension_coordinates:** 4444, 2231, 3333.</br>
**opening_hook:** ask what both natures share.

### Coordinate 2213 — The Dual-Nature Mediator
**core_pattern:** A world of two fixed natures, known by conflict, moved by a higher third.</br>
**world_assumption:** the two natures yield to a synthesis.</br>
**object_pattern:** two fixed natures seeking a whole.</br>
**knowing_pattern:** split, then synthesize.</br>
**direction_pattern:** unify the natures.</br>
**typical_reasoning:** find the principle that contains both natures.</br>
**strengths:** dialectical design.</br>
**blindspots:** the synthesis erases difference.</br>
**failure_mode:** forced unity.</br>
**questions_it_tends_to_ask:** What contains both natures?</br>
**what_it_notices:** the reconciliation.</br>
**what_it_misses:** the uncontainable.</br>
**opposite_or_tension_coordinates:** 4444, 2241, 3333.</br>
**opening_hook:** ask what no principle can contain.

### Coordinate 2214 — The Dual-Nature Breacher
**core_pattern:** A world of two fixed natures, opened to find what the duality hides.</br>
**world_assumption:** the duality is provisional.</br>
**object_pattern:** two fixed natures whose fixity is suspect.</br>
**knowing_pattern:** split, then open.</br>
**direction_pattern:** open beyond the duality.</br>
**typical_reasoning:** use the two natures as probes for the excluded.</br>
**strengths:** finds the residue of essentialism.</br>
**blindspots:** genuine duality.</br>
**failure_mode:** dissolves into shapelessness.</br>
**questions_it_tends_to_ask:** What do both natures jointly exclude?</br>
**what_it_notices:** the gap between natures.</br>
**what_it_misses:** when the duality is real.</br>
**opposite_or_tension_coordinates:** 4441, 2231, 3333.</br>
**opening_hook:** seek what the duality is protecting.</br>

*[Portraits 2221–4444 continue the same deterministic composition; the
PatternComposer rule in §32.2 plus the LevelArchetype table generates every
remaining coordinate. The frozen canonical full set is expanded from this
rule at build time; no runtime model call is used.]*