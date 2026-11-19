//! Owner: lane H-MATINEE. A Matinee movement track (`InterpTrackMove`) read by property name, and its keys evaluated
//! at a time: NATIVE_KISMET_MATINEE.md, "InterpTrackMove: how keys become a pose", section "Curve evaluation".
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-19-53-*`.
//!
//! **What is read**, all by property name with class defaults applied, nothing from a binary layout: `PosTrack` and
//! `EulerTrack` (each an `InterpCurveVector`: `Points`, each with `InVal`, `OutVal`, `ArriveTangent`, `LeaveTangent`,
//! `InterpMode`; and `InterpMethod`), `LookupTrack.Points[].GroupName`, `SubTracks`, `MoveFrame`, `RotMode`,
//! `bUseQuatInterpolation`, `bDisableMovement`. Enum values are read by name from each property's (or struct field's) enum.
//!
//! **What is evaluated** ([`Curve::eval`]): the record's rules, in f32. No keys: zero. At or before the first key: the
//! first value; at or after the last: the last. Else the segment from the previous key: its mode `constant` holds the
//! previous value, `linear` lerps, any curve mode is the cubic Hermite with the previous key's leave tangent and the
//! next key's arrive tangent, multiplied by the segment's length unless the method is broken-tangent evaluation. A
//! segment of zero or negative length holds the previous value. [`MoveTrack::key_pose`] gives the keyed location and
//! the Euler angles (X roll, Y pitch, Z yaw, degrees) as rotator units, `x 65536 / 360` truncated.
//!
//! **What stays `NotYet`** ([`read_move`]): quaternion interpolation, lookup keys, sub-tracks and the look-at rotation
//! mode (none traced by the record). Composing the keyed transform with the captured initial transform and the base
//! (the move frame), and applying the pose to the actor, are not done here: the record marks who applies the pose
//! UNCERTAIN, and the world runs no Matinee op yet (`super`'s module notes).
//!
//! UNVERIFIED in the running game: all of it.

use ow_core::NotYet;
use ow_core::units::Rotator;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use super::super::{field, read};

/// The record this module implements from.
pub const RECORD: &str = "NATIVE_KISMET_MATINEE.md";

/// How a segment from a key is evaluated (the key's `InterpMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Linear,
    Constant,
    /// `CIM_CurveAuto`, `CIM_CurveUser`, `CIM_CurveBreak`, `CIM_CurveAutoClamped`: the Hermite with the stored tangents.
    Curve,
}

/// One key of a vector curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    pub time: f32,
    pub value: [f32; 3],
    pub arrive: [f32; 3],
    pub leave: [f32; 3],
    pub mode: Mode,
}

/// A vector curve (`InterpCurveVector`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Curve {
    pub keys: Vec<Key>,
    /// The method is broken-tangent evaluation: the tangents are not multiplied by the segment's length.
    pub broken: bool,
}

impl Curve {
    /// The curve at time `t` (module notes).
    pub fn eval(&self, t: f32) -> [f32; 3] {
        let (Some(first), Some(last)) = (self.keys.first(), self.keys.last()) else {
            return [0.0; 3];
        };
        if self.keys.len() == 1 || t <= first.time {
            return first.value;
        }
        if t >= last.time {
            return last.value;
        }
        let i = self.keys.iter().position(|k| t < k.time).unwrap_or(self.keys.len() - 1);
        let (a, b) = (&self.keys[i - 1], &self.keys[i]);
        let dt = b.time - a.time;
        if dt <= 0.0 {
            return a.value;
        }
        let s = (t - a.time) / dt;
        match a.mode {
            Mode::Constant => a.value,
            Mode::Linear => std::array::from_fn(|j| a.value[j] + (b.value[j] - a.value[j]) * s),
            Mode::Curve => {
                let (s2, s3) = (s * s, s * s * s);
                let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
                let h10 = s3 - 2.0 * s2 + s;
                let h01 = -2.0 * s3 + 3.0 * s2;
                let h11 = s3 - s2;
                let scale = if self.broken { 1.0 } else { dt };
                std::array::from_fn(|j| {
                    h00 * a.value[j] + h10 * (scale * a.leave[j]) + h11 * (scale * b.arrive[j]) + h01 * b.value[j]
                })
            }
        }
    }
}

