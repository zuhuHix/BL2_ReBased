//! Owner: lane H7-ANIMTREE. The driver: named inputs and requests in, a tick that advances every node's state by
//! `dt`, and a pose out. Deterministic: nodes tick in export-index order, inputs are plain values, and nothing reads
//! a clock.
//!
//! **Which rules come from where** (decision 56 lists them with their evidence):
//! - From the tree data: each node's class, children, clip names, `Rate`, `bLooping`, `bPlaying`, `CurrentTime`
//!   (the node's starting time), speed `Constraints`, blend times, `Child2Weight`, `BaseSpeed`, branch bones.
//! - From the script: `WillowAnimNodeSequence.OnInit` starts its clip when `bAutoStart` is set.
//! - From the native record (`NATIVE_PHASELOCK_PRESENTATION.md` section 2.3, read, UNVERIFIED): the special-move
//!   blend times (negative to 0; `StopOnLastFrame` and `OnAnimEnd` drop the blend-out; both dropped when the clip is
//!   shorter than their sum, unless looping; only `Loop` loops).
//! - Fitted to the tree data (four directional nodes' stored weights): forward/side weights `1 - |a| / 90 deg` and
//!   `|a| / 90 deg` for a direction `a` in the front half. The back half mirrors it, UNVERIFIED.
//! - **Clip time.** A node's time is in the clip's own seconds (`SequenceLength` units) and advances by
//!   `dt * Rate * RateScale` (times the play-rate scale of `ScaleRate` nodes above it); the export folds `RateScale`
//!   into its frame rate, so it is divided back out to sample. A one-shot advances at `|rate| * RateScale` clip
//!   seconds per real second; its blend times are real seconds, scaled into clip seconds when it starts.
//! - **Stand-ins, UNVERIFIED** (no record, script or C++ settles them):
//!   - weights move linearly to their target over the blend time;
//!   - a blend is the weight-normalised sum of translations and the sign-aligned, normalised sum of rotations;
//!   - an additive delta is scaled from identity by its weight and applied on the base (rotation: after the base
//!     for a child bone, before it for the root, which is one order in the export's own convention; translation:
//!     added); a missing additive clip, and every bone an additive clip does not track, is the identity delta;
//!   - a slot's one-shot ramps in and out linearly; [`Driver::stop`] ends it at once, with no blend-out;
//!   - a slot's or special move's source is its first linked child;
//!   - a selection past a list's last child takes the last child;
//!   - a `ScaleRate` node reached by two paths takes the first path's scale (depth-first from the root);
//!   - a sequence whose clip no AnimSet has gives the bind pose;
//!   - every playing sequence ticks whatever its weight (its notifies are reported only while it has weight);
//!   - a later AnimSet's clip wins over an earlier one of the same name;
//!   - sync groups are not applied.

use std::fmt;

use ow_assets::animation::BoundClip;
use ow_core::{DetMap, DetSet, NotYet, OrderedMap};

use super::data::{Kind, NodeId, Selector, TreeDef};
use crate::anim::math::{Quat, mul, nlerp, normalize};
use crate::anim::pose::{AnimError, Local, Skeleton, Wrap};

/// Deepest evaluation recursion (the graph check bounds the tree to the same depth).
pub const MAX_EVAL_DEPTH: usize = super::graph::MAX_DEPTH;
/// Most path steps [`Driver::effective_weights`] walks (a shared node is walked once per path).
pub const MAX_WEIGHT_VISITS: usize = 100_000;

/// One notify of a clip: when, and what (the notify object's class).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Notify {
    /// Seconds in the clip's own time (`SequenceLength` units).
    pub time: f32,
    pub what: String,
}

impl Notify {
    pub fn new(time: f32, what: impl Into<String>) -> Self {
        Notify {
            time,
            what: what.into(),
        }
    }
}

/// A clip as the tree plays it.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ClipInfo {
    pub clip: BoundClip,
    /// The sequence's `RateScale` (the export folds it into the frame rate; the tree's time is in the clip's own
    /// seconds, so it is divided back out to sample).
    pub rate_scale: f32,
    pub notifies: Vec<Notify>,
}

impl ClipInfo {
    pub fn new(clip: BoundClip, rate_scale: f32, notifies: Vec<Notify>) -> Self {
        ClipInfo {
            clip,
            rate_scale,
            notifies,
        }
    }

    /// The clip's length in its own seconds (`SequenceLength`).
    pub fn length(&self) -> f64 {
        // usize to f64 is exact for any frame count a file holds.
        self.clip.frame_count as f64 / f64::from(self.clip.frame_rate) * f64::from(self.rate_scale)
    }
}

/// The AnimSets a mesh plays from, in the order they were added.
#[derive(Debug, Clone, Default)]
pub struct ClipLibrary {
    sets: Vec<(String, OrderedMap<String, ClipInfo>)>,
}

