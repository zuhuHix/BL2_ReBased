## 2026-11-06: Mind's Eye crit checked on a range enemy (Skills lane, after range landed)

AI-assisted (Claude). The range suite gains `crit_minds_eye`: a gun hit on an enemy's head through
`AOpenWillowWalker::ApplyShotDamage` does 2.0 x its damage untrained and 2.5 x at Mind's Eye 5; progression is restored in the
same tick. It may SKIP without the local skill effects file. This checks the host's rule (the event's bonus scaled by the
skill), not the game's crit damage, which stays UNVERIFIED until the live session. Test only; no behaviour change.

