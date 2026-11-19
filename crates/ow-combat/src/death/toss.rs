//! Owner: lane FIX-POOL-SPAWN-B (decision 2026-11-19-29). `GetItemTossVelocity`, the velocity `DropLootOnDeath`
//! drops each item with. AI-assisted (Claude Code). From NATIVE_LOOT_SPAWN.md, "GetItemTossVelocity(optional bool)",
//! alone; **every rule is UNVERIFIED in the running game**.
//!
//! **The natives, Off: `NotYet { NotRead }` before any draw.**
//! - `WillowPawn.GetItemTossVelocity`: the forward direction `F` is "a rotation the pawn reports through one of its
//!   engine interfaces". Which one (facing, view or controller) is the note's open question 3, so the native answers
//!   NotYet naming it, and draws nothing.
//! - `WillowAIPawn.GetItemTossVelocity`: with a non-empty entry in `ItemTossTargetActors`, the ballistic toss needs
//!   `ItemTossSpeed`, which the note identifies by meaning only, and a fixed vector for a flight time under 0.0001
//!   (open question 3): NotYet (hub brief). With none, the `WillowPawn` rule runs, so the same NotYet as above.
//!
//! **Reference: unbound.** `src/` has no stand-in.
//!
//! **The settled part, as functions** ([`base_toss`], [`forward_of`]), for the lane that answers question 3: the
//! arithmetic once `F` is known, with the random direction's draws (three per try on the caller's generator, the
//! loot path's one stream in Off; the first for x, the second for y, the third for z). Single precision per
//! operation. Draw order on that stream (the note's "Draw order"): per pool the death roll considers, its firing
//! `FRand`; for a firing pool, its roll with each new inventory's two `GenerateUniqueID` draws and its parts roll
//! inside it; then the toss draws of each item dropped from that pool; then the next pool.

use ow_core::num::{cos_f64, sin_f64, sqrt_f64};
use ow_core::rand::Rand15;
use ow_core::{NotYet, ParityMode};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::hit::access::get;
use crate::hit::{bind_fn, this};

/// The two natives' function paths.
pub const PAWN_TOSS: &str = "WillowGame.WillowPawn.GetItemTossVelocity";
pub const AI_PAWN_TOSS: &str = "WillowGame.WillowAIPawn.GetItemTossVelocity";

const ROTATION_OPEN: &str = "WillowPawn.GetItemTossVelocity: which rotation gives the forward direction (facing, view or controller; NATIVE_LOOT_SPAWN.md open question 3)";

/// Binds both natives in Off (module documentation); Reference binds nothing.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    bind_fn(vm, PAWN_TOSS, |_| {
        Err(NotYet::not_read(ROTATION_OPEN, RECORD).into())
    })?;
    bind_fn(vm, AI_PAWN_TOSS, |c| {
        let me = this(c, "GetItemTossVelocity")?;
        let targets = get(c.vm, me, "ItemTossTargetActors")?;
        let any = targets
            .as_ref()
            .and_then(|v| v.elements())
            .is_some_and(|e| e.iter().any(|v| matches!(v, Value::Object(Some(_)))));
        if any {
            return Err(NotYet::not_read(
                "WillowAIPawn.GetItemTossVelocity toward an ItemTossTargetActors entry (ItemTossSpeed is identified by meaning only, and the vector for a flight time under 0.0001 is open question 3)",
                RECORD,
            )
            .into());
        }
        // No target: the WillowPawn rule.
        Err(NotYet::not_read(ROTATION_OPEN, RECORD).into())
    })
}

/// The record the toss follows.
pub const RECORD: &str = "docs/verification/NATIVE_LOOT_SPAWN.md (GetItemTossVelocity)";

/// `WillowPawn`'s toss constants (the note's).
pub const FORWARD_SPEED: f32 = 200.0;
pub const RANDOM_SPEED: f32 = 50.0;
pub const UP_SPEED: f32 = 150.0;
pub const MAX_SPEED: f32 = 400.0;

