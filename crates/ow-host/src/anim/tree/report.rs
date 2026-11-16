//! Owner: lane H7-ANIMTREE. The installed-game check: Maya's two AnimTrees built from the installed packages, the
//! class census (evaluated against `NotYet`), and driven scenarios over UModel's ActorX exports under `local/`.
//! Returns JSON text; the ignored test `anim::tree::tests::installed_report` writes it.
//!
//! - **Census:** per class, how many nodes are evaluated and how many are `NotYet`; the skeletal controls (counted,
//!   not applied); the sequence nodes' root-motion options (reported, not applied).
//! - **First person:** idle, then `Reload_Jakobs` played once on the hands tree's slot, then idle again, at 30 Hz.
//!   Every pose must be finite. At a tick where the path to the idle sequence has weight 1, the tree's pose must
//!   equal the stage 1 sampler's pose of that clip at that node's time within 1e-4 uu ([`run`] fails otherwise).
//!   The Phaselock cast (`Phase_Lock_Lift`) is also played on the `WeaponAction` special-move node with the
//!   record's blend rules; which node the game plays it on is not read.
//! - **Third person:** standing, then walking forward, with physics set to walking.
//! - Notifies crossed are reported, never applied. `NotYet` nodes with weight are passed through to their first
//!   child under [`Unsupported::PassFirstChild`] and counted.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ow_assets::animation::read_psa_file;
use ow_assets::skeletal::{SkeletalMesh, read_psk_file};
use ow_core::fmt::{fmt_g, json_quote};
use ow_core::{DetMap, ObjectPath, OrderedMap, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;

use super::data::{Kind, TreeDef};
use super::eval::{ClipInfo, ClipLibrary, Driver, Ending, Inputs, Notify, Play, Unsupported};
use super::load::load_tree;
use crate::anim::pose::{Skeleton, Wrap};

pub const HANDS_TREE: &str = "GD_PlayerShared.Character.AnimTree_PlayerHands";
pub const BODY_TREE: &str = "GD_PlayerShared.Character.AnimTree_Player";

/// The first-person AnimSets in the order they are added: the hands component's three (its `AnimSets`), then the
/// pistol's, standing in for the weapon's set that the game adds at run time (how and when is not read).
pub const HANDS_SETS: [&str; 4] = [
    "Anim_1st_Person.1st_Person_Unarmed",
    "Anim_Siren.Siren_1st",
    "Anim_1st_Person.1st_Person_Generic",
    "Anim_1st_Person.1st_Person_Pistol",
];
/// The body component's `Base_Siren`, then the rocket launcher's set as the weapon's (not read how the game adds
/// it). It is the one installed Siren set that holds the clips the tree's additive nodes name (`ADD_Idle`,
/// `ADD_Run`); with the pistol's or rifle's set those clips are missing and the additive nodes add nothing.
pub const BODY_SETS: [&str; 2] = ["Anim_Siren.Base_Siren", "Anim_Siren.RocketLauncher_Siren"];

/// Largest difference allowed between the tree's single-clip pose and the stage 1 sampler (uu).
pub const SINGLE_CLIP_TOLERANCE: f64 = 1e-4;

#[non_exhaustive]
#[derive(Debug)]
pub enum ReportError {
    Setup(String),
    Failed(String),
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReportError::Setup(s) => write!(f, "setup: {s}"),
            ReportError::Failed(s) => write!(f, "check failed: {s}"),
        }
    }
}

impl std::error::Error for ReportError {}

fn setup(e: impl std::fmt::Display) -> ReportError {
    ReportError::Setup(e.to_string())
}

fn num(x: f64) -> String {
    if x.is_finite() {
        fmt_g(x, 7)
    } else {
        "null".into()
    }
}

/// The export file of `rel` (such as `AnimSet/Base_Siren.psa`) in the first of `roots` that has it.
fn locate(roots: &[PathBuf], rel: &str) -> Result<PathBuf, ReportError> {
    roots
        .iter()
        .map(|r| r.join(rel))
        .find(|p| p.is_file())
        .ok_or_else(|| ReportError::Setup(format!("no export {rel} under the given folders")))
}

