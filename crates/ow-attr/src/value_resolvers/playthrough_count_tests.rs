//! `PlayThroughCountAttributeValueResolver` on a synthetic world (rule 9: invented objects and numbers; the class and
//! field names are the packages' names). Each Off-mode test names the section of NATIVE_PLAYTHROUGH_ATTRIBUTES.md
//! ("note") it checks.

use ow_core::rand::{CrtStream, Rand15};
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::store::PackageStore;

use super::playthrough_count::{NOTE, count};
use super::testworld::{Fx, World, system};
use crate::api::{
    AttrError, AttrWorld, Comparison, EngineQuery, EvalCx, GlobalSlots, InitData, NumForms,
    Operand2Usage, WorldParts,
};

const CLASS: &str = "WillowGame.PlayThroughCountAttributeValueResolver";
const WILLOW_GRI: &str = "WillowGame.WillowGameReplicationInfo";
const MUL: u8 = 2;

/// A store-backed world in `mode` (a world without a store runs `Off`).
fn with_mode(fx: &mut Fx, mode: ParityMode) {
    fx.w.defs = Some(DefStore::new(
        PackageStore::new_empty(),
        IdentityPolicy::FirstInScanOrder,
        mode,
    ));
}

/// A resolver of the class with the given `IncludePlaythroughThree`.
fn resolver(fx: &mut Fx, include_three: i32) -> ObjRef {
    let c = fx.by_path(CLASS);
    let name = format!(
        "PlayThroughCountAttributeValueResolver_{}",
        fx.w.space.heap.len()
    );
    fx.obj(
        c,
        &name,
        vec![("IncludePlaythroughThree", Value::Int(include_three))],
    )
}

/// The installed shape: a NoContextNeeded attribute whose value chain is `before` then the resolver.
fn count_attr(fx: &mut Fx, include_three: i32, before: Vec<ObjRef>) -> ObjRef {
    let ctx = fx.no_context();
    let r = resolver(fx, include_three);
    let mut values = before;
    values.push(r);
    fx.attr("PlayThroughCount", vec![ctx], values)
}

fn value(fx: &mut Fx, attr: ObjRef) -> Result<f32, AttrError> {
    let sys = system();
    sys.value_of(attr, None, None, &mut EvalCx::new(&mut fx.w))
        .map(|v| v.expect("the no-context chain resolves").value)
}

fn base(fx: &mut Fx, attr: ObjRef) -> Result<f32, AttrError> {
    let sys = system();
    sys.base_value_of(attr, None, None, &mut EvalCx::new(&mut fx.w))
        .map(|v| v.expect("the no-context chain resolves").value)
}

/// The mission_script_test.py reward shape on the real class: 0.5, then x a conditional (4 when PlayThroughCount == 2,
/// else 3). Returns the initialization data to evaluate.
fn reward_data(fx: &mut Fx, count: ObjRef) -> InitData {
    let e = fx.expr(
        Some(count),
        Comparison::EqualTo,
        Operand2Usage::PreferAttribute,
        None,
        2.0,
    );
    let (four, three) = (fx.k(4.0), fx.k(3.0));
    let cond = fx.conditional(true, vec![(vec![e], four)], three);
    let multiplier = fx.definition(vec![("ConditionalInitialization", cond)]);
    let (ctx, half) = (fx.no_context(), fx.constant(0.5));
    let arg = fx.of_def(multiplier);
    let math = fx.math(MUL, arg, None);
    let pct = fx.attr("XPReward", vec![ctx], vec![half, math]);
    InitData {
        base_value_attribute: Some(pct),
        ..InitData::constant(0.0)
    }
}

fn reward(fx: &mut Fx, count: ObjRef) -> Result<f32, AttrError> {
    let data = reward_data(fx, count);
    let sys = system();
    sys.evaluate_init(&data, None, None, &mut EvalCx::new(&mut fx.w))
}

