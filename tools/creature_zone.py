"""Which parameter values a Master_Creature instance hands to the host's zone material (M_OW_NPC_Zone).

AI-assisted (Claude), 2026-11-08. Plain Python (no ``unreal``), so tests/creature_zone_test.py runs it without the editor.
tools/phaselock_target_editor.py calls it for the bullymong and the range dummy, both instances of
``Common_Materials.Creature.Master_Creature``.

Where the rule comes from (docs/verification/PHASELOCK_STOCK_DATA.md, "Master_Creature's colour model", our own words): the
compiled base-pass pixel shader of Master_Creature in the install's shader cache multiplies the light/dark map's red and green
by the scalars ``p_HighlightsIntensity`` and ``p_ShadowsIntensity`` before clamping them, and uses the colour vectors as they
are stored, with no gamma step. The scalars' defaults in the shader's uniform expressions are 2.0; an instance that does not
override them gets 2.0. The host's earlier zone material had no such scale (1.0) and raised the vectors to the power 2.2
(round 23), both host fits. UNVERIFIED against the running game: this is a reading of compiled data, and UE5's lighting
and tone curve are not BL2's.
"""

# Master_Creature's defaults for the two scalars, from its compiled uniform expressions (an instance may override them).
MASTER_CREATURE_SCALARS = {'p_HighlightsIntensity': 2.0, 'p_ShadowsIntensity': 2.0}
ZONE_VECTOR_ENDINGS = ('Hilight', 'Midtone', 'Shadow', 'ColorD')


def zone_parameters(slot, gamma=False):
    """{'vectors': {name: [r, g, b, a]}, 'scalars': {name: value}} for one material slot of an editor job.

    slot: the job's slot dict, with the stock instance's own 'vectors' and 'scalars' (UModel props). gamma=True keeps
    round 23's host fit (each colour channel raised to 2.2) for an A/B capture; the shader reading uses the values as stored.
    """
    vectors = {}
    for name, value in (slot.get('vectors') or {}).items():
        if not (name.startswith('p_') and name.endswith(ZONE_VECTOR_ENDINGS) and value):
            continue
        rgba = [float(v) for v in value]
        if gamma:
            rgba = [max(0.0, c) ** 2.2 for c in rgba[:3]] + rgba[3:]
        vectors[name] = rgba
    scalars = dict(MASTER_CREATURE_SCALARS)
    for name in MASTER_CREATURE_SCALARS:
        value = (slot.get('scalars') or {}).get(name)
        if value is not None:
            scalars[name] = float(value)
    return {'vectors': vectors, 'scalars': scalars}
