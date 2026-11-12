"""Synthetic tests for the new reads of tools/real_game/scripts/phaselock_integrity.py (no game: unrealsdk and the namespace it
expects are stubbed; the objects and numbers are invented). They catch mistakes that would otherwise only show in a real-game session.

Run: python tests/phaselock_integrity_probe_test.py
"""
import sys
import tempfile
import types
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / 'tools' / 'real_game' / 'scripts' / 'phaselock_integrity.py'


class Obj:
    def __init__(self, **fields):
        self.__dict__.update(fields)

    def _path_name(self):
        return 'Pkg.' + self.__dict__.get('Name', 'Obj')


def load():
    sdk = types.ModuleType('unrealsdk')
    sdk.make_struct = lambda name, **f: Obj(**f)
    sdk.find_class = lambda name: name
    sdk.find_all = lambda *a, **k: []
    hooks = types.ModuleType('unrealsdk.hooks')
    hooks.Type = Obj(PRE=1, POST=2, POST_UNCONDITIONAL=3)
    hooks.add_hook = hooks.remove_hook = lambda *a, **k: None
    sdk.hooks = hooks
    sys.modules['unrealsdk'], sys.modules['unrealsdk.hooks'] = sdk, hooks
    namespace = {'__name__': 'probe', 'plain': lambda v: v, 'mark': lambda *a, **k: None, 'RG_OUT': tempfile.gettempdir()}
    exec(compile(SCRIPT.read_text(encoding='utf-8'), str(SCRIPT), 'exec'), namespace)
    return namespace


def test_bones_read_location_and_scale_and_survive_a_missing_bone():
    ns = load()
    axis = Obj(X=2.0, Y=0.0, Z=0.0)
    def location(name):
        if name == 'Head':
            raise ValueError('no such bone')
        return Obj(X=11.0, Y=22.0, Z=33.0)
    mesh = Obj(GetBoneLocation=location, MatchRefBone=lambda n: 3, GetBoneMatrix=lambda i: Obj(XPlane=axis, YPlane=axis, ZPlane=axis))
    rows = ns['_bone_rows'](mesh, Obj(Location=Obj(X=1.0, Y=2.0, Z=3.0)))
    assert rows['Hips'] == {'rel': [10.0, 20.0, 30.0], 'scale': [2.0, 2.0, 2.0]}
    assert rows['Head'] is None
    assert set(rows) == set(ns['PI_BONES'])


def test_assets_list_the_mesh_materials_lod_and_what_is_attached():
    ns = load()
    static = Obj(Class=Obj(Name='StaticMeshComponent'), Name='Spikes', StaticMesh=Obj(Name='SM_Spikes'), HiddenGame=False)
    attachment = Obj(Component=static, SocketName='Back', BoneName='Spine2')
    mesh = Obj(SkeletalMesh=Obj(Name='Skel'), Materials=[Obj(Name='Mat'), None], PredictedLODLevel=1, ForcedLodModel=0, MinLodModel=0,
               Attachments=[attachment])
    row = ns['_asset_row'](Obj(Attached=[Obj(Class=Obj(Name='Actor'), Name='A_1')], Components=[static]), mesh)
    assert row['skeletal_mesh'] == 'Pkg.Skel' and row['materials'] == ['Pkg.Mat', None]
    assert row['lod']['PredictedLODLevel'] == 1
    assert row['attachments'][0]['socket'] == 'Back' and row['attachments'][0]['StaticMesh'] == 'Pkg.SM_Spikes'
    assert row['components'][0]['name'] == 'Spikes' and row['attached_actors'] == ['Actor:A_1']


def test_assets_survive_a_mesh_without_those_fields():
    row = load()['_asset_row'](Obj(), Obj())
    assert row['attachments'] == [] and row['components'] == []
    # Failed reads are JSON null, not an error-marker string.
    assert row['skeletal_mesh'] is None and row['materials'] is None and row['lod']['PredictedLODLevel'] is None


def test_null_helper_and_rotation_reads():
    ns = load()
    assert ns['_null'](lambda: 1 / 0) is None and ns['_null'](lambda: 5) == 5
    assert ns['_r'](None) is None
    source = SCRIPT.read_text(encoding='utf-8')
    for field in ('"rotation": plain(_null(', '"mesh_rotation": plain(_null(', '"mesh_scale": _r(_null(', '"mesh_scale3d": plain(_null(',
                  '"draw_scale": _r(_null(', '"bounds_radius": _r(_null('):
        assert field in source, field


