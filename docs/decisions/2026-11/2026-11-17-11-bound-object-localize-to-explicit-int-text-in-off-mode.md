## 2026-11-17: Bound Object.Localize to explicit INT text in Off mode

AI-assisted (Codex/GPT-6.1 Sol). The implementer used only the independently authored
own-words `Object.Localize` section of `docs/verification/NATIVE_CONTROLLER_HELPERS.md`
as native evidence. No executable analysis or disassembly was consulted. Native behavior
remains `UNVERIFIED` against the running game.

The world pack binds `Object.Localize(string,string,string)` only in Off mode. A caller
supplies an explicit localization backend, its readiness, ordered roots, and known INT
language/options. The slice CLI explicitly selects INT with SHOWMISSINGLOC off and roots
Engine, Gearbox, WillowGame under the install inferred from its cooked directory. No
localization configuration list or global language is represented in that CLI today.
Other callers can supply additional ordered roots. Reference keeps its existing binding
and stub policy, including the unchanged localization entry in its allow list.

The project-owned reader searches roots last to first and matches filenames, sections
and keys with the project's ASCII case-folding convention. UTF-16 decoding follows the
existing item-card reader pattern: BOM selects endianness, otherwise little endian;
invalid encoding is an error. Files are read lazily and cached per root/package. Matching
outer double quotes are stripped, interior spacing and Unicode values are preserved,
and the final string is cut at its first `###`, including placeholders and the explicit
not-ready key result. Missing files/entries continue to lower roots, then return the
note's exact INT placeholder. Missing service is typed `NotYet`, never implied readiness.

Selected escapes, embedded quotes and array keys are unread and remain typed `NotYet`;
unused entries with those forms do not prevent other lookups. Duplicate selected keys,
ambiguous filenames, malformed section boundaries, unmatched selected quotes, corrupt
UTF-16 and I/O failures are errors, with file/OS causes retained in the diagnostic.
Package identities use `PackageName` validation plus Windows filename restrictions;
invalid paths never fall through to a missing-text placeholder. A 16 MiB per-file reader
bound is an implementation limit, reported as typed `NotYet`, not a recovered game limit.
No dependency, extractor, license entry, generic configuration framework or AI guard changed.

Synthetic temporary packages/text cover the bound native and Reference preservation.
Installed lookup checks report counts only; captures stay ignored under
`local/p2/fix-localize-off/`. Gate outcomes are recorded in `docs/verification/GATES.md`.
This is an independent later native, not a fix for the earlier Pawn IAIInterface stop.
There was no host, visual or real-game run, and no slice-loop acceptance claim.