impl ClipLibrary {
    pub fn new() -> Self {
        ClipLibrary::default()
    }

    pub fn add_set(&mut self, name: impl Into<String>, clips: Vec<ClipInfo>) {
        let map = clips
            .into_iter()
            .map(|c| (c.clip.name.clone(), c))
            .collect();
        self.sets.push((name.into(), map));
    }

    /// The clip called `name`, searched from the last set added (stand-in order, UNVERIFIED), with its set's name.
    pub fn find(&self, name: &str) -> Option<(&str, &ClipInfo)> {
        self.sets
            .iter()
            .rev()
            .find_map(|(set, clips)| clips.get(name).map(|c| (set.as_str(), c)))
    }
}

/// How a special move ends (`NATIVE_PHASELOCK_PRESENTATION.md` section 2.3: decoded enum order 0 to 3).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    StopOnLastFrame,
    OnAnimEnd,
    OnBlendOut,
    Loop,
}

/// A one-shot clip over a slot or special-move node.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Play {
    pub clip: String,
    pub rate: f32,
    pub blend_in: f32,
    pub blend_out: f32,
    pub ending: Ending,
}

impl Play {
    /// A one-shot that blends out at its end (`OnBlendOut`).
    pub fn once(clip: impl Into<String>, blend_in: f32, blend_out: f32) -> Self {
        Play {
            clip: clip.into(),
            rate: 1.0,
            blend_in,
            blend_out,
            ending: Ending::OnBlendOut,
        }
    }

    pub fn with_ending(mut self, ending: Ending) -> Self {
        self.ending = ending;
        self
    }
}

/// The driver's inputs. Selections are child indices by input name ([`Selector::input_name`]); a missing one is 0.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Inputs {
    /// Horizontal speed, uu per second.
    pub speed: f32,
    /// Move direction relative to the view, radians (0 forward, positive to the right).
    pub direction: f32,
    pub select: DetMap<String, usize>,
    pub flags: DetMap<String, bool>,
    /// The blend time of a blend list whose class has no `BlendTime` (the native callers pass one; not read).
    pub default_blend_time: f32,
}

impl Default for Inputs {
    fn default() -> Self {
        Inputs {
            speed: 0.0,
            direction: 0.0,
            select: DetMap::new(),
            flags: DetMap::new(),
            default_blend_time: 0.2,
        }
    }
}

/// What evaluation does at a [`Kind::NotYet`] node that has weight.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// Fail with the node's `NotYet`.
    Refuse,
    /// Pass its first linked child through, and count it (a labelled stand-in, for reports).
    PassFirstChild,
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    NotYet(NotYet),
    Anim(AnimError),
    TooDeep,
    /// A request named a node the tree does not have, or one of the wrong kind.
    NoSuchNode(String),
    /// A request named a clip no AnimSet has.
    NoSuchClip(String),
    /// `dt` is negative or not finite.
    BadStep,
    /// A play request's rate is not finite and above 0, or a blend time is not finite.
    BadPlay(String),
    /// An input is not finite (the name of the input).
    BadInput(&'static str),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::NotYet(n) => write!(f, "{n}"),
            EvalError::Anim(e) => write!(f, "{e}"),
            EvalError::TooDeep => write!(f, "evaluation deeper than {MAX_EVAL_DEPTH}"),
            EvalError::NoSuchNode(n) => write!(f, "no node {n} of that kind"),
            EvalError::NoSuchClip(c) => write!(f, "no AnimSet has clip {c}"),
            EvalError::BadStep => write!(f, "the time step is negative or not finite"),
            EvalError::BadPlay(why) => write!(f, "bad play request: {why}"),
            EvalError::BadInput(name) => write!(f, "input {name} is not finite"),
        }
    }
}

impl std::error::Error for EvalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EvalError::NotYet(n) => Some(n),
            EvalError::Anim(e) => Some(e),
            _ => None,
        }
    }
}

impl From<AnimError> for EvalError {
    fn from(e: AnimError) -> Self {
        EvalError::Anim(e)
    }
}

/// A one-shot in progress. Every time here is in the clip's own seconds: the request's blend times (real seconds)
/// are multiplied by the clip's speed (`|rate| * RateScale`) when it starts.
#[derive(Debug, Clone, PartialEq)]
struct Shot {
    play: Play,
    /// Clip seconds since the start.
    time: f64,
    length: f64,
    blend_in: f64,
    blend_out: f64,
    /// Clip seconds per real second.
    speed: f64,
}

