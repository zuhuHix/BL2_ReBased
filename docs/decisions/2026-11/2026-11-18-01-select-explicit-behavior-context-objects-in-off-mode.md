## 2026-11-18: Select explicit behavior context objects in Off mode

AI-assisted (Codex / GPT-6.1 Sol).

The Off runner omitted the behavior's explicit `Context.ContextObject` when there was no Context-type variable link.
Use the existing published own-words rules in `docs/verification/NATIVE_BEHAVIOR_CONTEXT.md` lines 100-130 and
`docs/verification/NATIVE_BEHAVIOR_POPULATION.md` G2 lines 441-455: a Context link supplies the entire list, including
an empty list; otherwise a non-null explicit object supplies one context; otherwise the consumer supplies one context.
The selector byte remains the behavior's concern. This implementation used no raw analysis or new external tool.

`Provider::contexts_for` reads the current VM property at application time and checks selected object handles through
the existing generational heap. Heap failures remain typed VM heap errors; malformed property values are decode
errors. The Reference path retains the prior link-or-consumer selection. Library callers use the checked selection;
input binding, cleanup, latent execution, output ordering, native bindings and the AIHold refusal are unchanged.

Synthetic fixtures cover the explicit object with selector Self, Reference's consumer selection, populated and empty
Context-link precedence, null and absent properties, and stale consumer, behavior, linked and explicit handles.
Normal collection clears references; the stale explicit fixture deliberately injects an unchecked stored value after
collection. The corrected regression run before the implementation was 2 passed / 3 failed. Earlier fixture mistakes
and their failed attempts remain in ignored local evidence.

Native and gameplay behavior remain UNVERIFIED: no running-game, host, visual or GPU validation was performed.
This correction does not establish Pawn-to-mind forwarding or interface membership, and whether it affects the
installed Off slice's Pawn routing refusal remains unknown. Gate results are recorded at EOF in
`docs/verification/GATES.md`; private captures stay under ignored `local/p2/fix-behavior-context-object/`.