/// The test world with an answer to [`EngineQuery::GameReplicationInfo`]: `gri` (`None`: the world has none).
/// Everything else is forwarded (the inner world runs no script, so forwarding the notification is allowed).
struct GriWorld<'a> {
    inner: &'a mut World,
    gri: Option<ObjRef>,
    /// The sources the query was asked with.
    asked: Vec<ObjRef>,
}

impl<'a> GriWorld<'a> {
    fn new(inner: &'a mut World, gri: Option<ObjRef>) -> Self {
        GriWorld {
            inner,
            gri,
            asked: Vec::new(),
        }
    }
}

impl AttrWorld for GriWorld<'_> {
    fn parts(&mut self) -> WorldParts<'_> {
        self.inner.parts()
    }
    fn globals(&mut self) -> Result<ObjRef, AttrError> {
        self.inner.globals()
    }
    fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
        self.inner.game_info()
    }
    fn engine_query(
        &mut self,
        q: EngineQuery,
        source: ObjRef,
    ) -> Result<Option<ObjRef>, AttrError> {
        match q {
            EngineQuery::GameReplicationInfo => {
                self.asked.push(source);
                Ok(self.gri)
            }
            other => self.inner.engine_query(other, source),
        }
    }
    fn global_slots(&mut self) -> &mut GlobalSlots {
        self.inner.global_slots()
    }
    fn attribute_changed(
        &mut self,
        owner: ObjRef,
        value_prop: PropId,
        forms: NumForms,
    ) -> Result<(), AttrError> {
        self.inner.attribute_changed(owner, value_prop, forms)
    }
    fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
        self.inner.rng(s)
    }
}

fn value_in(fx: &mut Fx, gri: Option<ObjRef>, attr: ObjRef) -> Result<f32, AttrError> {
    let sys = system();
    let mut w = GriWorld::new(&mut fx.w, gri);
    sys.value_of(attr, None, None, &mut EvalCx::new(&mut w))
        .map(|v| v.expect("the no-context chain resolves").value)
}

fn base_in(fx: &mut Fx, gri: Option<ObjRef>, attr: ObjRef) -> Result<f32, AttrError> {
    let sys = system();
    let mut w = GriWorld::new(&mut fx.w, gri);
    sys.base_value_of(attr, None, None, &mut EvalCx::new(&mut w))
        .map(|v| v.expect("the no-context chain resolves").value)
}

/// A replication info class at `path`: under `Core.Object` declaring `CurrentPlaythrough` of `kind`, or under
/// `parent`, inheriting it.
fn gri_class(fx: &mut Fx, path: &str, parent: Option<&str>, kind: PropKind) -> ClassId {
    let decls = match parent {
        Some(_) => vec![],
        None => vec![PropertyDecl::new(
            fx.w.space.names.intern("CurrentPlaythrough"),
            kind,
        )],
    };
    let parent = fx.by_path(parent.unwrap_or("Core.Object"));
    fx.w.space
        .classes
        .define(path, Some(parent), decls)
        .unwrap()
}

/// A `WillowGameReplicationInfo` with `CurrentPlaythrough` = `cp` (an int, as the note reads it).
fn willow_gri(fx: &mut Fx, cp: i32) -> ObjRef {
    let c = match fx.w.space.classes.by_path(WILLOW_GRI) {
        Some(c) => c,
        None => gri_class(fx, WILLOW_GRI, None, PropKind::Int),
    };
    let name = format!("WillowGameReplicationInfo_{}", fx.w.space.heap.len());
    fx.obj(c, &name, vec![("CurrentPlaythrough", Value::Int(cp))])
}

// ----- Reference mode: the C++ reading -----

#[test]
fn reference_reads_one_whatever_the_field() {
    for include_three in [0, 1, 7] {
        let mut fx = Fx::new();
        with_mode(&mut fx, ParityMode::Reference);
        let attr = count_attr(&mut fx, include_three, vec![]);
        assert_eq!(value(&mut fx, attr), Ok(1.0), "field {include_three}");
        // Get-base is the same read.
        assert_eq!(base(&mut fx, attr), Ok(1.0), "field {include_three}");
    }
}