def injure_setup(ns):
    source = Obj(Name='Target', Controller='ctrl')
    ns['ai_pawn'] = lambda name: source
    me = Obj(bIsInjured=False, Location=Obj(), hits=0)
    def take(*args):
        me.hits += 1
    me.TakeDamage = take
    ns['get_pc'] = lambda: Obj(Pawn=me, WorldInfo=Obj(TimeSeconds=10.0))
    ns['_player'] = lambda pawn: {'injured': pawn.bIsInjured}
    return me


def test_injure_refuses_when_the_target_is_not_lifted():
    ns = load()
    me = injure_setup(ns)
    assert 'skipped' in ns['pi_injure']('Target')
    assert me.hits == 0 and ns['pi']['injure'] is None


def test_injure_repeats_until_injured_and_stops():
    ns = load()
    me = injure_setup(ns)
    ns['pi']['seen_lifted'] = {'Target'}
    ns['pi_injure']('Target')
    assert me.hits == 1
    assert ns['_tick_injure'](me, 10.2) is None             # not yet time
    assert ns['_tick_injure'](me, 10.5)['hit'] == 2
    me.bIsInjured = True
    assert ns['_tick_injure'](me, 11.0) == {'injured_after_hits': 2}
    assert ns['_tick_injure'](me, 12.0) is None and me.hits == 2


def test_injure_gives_up_after_eight_hits():
    ns = load()
    me = injure_setup(ns)
    ns['pi']['seen_lifted'] = {'Target'}
    ns['pi_injure']('Target')
    now, last = 10.0, None
    for _ in range(12):
        now += 0.6
        last = ns['_tick_injure'](me, now) or last
    assert 'gave_up_after_hits' in last and me.hits == 8


def test_result_says_whether_the_target_was_lifted():
    ns = load()
    ns['pi']['pawn'] = Obj(Name='Target', Controller='ctrl')
    ns['pi']['seen_lifted'] = {'Other'}
    result = ns['pi_result']()
    assert result['lifted'] == ['Other'] and result['target_lifted'] is False and result['target_has_controller'] is True
    ns['pi']['seen_lifted'] = {'Target', 'Other'}
    assert ns['pi_result']()['target_lifted'] is True


def test_an_empty_lift_list_is_a_miss_only_when_a_cast_was_observed():
    ns = load()
    ns['pi']['pawn'] = Obj(Name='Target', Controller='ctrl')
    assert ns['pi_result']()['cast_observed'] is False      # nothing ran: no rows, no calls
    ns['pi']['rows'] = 5
    assert ns['pi_result']()['cast_observed'] is False      # rows but the skill never ended or fizzled
    hook = ns['_mk']('pre')
    hook(Obj(), Obj(), None, Obj(func=Obj(Name='TakeDamage')))
    hook(Obj(), Obj(), None, Obj(func=Obj(Name='BeginLifting')))
    assert ns['pi_result']()['cast_observed'] is False      # a player call or a start does not count
    hook(Obj(), Obj(), None, Obj(func=Obj(Name='Fizzled')))
    result = ns['pi_result']()
    assert result['cast_observed'] is True and result['skill_calls'] == ['BeginLifting', 'Fizzled'] and result['rows'] == 5
    ns['pi']['rows'] = 0
    assert ns['pi_result']()['cast_observed'] is False      # a call but no rows


def test_stopping_the_probe_lets_go_of_the_pawn():
    ns = load()
    ns['pi']['pawn'] = Obj(Name='Target')
    ns['pi_stop']()
    assert ns['pi']['pawn'] is None


