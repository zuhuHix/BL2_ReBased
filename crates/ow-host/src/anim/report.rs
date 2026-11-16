//! Owner: lane H6-ANIM. The installed-game check over UModel's ActorX exports of Maya's meshes and AnimSets, which
//! stay under ignored `local/` (AGENTS.md rule 1): counts, the bind-pose skin check, every clip's load, and a pose
//! check of chosen clips. It returns JSON text; the ignored test `anim::tests::installed_report` writes it.
//!
//! Checks, per mesh:
//! - **bind skin**: the vertices skinned at the bind pose against the stored vertices (largest difference, uu);
//! - **nearest bone**: each vertex's distance to the bind position of its heaviest bone (largest and median);
//! - **handedness**: with Z up and "forward" taken from the mesh (the arms: from the `Camera` bone to the middle of
//!   the two hands; the body: from the middle of the feet to the middle of the toes), the `R_` bone of a pair lies
//!   on the right (`up x forward`) and the `L_` bone on the left. A mirrored reading puts them the other way round,
//!   so this checks the export-to-game conversion of `ow_assets::actorx` against the game's own bone names. It is
//!   a structural check, not an observation of the running game.
//!
//! Per clip: frames, rate and length (`frames / rate`); for the chosen clips, poses at the start, on frame 1, at the
//! middle and at the loop end, with every bone's model-space position: every bone finite, the largest bone distance from the root, and the **influence spread** (for vertices
//! with two or more influences of weight 0.2 or more, the largest distance between the positions each bone alone
//! gives). A clip in the wrong rotation convention tears the mesh apart; the spread is reported for the convention
//! used and for the keys as stored, so the two can be compared.

use std::path::Path;

use ow_assets::actorx::ActorXError;
use ow_assets::animation::{AnimSet, BoundClip, read_psa_file};
use ow_assets::skeletal::{SkeletalMesh, read_psk_file};
use ow_core::fmt::{fmt_g, json_quote};

use super::pose::{AnimError, Pose, Skeleton, Wrap};
use super::skin::{influence_positions, skin, skin_matrices};

/// What the report reads: a mesh, its sets, and the clips to pose-check (absolute clips; additive ones, which are
/// deltas for the AnimTree's additive nodes, are listed in `additive` and only loaded).
pub struct MeshSpec<'a> {
    pub mesh: &'a str,
    pub sets: &'a [&'a str],
    pub pose_clips: &'a [&'a str],
    pub additive: &'a [&'a str],
    pub hand: Handedness,
}

/// Which bones give "forward" and which pair is checked.
#[derive(Clone, Copy)]
pub enum Handedness {
    /// `Camera` to the middle of `L_Hand` and `R_Hand`; checks `L_Hand` / `R_Hand`.
    Arms,
    /// The middle of `L_Foot`/`R_Foot` to the middle of `L_Toe`/`R_Toe`; checks `L_Thigh` / `R_Thigh` from `Hips`.
    Body,
}

/// Maya's two meshes as this lane checks them. The additive names are the sequences whose `bIsAdditive` tag is
/// true in the installed package (read with `ow-package --properties`; `local/p2/h6-anim/props.json`).
pub const MAYA: [MeshSpec<'static>; 2] = [
    MeshSpec {
        mesh: "SkeletalMesh3/Hands_Siren.psk",
        sets: &[
            "AnimSet/1st_Person_Pistol.psa",
            "AnimSet/1st_Person_AssaultRifle.psa",
            "AnimSet/1st_Person_Generic.psa",
            "AnimSet/1st_Person_Unarmed.psa",
            "AnimSet/Siren_1st.psa",
        ],
        pose_clips: &[
            "Idle",
            "Reload_Jakobs",
            "Fire_Fan_Recoil",
            "Phase_Lock_Lift",
            "Phase_Lock_Fail",
        ],
        additive: &["ADD_Fire_Recoil", "ADD_Fire_Revolver_Recoil"],
        hand: Handedness::Arms,
    },
    MeshSpec {
        mesh: "SkeletalMesh3/Skel_SirenBody.psk",
        sets: &["AnimSet/Base_Siren.psa"],
        pose_clips: &["Idle_var1", "Walk_F", "Idle_Inventory", "AA_PhaseLock"],
        additive: &[],
        hand: Handedness::Body,
    },
];