#[test]
fn reference_ignores_the_previous_value() {
    let mut fx = Fx::new();
    with_mode(&mut fx, ParityMode::Reference);
    let seven = fx.constant(7.0);
    let attr = count_attr(&mut fx, 0, vec![seven]);
    assert_eq!(value(&mut fx, attr), Ok(1.0));
}

#[test]
fn reference_ignores_the_worlds_playthrough() {
    // The C++ reading does not ask the world: a UVHM replication info still reads 1.
    let mut fx = Fx::new();
    with_mode(&mut fx, ParityMode::Reference);
    let g = willow_gri(&mut fx, 2);
    let attr = count_attr(&mut fx, 1, vec![]);
    assert_eq!(value_in(&mut fx, Some(g), attr), Ok(1.0));
}

#[test]
fn reference_takes_the_first_playthrough_branch_of_a_conditional() {
    // 0.5 x 3: the "== 2" entry does not hold for a count of 1 (the C++ reward percentage reading).
    let mut fx = Fx::new();
    with_mode(&mut fx, ParityMode::Reference);
    let count = count_attr(&mut fx, 0, vec![]);
    assert_eq!(reward(&mut fx, count), Ok(1.5));
}

#[test]
fn reference_writes_are_not_ported() {
    let mut fx = Fx::new();
    with_mode(&mut fx, ParityMode::Reference);
    let r = resolver(&mut fx, 0);
    let globals = fx.w.globals;
    let sys = system();
    let res = sys.value_resolver(&fx.w.space, r).unwrap();
    match res.storage(&sys, r, globals, &mut EvalCx::new(&mut fx.w)) {
        Err(AttrError::NotYet(e)) => {
            assert_eq!(e.missing, Missing::NotPorted);
            assert_eq!(e.record, NOTE);
        }
        other => panic!("expected NotYet NotPorted, got {other:?}"),
    }
}

// ----- Off mode: the native rule, from NATIVE_PLAYTHROUGH_ATTRIBUTES.md alone -----

/// Off mode both ways: by default (a world without a store) and by a store in Off.
fn off_worlds() -> [Fx; 2] {
    let mut stored = Fx::new();
    with_mode(&mut stored, ParityMode::Off);
    [Fx::new(), stored]
}

#[test]
fn off_note_s3_row1_no_replication_info_reads_one() {
    // Note section 3, first row; section 4, first bullet (a front-end or offline evaluation).
    for mut fx in off_worlds() {
        for include_three in [0, 1] {
            let attr = count_attr(&mut fx, include_three, vec![]);
            assert_eq!(
                value_in(&mut fx, None, attr),
                Ok(1.0),
                "field {include_three}"
            );
            assert_eq!(
                base_in(&mut fx, None, attr),
                Ok(1.0),
                "field {include_three}"
            );
        }
    }
}

#[test]
fn off_note_s3_row1_a_replication_info_of_another_class_reads_one() {
    // Note section 3, first row: a GRI that is not a WillowGameReplicationInfo; its CurrentPlaythrough is not read.
    let mut fx = Fx::new();
    let c = gri_class(&mut fx, "Engine.GameReplicationInfo", None, PropKind::Int);
    let other = fx.obj(
        c,
        "GameReplicationInfo_0",
        vec![("CurrentPlaythrough", Value::Int(2))],
    );
    // This space has no WillowGameReplicationInfo class at all, so it has no such object.
    assert!(fx.w.space.classes.by_path(WILLOW_GRI).is_none());
    for include_three in [0, 1] {
        let attr = count_attr(&mut fx, include_three, vec![]);
        assert_eq!(
            value_in(&mut fx, Some(other), attr),
            Ok(1.0),
            "field {include_three}"
        );
    }
    // With the class present, an object of a sibling class is still not one.
    willow_gri(&mut fx, 0);
    let attr = count_attr(&mut fx, 1, vec![]);
    assert_eq!(value_in(&mut fx, Some(other), attr), Ok(1.0));
}