/// The track's reference frame (`MoveFrame`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveFrame {
    World,
    RelativeToInitial,
}

/// The track's rotation mode (`RotMode`) as far as it is supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotMode {
    Keyframed,
    Ignore,
}

/// A movement track, read (module notes).
#[derive(Debug, Clone, PartialEq)]
pub struct MoveTrack {
    pub position: Curve,
    pub euler: Curve,
    pub frame: MoveFrame,
    pub rot_mode: RotMode,
    pub disable_movement: bool,
}

impl MoveTrack {
    /// The keyed location (uu) and rotation at time `t`, before the move frame (module notes). `bDisableMovement`
    /// evaluates at time 0.
    pub fn key_pose(&self, t: f32) -> ([f32; 3], Rotator) {
        let t = if self.disable_movement { 0.0 } else { t };
        let location = self.position.eval(t);
        let e = self.euler.eval(t);
        let units = |deg: f32| ow_core::num::trunc_i32_f64(f64::from(deg * 65536.0 / 360.0));
        (location, Rotator::new(units(e[1]), units(e[2]), units(e[0])))
    }

    /// The last key time of either curve (0 with no keys).
    pub fn length(&self) -> f32 {
        let last = |c: &Curve| c.keys.last().map_or(0.0, |k| k.time);
        last(&self.position).max(last(&self.euler))
    }
}

/// The name of byte property `name` of `o` in its enum.
fn enum_name(space: &ObjectSpace, o: ObjRef, name: &str) -> Option<String> {
    let Value::Byte(b) = read(space, o, name)? else { return None };
    let class = space.heap.get(o).ok()?.class;
    let p = space.prop(class, name)?;
    let ow_object::layout::PropKind::Byte { enum_: Some(e) } = space.classes.decl(p)?.kind else { return None };
    let layout = space.classes.enum_layout(e)?;
    layout.values().get(usize::from(b)).map(|n| space.names.text(*n).into_owned())
}

fn vec3(space: &ObjectSpace, v: &Value, name: &str) -> Result<[f32; 3], String> {
    let v = field(space, v, name).ok_or_else(|| format!("{name} is missing"))?;
    let mut out = [0.0f32; 3];
    for (i, axis) in ["X", "Y", "Z"].iter().enumerate() {
        match field(space, v, axis) {
            Some(Value::Float(f)) => out[i] = *f,
            _ => return Err(format!("{name}.{axis} is not a float")),
        }
    }
    Ok(out)
}

/// Struct field `name` of value `v` as the name its byte enum gives it (`None` for another shape).
fn field_enum(space: &ObjectSpace, v: &Value, name: &str) -> Option<String> {
    let s = v.as_struct()?;
    let layout = space.classes.struct_layout(s.ty)?;
    let (entry, decl) = layout.field(space.names.find(name)?)?;
    let ow_object::layout::PropKind::Byte { enum_: Some(e) } = decl.kind else { return None };
    let Value::Byte(b) = s.fields.get(usize::try_from(entry).ok()?)? else { return None };
    let names = space.classes.enum_layout(e)?;
    names.values().get(usize::from(*b)).map(|n| space.names.text(*n).into_owned())
}

/// A key's `InterpMode`, by its enum name.
fn mode_of(name: &str) -> Result<Mode, String> {
    match name {
        "CIM_Linear" => Ok(Mode::Linear),
        "CIM_Constant" => Ok(Mode::Constant),
        "CIM_CurveAuto" | "CIM_CurveUser" | "CIM_CurveBreak" | "CIM_CurveAutoClamped" => Ok(Mode::Curve),
        other => Err(format!("InterpMode {other} is not one of the record's six modes")),
    }
}

