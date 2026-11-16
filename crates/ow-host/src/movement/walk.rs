//! Owner: lane H4-MOVEMENT. A scripted walk: segments of held input, stepped at the fixed simulation step, with the
//! positions sampled and two checks a run must pass: it does not fall through the world, and it does not stick.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-52-*`.
//!
//! The checks are ours, for a host run, not a parity test:
//! - **fell through**: the pawn fell for longer than [`MAX_FALL_SECONDS`] in one go, or ended more than
//!   [`MAX_DROP`] uu below where it first landed;
//! - **stuck**: a segment with move input *stalled* (moved the pawn less than [`STUCK_FRACTION`] of what its time
//!   at the full ground speed would cover) although nothing blocks a short move ([`BLOCKER_PROBE`] uu) along its
//!   input from where it ended, or the pawn moved less than [`MIN_TRAVEL`] uu over the whole run.
//!
//! A stall with a brush in the way is a wall contact, not a fault: the segment records the brush and its normal
//! ([`SegmentResult::blocker`]) so a reader can tell which wall it was.
//!
//! **One input path** (lane H4-PLAY). A script becomes per-tick [`MoveInput`]s through [`segment_ticks`] and
//! [`segment_input`] only: [`run`] here and the play mode's [`ScriptCursor`] (`crate::play`) both use them, and both
//! fold every tick's location into a [`PathDigest`]. Equal digests at every segment end mean the two runs put the
//! pawn at the same bits on every tick.
//!
//! **Script text** ([`parse_script`], [`script_text`]): one segment per line, `seconds forward strafe jump yaw
//! name...` (jump 0 or 1, yaw in rotator units per tick, the name the rest of the line); empty lines and lines
//! starting with `#` are skipped.

use std::borrow::Cow;

use super::collide::CollisionWorld;
use super::geom::{V3, add, floor_index, horizontal, length, normalize, scale, sub};
use crate::frame::rotator_axes;
use ow_core::units::Rotator;
use super::mover::{MoveInput, Mover, Physics};

/// One stretch of held input.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub name: Cow<'static, str>,
    pub seconds: f64,
    pub forward: f64,
    pub strafe: f64,
    /// Jump on the segment's first tick.
    pub jump: bool,
    /// View turn per tick, rotator units.
    pub yaw_per_tick: i32,
}

impl Segment {
    pub const fn new(name: &'static str, seconds: f64, forward: f64, strafe: f64, jump: bool, yaw_per_tick: i32) -> Segment {
        Segment { name: Cow::Borrowed(name), seconds, forward, strafe, jump, yaw_per_tick }
    }

    fn moves(&self) -> bool {
        self.forward != 0.0 || self.strafe != 0.0
    }
}

/// The longest one fall may last before the run counts as fallen through, seconds.
pub const MAX_FALL_SECONDS: f64 = 5.0;
/// How far below its first landing the pawn may end, uu.
pub const MAX_DROP: f64 = 2000.0;
/// The least share of full-speed distance a moving segment must cover.
pub const STUCK_FRACTION: f64 = 0.25;
/// How far along a stalled segment's input the blocker is looked for, uu.
pub const BLOCKER_PROBE: f64 = 60.0;
/// The most ticks one segment may run (about 46 hours at 1/60 s).
pub const MAX_SEGMENT_TICKS: usize = 10_000_000;
/// The least horizontal distance a run must travel in all, uu.
pub const MIN_TRAVEL: f64 = 1000.0;

/// How many ticks of `dt` segment `s` runs: its seconds rounded to the nearest whole tick, at most
/// [`MAX_SEGMENT_TICKS`].
pub fn segment_ticks(s: &Segment, dt: f64) -> u64 {
    floor_index((s.seconds / dt).round(), MAX_SEGMENT_TICKS) as u64
}

