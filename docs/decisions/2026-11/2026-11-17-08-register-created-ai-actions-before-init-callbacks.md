## 2026-11-17: Register created AI actions before Init callbacks

AI-assisted (Codex/GPT-6.1 Sol). PR #374 follow-up to decision 2026-11-17-05;
all published decisions and the own-words NATIVE_AI_ACTION_SEQUENCE.md note remain unchanged.
No raw analysis or new native evidence was consulted. Running-game behaviour remains **UNVERIFIED**.

The adapter previously registered ownership only after every Init callback returned. During Init,
IsRunning therefore treated a newly copied action with null AI as known unowned and answered false,
even though the port has not represented its AI setup. Register all created actions before invoking
any Init callback and release the service borrow before script executes. This includes sibling
actions whose own Init has not run. Owned actions keep the typed NotYet for unrepresented setup;
no AI interface, component or current-state link is invented. Reference natives remain unbound.

A synthetic script Init callback queries IsRunning on itself and its sibling. Both callbacks must
see both actions and get NotYet / NotRead, with their AI fields still null. The affected suite passes
57 tests with 0 failures and 2 installed-game tests ignored; affected Clippy passes with warnings denied.
No host, window, GPU or real-game session was run; this fixture establishes Rust behaviour only.

The workspace build passes, and workspace tests pass 1787 / 0 failed / 29 ignored. Crate-edge checks
pass 19 tests; Fire and combat checker self-tests pass. Builds use cached dev/test debug=0,
two jobs and BelowNormal priority under the machine build lock. C++ inputs match the covered
parent with CI 173/173 LZO on and 171/171 off; these are reused coverage, not fresh local results.
There is no local C++ reader build, so ctest and installed package-reader checks were not rerun.
