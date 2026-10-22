# Skills-page keyboard navigation

## Script evidence

`StatusMenuExGFxMovie.HandleSkillsInputKey` forwards Skills-page input to
`SkillTreeGFxObject.HandleInputKey`. The handler maps keyboard arrows and D-pad
directions to `Nav_Up`, `Nav_Down`, `Nav_Left` and `Nav_Right`; these inputs are
ignored while overview mode is active. `Nav_Up` and `Nav_Down` pass direction
values 0 and 1 to the native `SkillTreeGFxObject.Navigate`, then call `PostNav`
with the previous selection. The horizontal methods do the same with values 2
and 3. `PostNav` updates the highlighted cell and information panel, and adjusts
tree depth and positions if the selected cell belongs to another branch.

The `Navigate` function is native, so the package listing has no selection
algorithm. A local read-only Ghidra query resolved its registration to a shared
native dispatch thunk; it did not recover the grid policy. No decompiler output
is included here.

## Host fit and verification

The matched 2026-10-21 observation starts on the action skill in the middle
branch; four Down presses end on that branch's fourth regular tier, center
column. Comparing only the position with the ignored local tree data identifies
the destination as zero-based tier 3, cell 1. The game-specific tile label and
the source data are omitted from tracked files.

`skill_nav.js` moves one adjacent tier at a time, choosing the closest occupied
cell column. It treats the action skill as centered at column 1, breaks equal
distances toward the lower-numbered cell, returns to the action skill when Up
leaves tier 1, and stops at missing rows and the top and bottom edges. The
four-Down destination is observed; the column tie-break and edge rules are
`UNVERIFIED` because the native chooser has not been recovered or captured at
those cases.

The bytecode confirms that Left and Right also call the native chooser, but the
native policy for moving between cells and changing branches is not recovered.
The host retains its existing Left/Right tree-step behavior; horizontal parity
remains `UNVERIFIED`.

### D20 capture review (2026-10-22)

The real-game D6b sequence exits Q overview, waits 1.5 s, then sends four Down
presses at 0.5 s intervals. In the real capture this returns focus to the action
skill on Q, and the four Down presses reach the middle branch's tier index 3,
cell 1. This
confirms the existing one-tier-per-press fit for this path; it does not verify
ties or other tree edges.

The D20 host log records a `move` TypeError on each of the four Down events.
`skills.html` loaded `skills.js`, but `serve.py` returned HTTP 404 for the new
`/skill_nav.js` URL because its page allowlist does not include that file. The
page now defines the browser helper inline; `skill_nav.js` remains its Node-test
counterpart. The page's `toggleOverview()` already reselects the action skill
when Q exits overview. The 1.5 s delay exceeds the 0.3 s branch tween, so the
capture table needs no timing change.

Frame measurements at 1280×720: the host's incorrect skill card is about
x=204–527, y=140–339; the real D6 card is about x=212–526, y=149–389, and
the real D6b card is about x=209–526, y=146–386. The host width/anchor were
fit to the D6b bounds (x≈209, y≈146) with a 315 px requested width. Its shorter
height comes from the wrong selection and
two fewer body lines (real line pitch is about 21–22 px); the host's current
16 px body size and -3 leading produce about the same pitch in the shorter
text, so no typography change was supported by this pair. Correct wrap and full
card height still need a new host capture after the Skills page reloads.

D7 overview's left tree begins about x=242,y=177 in the host and x=251,y=185
in the real frame. An overview-only +9,+7 px offset was applied. In D6/D6b the
selected middle-branch board is already within a few pixels of the real capture, so
the selected-tree transform was left unchanged.

The host Skills frame also shows the movie's `ring` clip as a blue rectangle
with a hard right edge near x=935. The clip is hidden on the Skills page to
remove that edge; the curved glass sheet remains unrendered because its game
blend is unavailable over the transparent host viewport. The layout offsets
and placement fits remain `UNVERIFIED` until compared in a fresh host capture.

`tests/skill_navigation_test.js` uses synthetic tiers and checks the observed
four-step destination shape, column matching, tie handling and limits. It also
loads the inline browser helper and compares it with the tested module.
