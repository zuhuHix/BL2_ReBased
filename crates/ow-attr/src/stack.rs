//! Owner: lane W1-ATTRMATH. Modifier stacks (architecture section 3.5): the stack formula in both
//! [`CombineForm`]s, duplicate refusal, remove-all, plain attributes, and the int and byte stores. Pure `f32`
//! arithmetic on numbers; no objects (the caller maps each `AttributeModifier` object to a [`ModifierId`]).
//!
//! The rules are NATIVE_ATTRIBUTES.md section 3, read from native code and **UNVERIFIED in the running game**. The
//! stack formula is behind the main four stats of the 69 of 69 golden weapon cards matched with the live overlay
//! (NATIVE_WEAPON_RULES.md, REALGAME_GROUND_TRUTH.md, decision 2026-10-03-04), which checks the numbers of those
//! cards, not the formula's precision.
//!
//! # The two combine forms (architecture section 5.3)
//!
//! Two reference implementations combine a stack and they disagree in precision. Both are kept, as a parameter,
//! until the game's own form is settled (real-game question 22):
//!
//! - [`CombineForm::WeaponGen`] (`weapon_gen.cpp:871-875` with the per-add sums at `1018-1030`, the same as
//!   `tools/weapon_stats.py` `combine`): the four sums are kept in single precision, the negative scales as a sum of
//!   magnitudes; the factor `(1 + up) / (1 + |down|)` and the rest are in double, rounded to single once at the end.
//!   An integer attribute truncates its base first and the whole result in double.
//! - [`CombineForm::Vm5F`] (`interp.cpp:546-561`, the VM's opcode 5F recompute): every step in single precision,
//!   `(base + pre) * ((1 + up) / (1 - down)) + post` with `down` the sum of the non-positive scales; an integer or
//!   byte attribute truncates the single-precision result. NATIVE_ATTRIBUTES.md section 3 describes this form for
//!   the game's int routine ("the same arithmetic in float").
//!
//! The default is `WeaponGen` (the form behind the item parity fields and the golden cards). The 5F fixtures of
//! `tests/vm_test.py` also pass under it (tested below), so by section 5.3's rule the default stands. Where the two
//! forms give different answers on invented inputs is pinned by `forms_differ_on_a_clip_size_with_a_negative_scale`.

use ow_core::num::{f32r, trunc_i32, trunc_i32_f64};

/// The identity of an `AttributeModifier` on a stack: a stack refuses a second modifier with the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModifierId(pub u64);

/// `AttributeModifier.Type` (`EModifierType`): 0 scale, 1 pre-add, 2 post-add.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierType {
    /// `MT_Scale`: a positive value multiplies (`1 + up`), a negative or zero one divides (`1 + |down|`).
    Scale = 0,
    /// `MT_PreAdd`: added to the base before the scale.
    PreAdd = 1,
    /// `MT_PostAdd`: added after the scale.
    PostAdd = 2,
}

impl ModifierType {
    /// The type from its stored number. Any other number is `None`: the VM's recompute ignores such a modifier
    /// (`interp.cpp`, "anything else ignored"), and the plain-attribute add does nothing with it
    /// (NATIVE_ATTRIBUTES.md section 3), so the caller skips it.
    pub fn from_raw(raw: i64) -> Option<Self> {
        match raw {
            0 => Some(Self::Scale),
            1 => Some(Self::PreAdd),
            2 => Some(Self::PostAdd),
            _ => None,
        }
    }
}

/// One modifier on a stack: its type, its value (already evaluated, single precision) and its identity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Modifier {
    /// What the value does.
    pub kind: ModifierType,
    /// The amount, as the game stores it (single precision).
    pub value: f32,
    /// The modifier object's identity.
    pub id: ModifierId,
}

/// Which reference implementation's precision a stack combine follows (module doc, architecture section 5.3).
///
/// `#[non_exhaustive]`: real-game question 22 may show a third form.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CombineForm {
    /// `interp.cpp:546-561`: sums and result all `f32`; int and byte truncate the `f32` result.
    Vm5F,
    /// `weapon_gen.cpp:871-875` + `1018-1030`: `f32` sums, `f64` factor and result, one rounding at the end.
    #[default]
    WeaponGen,
}

