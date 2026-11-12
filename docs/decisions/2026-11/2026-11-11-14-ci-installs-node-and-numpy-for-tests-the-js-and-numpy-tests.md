## 2026-11-11: CI installs Node and numpy for tests; the JS and numpy tests run in ctest (lane ci-test-deps)

AI-assisted (Claude Opus 5.5). CI and test registration only: no tool, parser or host behaviour changes. The maintainer approved Node and numpy as test-only CI tools on 2026-11-11.

- **Why.** The 18 `tests/*_test.js` and the 8 Python tests that import numpy ran only by hand, so a break in them was found by accident.
- **What.** `.github/workflows/ci.yml` installs Node 26 (`actions/setup-node@v6`, no package-manager cache) and `numpy==2.5.4` in the existing test job. `CMakeLists.txt` registers `OW_NODE_TESTS` as `js-<name>` and `OW_NUMPY_PY_TESTS` as `py-<name>`, each group only when configure finds Node, or a Python that imports numpy, and with a configure message when it does not. A new CI step prints each of these entries' last lines from ctest's log, so the cases run on CI can be compared with a local run.
- **Versions.** Node 26 is the current LTS line; this PC runs 26.4. numpy 2.5.4 is what this PC has, and PyPI has a Python 3.12 Windows wheel for it.
- **Verified (automated, zuhu desktop, Python 3.14.6, Node 26.4):** ctest 145/145 (119 before), the Node group 1.8 s and the numpy group 2.5 s. Configured with a numpy-less Python and no Node: 119 entries and both messages.
- **Not verified here:** the same versions on Python 3.12; the PR's CI run is that check.
- **Pillow (commit 2, lands only with the maintainer's yes).** CI also installs `pillow==12.2.0` (this PC's version; PyPI has a Python 3.12 Windows wheel), and the 8 tests in `OW_PILLOW_PY_TESTS` run as `py-<name>` where Pillow and numpy both import (`sanctuary_pillar` needs both). Here: 8/8 in 3.3 s, ctest 153. Dropping this commit leaves the Node and numpy groups as they were.


