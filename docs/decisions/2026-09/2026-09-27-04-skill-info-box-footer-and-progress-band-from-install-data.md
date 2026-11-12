## 2026-09-27: Skill info box, footer and progress band from install data

Toward a 1:1 Skills tab. Everything below is decoded from the install or
observed in the local real-game UI trace; no Gearbox script logic was read.

Stat text. `SkillDefinition.SkillEffectDefinitions` (struct array) and
`SkillEffectPresentations` (object array) decode with the reader's existing
`--array-schema` option; no reader change was needed. An
`AttributePresentationDefinition` holds the label ("Melee Damage: $NUMBER$")
and number flags. The install's `GD_Siren_Skills.int` overrides the English
strings, under keys like `[Cataclysm.Immolate:AttributePresentationDefinition_0
AttributePresentationDefinition]`. `tools/skill_stats.py` implements the rules
fitted to the traced text. Effects scale as base + per-grade *
floor((grade - start) / interval). SignStyle, bDontDisplayPlusSign,
bDisplayAsPercentage (default true) and float rounding are handled. A line
without custom placement is "<number> <text>". Designer attributes resolve to
their own BaseValue (Recompense 10%), and constant resolvers are looked up
across packages (Phaselock cooldown 13 s from Startup.upk).
`tools/hud_overlay/skill_info.js` builds the info HTML as traced: description,
current block (numbers in one colour, another once trained to max), a "Next Level:"
block while below max, everything wrapped in a grey while the tier is
locked, and the cyan class-mod note. Traced values: `local/gunlook/measured_colours.md`, ignored.

Checks against the real game (local, not committed, because they read the
trace): all 63 distinct SetInfo texts in the two traces are reproduced byte
for byte for some (grade, locked, bonus). That includes eight with class-mod
notes and the lock and max colours. Only 16 of Maya's 31 skills appear in the
traces. The stats of the other 15 follow the same rules but are UNVERIFIED,
notably Converge (custom placement without $NUMBER$ shows no number) and
Thoughtlock. The trace's grey Mind's Eye text before Phaselock was bought,
with 41 points available, is direct evidence for the host's
action-skill-first gate. From the maintainer's capture (Ward +30% at 2 + 4
class-mod grades, shown orange), the maxed colour follows the trained grade;
no traced text covered that. Class-mod bonus grades are supported in the page
(`owSkills({bonuses})`), but the host has no class mods, so they are always 0.

Footer. `[SkillTreeGFxObject]` in `WillowGame.int` has Tooltips_SpendPoints,
Tooltips_Overview and Tooltips_Cancel. `<StringAliasMap:GFx_*>` tokens resolve
through `DefaultGame.ini` MenuInputMapArray (Set="PC") to
`GameMappedStrings`: "[Enter] Spend Point", "[Q] Toggle Overview", "[Escape]
Close". The page shows Spend Point only when the selected skill can take a
point. Toggle Overview is left out until the overview mode exists; the traces
contain no overview use. `Action.ActionSkill`/`Action.Melee` come from the
player's bindings, not the install. F matches the traced Phaselock text; V is
the default melee key (UNVERIFIED).

Selection. In the trace, rolling over a cell sends its highlight clip to
"over" ("over_KillSkill" for kill skills) and tweens the cell to Z 200 over
0.2 s; the previous one returns to "up" and Z 0. The screen opens with
Phaselock selected. The highlight clips are `SkillRowT.HighlightC` and
`ActiveAbility.highlight`. The page now does the same and drops its CSS hover
glow. Ruffle ignores Z, so the lift itself is not visible.

Progress band. The movie's `SetBranchProgression(branch, frame)` takes a
0-based branch and sends that tree's ProgressBackground (1029 frames; 1029
wraps to 1) to the frame. The traced calls were not recorded in detail
(per-function record cap), so the mapping is measured. The lit band grows
about 0.43 px per frame, and tier-row bottoms fall at frames 173, 333, 490,
646, 815, 962. The maintainer's capture has exactly 10 trained points in
Harmony and the band at the bottom of tier 3. The page therefore puts the band
at the bottom of the deepest open tier (frame 1 while the trees are closed)
and fills part of the way toward the next tier in between (UNVERIFIED).

Rendering. The info box's embedded font is a subset without ' : + %; Scaleform
falls back to the imported font library and Ruffle does not. The page wraps
the info HTML, and re-sets the SkillName field, with the imported $WillowBody
alias. Ruffle drops an italic capital at one line wrap ("ife Orbs" in Sweet
Release); cause not investigated.

Automated: `tests/skill_stats_test.py` (4) and `tests/skill_info_test.js` (9
cases), synthetic, not registered with ctest (that needs a CMakeLists.txt
change). ctest's six scripts, `tests/gfx_to_swf_test.py`,
`tests/weapon_stats_test.py` and `tools/verify_packages.py` pass, run with the
main checkout's 2026-09-22 `ow-package.exe` as before (unchanged `src/`).
Visual: headless Edge at 1280x720 in the maintainer's capture state (Phaselock
1/1, Mind's Eye 5/5, Wreck 5/5) shows the movie highlight, green stats and
the band at tier 3; the locked and 2/5 states render as traced. Not checked in
UE 5.8 (no C++ change in this step).

