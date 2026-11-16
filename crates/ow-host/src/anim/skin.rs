//! Owner: lane H6-ANIM. Linear blend skinning on the CPU: the reference the stage 2 GPU path will be tested against.
//!
//! A vertex at the bind pose moves to `sum_i w_i * (p * S_i)`, where bone `i`'s skin matrix `S_i` is its inverse
//! bind matrix followed by its posed model matrix (`inverse_bind.then(&pose)`). The weights are used as stored
//! (`ow_assets::skeletal` checks that they sum to 1); nothing is renormalised. That the game skins linearly with
//! these four weights is the standard UE3 GPU skin and is UNVERIFIED here: no native was read, and no skinned frame
//! of the running game was compared.

use ow_assets::skeletal::SkeletalMesh;

use super::pose::{AnimError, Pose, Skeleton};
use crate::frame::Affine;

/// Each bone's skin matrix for `pose`.
pub fn skin_matrices(skeleton: &Skeleton, pose: &Pose) -> Vec<Affine> {
    skeleton
        .inverse_bind
        .iter()
        .zip(&pose.model)
        .map(|(inv, m)| inv.then(m))
        .collect()
}

/// The mesh's vertices skinned by `matrices` (one per bone), in uu.
pub fn skin(mesh: &SkeletalMesh, matrices: &[Affine]) -> Result<Vec<[f32; 3]>, AnimError> {
    let mut out = Vec::with_capacity(mesh.vertices.len());
    for (vertex, v) in mesh.vertices.iter().enumerate() {
        let p = v.position.map(f64::from);
        let mut acc = [0.0f64; 3];
        for inf in &v.influences {
            let m = matrices
                .get(inf.bone)
                .ok_or(AnimError::VertexBone { vertex })?;
            let q = m.point(p);
            let w = f64::from(inf.weight);
            for k in 0..3 {
                acc[k] += w * q[k];
            }
        }
        out.push(to_f32(acc).ok_or(AnimError::NotFinite { vertex })?);
    }
    Ok(out)
}

/// The per-influence positions of vertex `v` (each bone's skin matrix alone), for the influence-spread check.
pub fn influence_positions(mesh: &SkeletalMesh, matrices: &[Affine], v: usize) -> Vec<[f64; 3]> {
    let Some(vertex) = mesh.vertices.get(v) else {
        return Vec::new();
    };
    let p = vertex.position.map(f64::from);
    vertex
        .influences
        .iter()
        .filter_map(|inf| matrices.get(inf.bone).map(|m| m.point(p)))
        .collect()
}

fn to_f32(p: [f64; 3]) -> Option<[f32; 3]> {
    if p.iter()
        .all(|c| c.is_finite() && c.abs() <= f64::from(f32::MAX))
    {
        // Float-to-float `as` rounds to nearest; the range was checked above.
        Some(p.map(|c| c as f32))
    } else {
        None
    }
}
