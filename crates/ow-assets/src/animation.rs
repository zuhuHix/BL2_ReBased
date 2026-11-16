//! Owner: lane H6-ANIM (host track, stage 1). Animation sequences as plain data, read from UModel's ActorX `.psa`
//! export of an `AnimSet`: the set's track bones, and per sequence its name, frame rate, frame count and one key
//! (rotation and translation, relative to the parent bone) per track per frame. In the game's frame.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-53-*`.
//!
//! **Why an export.** Our own decoders do not read `AnimSequence`'s compressed tracks from the packages (no public
//! spec, none recovered: AGENTS.md rule 4), so this reads UModel's ActorX export (chunk format in
//! [`crate::actorx`]). Native decoding is an open item. The sequence's tagged properties (`NumFrames`,
//! `SequenceLength`, `RateScale`) are readable with our own reader and are the oracle the frame counts and rates
//! here are compared with (`ow-host` `anim::report`).
//!
//! **What the file holds.** `BONENAMES`: the set's track bones (UModel writes no parents or poses there).
//! `ANIMINFO`: one 168-byte record per sequence (name, group, bone count, the frame rate, the first frame and the
//! frame count). `ANIMKEYS`: 32-byte keys (translation, rotation, a time field that is not read), frame by frame,
//! each frame one key per track, the sequences one after another. `SCALEKEYS` with any record is [`NotYet`]: no
//! scale track is read. As read from UModel's exporter (see `actorx`): every frame from 0 to the frame count minus
//! one is written as a full key, so the file holds no key-reduction; and the rate is the sequence's
//! `NumFrames / SequenceLength * RateScale`, so `RateScale` is already folded in.
//!
//! **Checks.** The bone count of every sequence equals the track table's; frame counts and rates are positive
//! (rates finite); each sequence's first frame is the sum of the frames before it; the key table holds exactly the
//! frames times the tracks; every key is finite with a unit quaternion.
//!
//! **Binding to a mesh** ([`AnimSet::bind`]): each track is matched to the mesh bone of the same name (a track with
//! no such bone is an error), and its rotations are put in [`BoneTransform`]'s convention the same way the mesh's
//! bones are (conjugated for every bone but the mesh's root). A mesh bone with no track keeps its bind pose. That the
//! keys follow the mesh bones' convention is checked on the installed clips by the influence spread of
//! `ow-host`'s `anim::report`, not in the running game.

use std::collections::BTreeMap;
use std::path::Path;

use ow_core::NotYet;

use crate::actorx::{
    self, ActorXError, all_finite, f32_at, fixed_name, i32_at, near_unit, quat_at,
    unmirror_position, unmirror_quat, vec3_at,
};
use crate::skeletal::{BoneTransform, SkeletalMesh, conjugate, read_bones};

/// One key as the file stores it, after the frame conversion: relative to the parent, not yet in
/// [`BoneTransform`]'s convention (see [`AnimSet::bind`]).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawKey {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
}

/// One sequence of a set.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct AnimSequence {
    pub name: String,
    pub group: String,
    /// Frames per second, as the file gives it (the sequence's `RateScale` folded in, see the module notes).
    pub frame_rate: f32,
    /// Keys per track (at least 1).
    pub frame_count: usize,
    /// The file's `TrackTime` (UModel writes the frame count).
    pub track_time: f32,
    /// `frame_count` frames of one key per track, frame-major.
    pub keys: Vec<RawKey>,
}

impl AnimSequence {
    /// The key of `track` at `frame`.
    pub fn key(&self, frame: usize, track: usize, track_count: usize) -> Option<&RawKey> {
        frame
            .checked_mul(track_count)
            .and_then(|f| f.checked_add(track))
            .and_then(|i| self.keys.get(i))
    }
}

/// An animation set: its track bones and its sequences.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct AnimSet {
    /// The `ANIMHEAD` type flag.
    pub version: u32,
    pub track_bones: Vec<String>,
    pub sequences: Vec<AnimSequence>,
    /// Chunks the reader does not read: (name, record count).
    pub ignored_chunks: Vec<(String, usize)>,
}

