"""Which parameter values a target's material instance hands to the host's zone material (M_OW_NPC_Zone).

AI-assisted (Claude), 2026-11-08; mask colour space and the fit's default 2026-11-10. Plain Python (no ``unreal``), so
tests/creature_zone_test.py runs it without the editor. tools/phaselock_target_editor.py calls it for every zone slot. Only targets
whose job entry names ``Master_Creature`` as the parent (the Bullymong and the TargetDummy, read with our reader) get that master's
defaults; any other (the Psycho, whose slots' parent is unknown) keeps a scale of 1.0 and gets a scale note. The zone mix itself is our
host model (M_OW_NPC_Zone), UNVERIFIED.

Where the rule comes from (docs/verification/PHASELOCK_STOCK_DATA.md, "Master_Creature's colour model", our own words): our
local reading of Master_Creature's compiled shader is that it multiplies the light/dark map's red and green by the scalars
``p_HighlightsIntensity`` and ``p_ShadowsIntensity`` before clamping them, and uses the colour vectors as they are stored, with no
gamma step. Both scalars default to 2.0 in the master; an instance that does not override them gets 2.0. The two mask textures the
host draws with this master (the Bullymong's and the dummy's) have their sRGB flag set in the package, so the game samples them
decoded to linear before that scale; the host had imported them as linear data (raw values, so more highlight and shadow).
UNVERIFIED against the running game: this is a reading of compiled and package data, and UE5's lighting and tone curve are not BL2's.
"""

# Round 23's x^2.2 on the zone vectors is a host fit (UNVERIFIED) that the shader reading does not support: the shader uses the vectors as
# stored. It was the default from 2026-11-09 (an A/B in identical light, runner-z36 claude-z36d off / claude-z36e on, a Luna critic, colour
# only, scored it 6.4/10 against 5.8), measured while the masks were still sampled raw. On the Bullymong's zone A (torso and the worn spikes)
# it roughly doubles the albedo's saturation and moves its hue toward orange, the "too yellow / golden" the critic named, so it is off by
# default again (2026-11-10). OPENWILLOW_ZONE_GAMMA=1 turns it on for an A/B.
GAMMA_FIT_DEFAULT = False


def gamma_from_env(value):
    """OPENWILLOW_ZONE_GAMMA's value -> whether to apply the fit: unset uses GAMMA_FIT_DEFAULT, '0' turns it off, anything else on."""
    if value is None or str(value).strip() == '':
        return GAMMA_FIT_DEFAULT
    return str(value).strip() != '0'


# Master_Creature's defaults for the two scalars, from its compiled uniform expressions (an instance may override them).
MASTER_CREATURE_SCALARS = {'p_HighlightsIntensity': 2.0, 'p_ShadowsIntensity': 2.0}
ZONE_VECTOR_ENDINGS = ('Hilight', 'Midtone', 'Shadow', 'ColorD')


def masks_srgb(master=None):
    """Whether the zone step imports a target's ``p_Masks`` texture as sRGB (decoded to linear when sampled), from its parent master.

    Master_Creature targets: True. Both of its mask textures the host draws (the Bullymong's and the dummy's) have the Texture2D ``SRGB``
    flag set in the package, read with our reader (``ow-package --object-dump ... --all``; the normal maps read False, so the flag is read,
    not defaulted), and UE3 samples such a texture decoded. Any other master: False, the earlier rule (raw values), because its masks'
    flag was not read.
    """
    return master == 'Master_Creature'


def zone_parameters(slot, master=None, gamma=GAMMA_FIT_DEFAULT):
    """{'vectors': {name: [r, g, b, a]}, 'scalars': {name: value}} for one material slot of an editor job.

    slot: the job's slot dict, with the stock instance's own 'vectors' and 'scalars' (UModel props). master: the instance's parent
    material as the job names it; only 'Master_Creature' has read defaults, any other master gets 1.0 (M_OW_NPC_Zone's default, the
    earlier rule) unless the instance sets a value. gamma (default GAMMA_FIT_DEFAULT, False) applies round 23's host fit, each
    colour channel raised to 2.2; gamma=False uses the values as stored, as the shader does.
    """
    vectors = {}
    for name, value in (slot.get('vectors') or {}).items():
        if not (name.startswith('p_') and name.endswith(ZONE_VECTOR_ENDINGS) and value):
            continue
        rgba = [float(v) for v in value]
        if gamma:
            rgba = [max(0.0, c) ** 2.2 for c in rgba[:3]] + rgba[3:]
        vectors[name] = rgba
    scalars = dict(MASTER_CREATURE_SCALARS) if master == 'Master_Creature' else {name: 1.0 for name in MASTER_CREATURE_SCALARS}
    for name in MASTER_CREATURE_SCALARS:
        value = (slot.get('scalars') or {}).get(name)
        if value is not None:
            scalars[name] = float(value)
    return {'vectors': vectors, 'scalars': scalars}


def scale_note(slot, master=None):
    """The zone step's log line for a slot whose scales were not chosen from a known master, else None.

    A target that names no master (the range's Psycho: its slot materials' parent was not read) keeps 1.0, today's look; this says so
    per slot instead of leaving it implicit. Reading the per-slot parents is a later item.
    """
    if master == 'Master_Creature':
        return None
    own = [name for name in MASTER_CREATURE_SCALARS if (slot.get('scalars') or {}).get(name) is not None]
    left = [name for name in MASTER_CREATURE_SCALARS if name not in own]
    if not left:
        return None
    return f'{slot.get("material", "?")}: parent unknown, scale left at 1.0 ({", ".join(left)})'