/// Why the report could not be made.
#[derive(Debug)]
pub enum ReportError {
    Load { file: String, error: ActorXError },
    Anim { what: String, error: AnimError },
    Missing(String),
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReportError::Load { file, error } => write!(f, "{file}: {error}"),
            ReportError::Anim { what, error } => write!(f, "{what}: {error}"),
            ReportError::Missing(what) => write!(f, "missing {what}"),
        }
    }
}

impl std::error::Error for ReportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ReportError::Load { error, .. } => Some(error),
            ReportError::Anim { error, .. } => Some(error),
            ReportError::Missing(_) => None,
        }
    }
}

fn num(x: f64) -> String {
    if x.is_finite() {
        fmt_g(x, 7)
    } else {
        "null".into()
    }
}

fn vec3(v: [f64; 3]) -> String {
    format!("[{},{},{}]", num(v[0]), num(v[1]), num(v[2]))
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    ow_core::num::sqrt_f64(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
}

/// The whole report for `specs`, reading files under `root` (UModel's per-package export folder).
pub fn run(root: &Path, specs: &[MeshSpec<'_>]) -> Result<String, ReportError> {
    let mut meshes = Vec::new();
    for spec in specs {
        meshes.push(mesh_report(root, spec)?);
    }
    Ok(format!(
        "{{\"format\":\"openwillow.h6_anim_report/1\",\"meshes\":[{}]}}\n",
        meshes.join(",")
    ))
}

fn load_mesh(root: &Path, rel: &str) -> Result<SkeletalMesh, ReportError> {
    read_psk_file(&root.join(rel)).map_err(|error| ReportError::Load {
        file: rel.into(),
        error,
    })
}

fn mesh_report(root: &Path, spec: &MeshSpec<'_>) -> Result<String, ReportError> {
    let mesh = load_mesh(root, spec.mesh)?;
    let skeleton = Skeleton::new(&mesh).map_err(|error| ReportError::Anim {
        what: spec.mesh.into(),
        error,
    })?;
    let bind = skeleton.bind_pose();
    let skinned =
        skin(&mesh, &skin_matrices(&skeleton, &bind)).map_err(|error| ReportError::Anim {
            what: spec.mesh.into(),
            error,
        })?;
    let mut bind_error: f64 = 0.0;
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let mut nearest = Vec::with_capacity(mesh.vertices.len());
    for (v, s) in mesh.vertices.iter().zip(&skinned) {
        for k in 0..3 {
            bind_error = bind_error.max((f64::from(v.position[k]) - f64::from(s[k])).abs());
            lo[k] = lo[k].min(f64::from(v.position[k]));
            hi[k] = hi[k].max(f64::from(v.position[k]));
        }
        let heaviest = v
            .influences
            .iter()
            .fold(None::<(usize, f32)>, |best, i| match best {
                Some((_, w)) if w >= i.weight => best,
                _ => Some((i.bone, i.weight)),
            });
        if let Some((b, _)) = heaviest {
            nearest.push(dist(
                v.position.map(f64::from),
                skeleton.bind_model[b].translation,
            ));
        }
    }
    nearest.sort_by(f64::total_cmp);
    let nearest_max = nearest.last().copied().unwrap_or(0.0);
    let nearest_median = nearest.get(nearest.len() / 2).copied().unwrap_or(0.0);
    let half = [0, 1, 2].map(|k| (hi[k] - lo[k]) / 2.0);
    let hand = handedness(&mesh, &bind, spec.hand)?;

    let mut sets = Vec::new();
    let mut checks = Vec::new();
    let mut found: Vec<&str> = Vec::new();
    for rel in spec.sets {
        let set = read_psa_file(&root.join(rel)).map_err(|error| ReportError::Load {
            file: (*rel).into(),
            error,
        })?;
        let mut seqs = Vec::new();
        let mut bound_ok = 0usize;
        for (i, seq) in set.sequences.iter().enumerate() {
            let clip = set.bind(i, &mesh);
            if clip.is_ok() {
                bound_ok += 1;
            }
            let rate = f64::from(seq.frame_rate);
            // usize to f64 is exact for any frame count a file holds.
            let length = seq.frame_count as f64 / rate;
            seqs.push(format!(
                "{{\"name\":{},\"frames\":{},\"rate\":{},\"length\":{},\"additive\":{},\"bound\":{}}}",
                json_quote(&seq.name),
                seq.frame_count,
                num(rate),
                num(length),
                spec.additive.contains(&seq.name.as_str()),
                clip.is_ok()
            ));
            // A clip name in two sets is checked once, from the first set that has it.
            if let Ok(clip) = clip
                && let Some(name) = spec.pose_clips.iter().copied().find(|c| *c == seq.name)
                && !found.contains(&name)
            {
                found.push(name);
                checks.push(clip_check(&mesh, &skeleton, &set, &clip, rel)?);
            }
        }
        sets.push(format!(
            "{{\"file\":{},\"version\":{},\"tracks\":{},\"sequences\":{},\"bound\":{},\"ignored_chunks\":{},\"clips\":[{}]}}",
            json_quote(rel),
            set.version,
            set.track_bones.len(),
            set.sequences.len(),
            bound_ok,
            ignored(&set.ignored_chunks),
            seqs.join(",")
        ));
    }
    for c in spec.pose_clips {
        if !found.contains(c) {
            return Err(ReportError::Missing(format!(
                "clip {c} in the sets of {}",
                spec.mesh
            )));
        }
    }
    Ok(format!(
        "{{\"file\":{},\"version\":{},\"bones\":{},\"vertices\":{},\"points\":{},\"triangles\":{},\"sections\":{},\
         \"materials\":[{}],\"ignored_chunks\":{},\"bind_skin_max_error\":{},\"bounds_min\":{},\"bounds_max\":{},\
         \"half_extent\":{},\"nearest_bone_max\":{},\"nearest_bone_median\":{},\"handedness\":{},\"sets\":[{}],\
         \"pose_checks\":[{}]}}",
        json_quote(spec.mesh),
        mesh.version,
        mesh.bones.len(),
        mesh.vertices.len(),
        mesh.point_count,
        mesh.triangle_count(),
        mesh.sections.len(),
        mesh.materials
            .iter()
            .map(|m| json_quote(m))
            .collect::<Vec<_>>()
            .join(","),
        ignored(&mesh.ignored_chunks),
        num(bind_error),
        vec3(lo),
        vec3(hi),
        vec3(half),
        num(nearest_max),
        num(nearest_median),
        hand,
        sets.join(","),
        checks.join(",")
    ))
}

fn ignored(list: &[(String, usize)]) -> String {
    let items: Vec<String> = list
        .iter()
        .map(|(n, c)| format!("[{},{}]", json_quote(n), c))
        .collect();
    format!("[{}]", items.join(","))
}

fn bone(mesh: &SkeletalMesh, pose: &Pose, name: &str) -> Result<[f64; 3], ReportError> {
    mesh.bone_index(name)
        .and_then(|i| pose.bone_position(i))
        .ok_or_else(|| ReportError::Missing(format!("bone {name}")))
}

fn mid(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|k| (a[k] + b[k]) / 2.0)
}

fn handedness(mesh: &SkeletalMesh, pose: &Pose, kind: Handedness) -> Result<String, ReportError> {
    let b = |n: &str| bone(mesh, pose, n);
    let (origin, front, left, right) = match kind {
        Handedness::Arms => (
            b("Camera")?,
            mid(b("L_Hand")?, b("R_Hand")?),
            b("L_Hand")?,
            b("R_Hand")?,
        ),
        Handedness::Body => {
            let feet = mid(b("L_Foot")?, b("R_Foot")?);
            let toes = mid(b("L_Toe")?, b("R_Toe")?);
            let hips = b("Hips")?;
            let shift = [0, 1, 2].map(|k| hips[k] + toes[k] - feet[k]);
            (hips, shift, b("L_Thigh")?, b("R_Thigh")?)
        }
    };
    let forward = [front[0] - origin[0], front[1] - origin[1]];
    // up x forward with up = +Z: (-forward.y, forward.x, 0).
    let side = [-forward[1], forward[0]];
    let along = |p: [f64; 3]| (p[0] - origin[0]) * side[0] + (p[1] - origin[1]) * side[1];
    let (l, r) = (along(left), along(right));
    Ok(format!(
        "{{\"forward_xy\":[{},{}],\"left_side\":{},\"right_side\":{},\"ok\":{}}}",
        num(forward[0]),
        num(forward[1]),
        num(l),
        num(r),
        l < 0.0 && r > 0.0
    ))
}

/// The same clip with every non-root track's rotation conjugated back: the keys as the file stores them.
fn as_stored(mesh: &SkeletalMesh, clip: &BoundClip) -> BoundClip {
    let tracks = clip
        .tracks
        .iter()
        .enumerate()
        .map(|(b, t)| {
            t.as_ref().map(|keys| {
                keys.iter()
                    .map(|k| {
                        let mut k = *k;
                        if mesh.bones[b].parent.is_some() {
                            k.rotation = [
                                -k.rotation[0],
                                -k.rotation[1],
                                -k.rotation[2],
                                k.rotation[3],
                            ];
                        }
                        k
                    })
                    .collect()
            })
        })
        .collect();
    BoundClip::new(clip.name.clone(), clip.frame_rate, clip.frame_count, tracks)
}

/// (largest, 99th percentile) influence spread at `pose`.
fn spread(mesh: &SkeletalMesh, skeleton: &Skeleton, pose: &Pose) -> (f64, f64) {
    let m = skin_matrices(skeleton, pose);
    let mut all = Vec::new();
    for (v, vertex) in mesh.vertices.iter().enumerate() {
        let heavy: Vec<usize> = vertex
            .influences
            .iter()
            .enumerate()
            .filter(|(_, i)| i.weight >= 0.2)
            .map(|(k, _)| k)
            .collect();
        if heavy.len() < 2 {
            continue;
        }
        let ps = influence_positions(mesh, &m, v);
        let mut worst: f64 = 0.0;
        for &a in &heavy {
            for &b in &heavy {
                if let (Some(pa), Some(pb)) = (ps.get(a), ps.get(b)) {
                    worst = worst.max(dist(*pa, *pb));
                }
            }
        }
        all.push(worst);
    }
    all.sort_by(f64::total_cmp);
    let max = all.last().copied().unwrap_or(0.0);
    let p99 = all.get(all.len() * 99 / 100).copied().unwrap_or(0.0);
    (max, p99)
}

fn clip_check(
    mesh: &SkeletalMesh,
    skeleton: &Skeleton,
    set: &AnimSet,
    clip: &BoundClip,
    file: &str,
) -> Result<String, ReportError> {
    let what = format!("{file} {}", clip.name);
    let rate = f64::from(clip.frame_rate);
    // usize to f64 is exact for any frame count a file holds.
    let length = clip.frame_count as f64 / rate;
    let stored = as_stored(mesh, clip);
    let mut samples = Vec::new();
    for (label, t) in [
        ("start", 0.0),
        ("frame_1", 1.0 / rate),
        ("middle", length / 2.0),
        ("loop_end", length - 1e-6),
    ] {
        let pose = skeleton
            .sample(clip, t, Wrap::Loop)
            .map_err(|error| ReportError::Anim {
                what: what.clone(),
                error,
            })?;
        let finite = pose.model.iter().all(|m| m.is_finite());
        let root = pose.bone_position(0).unwrap_or([0.0; 3]);
        let mut far: f64 = 0.0;
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for m in &pose.model {
            far = far.max(dist(m.translation, root));
            for k in 0..3 {
                lo[k] = lo[k].min(m.translation[k]);
                hi[k] = hi[k].max(m.translation[k]);
            }
        }
        let (s_max, s_p99) = spread(mesh, skeleton, &pose);
        let alt = skeleton
            .sample(&stored, t, Wrap::Loop)
            .map_err(|error| ReportError::Anim {
                what: what.clone(),
                error,
            })?;
        let (a_max, a_p99) = spread(mesh, skeleton, &alt);
        samples.push(format!(
            "{{\"at\":{},\"time\":{},\"finite\":{},\"farthest_bone_from_root\":{},\"bones_min\":{},\"bones_max\":{},\
             \"spread_max\":{},\"spread_p99\":{},\"stored_spread_max\":{},\"stored_spread_p99\":{},\"bones\":[{}]}}",
            json_quote(label),
            num(t),
            finite,
            num(far),
            vec3(lo),
            vec3(hi),
            num(s_max),
            num(s_p99),
            num(a_max),
            num(a_p99),
            pose.model.iter().map(|m| vec3(m.translation)).collect::<Vec<_>>().join(",")
        ));
    }
    Ok(format!(
        "{{\"set\":{},\"clip\":{},\"tracks\":{},\"frames\":{},\"rate\":{},\"length\":{},\"samples\":[{}]}}",
        json_quote(file),
        json_quote(&clip.name),
        set.track_bones.len(),
        clip.frame_count,
        num(rate),
        num(length),
        samples.join(",")
    ))
}