/// A sequence bound to a mesh: per mesh bone, its keys in [`BoneTransform`]'s convention, or `None` where the set
/// has no track for the bone (it keeps the bind pose).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct BoundClip {
    pub name: String,
    pub frame_rate: f32,
    pub frame_count: usize,
    pub tracks: Vec<Option<Vec<BoneTransform>>>,
}

impl BoundClip {
    pub fn new(
        name: impl Into<String>,
        frame_rate: f32,
        frame_count: usize,
        tracks: Vec<Option<Vec<BoneTransform>>>,
    ) -> Self {
        BoundClip {
            name: name.into(),
            frame_rate,
            frame_count,
            tracks,
        }
    }
}

impl AnimSet {
    /// The sequence called `name`.
    pub fn sequence(&self, name: &str) -> Option<&AnimSequence> {
        self.sequences.iter().find(|s| s.name == name)
    }

    /// Binds sequence `index` to `mesh` (see the module notes).
    pub fn bind(&self, index: usize, mesh: &SkeletalMesh) -> Result<BoundClip, ActorXError> {
        let seq = self
            .sequences
            .get(index)
            .ok_or(ActorXError::IndexOutOfRange {
                chunk: "ANIMINFO",
                index,
                value: i64::try_from(index).unwrap_or(i64::MAX),
                limit: self.sequences.len(),
            })?;
        let tracks_n = self.track_bones.len();
        let root = mesh
            .bones
            .first()
            .map(|b| b.name.clone())
            .unwrap_or_default();
        if self.track_bones.first() != Some(&root) {
            return Err(ActorXError::RootTrack {
                found: self.track_bones.first().cloned(),
                root,
            });
        }
        // One map of the mesh's bone names, so binding is not quadratic in the bone count.
        let by_name: BTreeMap<&str, usize> = mesh
            .bones
            .iter()
            .enumerate()
            .map(|(i, b)| (b.name.as_str(), i))
            .collect();
        let mut tracks: Vec<Option<Vec<BoneTransform>>> = vec![None; mesh.bones.len()];
        for (t, name) in self.track_bones.iter().enumerate() {
            let bone = *by_name
                .get(name.as_str())
                .ok_or_else(|| ActorXError::UnknownTrackBone(name.clone()))?;
            if tracks[bone].is_some() {
                return Err(ActorXError::DuplicateBone(name.clone()));
            }
            let mut keys = Vec::with_capacity(seq.frame_count);
            for frame in 0..seq.frame_count {
                let k = seq.key(frame, t, tracks_n).ok_or(ActorXError::KeyCount {
                    expected: seq.frame_count * tracks_n,
                    found: seq.keys.len(),
                })?;
                let rotation = if mesh.bones[bone].parent.is_some() {
                    conjugate(k.rotation)
                } else {
                    k.rotation
                };
                keys.push(BoneTransform::new(rotation, k.translation));
            }
            tracks[bone] = Some(keys);
        }
        Ok(BoundClip::new(
            seq.name.clone(),
            seq.frame_rate,
            seq.frame_count,
            tracks,
        ))
    }
}

/// Reads a `.psa` file.
pub fn read_psa_file(path: &Path) -> Result<AnimSet, ActorXError> {
    read_psa(&actorx::read_file(path)?)
}

const READ_CHUNKS: [&str; 5] = ["ANIMHEAD", "BONENAMES", "ANIMINFO", "ANIMKEYS", "SCALEKEYS"];

struct Info {
    name: String,
    group: String,
    frame_rate: f32,
    frame_count: usize,
    track_time: f32,
}