def test_a_kill_needs_the_target_lifted_now_and_alive_before():
    ns = load()
    health = [500.0]
    damage = []

    def take(*a):
        damage.append(a)
        health[0] = 0.0
    pawn = Obj(Name='Target', Location=Obj(X=0, Y=0, Z=0), GetHealth=lambda: health[0], TakeDamage=take)
    lifted = [None]
    ns['ai_pawn'] = lambda name: pawn
    ns['get_pc'] = lambda: Obj(WorldInfo=Obj(TimeSeconds=9.0))
    ns['lift_skill'] = lambda: Obj(LiftedPawn=lifted[0])
    ns['pi']['seen_lifted'] = {'Target'}        # lifted earlier: history is not enough
    assert 'skipped' in ns['pi_kill']('Target') and not damage and health[0] == 500.0
    lifted[0] = Obj(Name='Other')
    assert 'skipped' in ns['pi_kill']('Target') and not damage
    lifted[0] = pawn
    ns['pi']['pawn'] = pawn
    done = ns['pi_kill']('Target')
    assert done['before'] == 500.0 and done['after'] == 0.0 and done['killed'] is True and len(damage) == 1
    # The pawn is let go at once: the tick has none to touch, and the result still names and judges the target.
    assert ns['pi']['pawn'] is None and ns['pi_result']()['target'] == 'Target' and ns['pi_result']()['target_lifted'] is True
    damage.clear()
    health[0] = 0.0                             # already dead: no damage is sent
    done = ns['pi_kill']('Target')
    assert 'skipped' in done and not damage
    pawn.GetHealth = lambda: (_ for _ in ()).throw(RuntimeError('gone'))    # no real number: no damage is sent
    assert 'skipped' in ns['pi_kill']('Target') and not damage
    pawn.GetHealth = lambda: float('nan')       # not finite: no damage is sent
    assert 'skipped' in ns['pi_kill']('Target') and not damage
    pawn.GetHealth = lambda: health[0]
    pawn.TakeDamage = lambda *a: None           # the hit did nothing
    health[0] = 7.0
    assert ns['pi_kill']('Target')['killed'] is False


def test_hud_evidence_needs_samples_on_both_sides_of_the_kill():
    import json
    ns = load()
    with tempfile.TemporaryDirectory() as tmp:
        ns['RG_OUT'] = tmp
        assert ns['pi_hud_evidence'](5.0, 0.0)['usable'] is False       # no file
        file = Path(tmp) / 'kill_hud.jsonl'
        file.write_text('', encoding='utf-8')
        assert ns['pi_hud_evidence'](5.0, 0.0)['usable'] is False       # an empty file

        def row(when, frame=3):
            return json.dumps({'game_t': when, 'hud': {'p1.activeAbility': {'frame': frame}}})
        file.write_text('\n'.join([row(4.0, 0), row(6.0, 0)]), encoding='utf-8')
        assert ns['pi_hud_evidence'](5.0, 0.0)['usable'] is False       # a path that did not resolve reads 0
        file.write_text('\n'.join([row(4.0, 1), row(6.0, 1)]), encoding='utf-8')
        assert ns['pi_hud_evidence'](5.0, 0.0)['usable'] is False       # the emblem never seen running
        file.write_text('\n'.join([row(4.0), json.dumps({'error': 'x'}), 'not json']), encoding='utf-8')
        assert ns['pi_hud_evidence'](5.0, 0.0) == {'before': 1, 'running_before': 1, 'after': 0, 'usable': False}
        file.write_text('\n'.join([row(6.0), row(7.0, None)]), encoding='utf-8')
        assert ns['pi_hud_evidence'](5.0, 0.0)['usable'] is False       # nothing before, and a sample with no frame
        file.write_text('\n'.join([row(4.0), row(6.0)]), encoding='utf-8')
        assert ns['pi_hud_evidence'](5.0, 0.0)['usable'] is True
        # A failed first cast raised the emblem; this attempt, from 3.0, never did.
        file.write_text('\n'.join([row(2.0, 3), row(4.0, 1), row(6.0, 3)]), encoding='utf-8')
        result = ns['pi_hud_evidence'](5.0, 3.0)
        assert result['running_before'] == 0 and result['usable'] is False
        assert ns['pi_hud_evidence'](5.0, 1.0)['usable'] is True    # the same samples, counted from the earlier start
        # A sample at the kill's own game time is neither before nor after it.
        file.write_text('\n'.join([row(4.0, 3), row(5.0, 3)]), encoding='utf-8')
        result = ns['pi_hud_evidence'](5.0, 0.0)
        assert result['before'] == 1 and result['after'] == 0 and result['usable'] is False


