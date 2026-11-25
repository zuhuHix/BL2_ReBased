//! Asset output for the host: textures (TFC, mips, PNG with stored deflate), static meshes (OBJ), later skeletal
//! meshes, animations and level placement (port of `src/assets.*`).
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod actorx;
pub mod aim_profiles;
pub mod animation;
pub mod collision;
pub mod mesh;
pub mod movement;
pub mod navmesh;
pub mod obj;
pub mod placement;
pub mod png;
pub mod scene;
pub mod skeletal;
pub mod skeletal_lods;
pub mod texture;
pub mod tfc;