/// The numeric kind of the attribute property being stored (`FloatAttributeProperty`, `IntAttributeProperty`,
/// `ByteAttributeProperty`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumKind {
    /// A float attribute.
    Float,
    /// An int attribute: truncated toward zero.
    Int,
    /// A byte attribute: truncated toward zero, then the low 8 bits.
    Byte,
}

/// A combined attribute value in the kind of its property. `ow-attr`'s own type, so this file needs nothing from
/// `ow-object`; the caller turns it into a `Value`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Combined {
    /// A float attribute's value.
    Float(f32),
    /// An int attribute's value.
    Int(i32),
    /// A byte attribute's value.
    Byte(u8),
}

impl Combined {
    /// The value as a float, as the game's "read value" returns it (int and byte converted to float;
    /// NATIVE_ATTRIBUTES.md section 3).
    pub fn to_f32(self) -> f32 {
        match self {
            Self::Float(x) => x,
            // An i32 or u8 to f32 is the C conversion (round to nearest); no float-to-int cast here.
            Self::Int(i) => i as f32,
            Self::Byte(b) => f32::from(b),
        }
    }
}

/// The four single-precision sums of a stack. `down` is the non-positive scales' sum as `interp.cpp` keeps it
/// (zero or negative); `weapon_gen.cpp` keeps the same magnitude as a positive sum, which is `-down` exactly
/// (negating every term of a float sum negates the sum), so one struct serves both forms.
#[derive(Debug, Clone, Copy, Default)]
struct Sums {
    pre: f32,
    post: f32,
    up: f32,
    down: f32,
}

/// Sums each kind in stack order, one single-precision add per modifier (both oracles do so).
fn sums(mods: &[Modifier]) -> Sums {
    let mut s = Sums::default();
    for m in mods {
        match m.kind {
            // `x > 0` goes up; zero and negative go down (both oracles test `> 0`).
            ModifierType::Scale if m.value > 0.0 => s.up += m.value,
            ModifierType::Scale => s.down += m.value,
            ModifierType::PreAdd => s.pre += m.value,
            ModifierType::PostAdd => s.post += m.value,
        }
    }
    s
}

/// A byte attribute keeps the low 8 bits of the truncated value (`Value::makeByte`, `vm.cpp:54`).
fn low_byte(whole: i32) -> u8 {
    whole.to_le_bytes()[0]
}

/// A stack attribute's value from its base and its whole modifier stack, in stack order, with no clamp.
///
/// Status: UNVERIFIED (NATIVE_ATTRIBUTES.md section 3; architecture 5.3 for which form the game uses).
///
/// `NumKind::Byte` under `CombineForm::WeaponGen` has no oracle (`weapon_gen.cpp` has no byte attribute): it takes
/// the `WeaponGen` integer result and keeps its low 8 bits, as a byte store does.
pub fn combine(base: f32, mods: &[Modifier], form: CombineForm, kind: NumKind) -> Combined {
    let s = sums(mods);
    match form {
        CombineForm::Vm5F => {
            // interp.cpp:557: (float(base) + preAdd) * ((1.0f + scaleUp) / (1.0f - scaleDown)) + postAdd, all f32.
            let result = (base + s.pre) * ((1.0 + s.up) / (1.0 - s.down)) + s.post;
            match kind {
                NumKind::Float => Combined::Float(result),
                // interp.cpp:559: INT32_MIN out of range and for NaN, else truncation toward zero.
                NumKind::Int => Combined::Int(trunc_i32(result)),
                NumKind::Byte => Combined::Byte(low_byte(trunc_i32(result))),
            }
        }
        CombineForm::WeaponGen => {
            // weapon_gen.cpp:872: factor = (1.0 + scaleUp) / (1.0 + scaleDown) in double, scaleDown the magnitude.
            let factor = (1.0 + f64::from(s.up)) / (1.0 + f64::from(-s.down));
            let (base, pre, post) = (f64::from(base), f64::from(s.pre), f64::from(s.post));
            match kind {
                // weapon_gen.cpp:874: F32((start + pre) * factor + post), one rounding to single.
                NumKind::Float => Combined::Float(f32r((base + pre) * factor + post) as f32),
                // weapon_gen.cpp:873: trunc(factor * (trunc(start) + pre) + post) in double.
                NumKind::Int => Combined::Int(trunc_i32_f64(factor * (base.trunc() + pre) + post)),
                NumKind::Byte => Combined::Byte(low_byte(trunc_i32_f64(
                    factor * (base.trunc() + pre) + post,
                ))),
            }
        }
    }
}

