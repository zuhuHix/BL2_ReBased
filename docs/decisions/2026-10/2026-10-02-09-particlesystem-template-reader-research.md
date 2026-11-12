## 2026-10-02: ParticleSystem template reader (research)

AI-assisted. Research prototype only (`research/particle_system.py`, `tests/particle_system_test.py`, 16 synthetic
tests; not registered in CTest). Templates are delta-serialized against their archetype chain; baked distribution
tables are read as two range values plus entries of `ChunkSize` floats; `BurstList` and dynamic parameters are tagged
structs. All 17,506 non-empty baked tables in four packages fit that layout (structural oracle). Curve sampling between
table entries is fitted, UNVERIFIED. Record: `docs/verification/PHASELOCK_STOCK_DATA.md`, "Particle template reader".

