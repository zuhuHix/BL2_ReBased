## What and why

<!-- One paragraph. One observable capability per PR. -->

## Automated checks

<!-- Paste the output. -->

```text
ctest --test-dir build -C Release --output-on-failure
python tests/level_test.py
```

## Checks against an installed game

<!-- Reader changes: tools/verify_packages.py. Census/property/asset/level
     changes: the relevant tool's numbers against your install. Or "n/a". -->

```text
```

## In-game / visual checks

<!-- Reported separately from the above. Or "none". -->

## Decisions and provenance

- [ ] Parsing behaviour changed → dated entry added to `DECISIONS.md` stating what is verified and what is `UNVERIFIED`
- [ ] Reference implementation consulted → recorded in `THIRD_PARTY.md` with its license (read, not copied)
- [ ] Touches a sensitive area (`src/package.cpp`, `src/container.*`, struct/array/texture/mesh/material serialization, `CMakeLists.txt`, `THIRD_PARTY.md`, `LICENSE*`) → discussed in an issue first

## Certification

- [ ] No game files or game-derived data are included; fixtures are synthetic
- [ ] No leaked source or decompiled executable code was consulted
- [ ] I have read `docs/LEGAL.md` and the contributor certification holds for this change