#[test]
fn off_note_s3_row1_a_subclass_counts() {
    // Note section 3, first row: "a subclass counts".
    let mut fx = Fx::new();
    gri_class(&mut fx, WILLOW_GRI, None, PropKind::Int);
    let sub = gri_class(
        &mut fx,
        "Invented.SubGameReplicationInfo",
        Some(WILLOW_GRI),
        PropKind::Int,
    );
    let g = fx.obj(
        sub,
        "SubGameReplicationInfo_0",
        vec![("CurrentPlaythrough", Value::Int(2))],
    );
    let field0 = count_attr(&mut fx, 0, vec![]);
    let field1 = count_attr(&mut fx, 1, vec![]);
    assert_eq!(value_in(&mut fx, Some(g), field0), Ok(2.0));
    assert_eq!(value_in(&mut fx, Some(g), field1), Ok(3.0));
}

#[test]
fn off_note_s3_row2_a_negative_playthrough_reads_one() {
    // Note section 3, second row, both fields; i32::MIN is the far end of the same row.
    let mut fx = Fx::new();
    for cp in [-1, -7, i32::MIN] {
        let g = willow_gri(&mut fx, cp);
        for include_three in [0, 1] {
            let attr = count_attr(&mut fx, include_three, vec![]);
            let what = format!("CP {cp} field {include_three}");
            assert_eq!(value_in(&mut fx, Some(g), attr), Ok(1.0), "{what}");
            assert_eq!(base_in(&mut fx, Some(g), attr), Ok(1.0), "{what}");
        }
    }
}

#[test]
fn off_note_s3_row3_field_zero_clamps_to_one_and_two() {
    // Note section 3, third row: Normal 1, TVHM 2, UVHM 2 with the installed field (0).
    let mut fx = Fx::new();
    for (cp, want) in [(0, 1.0), (1, 2.0), (2, 2.0), (5, 2.0)] {
        let g = willow_gri(&mut fx, cp);
        let attr = count_attr(&mut fx, 0, vec![]);
        assert_eq!(value_in(&mut fx, Some(g), attr), Ok(want), "CP {cp}");
        assert_eq!(base_in(&mut fx, Some(g), attr), Ok(want), "CP {cp}");
    }
}

#[test]
fn off_note_s3_row4_a_nonzero_field_is_cp_plus_one() {
    // Note section 3, fourth row: Normal 1, TVHM 2, UVHM 3. "Not 0" includes a negative field.
    let mut fx = Fx::new();
    for include_three in [1, 7, -1] {
        for (cp, want) in [(0, 1.0), (1, 2.0), (2, 3.0), (5, 6.0)] {
            let g = willow_gri(&mut fx, cp);
            let attr = count_attr(&mut fx, include_three, vec![]);
            let what = format!("CP {cp} field {include_three}");
            assert_eq!(value_in(&mut fx, Some(g), attr), Ok(want), "{what}");
            assert_eq!(base_in(&mut fx, Some(g), attr), Ok(want), "{what}");
        }
    }
}

#[test]
fn off_note_s4_cp_max_wraps_without_a_panic() {
    // Note section 4, fourth bullet, and checklist item 4: CP + 1 is the wrapping 32-bit sum (this lane's choice of
    // the two the checklist allows), so i32::MAX + 1 is i32::MIN.
    let mut fx = Fx::new();
    let g = willow_gri(&mut fx, i32::MAX);
    let field0 = count_attr(&mut fx, 0, vec![]);
    let field1 = count_attr(&mut fx, 1, vec![]);
    assert_eq!(value_in(&mut fx, Some(g), field0), Ok(1.0));
    assert_eq!(base_in(&mut fx, Some(g), field0), Ok(1.0));
    assert_eq!(value_in(&mut fx, Some(g), field1), Ok(-2147483648.0));
    assert_eq!(base_in(&mut fx, Some(g), field1), Ok(-2147483648.0));
}