/// The unit vector of a rotation's pitch and yaw (UE3 units, 65536 to a turn), by the engine's usual conversion:
/// `(cos p cos y, cos p sin y, sin p)`. Roll does not enter.
pub fn forward_of(pitch: i32, yaw: i32) -> [f32; 3] {
    let unit = std::f32::consts::PI * 2.0 / 65536.0;
    let (p, y) = (pitch as f32 * unit, yaw as f32 * unit);
    let (p, y) = (f64::from(p), f64::from(y));
    // Each function in double and rounded once (ow_core::num); the native's own precision is UNVERIFIED.
    let (cp, sp, cy, sy) = (
        cos_f64(p) as f32,
        sin_f64(p) as f32,
        cos_f64(y) as f32,
        sin_f64(y) as f32,
    );
    [cp * cy, cp * sy, sp]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The length of `v`: the single-precision square root (through double, rounded once, which is exact).
fn length(v: [f32; 3]) -> f32 {
    sqrt_f64(f64::from(dot(v, v))) as f32
}

/// The random direction: three draws per try, the first for x, the second for y and the third for z, each
/// `rand x (1 / 32767) x 2 - 1`. A try is kept when its squared length is at most 1 (a point on the sphere's surface
/// is kept), otherwise all three are drawn again; the kept point is normalised.
pub fn random_direction(r: &mut dyn Rand15) -> [f32; 3] {
    let scale = 1.0f32 / 32767.0;
    loop {
        let mut v = [0.0f32; 3];
        for c in &mut v {
            *c = f32::from(r.rand15()) * scale * 2.0 - 1.0;
        }
        let sq = dot(v, v);
        if sq <= 1.0 {
            let len = sqrt_f64(f64::from(sq)) as f32;
            return [v[0] / len, v[1] / len, v[2] / len];
        }
    }
}

/// `WillowPawn`'s toss once `forward` is known. `fixed` is the bool argument: `forward x 200 + (0, 0, 150)` with no
/// draw. Without it (what `DropLootOnDeath` uses): `forward x (velocity . forward + 200) + R x 50 + (0, 0, 150)`, `R`
/// from [`random_direction`], and a result longer than 400 scaled down to length 400.
pub fn base_toss(
    forward: [f32; 3],
    velocity: [f32; 3],
    fixed: bool,
    r: &mut dyn Rand15,
) -> [f32; 3] {
    if fixed {
        return [
            forward[0] * FORWARD_SPEED,
            forward[1] * FORWARD_SPEED,
            forward[2] * FORWARD_SPEED + UP_SPEED,
        ];
    }
    let rd = random_direction(r);
    let along = dot(velocity, forward) + FORWARD_SPEED;
    let mut v = [
        forward[0] * along + rd[0] * RANDOM_SPEED,
        forward[1] * along + rd[1] * RANDOM_SPEED,
        forward[2] * along + rd[2] * RANDOM_SPEED + UP_SPEED,
    ];
    let len = length(v);
    if len > MAX_SPEED {
        let k = MAX_SPEED / len;
        v = [v[0] * k, v[1] * k, v[2] * k];
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use ow_core::rand::CrtRand;

    /// A generator that returns the given values in turn.
    struct Seq(Vec<u16>, usize);
    impl Rand15 for Seq {
        fn rand15(&mut self) -> u16 {
            self.1 += 1;
            self.0[self.1 - 1]
        }
    }

    #[test]
    fn forward_of_turns_by_yaw_and_pitch() {
        let f = forward_of(0, 0);
        assert_eq!(f, [1.0, 0.0, 0.0]);
        let f = forward_of(0, 16384);
        assert!(
            f[0].abs() < 1e-6 && (f[1] - 1.0).abs() < 1e-6 && f[2] == 0.0,
            "{f:?}"
        );
        let f = forward_of(16384, 0);
        assert!(f[0].abs() < 1e-6 && (f[2] - 1.0).abs() < 1e-6, "{f:?}");
    }

    #[test]
    fn the_fixed_toss_draws_nothing() {
        let mut r = Seq(vec![], 0);
        let v = base_toss([1.0, 0.0, 0.0], [999.0, 0.0, 0.0], true, &mut r);
        assert_eq!(v, [200.0, 0.0, 150.0]);
        assert_eq!(r.1, 0, "no draw");
    }

    #[test]
    fn the_random_direction_retries_outside_the_ball_three_draws_a_try() {
        // Try 1: (1, 1, 1), outside. Try 2: (0.5, 0, 0) roughly: 24575 -> 0.5000, 16383 -> -0.00003, twice.
        let mut r = Seq(vec![32767, 32767, 32767, 24575, 16383, 16383], 0);
        let d = random_direction(&mut r);
        assert_eq!(r.1, 6, "two tries, three draws each");
        assert!(
            (d[0] - 1.0).abs() < 1e-3 && d[1].abs() < 1e-3 && d[2].abs() < 1e-3,
            "{d:?}"
        );
        let len = length(d);
        assert!((len - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_random_toss_adds_the_pawns_speed_and_caps_at_400() {
        // R straight up: (16383.5 is not a value; 16384 -> +0.00003, 32767 -> 1): close to (0, 0, 1).
        let mut r = Seq(vec![16384, 16384, 32766], 0);
        let v = base_toss([1.0, 0.0, 0.0], [0.0, 0.0, 0.0], false, &mut r);
        // forward x 200 + R x 50 + (0, 0, 150): about (200, 0, 200), length about 283, not capped.
        assert!(
            (v[0] - 200.0).abs() < 0.01 && v[1].abs() < 0.01 && (v[2] - 200.0).abs() < 0.01,
            "{v:?}"
        );
        // Running forward at 600: forward x 800, capped to length 400.
        let mut r = Seq(vec![16384, 16384, 32766], 0);
        let v = base_toss([1.0, 0.0, 0.0], [600.0, 0.0, 0.0], false, &mut r);
        assert!((length(v) - 400.0).abs() < 1e-3, "{v:?}");
        assert!(v[0] > 380.0 && v[2] > 0.0);
    }

    /// A VM with toy `WillowPawn` / `WillowAIPawn` classes and the two natives declared, in `mode`.
    fn toss_vm(mode: ParityMode) -> (Vm, ow_object::ids::ClassId, ow_object::ids::ClassId) {
        use ow_object::layout::{PropKind, PropertyDecl};
        let mut space = ow_object::heap::ObjectSpace::new();
        let object = space.classes.define("Core.Object", None, vec![]).unwrap();
        let pawn = space
            .classes
            .define("WillowGame.WillowPawn", Some(object), vec![])
            .unwrap();
        let targets = PropertyDecl::new(
            space.names.intern("ItemTossTargetActors"),
            PropKind::Array(Box::new(PropKind::Object { class: object })),
        );
        let ai = space
            .classes
            .define("WillowGame.WillowAIPawn", Some(pawn), vec![targets])
            .unwrap();
        let mut vm = Vm::new(
            space,
            ow_defs::DefStore::new_empty(),
            std::rc::Rc::new(ow_attr::AttributeSystem::new_empty()),
            mode,
        );
        vm.stub_policy = ow_vm::api::StubPolicy::refuse_all();
        for (path, owner, key) in [
            (PAWN_TOSS, pawn, "WillowPawn.GetItemTossVelocity(bool)"),
            (AI_PAWN_TOSS, ai, "WillowAIPawn.GetItemTossVelocity(bool)"),
        ] {
            let f =
                ow_vm::api::FunctionDecl::native(ow_core::ObjectPath::parse(path).unwrap(), key)
                    .with_owner(owner);
            vm.declare_function(f).unwrap();
        }
        register(&mut vm).unwrap();
        (vm, pawn, ai)
    }

    fn call(vm: &mut Vm, path: &str, me: ow_object::ids::ObjRef) -> Result<Value, VmError> {
        let f = vm.find_function(path)?;
        Ok(vm.call(f, Some(me), vec![])?.ret)
    }

    fn not_read_text(r: Result<Value, VmError>) -> String {
        match r {
            Err(VmError::NotYet(n)) => {
                assert_eq!(n.missing, ow_core::notyet::Missing::NotRead);
                n.to_string()
            }
            other => panic!("expected NotYet, got {other:?}"),
        }
    }

    #[test]
    fn the_toss_natives_are_not_yet_in_off_before_any_draw() {
        use ow_object::heap::Origin;
        use ow_object::ids::PropAddr;
        let (mut vm, pawn, ai) = toss_vm(ParityMode::Off);
        let before = vm.rng.crt(ow_core::rand::CrtStream::LootPool).clone();
        let p = vm.space.alloc(pawn, "Pawn", None, Origin::Runtime).unwrap();
        assert!(not_read_text(call(&mut vm, PAWN_TOSS, p)).contains("open question 3"));
        // An AI pawn with no target takes the WillowPawn rule; with one, the ballistic toss.
        let a = vm.space.alloc(ai, "Ai", None, Origin::Runtime).unwrap();
        assert!(not_read_text(call(&mut vm, AI_PAWN_TOSS, a)).contains("which rotation"));
        let class = vm.space.heap.get(a).unwrap().class;
        let prop = vm.space.prop(class, "ItemTossTargetActors").unwrap();
        let t = vm
            .space
            .alloc(pawn, "Target", None, Origin::Runtime)
            .unwrap();
        vm.space
            .write(
                a,
                PropAddr::scalar(prop),
                Value::array(vec![Value::Object(None), Value::Object(Some(t))]),
            )
            .unwrap();
        assert!(not_read_text(call(&mut vm, AI_PAWN_TOSS, a)).contains("ItemTossSpeed"));
        assert_eq!(
            *vm.rng.crt(ow_core::rand::CrtStream::LootPool),
            before,
            "nothing drawn"
        );
    }

    #[test]
    fn reference_leaves_the_toss_natives_unbound() {
        use ow_object::heap::Origin;
        let (mut vm, pawn, ai) = toss_vm(ParityMode::Reference);
        for (path, class) in [(PAWN_TOSS, pawn), (AI_PAWN_TOSS, ai)] {
            let me = vm.space.alloc(class, "P", None, Origin::Runtime).unwrap();
            assert!(
                matches!(call(&mut vm, path, me), Err(VmError::UnboundNative(_))),
                "{path}"
            );
        }
    }

    #[test]
    fn a_real_generator_gives_a_unit_direction() {
        let mut r = CrtRand::new(7);
        for _ in 0..200 {
            let d = random_direction(&mut r);
            assert!((length(d) - 1.0).abs() < 1e-5);
        }
    }
}
