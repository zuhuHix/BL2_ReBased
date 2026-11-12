## 2026-11-06: Generated guns take their firing block from templates

AI-assisted (Claude). Rolled guns had the refire rule but fired a plain trace, so a rolled launcher launched nothing. Rather than port the resolver to C++ and keep two copies, `tools/weapon_gen_firing.py` runs `tools/weapon_firing.py` on rolls of every balance and writes one template per weapon type, firing mode and element to ignored `local/weapons/gen/weapon_gen_firing.json`. `OpenWillowWeaponGen::ToItem` fills it from the card and calls the parser recipes use. The card JSON gains `speed_multiplier`. **New local data step:** run the tool on each machine; without its file rolled guns keep the plain trace, the host warns, and the firing test fails naming the tool.

Evidence: 249 balances, 2 stages, 3 seeds gave 1494 rolls and 236 templates. The tool merges every roll back and stops before writing on any difference outside the pool; it found 0. That checks the Python merge rule, not the C++ one. Inside a key only the accuracy pool, the speed multiplier and damage-scaled explosions varied, beyond the fields the card supplies.

`UNVERIFIED`: the accuracy pool is the type's base (parts not applied, and they changed it in 169 of 236 keys); no spin-down; a key the sampled rolls never reached keeps the plain trace with a warning; nothing compared with a rolled gun in the running game. A passing host suite will mean the host did this, not parity.