def shield_world(ns, own_level=4, own_max=150.0):
    """A stub inventory, looked up fresh as the probe does; its levels and capacities are invented, not a save's. As live: equipping or unequipping makes a new object of the same shield."""
    made = [0]

    def shield(name, level):
        made[0] += 1
        path = f'Pkg.WillowShield_{made[0]}'
        return Obj(Class=Obj(Name='WillowShield'), Inventory=None, Name=f'WillowShield_{made[0]}', ExpLevel=level, GameStage=level, RarityLevel=1, MonetaryValue=5,
                   bReadied=True, GetShortHumanReadableName=lambda: name, GetManufacturerGradeIndex=lambda: level,
                   DefinitionData=Obj(BalanceDefinition=Obj(Name='Balance')), _path_name=lambda: path)

    def moved(item):
        return shield(item.GetShortHumanReadableName(), item.ExpLevel)
    world = {'max': own_max, 'next': [], 'shield': shield}
    own = shield('Own', own_level)
    gun = Obj(Class=Obj(Name='WillowWeapon'), Inventory=None)

    def ready(item):
        world['readied'] = world.get('readied', 0) + 1
        if world.get('ready_takes', True):
            old = im.ItemChain
            if old is not None:
                im.Backpack.append(moved(old))
            im.Backpack.remove(item)
            im.ItemChain = moved(item)

    def add(item):
        im.Backpack.append(item)
        if world.get('add_throws'):
            raise RuntimeError('the add took effect, then failed')

    # As in the game: the weapons in InventoryChain, the worn shield in ItemChain.
    im = Obj(InventoryChain=gun, ItemChain=own, Backpack=[], Class=Obj(_fields=lambda: [Obj(Name='ItemChain'), Obj(Name='Backpack')]), AddInventoryToBackpack=add,
             RemoveInventoryFromBackpack=lambda i: im.Backpack.remove(i), ReadyBackpackInventory=ready)
    pawn = Obj(GetMaxShieldStrength=lambda: world['max'], GetShieldStrength=lambda: 90.0)
    pc = Obj(GetPawnInventoryManager=lambda: im, Pawn=pawn, PlayerReplicationInfo=Obj(ExpLevel=8), WorldInfo=Obj(TimeSeconds=1.0))
    world['pc'], world['im'] = pc, im
    ns['get_pc'] = lambda: pc
    pool = Obj(SpawnBalancedInventoryFromInventoryBalanceDefinition=lambda *a: (True, world['next']))
    ns['unrealsdk'].find_class = lambda name: Obj(ClassDefaultObject=pool)
    return world


def test_a_shield_row_counts_only_for_its_own_level_and_a_fresh_read():
    ns = load()
    world = shield_world(ns)
    start = ns['pi_shields'](True)
    assert start['equipped']['name'] == 'Own' and start['max_strength'] == 150.0 and start['player_level'] == 8
    assert start['manager_fields'] == ['ItemChain']
    world['next'] = [world['shield']('Test30', 30)]
    tried = ns['pi_shield_try'](30)
    assert tried['requested_level'] == 30 and tried['item']['level'] == 30
    assert world['im'].ItemChain.GetShortHumanReadableName() == 'Test30'    # the equipped one is a new object
    read = ns['pi_shield_read']()
    assert read['counts'] is False and 'game time' in read['reason']        # no time has passed since the equip
    world['pc'].WorldInfo.TimeSeconds = 2.0
    read = ns['pi_shield_read']()
    assert read['counts'] is False and 'not updated' in read['reason']      # a read that never updates
    world['max'] = 900.0
    read = ns['pi_shield_read']()
    assert read['counts'] is True and read['max_strength'] == 900.0
    ns['pi_shield_restore']()
    world['max'] = 150.0
    assert ns['pi_shields_check']()['ok'] is True and not world['im'].Backpack

    # The level came back clamped: the row is kept and does not count.
    world['next'] = [world['shield']('Clamped', 20)]
    ns['pi_shield_try'](50)
    world['max'] = 700.0
    world['pc'].WorldInfo.TimeSeconds = 3.0
    read = ns['pi_shield_read']()
    assert read['counts'] is False and read['reason'] == 'level 20, requested 50' and read['max_strength'] == 700.0
    ns['pi_shield_restore']()
    world['max'] = 150.0

    # Her own level is refused, and nothing is spawned.
    world['next'] = [world['shield']('Same', 4)]
    assert 'skipped' in ns['pi_shield_try'](4) and len(world['next']) == 1 and not world['im'].Backpack

    # Nothing spawned, and an equip that does not take.
    world['next'] = []
    assert 'skipped' in ns['pi_shield_try'](5)
    world['next'] = [world['shield']('NotTaken', 15)]
    world['ready_takes'] = False
    ns['pi_shield_try'](15)
    world['pc'].WorldInfo.TimeSeconds = 5.0
    read = ns['pi_shield_read']()
    assert read['counts'] is False and 'not equipped' in read['reason']
    assert ns['pi_shield_restore']()['ok'] is True and not world['im'].Backpack