/// Per sequence of an AnimSet: `RateScale` and notifies (time, notify class), read through the store.
struct SetTags {
    rate_scale: DetMap<String, f32>,
    notifies: DetMap<String, Vec<Notify>>,
}

fn read(space: &ObjectSpace, o: ObjRef, name: &str) -> Option<Value> {
    let class = space.heap.get(o).ok()?.class;
    let p = space.prop(class, name)?;
    space.read(o, PropAddr::scalar(p)).ok().cloned()
}

fn field<'v>(space: &ObjectSpace, v: &'v Value, name: &str) -> Option<&'v Value> {
    let s = v.as_struct()?;
    let layout = space.classes.struct_layout(s.ty)?;
    let (entry, _) = layout.field(space.names.find(name)?)?;
    s.fields.get(usize::try_from(entry).ok()?)
}

fn class_name(space: &ObjectSpace, o: ObjRef) -> String {
    space
        .heap
        .get(o)
        .ok()
        .and_then(|h| space.classes.get_layout(h.class))
        .map(|l| l.path().as_str().to_string())
        .unwrap_or_default()
}

fn set_tags(
    defs: &mut DefStore,
    space: &mut ObjectSpace,
    path: &str,
) -> Result<SetTags, ReportError> {
    let p = ObjectPath::parse(path).map_err(|_| setup(format!("bad path {path}")))?;
    let set = defs
        .find(space, &p)
        .map_err(setup)?
        .ok_or_else(|| setup(format!("no holder for {path}")))?;
    let mut tags = SetTags {
        rate_scale: DetMap::new(),
        notifies: DetMap::new(),
    };
    let Some(Value::Array(seqs)) = read(space, set, "Sequences") else {
        return Err(setup(format!("{path}.Sequences is not an array")));
    };
    for s in seqs.iter() {
        let Value::Object(Some(seq)) = s else {
            continue;
        };
        defs.ensure_loaded(space, *seq).map_err(setup)?;
        let name = match read(space, *seq, "SequenceName") {
            Some(Value::Name(n)) => space.names.text(n).to_string(),
            _ => continue,
        };
        if let Some(Value::Float(r)) = read(space, *seq, "RateScale") {
            tags.rate_scale.insert(name.clone(), r);
        }
        let mut list = Vec::new();
        if let Some(Value::Array(items)) = read(space, *seq, "Notifies") {
            for item in items.iter() {
                let time = match field(space, item, "Time") {
                    Some(Value::Float(t)) => *t,
                    _ => continue,
                };
                let what = match field(space, item, "Notify") {
                    Some(Value::Object(Some(n))) => {
                        let n = *n;
                        defs.ensure_loaded(space, n).map_err(setup)?;
                        class_name(space, n)
                    }
                    _ => "None".to_string(),
                };
                list.push(Notify::new(time, what));
            }
        }
        tags.notifies.insert(name, list);
    }
    Ok(tags)
}

/// A clip library of `sets` (object paths) for `mesh`, from the `.psa` exports under `roots` and the store's tags.
fn library(
    defs: &mut DefStore,
    space: &mut ObjectSpace,
    mesh: &SkeletalMesh,
    sets: &[&str],
    roots: &[PathBuf],
) -> Result<ClipLibrary, ReportError> {
    let mut lib = ClipLibrary::new();
    for path in sets {
        let short = path.rsplit('.').next().unwrap_or(path);
        let set = read_psa_file(&locate(roots, &format!("AnimSet/{short}.psa"))?).map_err(setup)?;
        let tags = set_tags(defs, space, path)?;
        let mut clips = Vec::new();
        for i in 0..set.sequences.len() {
            let clip = set.bind(i, mesh).map_err(setup)?;
            let rate_scale = tags.rate_scale.get(&clip.name).copied().unwrap_or(1.0);
            let notifies = tags.notifies.get(&clip.name).cloned().unwrap_or_default();
            clips.push(ClipInfo::new(clip, rate_scale, notifies));
        }
        lib.add_set(short, clips);
    }
    Ok(lib)
}

