## 2026-11-06: skills live capture: verified backup and restored grades before the next run (Skills lane, #116 review)

AI-assisted (Claude). Tool safety only; no game behaviour. Every root is copied and hash-compared before the driver or the game starts; any
failure stops the run there and that root is never restored from. A root created during the run stays in place and makes the summary read
NOT IDENTICAL, naming it. A grade whose original cannot be read is not changed, a restore is read back, and a failed restore stops the later
steps. The capture also reads the running skills (`SkillEffectManager.ActiveSkills`), since a tree grade back at 0 can leave a skill running:
it deactivates that skill, a disagreement counts as not restored, and each step records whether another step's skill could be in its reading.
The session counts as complete, and exits 0, only when every requested step passed with no such doubt and cleanup is clean.
`python tests/skills_live_probe_test.py`. This is the smallest guard: the snapshot helpers from the Maya lane can replace it once they land.