def test_a_spawn_with_her_shields_identity_is_refused_and_she_keeps_it():
    ns = load()
    world = shield_world(ns)
    world['im'].Backpack.append(world['shield']('Kept', 4))
    ns['pi_shields'](True)
    readied = world.get('readied', 0)
    # A clamped level with her shield's name, then with the backpack shield's: not added, not equipped, nothing to remove.
    for name in ('Own', 'Kept'):
        world['next'] = [world['shield'](name, 4)]
        tried = ns['pi_shield_try'](15)
        assert 'skipped' in tried and 'identity' in tried['skipped'], tried
        assert world.get('readied', 0) == readied and len(world['im'].Backpack) == 1
    check = ns['pi_shield_restore']()
    assert check['ok'] is True and world['im'].ItemChain.GetShortHumanReadableName() == 'Own'


def test_the_equipped_row_must_be_this_attempts_shield():
    ns = load()
    world = shield_world(ns)
    ns['pi_shields'](True)
    world['next'] = [world['shield']('Test', 15)]
    ns['pi_shield_try'](15)
    world['pc'].WorldInfo.TimeSeconds = 2.0
    world['max'] = 900.0
    world['im'].ItemChain = world['shield']('Something Else', 15)      # of the right level, but not what this row spawned
    read = ns['pi_shield_read']()
    assert read['counts'] is False and 'not equipped' in read['reason']


def test_an_unclear_backpack_is_left_alone_and_reported():
    ns = load()
    world = shield_world(ns)
    ns['pi_shields'](True)
    world['next'] = [world['shield']('Test', 15)]
    ns['pi_shield_try'](15)
    # Two shields fit the test's key: neither is positively the spawned one.
    world['im'].Backpack.append(world['shield']('Test', 15))
    check = ns['pi_shield_restore']()
    assert world['im'].ItemChain.GetShortHumanReadableName() == 'Own'
    assert check['ok'] is False and len(world['im'].Backpack) == 2


def test_a_manager_without_an_item_chain_finds_no_shield_and_says_so():
    ns = load()
    world = shield_world(ns)
    del world['im'].__dict__['ItemChain']
    start = ns['pi_shields'](True)
    assert start['equipped'] is None and start['max_strength'] == 150.0


def test_her_shield_is_found_by_level_balance_and_name():
    ns = load()
    world = shield_world(ns)
    twin = world['shield']('Other Name', 4)      # the same level and balance, as the live save holds
    world['im'].Backpack.append(twin)
    ns['pi_shields'](True)
    world['next'] = [world['shield']('Test', 15)]
    ns['pi_shield_try'](15)
    assert ns['pi_shield_restore']()['ok'] is True
    assert world['im'].ItemChain.GetShortHumanReadableName() == 'Own' and world['im'].Backpack == [twin]


def test_a_restore_that_cannot_identify_her_shield_says_so_and_does_not_guess():
    for extra, count in ((False, 0), (True, 2)):
        ns = load()
        world = shield_world(ns)
        ns['pi_shields'](True)
        world['next'] = [world['shield']('Test', 15)]
        ns['pi_shield_try'](15)
        for item in list(world['im'].Backpack):
            world['im'].Backpack.remove(item)
        if extra:
            world['im'].Backpack += [world['shield']('Own', 4), world['shield']('Own', 4)]
        before = world.get('readied')
        try:
            ns['pi_shield_restore']()
        except RuntimeError as error:
            assert f'{count} shields match' in str(error) and world.get('readied') == before
        else:
            raise AssertionError('a guess was made')


def test_an_add_that_throws_after_taking_effect_is_undone():
    ns = load()
    world = shield_world(ns)
    ns['pi_shields'](True)
    world['next'] = [world['shield']('Test', 15)]
    world['add_throws'] = True
    try:
        ns['pi_shield_try'](15)
    except RuntimeError:
        pass
    else:
        raise AssertionError('the stub add did not throw')
    assert [i.GetShortHumanReadableName() for i in world['im'].Backpack] == ['Test']
    assert ns['pi_shield_restore']()['ok'] is True and not world['im'].Backpack


def test_a_restore_that_does_not_take_is_reported_and_asks_once():
    ns = load()
    world = shield_world(ns)
    ns['pi_shields'](True)
    world['next'] = [world['shield']('Test', 15)]
    ns['pi_shield_try'](15)
    world['ready_takes'] = False
    readied = world['readied']
    check = ns['pi_shield_restore']()
    assert check['ok'] is False and 'Test' in check['key'] and 'Own' in check['expected_key']
    assert world['readied'] == readied + 1
    ns['pi_shield_restore']()
    assert world['readied'] == readied + 1      # polling does not ask again
    world['ready_takes'] = True
    world['max'] = 500.0
    assert ns['pi_shields_check']()['ok'] is False      # also with the wrong maximum