/// The input of tick `t` (from 0) of segment `s`: its held move, its turn, and a jump on its first tick only.
pub fn segment_input(s: &Segment, t: u64) -> MoveInput {
    MoveInput::new(s.forward, s.strafe, s.jump && t == 0, s.yaw_per_tick, 0)
}

/// FNV-1a (64 bits) over the bits of every location folded in: two paths with the same digest at the same tick count
/// put the pawn at the same bits on every tick (up to a hash collision, which a report reads as a match).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathDigest(pub u64);

impl Default for PathDigest {
    fn default() -> Self {
        PathDigest(0xcbf2_9ce4_8422_2325)
    }
}

impl PathDigest {
    pub fn push(&mut self, location: V3) {
        for v in location {
            for byte in v.to_bits().to_le_bytes() {
                self.0 ^= u64::from(byte);
                self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
}

/// Walks a script tick by tick: the segment index and the input of each tick, through [`segment_input`].
#[derive(Debug, Clone)]
pub struct ScriptCursor {
    script: Vec<Segment>,
    dt: f64,
    segment: usize,
    tick: u64,
}

impl ScriptCursor {
    pub fn new(script: Vec<Segment>, dt: f64) -> ScriptCursor {
        ScriptCursor { script, dt, segment: 0, tick: 0 }
    }

    pub fn script(&self) -> &[Segment] {
        &self.script
    }

    /// The next tick's segment index and input, or `None` once the script is done. Segments of no tick are passed.
    pub fn next_input(&mut self) -> Option<(usize, MoveInput)> {
        while let Some(s) = self.script.get(self.segment) {
            if self.tick < segment_ticks(s, self.dt) {
                let input = segment_input(s, self.tick);
                self.tick = self.tick.saturating_add(1);
                return Some((self.segment, input));
            }
            self.segment = self.segment.saturating_add(1);
            self.tick = 0;
        }
        None
    }

    /// The index of the segment the last tick came from.
    pub fn segment(&self) -> usize {
        self.segment
    }

    /// True when no tick is left.
    pub fn done(&self) -> bool {
        let (mut segment, mut tick) = (self.segment, self.tick);
        while let Some(s) = self.script.get(segment) {
            if tick < segment_ticks(s, self.dt) {
                return false;
            }
            segment = segment.saturating_add(1);
            tick = 0;
        }
        true
    }

    /// True when the tick just taken was its segment's last.
    pub fn at_segment_end(&self) -> bool {
        self.script.get(self.segment).is_some_and(|s| self.tick == segment_ticks(s, self.dt))
    }
}

/// Parses script text (module documentation).
pub fn parse_script(text: &str) -> Result<Vec<Segment>, String> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let bad = |what: &str| format!("script line {}: {what}: {line:?}", n + 1);
        let mut words = line.split_whitespace();
        let mut number = |what: &str| -> Result<f64, String> {
            let w = words.next().ok_or_else(|| bad(&format!("no {what}")))?;
            w.parse::<f64>().ok().filter(|v| v.is_finite()).ok_or_else(|| bad(&format!("{what} is not a number")))
        };
        let seconds = number("seconds")?;
        let forward = number("forward")?;
        let strafe = number("strafe")?;
        if !(seconds >= 0.0) || !(-1.0..=1.0).contains(&forward) || !(-1.0..=1.0).contains(&strafe) {
            return Err(bad("seconds must be 0 or more, forward and strafe within -1..1"));
        }
        let jump = match words.next() {
            Some("0") => false,
            Some("1") => true,
            _ => return Err(bad("jump must be 0 or 1")),
        };
        let yaw = words
            .next()
            .and_then(|w| w.parse::<i32>().ok())
            .ok_or_else(|| bad("yaw is not a whole number"))?;
        let name: Vec<&str> = words.collect();
        out.push(Segment {
            name: Cow::Owned(name.join(" ")),
            seconds,
            forward,
            strafe,
            jump,
            yaw_per_tick: yaw,
        });
    }
    Ok(out)
}

/// The text of `script`, which [`parse_script`] reads back to the same segments (numbers in Rust's shortest form,
/// which parses back exactly).
pub fn script_text(script: &[Segment]) -> String {
    let mut out = String::new();
    for s in script {
        out.push_str(&format!(
            "{} {} {} {} {} {}\n",
            ow_core::fmt::fmt_py_repr(s.seconds),
            ow_core::fmt::fmt_py_repr(s.forward),
            ow_core::fmt::fmt_py_repr(s.strafe),
            u8::from(s.jump),
            s.yaw_per_tick,
            s.name
        ));
    }
    out
}

/// What stopped a stalled segment: the brush a short move along its input runs into.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blocker {
    pub brush: u32,
    pub normal: V3,
}

