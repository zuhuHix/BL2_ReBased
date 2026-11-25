## 2026-11-25: Write-free checks also count class code loads

AI-assisted (Claude Code, Opus 5.5). Lane COUNTERS-MISS-CLASS-CODE-LOADS, a verification-rule change.
No game data and no native reading; synthetic fixtures
only.

- **The gap (found on #682):** loading a cold class's code registers functions and can intern names, so in Off it is
  a write. But it moves none of #518's three counters (heap writes committed, random draws, store load transitions),
  and usually not the name table's length either. A refusal test that asserts "write-free" with those alone does not
  see it.
- **What changes:**
  - `ow-vm`: `Vm::class_code_loads()`, a count that only grows. It goes up once for each class whose functions and
    states are loaded, and once for each package function declared.
  - `ow-world`: `pools::Counters` and `body_composition::Counters` hold it as a fourth field, so tests that compare
    two readings catch a code load. Their runtime use is unchanged: `Counters::moved` and `body_composition::after_writes`
    still read only #518's three, so no runtime refusal gains a "stopped after partial writes" prefix from a code
    load. Whether it should is a separate ruling, left to the maintainer.
  - **Tests:** every test-side counter reading on a VM's store adds the code loads: 68 sites in 34 files, each
    `defs.load_transitions()` becoming `defs.load_transitions() + class_code_loads()`. ow-attr's tests have no VM and
    are unchanged.
- **What it caught:** one existing test. `ow-inventory` `equip::particle_effect`
  `the_effect_handle_is_returned_unloaded_and_nothing_is_written_or_loaded`: on its fixture, `GetParticleEffect`'s
  evaluator dispatch loads `GearboxFramework.ExpressionEvaluator`'s and `GearboxFramework.Object`'s code (three code
  loads). That test keeps #518's three. A new ignored test, `the_effect_lookup_loads_no_class_code`, pins the
  finding for the hub to route; it is not fixed here.
- **Its limit:** the check only sees loads a fixture leaves cold. Most fixtures look their classes up with
  `find_class`, which loads their code first, so a native that would load cold code in the game's Off world can
  still pass. #682's queue test needed a fixture that never warms the class. A test that means "loads no code" should
  use such a fixture.
- **Verified (host only):** a new `ow-vm` test shows a cold class's load moving the counter while #518's three stay
  put, and a read-only lookup or a second lookup not moving it. The workspace suite passes, apart from the 4
  expected Linux N1 tests and the one ignored finding. Nothing here touches the running game.