/// Reads an ActorX animation set from its bytes.
pub fn read_psa(data: &[u8]) -> Result<AnimSet, ActorXError> {
    let chunks = actorx::chunks(data)?;
    let first = chunks
        .first()
        .map(|c| c.name.to_string())
        .unwrap_or_default();
    if first != "ANIMHEAD" {
        return Err(ActorXError::WrongFile {
            expected: "ANIMHEAD",
            found: first,
        });
    }
    let version = actorx::require(&chunks, "ANIMHEAD")?.type_flag;
    let track_bones: Vec<String> =
        read_bones(actorx::require(&chunks, "BONENAMES")?, "BONENAMES", false)?
            .into_iter()
            .map(|b| b.name)
            .collect();
    if let Some(scale) = actorx::find(&chunks, "SCALEKEYS")?
        && scale.count > 0
    {
        return Err(ActorXError::NotYet(NotYet::not_ported(
            "ActorX scale keys (SCALEKEYS with records)",
            "docs/decisions/2026-11/2026-11-16-53",
        )));
    }

    let info_chunk = actorx::require(&chunks, "ANIMINFO")?;
    info_chunk.expect_size(168)?;
    let mut infos = Vec::with_capacity(info_chunk.count);
    let mut frames_so_far = 0usize;
    for i in 0..info_chunk.count {
        let r = info_chunk.record(i);
        let name = fixed_name(&r[..64])
            .ok_or(ActorXError::BadName {
                chunk: "ANIMINFO",
                index: i,
            })?
            .to_string();
        let group = fixed_name(&r[64..128])
            .ok_or(ActorXError::BadName {
                chunk: "ANIMINFO",
                index: i,
            })?
            .to_string();
        let bones = i64::from(i32_at(r, 128));
        let track_time = f32_at(r, 148);
        let frame_rate = f32_at(r, 152);
        let first = i64::from(i32_at(r, 160));
        let frames = usize::try_from(i32_at(r, 164)).ok().filter(|&n| n > 0);
        let bones_ok = usize::try_from(bones).ok() == Some(track_bones.len());
        let (Some(frame_count), true, true) =
            (frames, bones_ok, frame_rate.is_finite() && frame_rate > 0.0)
        else {
            return Err(ActorXError::BadSequence { sequence: i });
        };
        if !track_time.is_finite() {
            return Err(ActorXError::NonFinite {
                chunk: "ANIMINFO",
                index: i,
            });
        }
        if usize::try_from(first).ok() != Some(frames_so_far) {
            return Err(ActorXError::FirstFrame {
                sequence: i,
                expected: frames_so_far,
                found: first,
            });
        }
        frames_so_far = frames_so_far
            .checked_add(frame_count)
            .ok_or(ActorXError::BadSequence { sequence: i })?;
        infos.push(Info {
            name,
            group,
            frame_rate,
            frame_count,
            track_time,
        });
    }

    let key_chunk = actorx::require(&chunks, "ANIMKEYS")?;
    key_chunk.expect_size(32)?;
    let expected = frames_so_far
        .checked_mul(track_bones.len())
        .ok_or(ActorXError::KeyCount {
            expected: usize::MAX,
            found: key_chunk.count,
        })?;
    if key_chunk.count != expected {
        return Err(ActorXError::KeyCount {
            expected,
            found: key_chunk.count,
        });
    }
    let mut next = 0usize;
    let mut sequences = Vec::with_capacity(infos.len());
    for info in infos {
        let n = info.frame_count * track_bones.len();
        let mut keys = Vec::with_capacity(n);
        for k in next..next + n {
            let r = key_chunk.record(k);
            let t = vec3_at(r, 0);
            let q = quat_at(r, 12);
            if !all_finite(&t) || !all_finite(&q) {
                return Err(ActorXError::NonFinite {
                    chunk: "ANIMKEYS",
                    index: k,
                });
            }
            if !near_unit(q) {
                return Err(ActorXError::NotUnit {
                    chunk: "ANIMKEYS",
                    index: k,
                });
            }
            keys.push(RawKey {
                translation: unmirror_position(t),
                rotation: unmirror_quat(q),
            });
        }
        next += n;
        sequences.push(AnimSequence {
            name: info.name,
            group: info.group,
            frame_rate: info.frame_rate,
            frame_count: info.frame_count,
            track_time: info.track_time,
            keys,
        });
    }
    let ignored_chunks = chunks
        .iter()
        .filter(|c| !READ_CHUNKS.contains(&c.name))
        .map(|c| (c.name.to_string(), c.count))
        .collect();
    Ok(AnimSet {
        version,
        track_bones,
        sequences,
        ignored_chunks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actorx::build::{chunk, floats, ints, name};

    // Invented values only (AGENTS.md rule 9).
    fn bone_name(n: &str) -> Vec<u8> {
        let mut r = name(n, 64);
        r.extend([0u8; 56]);
        r
    }

    fn info(n: &str, bones: i32, rate: f32, first: i32, frames: i32) -> Vec<u8> {
        let mut r = name(n, 64);
        r.extend(name("None", 64));
        r.extend(ints(&[bones, 0, 0, frames * bones]));
        r.extend(floats(&[
            0.0,
            f32::from(u16::try_from(frames).unwrap()),
            rate,
        ]));
        r.extend(ints(&[0, first, frames]));
        r
    }

    fn key(t: [f32; 3], q: [f32; 4]) -> Vec<u8> {
        let mut r = floats(&t);
        r.extend(floats(&q));
        r.extend(floats(&[1.0]));
        r
    }

    /// Two tracks ("Child", "Root": not the mesh's order), sequences "A" (2 frames) and "B" (1 frame).
    pub(crate) fn two_clip_psa() -> Vec<u8> {
        let mut f = chunk("ANIMHEAD", 20100422, 0, &[]);
        f.extend(chunk(
            "BONENAMES",
            0,
            120,
            &[bone_name("Root"), bone_name("Child")],
        ));
        f.extend(chunk(
            "ANIMINFO",
            0,
            168,
            &[info("A", 2, 30.0, 0, 2), info("B", 2, 15.0, 2, 1)],
        ));
        let id = [0.0, 0.0, 0.0, 1.0];
        // Stored as the export stores them (mirrored): the game's Child translation (0, 5, f) is written (0, -5, f).
        // Each frame holds the Root key, then the Child key. Root's frame-1 rotation (0, 0, 0.8, 0.6) reads as
        // (0, -0, 0.8, -0.6) in the game's frame and is kept; a reader that also conjugated the root would give
        // (-0, 0, -0.8, -0.6).
        let keys = [
            key([0.0; 3], id),
            key([0.0, -5.0, 0.0], id),
            key([0.0; 3], [0.0, 0.0, 0.8, 0.6]),
            key([0.0, -5.0, 1.0], [0.0, 0.0, 0.6, 0.8]),
            key([1.0, 0.0, 0.0], id),
            key([0.0, -5.0, 2.0], id),
        ];
        f.extend(chunk("ANIMKEYS", 0, 32, &keys));
        f.extend(chunk("SCALEKEYS", 0, 16, &[]));
        f
    }

    #[test]
    fn reads_sequences_and_keys() {
        let set = read_psa(&two_clip_psa()).unwrap();
        assert_eq!(
            set.track_bones,
            vec!["Root".to_string(), "Child".to_string()]
        );
        assert_eq!(set.sequences.len(), 2);
        let a = set.sequence("A").unwrap();
        assert_eq!(
            (a.frame_rate, a.frame_count, a.group.as_str()),
            (30.0, 2, "None")
        );
        assert_eq!(a.key(1, 1, 2).unwrap().translation, [0.0, 5.0, 1.0]);
        // (0, 0, 0.6, 0.8) written mirrored is (0, -0, 0.6, -0.8) in the game's frame.
        assert_eq!(a.key(1, 1, 2).unwrap().rotation, [0.0, -0.0, 0.6, -0.8]);
        assert_eq!(a.key(1, 0, 2).unwrap().rotation, [0.0, -0.0, 0.8, -0.6]);
        let b = set.sequence("B").unwrap();
        assert_eq!((b.frame_rate, b.frame_count), (15.0, 1));
        assert_eq!(b.key(0, 0, 2).unwrap().translation, [1.0, 0.0, 0.0]);
        assert!(set.ignored_chunks.is_empty());
    }

    #[test]
    fn refuses_inconsistent_tables() {
        let mut f = two_clip_psa();
        // Drop the last key: the key count no longer meets the frames.
        let at = f.len() - 32 - 32 - 32;
        let keys_header = find(&f, "ANIMKEYS");
        f[keys_header + 28..keys_header + 32].copy_from_slice(&5i32.to_le_bytes());
        f.drain(at..at + 32);
        assert_eq!(
            read_psa(&f),
            Err(ActorXError::KeyCount {
                expected: 6,
                found: 5
            })
        );

        let mut g = two_clip_psa();
        let info_at = find(&g, "ANIMINFO") + 32 + 168 + 160;
        g[info_at..info_at + 4].copy_from_slice(&3i32.to_le_bytes());
        assert_eq!(
            read_psa(&g),
            Err(ActorXError::FirstFrame {
                sequence: 1,
                expected: 2,
                found: 3
            })
        );

        let mut h = two_clip_psa();
        let rate_at = find(&h, "ANIMINFO") + 32 + 152;
        h[rate_at..rate_at + 4].copy_from_slice(&0.0f32.to_le_bytes());
        assert_eq!(read_psa(&h), Err(ActorXError::BadSequence { sequence: 0 }));
    }

    #[test]
    fn scale_keys_are_not_yet() {
        let mut f = two_clip_psa();
        let scale = find(&f, "SCALEKEYS");
        f[scale + 28..scale + 32].copy_from_slice(&1i32.to_le_bytes());
        f.extend([0u8; 16]);
        assert!(matches!(read_psa(&f), Err(ActorXError::NotYet(_))));
    }

    /// Byte offset of chunk `name`'s header.
    fn find(f: &[u8], name: &str) -> usize {
        let mut at = 0;
        loop {
            if fixed_name(&f[at..at + 20]) == Some(name) {
                return at;
            }
            let size = usize::try_from(i32_at(f, at + 24)).unwrap();
            let count = usize::try_from(i32_at(f, at + 28)).unwrap();
            at += 32 + size * count;
        }
    }

    #[test]
    fn binds_tracks_by_name_with_the_mesh_convention() {
        let mesh = crate::skeletal::read_psk(&crate::skeletal::tests::quad_psk(&[
            (1.0, 0, 0),
            (1.0, 1, 0),
            (1.0, 2, 1),
            (1.0, 3, 1),
        ]))
        .unwrap();
        let set = read_psa(&two_clip_psa()).unwrap();
        let clip = set.bind(0, &mesh).unwrap();
        assert_eq!((clip.frame_count, clip.frame_rate), (2, 30.0));
        // Mesh bone 1 is "Child", track 1: conjugated (a non-root bone).
        let child = clip.tracks[1].as_ref().unwrap();
        assert_eq!(child[1].translation, [0.0, 5.0, 1.0]);
        assert_eq!(child[1].rotation, [-0.0, 0.0, -0.6, -0.8]);
        // Mesh bone 0 is "Root", track 0: kept as stored, so its Z stays positive.
        let root = clip.tracks[0].as_ref().unwrap();
        assert_eq!(root[1].rotation, [0.0, -0.0, 0.8, -0.6]);
        assert!(root[1].rotation[2] > 0.0, "the root is not conjugated");
        let mut other = set.clone();
        other.track_bones[1] = "Missing".into();
        assert_eq!(
            other.bind(0, &mesh),
            Err(ActorXError::UnknownTrackBone("Missing".into()))
        );
        // A set whose first track is not the mesh's root is refused, not bound with the wrong rule.
        let mut swapped = set.clone();
        swapped.track_bones.swap(0, 1);
        assert_eq!(
            swapped.bind(0, &mesh),
            Err(ActorXError::RootTrack {
                found: Some("Child".into()),
                root: "Root".into()
            })
        );
        let mut empty = set.clone();
        empty.track_bones.clear();
        assert_eq!(
            empty.bind(0, &mesh),
            Err(ActorXError::RootTrack {
                found: None,
                root: "Root".into()
            })
        );
        assert!(set.bind(9, &mesh).is_err());
    }
}
