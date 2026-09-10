# OpenWillow

An AI-assisted Borderlands 2 engine reimplementation experiment. The intended
runtime reads the player's own installed game. No game assets or Gearbox code
are distributed here. **There is no playable engine or renderer yet.**

## First working slice

A standalone x64 C++20 tool reads version 832/46 package name/import/export
tables and prints their counts as JSON. An optional miniLZO research build reads
fully compressed code packages directly. It does not yet retain an object graph,
read properties, handle partially compressed packages, or load maps.
Python is used for tests and an independent execution path for comparison;
the enabled executable itself does not require Python.

Requirements: CMake, Visual Studio 2022 C++ build tools, Python 3.

```powershell
cmake -S . -B build -G "Visual Studio 17 2022" -A x64 -DOPENWILLOW_RESEARCH_LZO=ON
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure
python tools/verify_packages.py --reader build/Release/ow-package.exe
& ./build/Release/ow-package.exe "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2/WillowGame/CookedPCConsole/Core.upk"
```

The first configure downloads a hash-pinned miniLZO 2.10 source archive.
This optional dependency is GPL-2.0-or-later; see [THIRD_PARTY.md](THIRD_PARTY.md).
For a dependency-free build use `-DOPENWILLOW_RESEARCH_LZO=OFF`; that build
accepts only decompressed packages and cannot run the direct-load comparison.

For a different install, add `--cooked "D:/path/Borderlands 2/WillowGame/CookedPCConsole"`
to the verification command. It only reads installed packages; temporary
decompressed files are removed when the check exits.

Verified locally on 2026-09-10: Release build, synthetic malformed-input tests,
and byte-for-byte decompression plus matching name/import/export counts for all
nine installed code packages. Core yields
234,397 decompressed bytes and 1,621 exports; WillowGame yields 56,443 exports.
These checks establish agreement with the research reader, not independent
proof of every format field or gameplay compatibility. Synthetic tests cover
truncation, block totals, output limits, corrupt LZO streams, multiple blocks,
and exact-block boundaries. Decoded containers are capped at 512 MiB for now.

## Project rules

- Never distribute game files, asset dumps, or proprietary code. Fixtures must be synthetic.
- Do not use leaked source or transcribe decompiled executable code.
- Use observed behavior and documented formats; record reference provenance.
- Check licenses before incorporating reference implementations.
- The intended runtime requires the original installed game.
- No paid builds or premium features; any donations support engine development.
- Disclose AI assistance and keep verification evidence honest.

License selection is pending a provenance review; no project-wide open-source
license is granted yet. See [DECISIONS.md](DECISIONS.md) and
[OPENWILLOW_ENGINE_PLAN.md](OPENWILLOW_ENGINE_PLAN.md).

Next slice: retain names and object records, then decode tagged properties and
expand coverage beyond the nine code packages toward the asset census.