/// The curve in struct property `name` of `track`.
fn curve(space: &ObjectSpace, track: ObjRef, name: &str) -> Result<Curve, String> {
    let v = read(space, track, name).ok_or_else(|| format!("{name} is missing"))?;
    let Some(Value::Array(points)) = field(space, &v, "Points") else {
        return Err(format!("{name}.Points is not an array"));
    };
    let mut keys = Vec::new();
    for p in points.iter() {
        let time = match field(space, p, "InVal") {
            Some(Value::Float(f)) => *f,
            _ => return Err(format!("{name}: a key's InVal is not a float")),
        };
        let mode = match field_enum(space, p, "InterpMode") {
            Some(m) => mode_of(&m)?,
            None => return Err(format!("{name}: a key's InterpMode is not an enum byte")),
        };
        keys.push(Key {
            time,
            value: vec3(space, p, "OutVal")?,
            arrive: vec3(space, p, "ArriveTangent")?,
            leave: vec3(space, p, "LeaveTangent")?,
            mode,
        });
    }
    // The record's evaluation assumes keys in time order; a curve out of order is refused, not sorted.
    if keys.windows(2).any(|w| w[1].time < w[0].time) {
        return Err(format!("{name}: the keys are not in time order"));
    }
    // The record: method byte 2 is broken-tangent evaluation; read by name, `IMT_UseBrokenTangentEval`.
    let broken = match (field(space, &v, "InterpMethod"), field_enum(space, &v, "InterpMethod")) {
        (None, _) => false,
        (Some(_), Some(m)) => m == "IMT_UseBrokenTangentEval",
        (Some(_), None) => return Err(format!("{name}.InterpMethod is not an enum byte")),
    };
    Ok(Curve { keys, broken })
}

/// Reads movement track `track` (module notes): `Err` with a `NotYet` for what is not supported, or with a data error
/// as a `NotYet` naming the property.
pub fn read_move(space: &ObjectSpace, track: ObjRef) -> Result<MoveTrack, NotYet> {
    let not_yet = |what: String| NotYet::not_read(what, RECORD);
    let data = |e: String| NotYet::not_read(format!("an InterpTrackMove the reader refuses: {e}"), RECORD);
    if matches!(read(space, track, "bUseQuatInterpolation"), Some(Value::Bool(true))) {
        return Err(not_yet("an InterpTrackMove with quaternion interpolation".into()));
    }
    if let Some(Value::Array(subs)) = read(space, track, "SubTracks")
        && !subs.is_empty()
    {
        return Err(not_yet("an InterpTrackMove with sub-tracks".into()));
    }
    // The lookup track runs beside the keys; a key is a lookup key when its point names a group (not `None`).
    if let Some(look) = read(space, track, "LookupTrack")
        && let Some(Value::Array(points)) = field(space, &look, "Points")
        && points.iter().any(|p| match field(space, p, "GroupName") {
            Some(Value::Name(n)) => space.names.text(*n) != "None",
            _ => true,
        })
    {
        return Err(not_yet("an InterpTrackMove with lookup keys (a key that names a group)".into()));
    }
    let frame = match enum_name(space, track, "MoveFrame").as_deref() {
        Some("IMF_World") => MoveFrame::World,
        Some("IMF_RelativeToInitial") => MoveFrame::RelativeToInitial,
        other => return Err(not_yet(format!("an InterpTrackMove with MoveFrame {other:?}"))),
    };
    let rot_mode = match enum_name(space, track, "RotMode").as_deref() {
        Some("IMR_Keyframed") => RotMode::Keyframed,
        Some("IMR_Ignore") => RotMode::Ignore,
        other => return Err(not_yet(format!("an InterpTrackMove with RotMode {other:?}"))),
    };
    Ok(MoveTrack {
        position: curve(space, track, "PosTrack").map_err(data)?,
        euler: curve(space, track, "EulerTrack").map_err(data)?,
        frame,
        rot_mode,
        disable_movement: matches!(read(space, track, "bDisableMovement"), Some(Value::Bool(true))),
    })
}

#[cfg(test)]
mod tests;
