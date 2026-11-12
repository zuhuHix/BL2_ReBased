## 2026-10-06: script swap 6a: the use key chooses Marcus with the stock use ray (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1 from NATIVE_USE_INTERACTION.md only. The host's 250 cm reach stand-in is replaced by one
ray from the camera along the view, `GlobalsDefinition.PlayerInteractionDistance` long (read from the installed data by the VM
bridge: 350 uu), first usable hit before a blocking hit, no radius or cone. Marcus's mesh has no collision, so the host adds a
query-only capsule from the mesh bounds that blocks only the visibility channel (host-chosen shape, UNVERIFIED). Not modelled: the
1/30 s throttle, blocking flags, icons and prompts. The use key still accepts and turns in through the slice's direct calls; 6b
(Marcus's `OnUsed` chain as script) waits for a note on the native `Behavior_IsSequenceEnabled`, which opens his chain. Checks:
CTest 11/11, packages 9/9, UE build Succeeded, quest PASS 86/86 (three new use-ray checks) and resume 11/11, door PASS 16/16.