/// A sampled tick.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub tick: u64,
    pub segment: usize,
    pub location: V3,
    pub velocity: V3,
    pub physics: Physics,
    pub yaw: i32,
}

/// What one segment did.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentResult {
    pub name: Cow<'static, str>,
    pub start: V3,
    pub end: V3,
    /// Horizontal distance from start to end, uu.
    pub moved: f64,
    /// What the segment's time at full ground speed covers, uu (0 for a segment without move input).
    pub full_speed: f64,
    pub landings: u32,
    pub stepped_up: f64,
    /// Moved less than [`STUCK_FRACTION`] of its full-speed distance.
    pub stalled: bool,
    /// What a stalled segment ran into (`None`: nothing within [`BLOCKER_PROBE`]).
    pub blocker: Option<Blocker>,
    /// Stalled with nothing in the way.
    pub stuck: bool,
    /// The view yaw at the segment's end, and the segment's move input (forward, strafe).
    pub end_yaw: i32,
    pub input: (f64, f64),
    /// The path digest over every tick so far, at this segment's end.
    pub digest: PathDigest,
}

/// The run's result.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct WalkReport {
    pub samples: Vec<Sample>,
    pub segments: Vec<SegmentResult>,
    /// Where the pawn first landed (`None`: never).
    pub first_landing: Option<V3>,
    pub longest_fall: f64,
    pub lowest_z: f64,
    pub start_solid_ticks: u64,
    pub fell_through: bool,
    pub stuck: bool,
    pub end: V3,
    pub ticks: u64,
    /// Horizontal distance over all segments, uu.
    pub travelled: f64,
    /// The path digest over every tick.
    pub digest: PathDigest,
}

impl WalkReport {
    pub fn passed(&self) -> bool {
        !self.fell_through && !self.stuck && self.first_landing.is_some()
    }
}