/// A float value stored into a property of `kind` the way the attribute path converts it (`interp.cpp:559-560`):
/// truncation toward zero, `i32::MIN` out of range or for NaN, a byte keeping the low 8 bits. For the plain
/// attribute add ("integers truncate", NATIVE_ATTRIBUTES.md section 3; its out-of-range case is not read and is
/// taken to be the same conversion, UNVERIFIED).
pub fn store_as(x: f32, kind: NumKind) -> Combined {
    match kind {
        NumKind::Float => Combined::Float(x),
        NumKind::Int => Combined::Int(trunc_i32(x)),
        NumKind::Byte => Combined::Byte(low_byte(trunc_i32(x))),
    }
}

/// A plain ("simple") attribute takes a modifier by editing its stored value in place, permanently: Scale
/// multiplies by the raw value (no `1 +`), PreAdd and PostAdd add it. Removing a modifier from a plain attribute is
/// a successful no-op, so there is no `plain_remove`. Integer attributes then go through [`store_as`].
///
/// Status: UNVERIFIED (NATIVE_ATTRIBUTES.md section 3). Single precision: one operation on two floats.
pub fn plain_apply(stored: f32, m: &Modifier) -> f32 {
    match m.kind {
        ModifierType::Scale => stored * m.value,
        ModifierType::PreAdd | ModifierType::PostAdd => stored + m.value,
    }
}

/// What [`Stack::add`] did.
///
/// `#[non_exhaustive]`: the game's add also answers false for a property that is not an attribute
/// (NATIVE_ATTRIBUTES.md section 3), a case for the layer that knows properties (W2-ATTRAPI).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddOutcome {
    /// The modifier was appended; the caller recomputes the value.
    Added,
    /// A modifier with the same id is already on the stack: nothing changed (the game's add returns false).
    Duplicate,
}

/// A stack attribute's modifier list (`<Name>ModifierStack`), in the order the modifiers were added.
///
/// Each [`Modifier::value`] is a copy. The C++ recompute (`interp.cpp:550`) reads every modifier object's `Value`
/// when it recomputes, so a caller whose modifier objects can change rebuilds the list from the objects
/// ([`Stack::from_loaded`]) before each recompute instead of keeping a `Stack` across changes.
///
/// Status: UNVERIFIED (NATIVE_ATTRIBUTES.md section 3: add refuses a modifier already on the stack, remove deletes
/// every copy).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stack(Vec<Modifier>);

impl Stack {
    /// An empty stack.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// A stack read back from a loaded `<Name>ModifierStack` array, as it is: order and any repeated modifier kept
    /// (only [`Stack::add`] refuses a duplicate; an array from a package or a script may hold one twice, and both
    /// copies count in the sums and in [`Stack::remove_all`]).
    ///
    /// ```
    /// use ow_attr::stack::{CombineForm, Combined, Modifier, ModifierId, ModifierType, NumKind, Stack};
    /// let twice = Modifier { kind: ModifierType::PreAdd, value: 1.0, id: ModifierId(7) };
    /// let mut stack = Stack::from_loaded(vec![twice, twice]);
    /// assert_eq!(stack.combine(10.0, CombineForm::WeaponGen, NumKind::Float), Combined::Float(12.0));
    /// assert_eq!(stack.remove_all(ModifierId(7)), 2);
    /// ```
    pub fn from_loaded(modifiers: Vec<Modifier>) -> Self {
        Self(modifiers)
    }