#[test]
fn off_note_s4_large_counts_round_to_the_nearest_f32() {
    // Note section 4, fourth bullet: 16777219 is not an f32; the nearest (ties to even) is 16777220.
    let mut fx = Fx::new();
    let g = willow_gri(&mut fx, 16_777_218);
    let attr = count_attr(&mut fx, 1, vec![]);
    assert_eq!(value_in(&mut fx, Some(g), attr), Ok(16_777_220.0));
}

#[test]
fn off_note_s3_the_table_on_its_two_inputs() {
    // Note sections 3 and 4, every row, on the pure rule: CP -1, 0, 1, 2 and i32::MAX; the field 0 and 1.
    let rows = [
        (None, 0, 1),
        (None, 1, 1),
        (Some(-1), 0, 1),
        (Some(-1), 1, 1),
        (Some(0), 0, 1),
        (Some(0), 1, 1),
        (Some(1), 0, 2),
        (Some(1), 1, 2),
        (Some(2), 0, 2),
        (Some(2), 1, 3),
        (Some(i32::MAX), 0, 1),
        (Some(i32::MAX), 1, i32::MIN),
    ];
    for (cp, field, want) in rows {
        assert_eq!(count(cp, field), want, "CP {cp:?} field {field}");
    }
}

#[test]
fn off_note_s3_ignores_the_previous_value() {
    // Note section 3 ("the chain's previous value [does] not change it"); checklist item 1.
    let mut fx = Fx::new();
    let g = willow_gri(&mut fx, 1);
    let seven = fx.constant(7.0);
    let attr = count_attr(&mut fx, 0, vec![seven]);
    assert_eq!(value_in(&mut fx, Some(g), attr), Ok(2.0));
    assert_eq!(base_in(&mut fx, Some(g), attr), Ok(2.0));
}

#[test]
fn off_note_s3_ignores_the_context() {
    // Note section 3 ("the resolved context ... [does] not change it"); checklist item 1. The no-context chain
    // resolves to each source in turn; the query is asked with it as its source, which it does not read.
    let mut fx = Fx::new();
    let g = willow_gri(&mut fx, 2);
    let attr = count_attr(&mut fx, 1, vec![]);
    let plain = fx.by_path("Core.Object");
    let a = fx.obj(plain, "SourceA", vec![]);
    let b = fx.obj(plain, "SourceB", vec![]);
    let sys = system();
    for s in [a, b] {
        let mut w = GriWorld::new(&mut fx.w, Some(g));
        let v = sys
            .value_of(attr, Some(s), None, &mut EvalCx::new(&mut w))
            .unwrap()
            .expect("the no-context chain resolves")
            .value;
        assert_eq!(v, 3.0);
        assert_eq!(w.asked, vec![s]);
    }
}

#[test]
fn off_note_s7_a_conditional_on_count_two_matches_in_tvhm_and_uvhm() {
    // Note section 7's visible check, on invented numbers: a value keyed on PlayThroughCount == 2 is the same in TVHM
    // and UVHM with the installed field. 0.5 x (4 when the count is 2, else 3).
    for (cp, want) in [(0, 1.5), (1, 2.0), (2, 2.0)] {
        let mut fx = Fx::new();
        with_mode(&mut fx, ParityMode::Off);
        let g = willow_gri(&mut fx, cp);
        let count = count_attr(&mut fx, 0, vec![]);
        let data = reward_data(&mut fx, count);
        let sys = system();
        let mut w = GriWorld::new(&mut fx.w, Some(g));
        assert_eq!(
            sys.evaluate_init(&data, None, None, &mut EvalCx::new(&mut w)),
            Ok(want),
            "CP {cp}"
        );
    }
}

#[test]
fn off_note_s2_get_base_is_get() {
    // Note section 2 and checklist item 5, over the table's inputs.
    let mut fx = Fx::new();
    for cp in [-1, 0, 1, 2, i32::MAX] {
        let g = willow_gri(&mut fx, cp);
        for include_three in [0, 1] {
            let attr = count_attr(&mut fx, include_three, vec![]);
            assert_eq!(
                value_in(&mut fx, Some(g), attr),
                base_in(&mut fx, Some(g), attr),
                "CP {cp} field {include_three}"
            );
        }
    }
}