/// Runs `script` from `mover` in `world` at time step `dt`, sampling every `sample_every` ticks (and the last).
pub fn run(world: &CollisionWorld, mover: &mut Mover, script: &[Segment], dt: f64, sample_every: u64) -> WalkReport {
    let mut report = WalkReport {
        samples: Vec::new(),
        segments: Vec::new(),
        first_landing: None,
        longest_fall: 0.0,
        lowest_z: mover.location[2],
        start_solid_ticks: 0,
        fell_through: false,
        stuck: false,
        end: mover.location,
        ticks: 0,
        travelled: 0.0,
        digest: PathDigest::default(),
    };
    let mut falling_for = 0.0;
    let sample_every = sample_every.max(1);
    for (index, segment) in script.iter().enumerate() {
        let start = mover.location;
        let mut last = mover.location;
        let mut result = SegmentResult {
            name: segment.name.clone(),
            start,
            end: start,
            moved: 0.0,
            full_speed: 0.0,
            landings: 0,
            stepped_up: 0.0,
            stalled: false,
            blocker: None,
            stuck: false,
            end_yaw: mover.view.yaw,
            input: (segment.forward, segment.strafe),
            digest: report.digest,
        };
        let ticks = segment_ticks(segment, dt);
        for t in 0..ticks {
            let events = mover.tick(world, &segment_input(segment, t), dt);
            report.digest.push(mover.location);
            report.ticks = report.ticks.saturating_add(1);
            if events.start_solid {
                report.start_solid_ticks = report.start_solid_ticks.saturating_add(1);
            }
            if events.landed.is_some() {
                result.landings = result.landings.saturating_add(1);
                if report.first_landing.is_none() {
                    report.first_landing = Some(mover.location);
                }
            }
            result.stepped_up += events.stepped_up;
            if mover.physics == Physics::Falling {
                falling_for += dt;
                report.longest_fall = report.longest_fall.max(falling_for);
            } else {
                falling_for = 0.0;
            }
            report.lowest_z = report.lowest_z.min(mover.location[2]);
            report.travelled += length(horizontal(sub(mover.location, last)));
            last = mover.location;
            if report.ticks % sample_every == 0 {
                report.samples.push(Sample {
                    tick: report.ticks,
                    segment: index,
                    location: mover.location,
                    velocity: mover.velocity,
                    physics: mover.physics,
                    yaw: mover.view.yaw,
                });
            }
        }
        result.end = mover.location;
        result.end_yaw = mover.view.yaw;
        result.digest = report.digest;
        result.moved = length(horizontal(sub(result.end, start)));
        if segment.moves() {
            result.full_speed = mover.params.ground_speed * mover.params.speed_fraction * ticks as f64 * dt;
            result.stalled = result.moved < STUCK_FRACTION * result.full_speed;
            if result.stalled {
                result.blocker = blocker(world, mover, &result);
                result.stuck = result.blocker.is_none();
            }
        }
        report.stuck |= result.stuck;
        report.segments.push(result);
    }
    report.end = mover.location;
    report.samples.push(Sample {
        tick: report.ticks,
        segment: script.len().saturating_sub(1),
        location: mover.location,
        velocity: mover.velocity,
        physics: mover.physics,
        yaw: mover.view.yaw,
    });
    let dropped = report.first_landing.map(|l| l[2] - report.end[2] > MAX_DROP).unwrap_or(false);
    report.fell_through = report.longest_fall > MAX_FALL_SECONDS || dropped;
    report.stuck |= report.travelled < MIN_TRAVEL;
    report
}

/// The brush a short move along segment `s`'s input runs into from where it ended.
fn blocker(world: &CollisionWorld, mover: &Mover, s: &SegmentResult) -> Option<Blocker> {
    let axes = rotator_axes(Rotator::new(0, s.end_yaw, 0));
    let dir = [
        axes[0][0] * s.input.0 + axes[1][0] * s.input.1,
        axes[0][1] * s.input.0 + axes[1][1] * s.input.1,
        0.0,
    ];
    let dir = normalize(dir)?;
    let hit = world.trace(&mover.params.shape, s.end, add(s.end, scale(dir, BLOCKER_PROBE)));
    hit.brush.map(|brush| Blocker { brush, normal: hit.normal })
}

/// The Sanctuary walk: settle, then walk the player start's facing, turn, jump, strafe and come back. 16,384
/// rotator units are a quarter turn; at 1/60 s a turn of 273 units a tick for 60 ticks is about a quarter turn.
pub const SANCTUARY_SCRIPT: [Segment; 9] = [
    Segment::new("settle", 1.5, 0.0, 0.0, false, 0),
    Segment::new("forward", 3.0, 1.0, 0.0, false, 0),
    Segment::new("turn right while walking", 1.0, 1.0, 0.0, false, 273),
    Segment::new("forward", 2.0, 1.0, 0.0, false, 0),
    Segment::new("jump forward", 2.0, 1.0, 0.0, true, 0),
    Segment::new("strafe left", 2.0, 0.0, -1.0, false, 0),
    Segment::new("turn around", 1.0, 0.0, 0.0, false, 546),
    Segment::new("forward back", 4.0, 1.0, 0.0, false, 0),
    Segment::new("stand", 1.0, 0.0, 0.0, false, 0),
];
