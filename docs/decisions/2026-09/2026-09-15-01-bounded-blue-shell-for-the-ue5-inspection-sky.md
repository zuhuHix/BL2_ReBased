## 2026-09-15: bounded blue shell for the UE5 inspection sky

The recovered Sanctuary scene still rendered a brown or black upper field when
the temporary atmosphere was the only host fallback. The importer now adds a
centred, two-sided, reverse-culled UE5 sphere with a host-created Unlit blue
constant material, no collision, and no shadow casting. It is labelled
`OpenWillow_SkyFallback`, checked by the saved-scene verifier, and paired with
the existing `OpenWillow_SkyAtmosphere` actor. This keeps the inspection view
readable without claiming recovery of the native sky graph, cloud layers,
time-of-day controls, or lighting parity. The broad lower white regions remain
the separately observed `IcePlate` geometry and were not reclassified as sky.
See docs/verification/SKY_FALLBACK_VERIFICATION.md.