#[test]
fn off_note_s2_writes_answer_false_and_change_nothing() {
    // Note section 2 and checklist item 6: the class default, storage None; add, remove and set-base answer false.
    let mut fx = Fx::new();
    let g = willow_gri(&mut fx, 1);
    let r = resolver(&mut fx, 0);
    let globals = fx.w.globals;
    let sys = system();
    let res = sys.value_resolver(&fx.w.space, r).unwrap();
    {
        let mut w = GriWorld::new(&mut fx.w, Some(g));
        let got = res.storage(&sys, r, globals, &mut EvalCx::new(&mut w));
        assert!(matches!(got, Ok(None)), "{got:?}");
    }
    let attr = count_attr(&mut fx, 0, vec![]);
    let modifier = fx.constant(9.0);
    {
        let mut w = GriWorld::new(&mut fx.w, Some(g));
        let cx = &mut EvalCx::new(&mut w);
        assert_eq!(sys.set_base(attr, None, 5.0, false, cx), Ok(false));
        let added = sys
            .add_modifier(attr, Some(globals), modifier, false, cx)
            .unwrap();
        assert!(!added.succeeded(), "{added:?}");
        let removed = sys
            .remove_modifier(attr, Some(globals), modifier, false, cx)
            .unwrap();
        assert!(!removed.succeeded(), "{removed:?}");
    }
    // Nothing was written: the count and the replication info are as they were.
    assert_eq!(value_in(&mut fx, Some(g), attr), Ok(2.0));
    let class = fx.by_path(WILLOW_GRI);
    let p = fx.w.space.prop(class, "CurrentPlaythrough").unwrap();
    assert_eq!(
        fx.w.space.read(g, PropAddr::scalar(p)).unwrap(),
        &Value::Int(1)
    );
}

#[test]
fn off_a_world_that_cannot_answer_the_query_stops_the_read() {
    // Not in the note (this lane's choice, decision 2026-11-16-47): a world that does not model the query answers
    // NotYet, which stops the read; only Ok(None), "no replication info", is the first row. The plain test world
    // gives the trait's LayerAbsent.
    let mut fx = Fx::new();
    let attr = count_attr(&mut fx, 0, vec![]);
    for r in [value(&mut fx, attr), base(&mut fx, attr)] {
        match r {
            Err(AttrError::NotYet(e)) => {
                assert_eq!(e.missing, Missing::LayerAbsent("ow-world"))
            }
            other => panic!("expected NotYet LayerAbsent, got {other:?}"),
        }
    }
}

#[test]
fn off_a_byte_playthrough_reads_as_its_number_and_a_float_is_bad_data() {
    // Note section 1 reads CurrentPlaythrough as an int. A class declaring it as a byte (the offline world's
    // synthetic fixture does) reads the same number; another kind is bad data, not a guess.
    let mut fx = Fx::new();
    let c = gri_class(&mut fx, WILLOW_GRI, None, PropKind::Byte { enum_: None });
    let g = fx.obj(
        c,
        "WillowGameReplicationInfo_0",
        vec![("CurrentPlaythrough", Value::Byte(2))],
    );
    let field1 = count_attr(&mut fx, 1, vec![]);
    assert_eq!(value_in(&mut fx, Some(g), field1), Ok(3.0));

    let mut fx = Fx::new();
    let c = gri_class(&mut fx, WILLOW_GRI, None, PropKind::Float);
    let g = fx.obj(
        c,
        "WillowGameReplicationInfo_0",
        vec![("CurrentPlaythrough", Value::Float(1.0))],
    );
    let attr = count_attr(&mut fx, 0, vec![]);
    let got = value_in(&mut fx, Some(g), attr);
    assert!(matches!(got, Err(AttrError::BadData { .. })), "{got:?}");
}