impl Shot {
    /// The one-shot's weight over its source now.
    fn weight(&self) -> f64 {
        let w_in = if self.blend_in > 0.0 {
            (self.time / self.blend_in).min(1.0)
        } else {
            1.0
        };
        if self.play.ending == Ending::Loop {
            return w_in;
        }
        let left = self.length - self.time;
        let w_out = if self.blend_out > 0.0 {
            (left / self.blend_out).clamp(0.0, 1.0)
        } else {
            1.0
        };
        w_in.min(w_out)
    }

    fn finished(&self) -> bool {
        self.play.ending != Ending::Loop && self.time >= self.length
    }
}

/// One node's state.
#[derive(Debug, Clone, PartialEq, Default)]
struct State {
    /// Sequence time in the clip's own seconds.
    time: f64,
    playing: bool,
    /// Blend weights per child (lists, speed, directional).
    weights: Vec<f64>,
    shot: Option<Shot>,
    /// Play-rate multiplier from `ScaleRate` ancestors, refreshed each tick.
    rate_scale: f64,
}

/// A notify passed by a playing clip during a tick: reported, not applied.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct NotifyEvent {
    pub node: NodeId,
    pub clip: String,
    pub notify: Notify,
}

/// What evaluations met, accumulated over a run.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EvalLog {
    /// `NotYet` nodes passed through under [`Unsupported::PassFirstChild`], with how many evaluations.
    pub passed_through: DetMap<NodeId, u64>,
    /// Clips a sequence node named that no AnimSet has (the node gave the bind pose).
    pub missing_clips: DetSet<String>,
    /// Branch bones of per-bone blends that the skeleton does not have.
    pub missing_bones: DetSet<String>,
    /// Path steps [`Driver::effective_weights`] dropped at its [`MAX_WEIGHT_VISITS`] budget, summed over ticks
    /// (0 on the installed trees). Notify relevance is then incomplete; reports show the count.
    pub weight_walk_truncated: u64,
    pub notifies: Vec<NotifyEvent>,
}

/// The driver over one tree, one skeleton and one clip library.
pub struct Driver<'a> {
    pub tree: &'a TreeDef,
    pub clips: &'a ClipLibrary,
    pub skeleton: &'a Skeleton,
    pub inputs: Inputs,
    pub unsupported: Unsupported,
    pub log: EvalLog,
    state: DetMap<NodeId, State>,
    elapsed: f64,
}

impl<'a> Driver<'a> {
    /// A driver with every node at its stored state: sequences at their `CurrentTime`, playing as `bPlaying` (or
    /// `bAutoStart`) says, lists on their selected child (input 0 unless set), weights at their targets. Refuses
    /// inputs that are not finite.
    pub fn new(
        tree: &'a TreeDef,
        clips: &'a ClipLibrary,
        skeleton: &'a Skeleton,
        inputs: Inputs,
    ) -> Result<Self, EvalError> {
        check_inputs(&inputs)?;
        let mut d = Driver {
            tree,
            clips,
            skeleton,
            inputs,
            unsupported: Unsupported::Refuse,
            log: EvalLog::default(),
            state: DetMap::new(),
            elapsed: 0.0,
        };
        for (id, node) in tree.nodes.iter() {
            let mut s = State {
                rate_scale: 1.0,
                ..State::default()
            };
            if let Kind::Sequence(p) = &node.kind {
                s.time = f64::from(p.start_time);
                s.playing = p.playing || p.auto_start;
            }
            let n = node.children.len();
            if n > 0 {
                let target = d.target(*id).unwrap_or(0).min(n - 1);
                s.weights = one_hot(n, target);
                if matches!(node.kind, Kind::Directional) {
                    s.weights = directional(f64::from(d.inputs.direction), n);
                }
            }
            d.state.insert(*id, s);
        }
        Ok(d)
    }

    /// A sequence node's time (its clip's own seconds), or `None` for another kind of node.
    pub fn node_time(&self, id: NodeId) -> Option<f64> {
        match self.tree.nodes.get(&id)?.kind {
            Kind::Sequence(_) => self.state.get(&id).map(|s| s.time),
            _ => None,
        }
    }

