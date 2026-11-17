## 2026-11-17: Attribute resolver review fixes

AI-assisted (Codex/GPT-6.1 Sol). PR #381 follow-up from the recorded own-words notes only;
no raw analysis or new native evidence. All runtime behaviour remains **UNVERIFIED** in the running game.
The published decision 2026-11-16-80 remains unchanged; this record corrects its claim that one
Constraint implementation serves both modes for every input.

- Currency: the VM attribute bridge refuses Off golden-key reads with typed NotYet / NotRead until
  the owner controller key count required by NATIVE_CONTROLLER_HELPERS.md is represented. It must
  not expose the combat native's Reference zero stand-in as a known count. Reference is unchanged.
- Random: persist the ordered expired-entry cleanup before evaluating bounds, then append to the
  current saved array. A bound may evaluate the same resolver for another context; its saved entry
  must survive the outer append. Reference is unchanged.
- Constraint: Off uses the minimum's at-or-below comparison followed by the maximum's above
  comparison from NATIVE_ATTR_RESOLVERS.md section 5. A NaN running value remains unordered and
  unchanged. Reference keeps its existing float max/min stand-in.

Synthetic regressions cover the VM bridge with the combat native bound, nested Random bound
reads for distinct contexts with an expired entry, reuse without more draws, and NaN through
both enabled bounds separately and together. After the explicit compile-slot release, affected Rust suites passed 514 / 0 failed / 4 ignored;
the workspace build passed, and workspace tests passed 1814 / 0 failed / 36 ignored, using cached
dev/test debug=0, two jobs and BelowNormal priority under the machine build lock. Crate-edge
checks passed 19 tests; Fire and combat checker self-tests passed. C++ inputs match the covered
parent, whose CI passed 173/173 with LZO on and 171/171 off; these are reused coverage, not new
local results. No local C++ build exists, so ctest and the installed package-reader check were
not rerun. No installed-game or host run is claimed.