fn census(tree: &TreeDef) -> String {
    let mut by_class: OrderedMap<String, (usize, usize)> = OrderedMap::new();
    let mut classes: Vec<&str> = tree.nodes.values().map(|n| n.class.as_str()).collect();
    classes.sort_unstable();
    classes.dedup();
    for c in classes {
        by_class.insert(c.to_string(), (0, 0));
    }
    let mut root_motion = Vec::new();
    for (id, n) in tree.nodes.iter() {
        if let Some(e) = by_class.get_mut(&n.class) {
            if n.kind.is_not_yet() {
                e.1 += 1;
            } else {
                e.0 += 1;
            }
        }
        if let Kind::Sequence(p) = &n.kind
            && (p.root_bone_option != [0; 3] || p.root_rotation_option != [0; 3])
        {
            root_motion.push(format!(
                "{{\"node\":{id},\"clip\":{},\"root_bone_option\":{:?},\"root_rotation_option\":{:?}}}",
                json_quote(p.clip.as_deref().unwrap_or("None")),
                p.root_bone_option,
                p.root_rotation_option
            ));
        }
    }
    let evaluated: usize = by_class.values().map(|v| v.0).sum();
    let not_yet: usize = by_class.values().map(|v| v.1).sum();
    let rows: Vec<String> = by_class
        .iter()
        .map(|(c, (e, n))| {
            format!(
                "{{\"class\":{},\"evaluated\":{e},\"not_yet\":{n}}}",
                json_quote(c)
            )
        })
        .collect();
    let controls: Vec<String> = tree
        .skel_controls
        .iter()
        .map(|c| {
            format!(
                "{{\"bone\":{},\"class\":{}}}",
                json_quote(&c.bone),
                json_quote(&c.class)
            )
        })
        .collect();
    format!(
        "{{\"tree\":{},\"file\":{},\"nodes\":{},\"depth\":{},\"evaluated\":{evaluated},\"not_yet\":{not_yet},\
         \"classes\":[{}],\"skel_controls_not_applied\":[{}],\"root_motion_options_not_applied\":[{}]}}",
        json_quote(&tree.path),
        json_quote(&tree.file),
        tree.nodes.len(),
        super::graph::check(tree).unwrap_or(0),
        rows.join(","),
        controls.join(","),
        root_motion.join(",")
    )
}

/// Range of one pose: finite, farthest bone from the root, and the model-space positions.
struct PoseStats {
    finite: bool,
    farthest: f64,
}