    /// Seconds ticked so far.
    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    /// Plays `play` once over the slot or special-move node called `node_name` (its `NodeName`, or its path's last
    /// part). A special move takes the blend rules of the record; a slot takes the times as given.
    pub fn play(&mut self, node_name: &str, play: Play) -> Result<(), EvalError> {
        let (id, special) = self
            .tree
            .nodes
            .iter()
            .find_map(|(id, n)| {
                let short = n.path.rsplit('.').next().unwrap_or(&n.path);
                let named = n.name == node_name || short == node_name;
                match n.kind {
                    Kind::Slot if named => Some((*id, false)),
                    Kind::SpecialMove if named => Some((*id, true)),
                    _ => None,
                }
            })
            .ok_or_else(|| EvalError::NoSuchNode(node_name.into()))?;
        let (_, info) = self
            .clips
            .find(&play.clip)
            .ok_or_else(|| EvalError::NoSuchClip(play.clip.clone()))?;
        if !(play.rate.is_finite() && play.rate > 0.0) {
            return Err(EvalError::BadPlay(format!(
                "rate of {} must be finite and above 0",
                play.clip
            )));
        }
        if !(play.blend_in.is_finite() && play.blend_out.is_finite()) {
            return Err(EvalError::BadPlay(format!(
                "blend times of {} must be finite",
                play.clip
            )));
        }
        let length = info.length();
        // Clip seconds per real second; the blend times below are real seconds until they are scaled by it.
        let speed = f64::from(play.rate) * f64::from(info.rate_scale);
        let mut blend_in = f64::from(play.blend_in).max(0.0);
        let mut blend_out = f64::from(play.blend_out).max(0.0);
        if special {
            if matches!(play.ending, Ending::StopOnLastFrame | Ending::OnAnimEnd) {
                blend_out = 0.0;
            }
            // The record's rule, on the play length in real seconds.
            if play.ending != Ending::Loop && length / speed < blend_in + blend_out {
                blend_in = 0.0;
                blend_out = 0.0;
            }
        }
        if let Some(s) = self.state.get_mut(&id) {
            s.shot = Some(Shot {
                play,
                time: 0.0,
                length,
                blend_in: blend_in * speed,
                blend_out: blend_out * speed,
                speed,
            });
        }
        Ok(())
    }

    /// Stops the one-shot on the named slot or special-move node at once (no blend-out: a stand-in), so a `Loop`
    /// shot can end. `Ok(false)` when nothing was playing there.
    pub fn stop(&mut self, node_name: &str) -> Result<bool, EvalError> {
        let id = self
            .tree
            .nodes
            .iter()
            .find_map(|(id, n)| {
                let short = n.path.rsplit('.').next().unwrap_or(&n.path);
                let named = n.name == node_name || short == node_name;
                (named && matches!(n.kind, Kind::Slot | Kind::SpecialMove)).then_some(*id)
            })
            .ok_or_else(|| EvalError::NoSuchNode(node_name.into()))?;
        Ok(self
            .state
            .get_mut(&id)
            .and_then(|s| s.shot.take())
            .is_some())
    }

    /// Whether a one-shot is playing on the named node.
    pub fn is_playing(&self, node_name: &str) -> bool {
        self.tree.nodes.iter().any(|(id, n)| {
            let short = n.path.rsplit('.').next().unwrap_or(&n.path);
            (n.name == node_name || short == node_name)
                && self.state.get(id).is_some_and(|s| s.shot.is_some())
        })
    }

    /// The child a list, speed or condition node aims at now.
    fn target(&self, id: NodeId) -> Option<usize> {
        let node = self.tree.nodes.get(&id)?;
        match &node.kind {
            Kind::List {
                selector: Selector::Flag(path),
                ..
            } => Some(usize::from(
                self.inputs.flags.get(path).copied().unwrap_or(false),
            )),
            Kind::List { selector, .. } => Some(
                self.inputs
                    .select
                    .get(selector.input_name())
                    .copied()
                    .unwrap_or(0),
            ),
            Kind::Speed { constraints, .. } => {
                let speed = self.inputs.speed;
                let bands = constraints.len().saturating_sub(1).max(1);
                let band = constraints
                    .iter()
                    .skip(1)
                    .take_while(|c| speed >= **c)
                    .count();
                Some(band.min(bands - 1))
            }
            Kind::Root
            | Kind::Slot
            | Kind::SpecialMove
            | Kind::Additive { .. }
            | Kind::PerBone { .. }
            | Kind::ScaleRate { .. }
            | Kind::NotYet { .. } => Some(0),
            _ => None,
        }
    }