    /// Appends `m` unless a modifier with the same id is already on the stack.
    pub fn add(&mut self, m: Modifier) -> AddOutcome {
        if self.0.iter().any(|held| held.id == m.id) {
            return AddOutcome::Duplicate;
        }
        self.0.push(m);
        AddOutcome::Added
    }

    /// Removes every modifier with this id and returns how many went (0: none was there, the game's false).
    pub fn remove_all(&mut self, id: ModifierId) -> usize {
        let before = self.0.len();
        self.0.retain(|held| held.id != id);
        before - self.0.len()
    }

    /// The modifiers in stack order.
    pub fn modifiers(&self) -> &[Modifier] {
        &self.0
    }

    /// The attribute's value from `base` and this stack ([`combine`]).
    pub fn combine(&self, base: f32, form: CombineForm, kind: NumKind) -> Combined {
        combine(base, &self.0, form, kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORMS: [CombineForm; 2] = [CombineForm::WeaponGen, CombineForm::Vm5F];

    fn m(kind: ModifierType, value: f32, id: u64) -> Modifier {
        Modifier {
            kind,
            value,
            id: ModifierId(id),
        }
    }
    fn scale(v: f32, id: u64) -> Modifier {
        m(ModifierType::Scale, v, id)
    }
    fn pre(v: f32, id: u64) -> Modifier {
        m(ModifierType::PreAdd, v, id)
    }
    fn post(v: f32, id: u64) -> Modifier {
        m(ModifierType::PostAdd, v, id)
    }
    fn float(c: Combined) -> f32 {
        match c {
            Combined::Float(x) => x,
            other => panic!("expected a float, got {other:?}"),
        }
    }
    fn bits_eq(got: Combined, want: f32) {
        assert_eq!(
            float(got).to_bits(),
            want.to_bits(),
            "got {got:?}, want {want:?}"
        );
    }

    // ---- tests/weapon_gen_test.py, re-expressed as numbers (CombineForm::WeaponGen) ----

    #[test]
    fn weapon_gen_damage_stack() {
        // test_modifier_stack_and_card_rounding: (10 + 2) * (1 + 0.5) / (1 + 0.2f) + 3 = 17.99999996 -> float 18.
        let mods = [scale(-0.2, 1), scale(0.5, 2), pre(2.0, 3), post(3.0, 4)];
        bits_eq(
            combine(10.0, &mods, CombineForm::WeaponGen, NumKind::Float),
            18.0,
        );
        // Stage 8 (no grip effects): 10 / (1 + 0.2f) + 3 = 11.3333333 -> float 11.333333015441895.
        let late = [scale(-0.2, 1), post(3.0, 4)];
        let got = float(combine(10.0, &late, CombineForm::WeaponGen, NumKind::Float));
        assert_eq!(f64::from(got), 11.333333015441895);
    }

    #[test]
    fn weapon_gen_clip_size_truncates() {
        // Clip: 12 / (1 + 0.2f) = 9.99999998, an integer attribute: truncated to 9.
        let got = combine(
            12.0,
            &[scale(-0.2, 1)],
            CombineForm::WeaponGen,
            NumKind::Int,
        );
        assert_eq!(got, Combined::Int(9));
    }

    #[test]
    fn weapon_gen_burst_and_accuracy_pool() {
        // test_burst_rule_goes_through_the_modifier_stack: the type's 1 + the part's 2; from zero, 2.
        assert_eq!(
            combine(1.0, &[pre(2.0, 1)], CombineForm::WeaponGen, NumKind::Int),
            Combined::Int(3)
        );
        assert_eq!(
            combine(0.0, &[pre(2.0, 1)], CombineForm::WeaponGen, NumKind::Int),
            Combined::Int(2)
        );
        // test_accuracy_pool_goes_through_the_modifier_stack: (2 + 0.5) x (1 + 0.5) = 3.75.
        bits_eq(
            combine(
                2.0,
                &[pre(0.5, 1), scale(0.5, 2)],
                CombineForm::WeaponGen,
                NumKind::Float,
            ),
            3.75,
        );
        // test_slot_grade_base_clamps_and_constant_one: spread 1.5 with a PreAdd of -0.5 or 1.5.
        bits_eq(
            combine(1.5, &[pre(-0.5, 1)], CombineForm::WeaponGen, NumKind::Float),
            1.0,
        );
        bits_eq(
            combine(1.5, &[pre(1.5, 1)], CombineForm::WeaponGen, NumKind::Float),
            3.0,
        );
    }

    #[test]
    fn weapon_gen_integer_truncates_the_base_first() {
        // weapon_stats.combine: an integer attribute's base is truncated before the sum. 2.9 + 0.5 = 3.4 -> 3 if
        // the base were kept, 2 + 0.5 = 2.5 -> 2 with it truncated first.
        assert_eq!(
            combine(2.9, &[pre(0.5, 1)], CombineForm::WeaponGen, NumKind::Int),
            Combined::Int(2)
        );
        assert_eq!(
            combine(2.9, &[pre(0.5, 1)], CombineForm::Vm5F, NumKind::Int),
            Combined::Int(3)
        );
    }

    // ---- tests/vm_test.py opcode 5F cases, re-expressed (CombineForm::Vm5F), and under the default ----

    /// (base, modifiers, kind, expected) from the 5F block of `tests/vm_test.py`.
    fn vm_5f_cases() -> Vec<(f32, Vec<Modifier>, NumKind, Combined)> {
        let speed = vec![pre(5.0, 1), scale(0.5, 2), post(3.0, 3)];
        let mut speed_down = speed.clone();
        speed_down.push(scale(-1.0, 4));
        let cap = vec![scale(0.25, 5), post(0.9, 6)];
        vec![
            // SetSpeed 10 with an empty stack: value 10.
            (10.0, vec![], NumKind::Float, Combined::Float(10.0)),
            // (20 + 5) * 1.5 + 3.
            (20.0, speed, NumKind::Float, Combined::Float(40.5)),
            // Scale -1 joins the denominator: (2 + 5) * (1.5 / 2) + 3.
            (2.0, speed_down, NumKind::Float, Combined::Float(8.25)),
            // 10 * 1.25 + 0.9 = 13.4 truncated.
            (10.0, cap.clone(), NumKind::Int, Combined::Int(13)),
            // -12.5 + 0.9 = -11.6 truncates toward zero, not to -12.
            (-10.0, cap, NumKind::Int, Combined::Int(-11)),
        ]
    }

    #[test]
    fn vm_5f_fixtures_under_vm5f() {
        for (base, mods, kind, want) in vm_5f_cases() {
            assert_eq!(
                combine(base, &mods, CombineForm::Vm5F, kind),
                want,
                "base {base}, {mods:?}"
            );
        }
    }

    #[test]
    fn vm_5f_fixtures_also_hold_under_the_default_form() {
        // Architecture 5.3: the VM's 5F fixtures must pass under the default (WeaponGen) too, or the lane stops and
        // reports. They do.
        for (base, mods, kind, want) in vm_5f_cases() {
            assert_eq!(
                combine(base, &mods, CombineForm::default(), kind),
                want,
                "base {base}, {mods:?}"
            );
        }
    }

    // ---- where the two forms differ (the ready case for architecture 5.3 and real-game question 22) ----

    #[test]
    fn forms_differ_on_a_clip_size_with_a_negative_scale() {
        // Base 12, one Scale of -0.2 (weapon_gen_test.py's clip). WeaponGen: 12 / (1 + 0.2f) in double is
        // 9.99999998 -> 9. Vm5F: 1 - (-0.2f) rounds to the float 1.20000005, 1 / that to 0.833333313, times 12 is
        // 9.99999976 which rounds to the float 10.0 -> 10. A live ClipSize with a negative scale settles it.
        let mods = [scale(-0.2, 1)];
        assert_eq!(
            combine(12.0, &mods, CombineForm::WeaponGen, NumKind::Int),
            Combined::Int(9)
        );
        assert_eq!(
            combine(12.0, &mods, CombineForm::Vm5F, NumKind::Int),
            Combined::Int(10)
        );
        // As floats both are 10.0 (WeaponGen rounds 9.99999998 once, to 10.0): only the integer store differs.
        let wg = float(combine(12.0, &mods, CombineForm::WeaponGen, NumKind::Float));
        let vm = float(combine(12.0, &mods, CombineForm::Vm5F, NumKind::Float));
        assert_eq!((wg, vm), (10.0, 10.0));
    }

    #[test]
    fn forms_differ_in_float_bits_on_invented_inputs() {
        // Invented: base 3, Scales 0.3 and -0.2. f64: 3 * (1.3000000119) / (1.2000000030) = 3.25000002, rounded
        // once to 3.25. f32: 1 + 0.3f and 1 - (-0.2f) round first, their quotient rounds to 1.08333325, times 3
        // rounds to 3.2499998 (one float below).
        let mods = [scale(0.3, 1), scale(-0.2, 2)];
        let wg = float(combine(3.0, &mods, CombineForm::WeaponGen, NumKind::Float));
        let vm = float(combine(3.0, &mods, CombineForm::Vm5F, NumKind::Float));
        assert_eq!(
            (wg.to_bits(), vm.to_bits()),
            (0x4050_0000, 0x404f_ffff),
            "{wg:?} vs {vm:?}"
        );
    }

    // ---- stores ----

    #[test]
    fn int_and_byte_edges_under_vm5f() {
        // Out of range and NaN become INT32_MIN (interp.cpp:559); a byte keeps the low 8 bits of that (0).
        let big = combine(3.0e9, &[], CombineForm::Vm5F, NumKind::Int);
        assert_eq!(big, Combined::Int(i32::MIN));
        let nan = combine(f32::NAN, &[], CombineForm::Vm5F, NumKind::Int);
        assert_eq!(nan, Combined::Int(i32::MIN));
        assert_eq!(
            combine(3.0e9, &[], CombineForm::Vm5F, NumKind::Byte),
            Combined::Byte(0)
        );
        // 250 + 10 = 260 -> low 8 bits 4; -1 -> 255.
        assert_eq!(
            combine(250.0, &[post(10.0, 1)], CombineForm::Vm5F, NumKind::Byte),
            Combined::Byte(4)
        );
        assert_eq!(
            combine(-1.0, &[], CombineForm::Vm5F, NumKind::Byte),
            Combined::Byte(255)
        );
        // 2^31 exactly is out of range; the largest float below it is not.
        assert_eq!(
            store_as(2_147_483_648.0, NumKind::Int),
            Combined::Int(i32::MIN)
        );
        assert_eq!(
            store_as(2_147_483_520.0, NumKind::Int),
            Combined::Int(2_147_483_520)
        );
        assert_eq!(
            store_as(-2_147_483_648.0, NumKind::Int),
            Combined::Int(i32::MIN)
        );
        assert_eq!(store_as(-7.9, NumKind::Int), Combined::Int(-7));
    }

    #[test]
    fn byte_under_weapon_gen_keeps_the_low_bits_of_the_integer_result() {
        assert_eq!(
            combine(
                250.0,
                &[post(10.0, 1)],
                CombineForm::WeaponGen,
                NumKind::Byte
            ),
            Combined::Byte(4)
        );
    }

    #[test]
    fn empty_stack_is_the_base_in_both_forms() {
        for form in FORMS {
            bits_eq(combine(33.35, &[], form, NumKind::Float), 33.35);
            bits_eq(combine(-0.0, &[], form, NumKind::Float), 0.0);
        }
    }

    #[test]
    fn zero_scale_counts_on_the_down_side_and_changes_nothing() {
        for form in FORMS {
            bits_eq(combine(7.0, &[scale(0.0, 1)], form, NumKind::Float), 7.0);
        }
    }

    #[test]
    fn no_clamp_a_negative_result_stays() {
        // The stack has no clamp at 0 (weapon_stats.py's old 'sum' rule had one).
        for form in FORMS {
            bits_eq(combine(1.0, &[post(-5.0, 1)], form, NumKind::Float), -4.0);
        }
    }

    #[test]
    fn to_f32_reads_every_kind() {
        assert_eq!(Combined::Float(1.5).to_f32(), 1.5);
        assert_eq!(Combined::Int(-3).to_f32(), -3.0);
        assert_eq!(Combined::Byte(200).to_f32(), 200.0);
    }

    // ---- the stack's add and remove ----

    #[test]
    fn add_refuses_a_duplicate_id_and_remove_takes_every_copy() {
        let mut stack = Stack::new();
        assert_eq!(stack.add(scale(0.5, 1)), AddOutcome::Added);
        assert_eq!(stack.add(pre(2.0, 2)), AddOutcome::Added);
        // The same modifier object again (same id, any value): refused, nothing changes.
        assert_eq!(stack.add(scale(0.5, 1)), AddOutcome::Duplicate);
        assert_eq!(stack.add(post(9.0, 1)), AddOutcome::Duplicate);
        assert_eq!(stack.modifiers().len(), 2);
        bits_eq(
            stack.combine(10.0, CombineForm::WeaponGen, NumKind::Float),
            18.0,
        );
        assert_eq!(stack.remove_all(ModifierId(1)), 1);
        assert_eq!(stack.remove_all(ModifierId(1)), 0);
        bits_eq(
            stack.combine(10.0, CombineForm::WeaponGen, NumKind::Float),
            12.0,
        );
        assert_eq!(stack.modifiers(), &[pre(2.0, 2)]);
    }

    #[test]
    fn remove_all_counts_copies_and_keeps_order() {
        // A stack built from a package can hold the same modifier twice (add refuses that, a loaded array may not);
        // remove deletes every copy.
        let mut stack =
            Stack::from_loaded(vec![pre(1.0, 7), post(2.0, 8), pre(1.0, 7), scale(0.5, 9)]);
        // Both copies count: (0 + 1 + 1) * 1.5 + 2.
        bits_eq(
            stack.combine(0.0, CombineForm::WeaponGen, NumKind::Float),
            5.0,
        );
        assert_eq!(stack.remove_all(ModifierId(7)), 2);
        assert_eq!(stack.modifiers(), &[post(2.0, 8), scale(0.5, 9)]);
    }

    #[test]
    fn stack_order_matters_for_the_single_precision_sums() {
        // Invented: 1e8 + 1 - 1e8 in f32 is 0 (1 is lost), 1e8 - 1e8 + 1 is 1. Both forms sum per add in f32.
        let lost = [pre(1.0e8, 1), pre(1.0, 2), pre(-1.0e8, 3)];
        let kept = [pre(1.0e8, 1), pre(-1.0e8, 3), pre(1.0, 2)];
        for form in FORMS {
            bits_eq(combine(0.0, &lost, form, NumKind::Float), 0.0);
            bits_eq(combine(0.0, &kept, form, NumKind::Float), 1.0);
        }
    }

    // ---- plain attributes ----

    #[test]
    fn plain_apply_scales_raw_and_adds() {
        // Scale multiplies by the raw value: 5 * 2 = 10, not 5 * (1 + 2) = 15.
        assert_eq!(plain_apply(5.0, &scale(2.0, 1)), 10.0);
        assert_eq!(plain_apply(5.0, &scale(-0.5, 1)), -2.5);
        assert_eq!(plain_apply(5.0, &pre(2.0, 1)), 7.0);
        assert_eq!(plain_apply(5.0, &post(-2.0, 1)), 3.0);
        // An int plain attribute truncates when stored.
        assert_eq!(
            store_as(plain_apply(7.0, &scale(0.5, 1)), NumKind::Int),
            Combined::Int(3)
        );
    }

    #[test]
    fn modifier_type_from_raw() {
        assert_eq!(ModifierType::from_raw(0), Some(ModifierType::Scale));
        assert_eq!(ModifierType::from_raw(1), Some(ModifierType::PreAdd));
        assert_eq!(ModifierType::from_raw(2), Some(ModifierType::PostAdd));
        assert_eq!(ModifierType::from_raw(3), None);
        assert_eq!(ModifierType::from_raw(-1), None);
    }
}
