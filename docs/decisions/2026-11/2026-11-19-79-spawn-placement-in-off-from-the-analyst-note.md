## 2026-11-19: The spawn's placement in Off from the analyst note

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SPAWN-PLACEMENT, implementer. This lane coded the spawn's spot search
and encroachment rules from `docs/verification/NATIVE_SPAWN_PLACEMENT.md` alone. That is analyst lane
ANALYST-SPAWN-PLACEMENT, decision 2026-11-19-77, PR #459, at its final head 1a83fcbd. This lane opened no
analysis output, Ghidra or REA. There was no real-game session: every rule stays **UNVERIFIED** in the running game.
No dependency and no crate edge was added. No bounds check was loosened. Reference is unchanged.

- **`ow-world` `placement.rs` (new), Off only, wired into `actors::spawn`.**
  - **The collision service** (note section 7): a `SpawnCollision` trait with three queries, installed as a VM
    service. The queries are a box test in template or simple mode, a box sweep, and a point sweep. Each answers
    Clear, Hit (a sweep with its location, normal and fraction), or Unsupported. An answer needs the shape, and the
    geometry over the whole region the query touches (the full box, or the full swept volume) in the requested mode;
    anything less is Unsupported, so an empty candidate set is Clear only over full coverage.
  - **Precedence** (note section 6):
    1. `bNoCollisionFail` skips the search.
    2. The search runs on the template's flags alone. The template is the archetype, else the class default; the old
       either-of-the-two pre-check stays for Reference only.
    3. No service is `NotYet { LayerAbsent }`, before the extent question.
    4. No cylinder extent is `NotYet { NotRead }`.
    5. **The trace filter** (note section 7): every box test and every sweep depends on an unread engine flag set
       (8902 and 8326). Until it is read, each answers `NotYet { NotRead }`, "the spawn spot search's trace filter
       (flag sets 8902 and 8326)", before the service is asked. No Off filter is defined. So past the service and
       extent checks, the first test stops every search there.
    6. The search's steps behind the filter (note section 3) are coded and unit-tested against a scripted service
       with the filter taken as read, but not reached in Off: the first test, the zero-extent failure, the slice
       step with its floor sweep and common tail, the corner nudges in the note's order (the first two free corners
       counted, one doubled), the point sweep, the box test with the slice correction, and the final sweep. There the
       first Unsupported query would be `NotYet { NotRead }` naming it, a failed search None, a successful one the
       moved location. The lane that reads the filter replaces the gate (`placement::FILTER_READ`).
  - **After `PreBeginPlay`:** the eager `bCollideActors` refusal is dropped in Off. After the destroyed check, the
    live actor's flag decides, so `PreBeginPlay` can change it:
    - false passes. This is the port's Off answer, kept labelled UNVERIFIED: the spawn routine calls the check
      whatever the flag;
    - true is the encroachment check: `NotYet { LayerAbsent }` without a service, `NotYet { NotRead }` with one;
    - true with `bNoCollisionFail` is the relocation step, `NotYet { NotRead }`.

    Each is terminal, prefixed "stopped after partial writes", its kind kept: the actor was made.
- **Review r1 of #463:**
  - Every Off error after the actor joins the level list goes through the partial-write rule: a `NotYet` is terminal
    and prefixed, its kind kept, the actor stays; other errors propagate unchanged. That covers the owner change,
    each begin-play event and the final base check.
  - The Off placement check runs before the base precheck, both before anything is made.
  - A borrowed collision service is `VmError::ServiceBorrowed`, not an absent service.
  - The Reference test installs a service and a cylinder, so it tells Reference's legacy refusal from Off's filter
    `NotYet`.
- **The collision service in the tree:** none is installed. H4's `CollisionWorld` (`ow-host`, which already depends on
  `ow-world`) could implement the trait with no new crate edge. It would answer box queries Unsupported (it has no box
  shape), and template-mode tests for a `bCollideComplex` template too. Link 9 runs on its own world with no level
  geometry. The hub's ruling (2026-11-19): no service is wired for link 9 now, since with the filter unread it would
  only move the stop to the filter `NotYet`. Wiring it follows the filter lane (decision 2026-11-19-84).
- **Tests changed:** two existing Off spawn tests now expect a class-default `bCollideActors` actor to be made and
  then refused with the prefixed `NotYet`, where they expected nothing made: the note's replacement of the eager guard.
- **Verified (automated, local, pc-hummer):**
  - 6 spawn tests on the toy package: no service before the extent question, no cylinder, the filter `NotYet` with
    the service unasked, template-only flags, the encroachment and relocation `NotYet`s on the live actor, and
    Reference unchanged;
  - 7 unit tests of the search's steps with the filter taken as read: the unread filter, a clear first test, an
    Unsupported query named, a zero extent, the slice's two floor rules, a failed slice, and the single-corner
    doubling;
  - the workspace tests and crate edges (numbers in GATES.md).
- **Installed, Off:** `--slice-gate` strict 0/14/0. Link 9 is 0 of 1 driver steps and still stops at `spawn`, on the
  spot search's `LayerAbsent`: the slice world installs no service.
- **UNVERIFIED:** every rule above, the engine flag sets of the sweeps, and the shortcut for an actor that does not
  collide with actors.