    /// Advances every node by `dt` seconds (see the module notes for the rules).
    pub fn tick(&mut self, dt: f64) -> Result<(), EvalError> {
        if !dt.is_finite() || dt < 0.0 {
            return Err(EvalError::BadStep);
        }
        check_inputs(&self.inputs)?;
        self.refresh_rate_scales();
        let ids: Vec<NodeId> = self.tree.nodes.keys().copied().collect();
        // Pass 0 moves the blend weights; pass 1 advances clip times, with the notify relevance taken from the
        // weights pass 0 left.
        let mut weights = DetMap::new();
        for pass in 0..2 {
            if pass == 1 {
                let (w, dropped) = self.weights_counted();
                weights = w;
                self.log.weight_walk_truncated += dropped;
            }
            for &id in &ids {
                let Some(node) = self.tree.nodes.get(&id) else {
                    continue;
                };
                let target = self.target(id);
                let kind = node.kind.clone();
                let n = node.children.len();
                let default_blend = f64::from(self.inputs.default_blend_time);
                let direction = f64::from(self.inputs.direction);
                let clips = self.clips;
                let relevant = weights.get(&id).is_some_and(|w| *w > 0.0);
                let mut sink = Vec::new();
                let events = if relevant {
                    &mut self.log.notifies
                } else {
                    &mut sink
                };
                let Some(s) = self.state.get_mut(&id) else {
                    continue;
                };
                match &kind {
                    Kind::List { blend_time, .. } if pass == 0 => {
                        let t = blend_time.map(f64::from).unwrap_or(default_blend);
                        approach(
                            &mut s.weights,
                            target.unwrap_or(0).min(n.saturating_sub(1)),
                            t,
                            dt,
                        );
                    }
                    Kind::Speed {
                        blend_up,
                        blend_down,
                        ..
                    } if pass == 0 => {
                        let goal = target.unwrap_or(0).min(n.saturating_sub(1));
                        let current = argmax(&s.weights);
                        let t = if goal >= current {
                            *blend_up
                        } else {
                            *blend_down
                        };
                        approach(&mut s.weights, goal, f64::from(t), dt);
                    }
                    Kind::Directional if pass == 0 => s.weights = directional(direction, n),
                    Kind::Sequence(p) if pass == 1 => {
                        if s.playing
                            && let Some(name) = &p.clip
                            && let Some((_, info)) = clips.find(name)
                        {
                            let before = s.time;
                            let length = info.length();
                            // The clip's own seconds advance by Rate times its RateScale (and the play-rate
                            // scale of ScaleRate nodes above it).
                            let mut after = before
                                + dt * f64::from(p.rate)
                                    * f64::from(info.rate_scale)
                                    * s.rate_scale;
                            let mut wrapped = false;
                            if length > 0.0 {
                                if p.looping {
                                    if after >= length {
                                        wrapped = true;
                                    }
                                    after = after.rem_euclid(length);
                                } else if after >= length {
                                    after = length;
                                    s.playing = false;
                                }
                            }
                            s.time = after;
                            notifies(events, id, name, info, before, after, wrapped);
                        }
                    }
                    Kind::Slot | Kind::SpecialMove if pass == 1 => {
                        if let Some(shot) = &mut s.shot
                            && let Some((_, info)) = clips.find(&shot.play.clip)
                        {
                            let before = shot.time;
                            let mut after = before + dt * shot.speed;
                            let mut wrapped = false;
                            if shot.play.ending == Ending::Loop && shot.length > 0.0 {
                                wrapped = after >= shot.length;
                                after = after.rem_euclid(shot.length);
                            }
                            shot.time = after;
                            let clip = shot.play.clip.clone();
                            notifies(
                                events,
                                id,
                                &clip,
                                info,
                                before,
                                after.min(shot.length),
                                wrapped,
                            );
                            if shot.finished() {
                                s.shot = None;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        self.elapsed += dt;
        Ok(())
    }

    /// Each sequence's play-rate multiplier: the product of `speed / BaseSpeed` over the `ScaleRate` nodes on the
    /// first path the depth-first walk takes from the root (a node shared by two paths takes the first).
    fn refresh_rate_scales(&mut self) {
        let speed = f64::from(self.inputs.speed);
        let mut seen: DetSet<NodeId> = DetSet::new();
        let mut stack = vec![(self.tree.root, 1.0f64)];
        while let Some((id, scale)) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let Some(node) = self.tree.nodes.get(&id) else {
                continue;
            };
            let here = match node.kind {
                Kind::ScaleRate { base_speed } if base_speed > 0.0 => {
                    scale * speed / f64::from(base_speed)
                }
                _ => scale,
            };
            if let Some(s) = self.state.get_mut(&id) {
                s.rate_scale = here;
            }
            for c in node.children.iter().rev().filter_map(|c| c.node) {
                stack.push((c, here));
            }
        }
    }

    /// Each node's total weight in the pose now: the sum, over the paths from the root, of the product of the
    /// child weights along the path (a `NotYet` node passes its weight to its first child, as evaluation does
    /// under [`Unsupported::PassFirstChild`]). Walks at most [`MAX_WEIGHT_VISITS`] path steps.
    pub fn effective_weights(&self) -> DetMap<NodeId, f64> {
        self.weights_counted().0
    }

    /// [`Driver::effective_weights`] and how many path steps it dropped at the budget.
    fn weights_counted(&self) -> (DetMap<NodeId, f64>, u64) {
        let mut out: DetMap<NodeId, f64> = DetMap::new();
        let mut stack = vec![(self.tree.root, 1.0f64)];
        let mut visits = 0usize;
        let mut dropped = 0u64;
        while let Some((id, w)) = stack.pop() {
            visits += 1;
            if visits > MAX_WEIGHT_VISITS {
                dropped += 1;
                continue;
            }
            if w <= 0.0 {
                continue;
            }
            *out.entry(id).or_insert(0.0) += w;
            let (Some(node), Some(s)) = (self.tree.nodes.get(&id), self.state.get(&id)) else {
                continue;
            };
            let first = node.children.iter().find_map(|c| c.node);
            let link = |k: usize| node.children.get(k).and_then(|c| c.node);
            match &node.kind {
                Kind::Root | Kind::ScaleRate { .. } | Kind::NotYet { .. } => {
                    stack.extend(first.map(|c| (c, w)))
                }
                Kind::List { .. } | Kind::Speed { .. } | Kind::Directional => {
                    let total: f64 = s.weights.iter().sum();
                    if total > 0.0 {
                        for (k, cw) in s.weights.iter().enumerate() {
                            stack.extend(link(k).map(|c| (c, w * cw / total)));
                        }
                    }
                }
                Kind::Additive { child2_weight } | Kind::PerBone { child2_weight, .. } => {
                    stack.extend(link(0).map(|c| (c, w)));
                    stack.extend(link(1).map(|c| (c, w * f64::from(*child2_weight))));
                }
                Kind::Slot | Kind::SpecialMove => {
                    let shot = s.shot.as_ref().map_or(0.0, Shot::weight);
                    stack.extend(first.map(|c| (c, w * (1.0 - shot))));
                }
                Kind::Sequence(_) => {}
            }
        }
        (out, dropped)
    }

    /// The pose now: one local transform per skeleton bone.
    pub fn locals(&mut self) -> Result<Vec<Local>, EvalError> {
        let mut memo: Memo = DetMap::new();
        self.eval(self.tree.root, 0, false, &mut memo)
    }

    /// Node `id`'s pose. `additive` is set below an additive input: there a missing clip is the identity delta, not
    /// the bind pose.
    fn eval(
        &mut self,
        id: NodeId,
        depth: usize,
        additive: bool,
        memo: &mut Memo,
    ) -> Result<Vec<Local>, EvalError> {
        if depth > MAX_EVAL_DEPTH {
            return Err(EvalError::TooDeep);
        }
        if let Some(p) = memo.get(&(id, additive)) {
            return Ok(p.clone());
        }
        let tree = self.tree;
        let node = tree
            .nodes
            .get(&id)
            .ok_or_else(|| EvalError::NoSuchNode(id.to_string()))?;
        let state = self.state.get(&id).cloned().unwrap_or_default();
        let bind = if additive {
            identity(self.skeleton.bone_count())
        } else {
            self.skeleton.bind.clone()
        };
        let first_child = node.children.iter().find_map(|c| c.node);
        let out = match &node.kind {
            Kind::Root | Kind::ScaleRate { .. } => match first_child {
                Some(c) => self.eval(c, depth + 1, additive, memo)?,
                None => bind,
            },
            Kind::Sequence(p) => {
                let base = self.sample_named(p.clip.as_deref(), state.time, p.looping, additive)?;
                match &p.add_clip {
                    Some(add) => {
                        let delta = self.sample_named(Some(add), state.time, p.looping, true)?;
                        self.apply_additive(&base, &delta, 1.0)
                    }
                    None => base,
                }
            }
            Kind::List { .. } | Kind::Speed { .. } | Kind::Directional => {
                let mut parts = Vec::new();
                for (k, child) in node.children.iter().enumerate() {
                    let w = state.weights.get(k).copied().unwrap_or(0.0);
                    if w <= 0.0 {
                        continue;
                    }
                    let pose = match child.node {
                        Some(c) => self.eval(c, depth + 1, additive, memo)?,
                        None => bind.clone(),
                    };
                    parts.push((pose, w));
                }
                if parts.is_empty() {
                    bind
                } else {
                    blend(&parts)
                }
            }
            Kind::Additive { child2_weight } => {
                let base = self.child_pose(
                    node.children.first().and_then(|c| c.node),
                    depth,
                    additive,
                    memo,
                )?;
                let w = f64::from(*child2_weight);
                if w > 0.0 {
                    let delta = self.child_pose(
                        node.children.get(1).and_then(|c| c.node),
                        depth,
                        true,
                        memo,
                    )?;
                    self.apply_additive(&base, &delta, w)
                } else {
                    base
                }
            }
            Kind::PerBone {
                branches,
                child2_weight,
            } => {
                let a = self.child_pose(
                    node.children.first().and_then(|c| c.node),
                    depth,
                    additive,
                    memo,
                )?;
                let w = f64::from(*child2_weight);
                if w > 0.0 {
                    let b = self.child_pose(
                        node.children.get(1).and_then(|c| c.node),
                        depth,
                        additive,
                        memo,
                    )?;
                    let mask = self.branch_mask(branches);
                    a.iter()
                        .zip(&b)
                        .zip(&mask)
                        .map(|((x, y), on)| {
                            if *on {
                                blend(&[(vec![*x], 1.0 - w), (vec![*y], w)])[0]
                            } else {
                                *x
                            }
                        })
                        .collect()
                } else {
                    a
                }
            }
            Kind::Slot | Kind::SpecialMove => {
                let source = self.child_pose(first_child, depth, additive, memo)?;
                match &state.shot {
                    Some(shot) => {
                        let w = shot.weight();
                        let looping = shot.play.ending == Ending::Loop;
                        let clip = self.sample_named(
                            Some(&shot.play.clip),
                            shot.time.min(shot.length),
                            looping,
                            additive,
                        )?;
                        if w >= 1.0 {
                            clip
                        } else if w <= 0.0 {
                            source
                        } else {
                            blend(&[(source, 1.0 - w), (clip, w)])
                        }
                    }
                    None => source,
                }
            }
            Kind::NotYet { reason } => match self.unsupported {
                Unsupported::Refuse => {
                    return Err(EvalError::NotYet(NotYet::not_read(
                        format!("{} ({}): {reason}", node.class, node.path),
                        "docs/decisions/2026-11/2026-11-16-56",
                    )));
                }
                Unsupported::PassFirstChild => {
                    *self.log.passed_through.entry(id).or_insert(0) += 1;
                    self.child_pose(first_child, depth, additive, memo)?
                }
            },
        };
        memo.insert((id, additive), out.clone());
        Ok(out)
    }

    fn child_pose(
        &mut self,
        child: Option<NodeId>,
        depth: usize,
        additive: bool,
        memo: &mut Memo,
    ) -> Result<Vec<Local>, EvalError> {
        match child {
            Some(c) => self.eval(c, depth + 1, additive, memo),
            None if additive => Ok(identity(self.skeleton.bone_count())),
            None => Ok(self.skeleton.bind.clone()),
        }
    }

    /// Clip `name` at `time` (its own seconds). A missing clip (logged) gives the bind pose, or the identity delta
    /// where it is `additive`.
    fn sample_named(
        &mut self,
        name: Option<&str>,
        time: f64,
        looping: bool,
        additive: bool,
    ) -> Result<Vec<Local>, EvalError> {
        let missing = |s: &Self| {
            if additive {
                identity(s.skeleton.bone_count())
            } else {
                s.skeleton.bind.clone()
            }
        };
        let Some(name) = name else {
            return Ok(missing(self));
        };
        let Some((_, info)) = self.clips.find(name) else {
            self.log.missing_clips.insert(name.to_string());
            return Ok(missing(self));
        };
        let t = time / f64::from(info.rate_scale);
        let wrap = if looping { Wrap::Loop } else { Wrap::Clamp };
        let mut locals = self.skeleton.sample(&info.clip, t, wrap)?.locals;
        if additive {
            // The sampler keeps the bind local for a bone the clip does not track; as a delta that would add the
            // bind pose again. An untracked bone of an additive clip changes nothing.
            for (local, track) in locals.iter_mut().zip(&info.clip.tracks) {
                if track.is_none() {
                    *local = Local {
                        rotation: [0.0, 0.0, 0.0, 1.0],
                        translation: [0.0; 3],
                    };
                }
            }
        }
        Ok(locals)
    }

    /// `base` with `delta` applied at weight `w` (stand-in, module notes).
    fn apply_additive(&self, base: &[Local], delta: &[Local], w: f64) -> Vec<Local> {
        base.iter()
            .zip(delta)
            .enumerate()
            .map(|(bone, (b, d))| {
                let dw = nlerp([0.0, 0.0, 0.0, 1.0], d.rotation, w).unwrap_or(b.rotation);
                let rotation = if self.skeleton.parents.get(bone).copied().flatten().is_some() {
                    mul(b.rotation, dw)
                } else {
                    mul(dw, b.rotation)
                };
                let rotation = normalize(rotation).unwrap_or(b.rotation);
                let translation = [0, 1, 2].map(|k| b.translation[k] + w * d.translation[k]);
                Local {
                    rotation,
                    translation,
                }
            })
            .collect()
    }

    /// Bones at or below any of `branches` (by name); unknown names are logged.
    fn branch_mask(&mut self, branches: &[String]) -> Vec<bool> {
        let mut mask = vec![false; self.skeleton.bone_count()];
        for name in branches {
            match self.skeleton.names.iter().position(|n| n == name) {
                Some(i) => mask[i] = true,
                None => {
                    self.log.missing_bones.insert(name.clone());
                }
            }
        }
        // Parents come before children, so one pass in index order spreads the mask down the branches.
        for i in 0..mask.len() {
            if let Some(p) = self.skeleton.parents[i]
                && mask[p]
            {
                mask[i] = true;
            }
        }
        mask
    }
}

/// Refuses inputs that would carry NaN or infinity into weights, rates or blend times.
fn check_inputs(inputs: &Inputs) -> Result<(), EvalError> {
    if !inputs.speed.is_finite() {
        return Err(EvalError::BadInput("speed"));
    }
    if !inputs.direction.is_finite() {
        return Err(EvalError::BadInput("direction"));
    }
    if !(inputs.default_blend_time.is_finite() && inputs.default_blend_time >= 0.0) {
        return Err(EvalError::BadInput("default_blend_time"));
    }
    Ok(())
}

/// Memoised poses by (node, additive context).
type Memo = DetMap<(NodeId, bool), Vec<Local>>;

/// The identity delta: no rotation, no translation, for every bone.
fn identity(bones: usize) -> Vec<Local> {
    vec![
        Local {
            rotation: [0.0, 0.0, 0.0, 1.0],
            translation: [0.0; 3]
        };
        bones
    ]
}

fn one_hot(n: usize, k: usize) -> Vec<f64> {
    (0..n).map(|i| if i == k { 1.0 } else { 0.0 }).collect()
}

fn argmax(w: &[f64]) -> usize {
    let mut best = 0;
    for (i, v) in w.iter().enumerate() {
        if *v > w[best] {
            best = i;
        }
    }
    best
}

/// Moves each weight linearly toward the one-hot target `k` by `dt / blend_time` (at once when the time is 0).
fn approach(weights: &mut [f64], k: usize, blend_time: f64, dt: f64) {
    let step = if blend_time > 0.0 {
        dt / blend_time
    } else {
        f64::INFINITY
    };
    for (i, w) in weights.iter_mut().enumerate() {
        let goal = if i == k { 1.0 } else { 0.0 };
        if *w < goal {
            *w = (*w + step).min(goal);
        } else {
            *w = (*w - step).max(goal);
        }
    }
}

/// Forward, backward, left, right weights for direction `a` (radians, positive right). Front half fitted to the
/// stored weights of the installed directional nodes; back half mirrored (UNVERIFIED).
pub fn directional(a: f64, n: usize) -> Vec<f64> {
    let quarter = std::f64::consts::FRAC_PI_2;
    let a =
        (a + std::f64::consts::PI).rem_euclid(2.0 * std::f64::consts::PI) - std::f64::consts::PI;
    let mut w = vec![0.0; n.max(4)];
    let side = if a < 0.0 { 2 } else { 3 };
    let m = a.abs();
    if m <= quarter {
        w[0] = 1.0 - m / quarter;
        w[side] = m / quarter;
    } else {
        w[1] = (m - quarter) / quarter;
        w[side] = 1.0 - w[1];
    }
    w.truncate(n);
    w
}

/// The weight-normalised blend of `parts` (each a full pose): translations summed, rotations summed after aligning
/// each to the first's hemisphere, then normalised.
pub fn blend(parts: &[(Vec<Local>, f64)]) -> Vec<Local> {
    let total: f64 = parts.iter().map(|(_, w)| *w).sum();
    let Some((first, _)) = parts.first() else {
        return Vec::new();
    };
    if parts.len() == 1 || total <= 0.0 {
        return first.clone();
    }
    (0..first.len())
        .map(|bone| {
            let reference = first[bone].rotation;
            let mut q: Quat = [0.0; 4];
            let mut t = [0.0; 3];
            for (pose, w) in parts {
                let w = w / total;
                let r = pose[bone].rotation;
                let sign = if super::super::math::dot(reference, r) < 0.0 {
                    -1.0
                } else {
                    1.0
                };
                for (qk, rk) in q.iter_mut().zip(r) {
                    *qk += sign * w * rk;
                }
                for (tk, pk) in t.iter_mut().zip(pose[bone].translation) {
                    *tk += w * pk;
                }
            }
            Local {
                rotation: normalize(q).unwrap_or(reference),
                translation: t,
            }
        })
        .collect()
}

/// Reports the notifies between `before` and `after` (clip seconds; across the end when `wrapped`).
fn notifies(
    out: &mut Vec<NotifyEvent>,
    node: NodeId,
    clip: &str,
    info: &ClipInfo,
    before: f64,
    after: f64,
    wrapped: bool,
) {
    for n in &info.notifies {
        let t = f64::from(n.time);
        let hit = if wrapped {
            t > before || t <= after
        } else {
            t > before && t <= after
        };
        if hit {
            out.push(NotifyEvent {
                node,
                clip: clip.to_string(),
                notify: n.clone(),
            });
        }
    }
}
