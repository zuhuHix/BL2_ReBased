## 2026-09-27: Skills visual states and motion from the installed movie

The maintainer's original-game reference shows rank badges, stronger depth
between the selected and side branches, a green action-skill frame, class-mod
text when equipped, richer grade descriptions, and contextual footer text.
The converted movie already contains `SkillTreeCellController.SetState` frames
for disabled, enabled, partly invested and maxed skills (with separate kill
skill frames), plus `SetCharacter`, `TweenBranch`, the sway clip and tooltip
text. Browser probes against the local movie confirmed these methods and the
resulting colours and rank badges. No movie art was copied into the repo.

The overlay now drives those frames from a grade map, displays Phaselock as
1/1 in the current UE slice, uses the movie's imported full WillowBody font
for the badge slash, starts sway, tweens the initial branch layout, and shows
the controls that actually work. Ruffle did not apply the traced Z depth to
the side branches in the browser check, so their 2D scale and positions are
adjusted in the page. A host supplied class-mod label can appear through the
movie, but the current prototype has no class mod equipped or represented.
Current branch grades and available points remain zero. The original-game
capture's invested grades and calculated current/next grade stats cannot be
claimed for this host yet; they require actual skill state and attribute
evaluation. Pointer hover animation is an overlay effect because native
Ruffle rollover callbacks did not fire in the browser probe.

Runtime check: UE 5.8 opened the updated menu in a Sanctuary game window and
captured `OWCombat_8_Skills.png` under ignored UE `Saved/Screenshots`. The
image shows the 1/1 action badge, 0/5 first-tier badges, dimmed deeper tiers,
the receded side branches and corrected footer. The movie logged ready about
9.6 seconds after the overlay opened in this run; the scripted page close
restored the game view. This checks rendering and the close route, not manual
mouse interaction or skill spending.

The first runtime capture still showed gameplay health, ammo, level and
minimap HUD behind Skills. The host now hides both its Flash HUD viewport
widget and native HUD drawing while Skills is open and restores them on close;
the subsequent `OWCombat_8_Skills.png` capture shows none of those HUD elements,
and `OWCombat_9_AfterSkills.png` shows them restored after the page close route.

How the interface was learned, and what it rests on:
- `tools/hud_harness_swf.py` gained four ops besides get/set/call: `apply`
  (call with an argument array), `keys` (member names), `unhide`
  (`ASSetPropFlags(target, null, 0, 1)`, bench inspection only, to list class
  methods) and `forward` (installs a clip function that relays to the page
  through ExternalInterface, the page's stand-in for Scaleform's
  `SetFunction`). Ruffle ends `Enumerate2` with undefined rather than the
  spec's null, so `keys` compares with `==`.
- Method names and parameter lists (for example `SetCellVisible(BranchNum,
  TierNum, CellNum)`, `TweenBranch(BranchNum, bImmediate, TweenDuration, XPos,
  YPos, ZPos, XScale, YScale, Alpha)`) come from a local signature listing of
  the converted StatusMenu (`local/ui/as2_signatures.py`, output kept under
  `local/` per docs/LEGAL.md). No function bodies were transcribed. Cell
  states are the cell sprite's frame labels.
- The movie calls `extCellClicked(branch, tier, cell)` itself on mouse
  release (trace seq 25355), so the game does not hit-test cells.
- `tools/prepare_skill_tree.py` places cells by tier size (1 skill: column 1;
  2: columns 0 and 2; 3: all). All 28 tree cells hovered in the two traces
  match; `bCellIsOccupied` is a bool array that `ow-package` does not decode
  yet, and extending the array decoder is a sensitive-area change not made
  here. Traced branch layouts fit X = 15 + 330d, Z = -5500|d|, alpha =
  100 - 15|d| for branch offset d (two observations; UNVERIFIED beyond them).