fn stats(
    skeleton: &Skeleton,
    driver: &mut Driver<'_>,
) -> Result<(PoseStats, Vec<[f64; 3]>), ReportError> {
    let locals = driver
        .locals()
        .map_err(|e| ReportError::Failed(e.to_string()))?;
    let pose = skeleton
        .pose_from_locals(locals)
        .map_err(|e| ReportError::Failed(e.to_string()))?;
    let positions: Vec<[f64; 3]> = pose.model.iter().map(|m| m.translation).collect();
    let root = positions.first().copied().unwrap_or([0.0; 3]);
    let finite = pose.model.iter().all(|m| m.is_finite());
    let farthest = positions
        .iter()
        .map(|p| {
            let d = [p[0] - root[0], p[1] - root[1], p[2] - root[2]];
            ow_core::num::sqrt_f64(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
        })
        .fold(0.0, f64::max);
    Ok((PoseStats { finite, farthest }, positions))
}

/// Ticks `driver` `ticks` times at `dt`, recording each pose; fails on a non-finite pose.
fn run_ticks(
    skeleton: &Skeleton,
    driver: &mut Driver<'_>,
    ticks: usize,
    dt: f64,
    phase: &str,
    rows: &mut Vec<String>,
) -> Result<(f64, f64), ReportError> {
    let mut far_min = f64::INFINITY;
    let mut far_max: f64 = 0.0;
    for _ in 0..ticks {
        driver
            .tick(dt)
            .map_err(|e| ReportError::Failed(format!("{phase}: {e}")))?;
        let (s, _) = stats(skeleton, driver)?;
        if !s.finite {
            return Err(ReportError::Failed(format!(
                "{phase}: a non-finite pose at {} s",
                num(driver.elapsed())
            )));
        }
        far_min = far_min.min(s.farthest);
        far_max = far_max.max(s.farthest);
    }
    rows.push(format!(
        "{{\"phase\":{},\"ticks\":{ticks},\"end_time\":{},\"farthest_bone_min\":{},\"farthest_bone_max\":{},         \"weighted_sequences_at_end\":[{}]}}",
        json_quote(phase),
        num(driver.elapsed()),
        num(far_min),
        num(far_max),
        weighted(driver)
    ));
    Ok((far_min, far_max))
}

/// The sequence nodes with weight now: node, clip, weight.
fn weighted(driver: &Driver<'_>) -> String {
    let weights = driver.effective_weights();
    let mut rows = Vec::new();
    for (id, w) in weights.iter() {
        if let Some(n) = driver.tree.nodes.get(id)
            && let Kind::Sequence(p) = &n.kind
            && *w > 1e-6
        {
            rows.push(format!(
                "{{\"node\":{id},\"clip\":{},\"add_clip\":{},\"weight\":{}}}",
                json_quote(p.clip.as_deref().unwrap_or("None")),
                json_quote(p.add_clip.as_deref().unwrap_or("None")),
                num(*w)
            ));
        }
    }
    rows.join(",")
}

fn log_json(tree: &TreeDef, driver: &Driver<'_>) -> String {
    let passed: Vec<String> = driver
        .log
        .passed_through
        .iter()
        .map(|(id, n)| {
            let class = tree.nodes.get(id).map(|n| n.class.as_str()).unwrap_or("");
            format!(
                "{{\"node\":{id},\"class\":{},\"evaluations\":{n}}}",
                json_quote(class)
            )
        })
        .collect();
    let missing: Vec<String> = driver
        .log
        .missing_clips
        .iter()
        .map(|c| json_quote(c))
        .collect();
    let bones: Vec<String> = driver
        .log
        .missing_bones
        .iter()
        .map(|c| json_quote(c))
        .collect();
    let notifies: Vec<String> = driver
        .log
        .notifies
        .iter()
        .map(|e| {
            format!(
                "{{\"node\":{},\"clip\":{},\"time\":{},\"notify\":{}}}",
                e.node,
                json_quote(&e.clip),
                num(f64::from(e.notify.time)),
                json_quote(&e.notify.what)
            )
        })
        .collect();
    format!(
        "\"not_yet_passed_through\":[{}],\"missing_clips\":[{}],\"missing_branch_bones\":[{}],\
         \"notifies_not_applied\":[{}]",
        passed.join(","),
        missing.join(","),
        bones.join(","),
        notifies.join(",")
    )
}

/// For each clip whose name starts `ADD_` (any case): the largest key translation (uu) and the largest key rotation
/// (degrees, from identity) over every track, and the same for every other clip. Additive clips that hold deltas
/// sit near zero; absolute clips hold bone offsets. A structural check of what the export's additive keys are.
fn delta_check(lib: &ClipLibrary, names: &[&str]) -> String {
    let mut rows = Vec::new();
    let mut absolute_t: f64 = 0.0;
    let mut absolute_r: f64 = 0.0;
    for name in names {
        let Some((set, info)) = lib.find(name) else {
            continue;
        };
        let mut t_max: f64 = 0.0;
        let mut r_max: f64 = 0.0;
        for keys in info.clip.tracks.iter().flatten() {
            for k in keys {
                let t = k.translation.map(f64::from);
                t_max = t_max.max(ow_core::num::sqrt_f64(
                    t[0] * t[0] + t[1] * t[1] + t[2] * t[2],
                ));
                let w = f64::from(k.rotation[3]).abs().min(1.0);
                r_max = r_max.max(2.0 * ow_core::num::acos_f64(w).to_degrees());
            }
        }
        if name.to_ascii_uppercase().starts_with("ADD_") {
            rows.push(format!(
                "{{\"clip\":{},\"set\":{},\"max_translation\":{},\"max_rotation_degrees\":{}}}",
                json_quote(name),
                json_quote(set),
                num(t_max),
                num(r_max)
            ));
        } else {
            absolute_t = absolute_t.max(t_max);
            absolute_r = absolute_r.max(r_max);
        }
    }
    format!(
        "{{\"additive\":[{}],\"absolute_compared\":{{\"clips\":{:?},\"max_translation\":{},\"max_rotation_degrees\":{}}}}}",
        rows.join(","),
        names
            .iter()
            .filter(|n| !n.to_ascii_uppercase().starts_with("ADD_"))
            .collect::<Vec<_>>(),
        num(absolute_t),
        num(absolute_r)
    )
}

/// The whole report. `cooked` is the install's `CookedPCConsole`; `roots` are UModel export folders for
/// `GD_Siren_Streaming_SF` (meshes under `SkeletalMesh3/`, sets under `AnimSet/`).
pub fn run(cooked: &Path, roots: &[PathBuf]) -> Result<String, ReportError> {
    let mut defs = DefStore::open(
        cooked.to_path_buf(),
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .map_err(setup)?;
    let mut space = ObjectSpace::new();
    let hands = load_tree(&mut defs, &mut space, HANDS_TREE).map_err(setup)?;
    let body = load_tree(&mut defs, &mut space, BODY_TREE).map_err(setup)?;
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"format\":\"openwillow.h7_animtree_report/1\",\"census\":[{},{}],",
        census(&hands),
        census(&body)
    );

    // First person.
    let arms = read_psk_file(&locate(roots, "SkeletalMesh3/Hands_Siren.psk")?).map_err(setup)?;
    let arms_skeleton = Skeleton::new(&arms).map_err(setup)?;
    let lib = library(&mut defs, &mut space, &arms, &HANDS_SETS, roots)?;
    let mut d = Driver::new(&hands, &lib, &arms_skeleton, Inputs::default());
    d.unsupported = Unsupported::PassFirstChild;
    let dt = 1.0 / 30.0;
    let mut rows = Vec::new();
    run_ticks(&arms_skeleton, &mut d, 30, dt, "idle", &mut rows)?;

    // Single-clip check: the idle sequence's node, its time, and the stage 1 sampler.
    let idle_node = hands
        .nodes
        .iter()
        .find_map(|(id, n)| match &n.kind {
            Kind::Sequence(p) if p.clip.as_deref() == Some("Idle") => Some(*id),
            _ => None,
        })
        .ok_or_else(|| ReportError::Failed("no Idle sequence node".into()))?;
    let time = d
        .node_time(idle_node)
        .ok_or_else(|| ReportError::Failed("no idle node state".into()))?;
    let (_, tree_positions) = stats(&arms_skeleton, &mut d)?;
    let (set, info) = lib
        .find("Idle")
        .ok_or_else(|| ReportError::Failed("no Idle clip".into()))?;
    let direct = arms_skeleton
        .sample(&info.clip, time / f64::from(info.rate_scale), Wrap::Loop)
        .map_err(|e| ReportError::Failed(e.to_string()))?;
    let worst = tree_positions
        .iter()
        .zip(direct.model.iter().map(|m| m.translation))
        .flat_map(|(a, b)| (0..3).map(move |k| (a[k] - b[k]).abs()))
        .fold(0.0, f64::max);
    if worst > SINGLE_CLIP_TOLERANCE {
        return Err(ReportError::Failed(format!(
            "single-clip pose differs from the sampler by {} uu",
            num(worst)
        )));
    }
    let single = format!(
        "{{\"clip\":\"Idle\",\"set\":{},\"node\":{idle_node},\"time\":{},\"worst_bone_difference\":{}}}",
        json_quote(set),
        num(time),
        num(worst)
    );

    let slot = hands
        .nodes
        .values()
        .find(|n| matches!(n.kind, Kind::Slot))
        .map(|n| n.path.rsplit('.').next().unwrap_or("").to_string())
        .ok_or_else(|| ReportError::Failed("no slot node".into()))?;
    d.play(&slot, Play::once("Reload_Jakobs", 0.2, 0.2))
        .map_err(|e| ReportError::Failed(e.to_string()))?;
    // Tick until the one-shot has finished (at most 20 s), keeping the range of every pose on the way.
    let mut ticks = 0;
    let (mut far_min, mut far_max) = (f64::INFINITY, 0.0f64);
    while d.is_playing(&slot) && ticks < 600 {
        let (lo, hi) = run_ticks(&arms_skeleton, &mut d, 1, dt, "reload", &mut Vec::new())?;
        far_min = far_min.min(lo);
        far_max = far_max.max(hi);
        ticks += 1;
    }
    if d.is_playing(&slot) {
        return Err(ReportError::Failed("the reload one-shot did not finish within 20 s".into()));
    }
    rows.push(format!(
        "{{\"phase\":\"reload\",\"ticks\":{ticks},\"end_time\":{},\"farthest_bone_min\":{},\
         \"farthest_bone_max\":{}}}",
        num(d.elapsed()),
        num(far_min),
        num(far_max)
    ));
    run_ticks(&arms_skeleton, &mut d, 30, dt, "idle_again", &mut rows)?;
    d.play(
        "WeaponAction",
        Play::once("Phase_Lock_Lift", 0.1, 0.1).with_ending(Ending::OnAnimEnd),
    )
    .map_err(|e| ReportError::Failed(e.to_string()))?;
    run_ticks(
        &arms_skeleton,
        &mut d,
        30,
        dt,
        "phaselock_cast_special_move",
        &mut rows,
    )?;
    let first = format!(
        "\"first_person\":{{\"sets\":{:?},\"slot\":{},\"single_clip_check\":{single},\"delta_check\":{},\
         \"phases\":[{}],{}}}",
        HANDS_SETS,
        json_quote(&slot),
        delta_check(
            &lib,
            &[
                "ADD_Fire_Recoil",
                "ADD_Fire_Revolver_Recoil",
                "Idle",
                "Reload_Jakobs"
            ]
        ),
        rows.join(","),
        log_json(&hands, &d)
    );

    // Third person.
    let body_mesh =
        read_psk_file(&locate(roots, "SkeletalMesh3/Skel_SirenBody.psk")?).map_err(setup)?;
    let body_skeleton = Skeleton::new(&body_mesh).map_err(setup)?;
    let body_lib = library(&mut defs, &mut space, &body_mesh, &BODY_SETS, roots)?;
    let mut inputs = Inputs::default();
    inputs.select.insert("physics".into(), 1);
    let mut b = Driver::new(&body, &body_lib, &body_skeleton, inputs);
    b.unsupported = Unsupported::PassFirstChild;
    let mut rows = Vec::new();
    run_ticks(&body_skeleton, &mut b, 30, dt, "standing", &mut rows)?;
    b.inputs.speed = 200.0;
    run_ticks(&body_skeleton, &mut b, 60, dt, "walking_forward", &mut rows)?;
    let third = format!(
        "\"third_person\":{{\"sets\":{:?},\"delta_check\":{},\"phases\":[{}],{}}}",
        BODY_SETS,
        delta_check(
            &body_lib,
            &["ADD_Idle", "ADD_Run", "ADD_Fire", "Idle_var1", "Walk_F"]
        ),
        rows.join(","),
        log_json(&body, &b)
    );
    let _ = writeln!(out, "{first},{third}}}");
    Ok(out)
}