def test_leftovers_fail_the_check():
    ns = load()
    world = shield_world(ns)
    ns['pi_shields'](True)
    assert ns['pi_shields_check']()['ok'] is True
    world['im'].Backpack.append(world['shield']('Leftover', 15))
    check = ns['pi_shields_check']()
    assert check['ok'] is False and len(check['backpack']) == 1 and check['expected_backpack'] == []


def test_no_shield_object_is_kept_between_commands():
    text = SCRIPT.read_text(encoding='utf-8')
    assert 'pi["shield_own"]' not in text and 'pi["shield_test"]' not in text and 'equipped is test' not in text


def test_giving_up_on_the_injure_does_not_count_as_down():
    ns = load()
    me = Obj(bIsInjured=False, Health=1)
    ns['pi']['injure'] = {'source': Obj(), 'hits': 8, 'next': 0.0, 'done': False, 'injured': False}
    ns['_tick_injure'](me, 1.0)
    assert ns['pi']['injure']['done'] is True and ns['pi']['injure']['injured'] is False
    ns['pi']['injure'] = {'source': Obj(), 'hits': 2, 'next': 0.0, 'done': False, 'injured': False}
    me.bIsInjured = True
    ns['_tick_injure'](me, 1.0)
    assert ns['pi']['injure']['injured'] is True


def test_the_pick_takes_only_the_exact_adult():
    pick = load()['pi_pick_target']

    def row(name, cls, dist, dead=False, ctrl=True, radius=150):
        return {'name': name, 'class': cls, 'dist': dist, 'dead': dead, 'ctrl': ctrl, 'radius': radius}
    variants = [row('Ranged', 'CharClass_PrimalBeast_Ranged', 100), row('Plain', 'CharClass_PrimalBeast', 200)]
    assert pick(variants) is None       # only variants: the capture keeps polling
    assert pick([]) is None
    assert pick(variants + [row('Far', 'CharClass_PrimalBeastAdult', 900), row('Near', 'CharClass_PrimalBeastAdult', 800)])['name'] == 'Near'
    # A name that only starts like the Adult's is not the Adult, and an unusable Adult is not a candidate.
    assert pick([row('Other', 'CharClass_PrimalBeastAdult_Fire', 100)]) is None
    for bad in (row('Dead', 'CharClass_PrimalBeastAdult', 100, dead=True), row('Free', 'CharClass_PrimalBeastAdult', 100, ctrl=False),
                row('Small', 'CharClass_PrimalBeastAdult', 100, radius=90)):
        assert pick([bad]) is None


def test_placement_reads_both_matrices_and_nulls_what_fails():
    ns = load()
    def plane(x):
        return Obj(X=x, Y=0.0, Z=0.0)
    matrix = Obj(XPlane=plane(1.0), YPlane=plane(2.0), ZPlane=plane(3.0), WPlane=plane(4.0))
    mesh = Obj(MatchRefBone=lambda name: 7, GetBoneMatrix=lambda index: matrix)
    bounds = Obj(Origin=Obj(X=10.0, Y=20.0, Z=30.0), BoxExtent=Obj(X=1.0, Y=2.0, Z=3.0), SphereRadius=4.0)
    comp = Obj(LocalToWorld=matrix, Bounds=bounds, Translation=(1, 2, 3), Rotation=(0, 0, 0), Scale=1.5, Scale3D=(1, -1, 1))
    row = ns['_placement_row'](mesh, comp, 'Head')
    assert row['local_to_world'][3] == [4.0, 0.0, 0.0] and row['bone_matrix'][0] == [1.0, 0.0, 0.0]
    assert row['bounds'] == {'origin': [10.0, 20.0, 30.0], 'extent': [1.0, 2.0, 3.0], 'radius': 4.0}
    assert row['scale'] == 1.5 and row['scale3d'] == (1, -1, 1)
    # A field the SDK does not expose gives JSON null, and a bad matrix does not stop the row.
    bare = ns['_placement_row'](Obj(MatchRefBone=lambda name: 7, GetBoneMatrix=lambda index: Obj()), Obj(), 'Head')
    assert bare['local_to_world'] is None and bare['bone_matrix'] is None and bare['scale'] is None and bare['bounds'] is None


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
