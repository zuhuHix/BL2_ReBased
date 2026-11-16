//! Owner: lane H2-DRAW. A free-flying viewer camera for looking at a scene: WASD to move along the view, Q and E down
//! and up, Shift to go faster, the mouse to turn while the right button is held. It is a viewer, not the player:
//! walking, gravity and collision are lane H4's (`NATIVE_PLAYER_MOVEMENT.md`), and nothing here claims the game's
//! camera, field of view or speeds.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-51-*`.
//!
//! The camera keeps the game's units: a position in uu and a pitch and yaw in rotator units (65,536 per turn), roll
//! always 0. Its matrices come from the boundary module ([`crate::frame`]): [`frame::view_matrix`] and
//! [`frame::perspective_reverse_z_infinite`]. It starts at a player start's pose ([`FlyCamera::at`]).

use ow_core::units::Rotator;

use crate::frame::{self, Mat4, Vec3};

/// Movement speed, uu per second (a viewer's choice, not the game's).
pub const SPEED: f32 = 800.0;
/// The factor Shift applies to [`SPEED`].
pub const FAST: f32 = 4.0;
/// Rotator units turned per pixel of mouse motion (about 0.11 degrees).
pub const TURN_PER_PIXEL: f32 = 20.0;
/// The pitch limit, a little short of straight up or down, so the view never flips.
pub const PITCH_LIMIT: f32 = 16000.0;
/// The vertical field of view in degrees (a viewer's choice; the game's FOV rules are not read here).
pub const FOV_DEG: f32 = 70.0;
/// The near plane, uu. With the infinite reverse-Z projection there is no far plane.
pub const NEAR: f32 = 10.0;

/// The keys and button the camera listens to, held or not.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Controls {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub down: bool,
    pub up: bool,
    pub fast: bool,
    /// The right mouse button: mouse motion turns the camera only while it is held.
    pub look: bool,
}

/// The camera.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlyCamera {
    /// uu.
    pub position: Vec3,
    /// Rotator units.
    pub pitch: f32,
    pub yaw: f32,
}

impl FlyCamera {
    /// A camera at `position`, facing `rotation` (its roll dropped; pitch clamped to [`PITCH_LIMIT`]).
    pub fn at(position: Vec3, rotation: Rotator) -> FlyCamera {
        let normal = frame::normalize(rotation);
        FlyCamera {
            position,
            pitch: (normal.pitch as f32).clamp(-PITCH_LIMIT, PITCH_LIMIT),
            yaw: normal.yaw as f32,
        }
    }

    /// The rotator the matrices are built from: pitch and yaw rounded to whole units, roll 0.
    pub fn rotation(&self) -> Rotator {
        let units = |x: f32| ow_core::num::trunc_i32(ow_core::num::round_half_away_f32(x));
        Rotator::new(units(self.pitch), units(self.yaw), 0)
    }

    /// Turns by a mouse motion of `dx`, `dy` pixels (right and down positive) when [`Controls::look`] is held.
    pub fn turn(&mut self, controls: &Controls, dx: f64, dy: f64) {
        if !controls.look || !dx.is_finite() || !dy.is_finite() {
            return;
        }
        // The yaw wraps by whole turns, so the f32 stays small whatever the motion.
        let yaw = f64::from(self.yaw) + dx * f64::from(TURN_PER_PIXEL);
        self.yaw = (yaw % 65536.0) as f32;
        let pitch = f64::from(self.pitch) - dy * f64::from(TURN_PER_PIXEL);
        self.pitch = (pitch as f32).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Moves for `dt` seconds with the keys held: along the view direction (W, S), its right (D, A) and the world's
    /// up (E, Q).
    pub fn advance(&mut self, controls: &Controls, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let axes = frame::rotator_axes(self.rotation());
        let axis = |plus: bool, minus: bool| f32::from(u8::from(plus)) - f32::from(u8::from(minus));
        let (f, r, u) = (
            axis(controls.forward, controls.back),
            axis(controls.right, controls.left),
            axis(controls.up, controls.down),
        );
        let step = SPEED * if controls.fast { FAST } else { 1.0 } * dt;
        for (k, p) in self.position.iter_mut().enumerate() {
            let along = f * axes[0][k] as f32 + r * axes[1][k] as f32;
            let lift = if k == 2 { u } else { 0.0 };
            *p += (along + lift) * step;
        }
    }

    /// The view matrix ([`frame::view_matrix`]); `None` for a position the boundary refuses.
    pub fn view(&self) -> Option<Mat4> {
        frame::view_matrix(self.position, self.rotation())
    }

    /// Projection times view for a `width` x `height` drawable, reverse-Z with no far plane; `None` for a zero
    /// size or a refused position.
    pub fn view_projection(&self, width: u32, height: u32) -> Option<Mat4> {
        if width == 0 || height == 0 {
            return None;
        }
        let aspect = width as f32 / height as f32;
        let projection = frame::perspective_reverse_z_infinite(FOV_DEG, aspect, NEAR)?;
        Some(projection.mul(&self.view()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_starts_at_the_pose_it_is_given() {
        let c = FlyCamera::at([1.0, 2.0, 3.0], Rotator::new(0, 8192, 500));
        assert_eq!(c.rotation(), Rotator::new(0, 8192, 0));
        assert_eq!(
            FlyCamera::at([0.0; 3], Rotator::new(20000, 0, 0)).pitch,
            PITCH_LIMIT
        );
    }

    #[test]
    fn w_moves_along_the_view_and_e_lifts() {
        let mut c = FlyCamera::at([0.0; 3], Rotator::new(0, 16384, 0));
        let held = Controls {
            forward: true,
            up: true,
            ..Controls::default()
        };
        c.advance(&held, 0.5);
        // Yaw 16384 faces +Y.
        assert!(c.position[0].abs() < 1e-3);
        assert!((c.position[1] - SPEED * 0.5).abs() < 1e-2);
        assert!((c.position[2] - SPEED * 0.5).abs() < 1e-2);
        let before = c.position;
        c.advance(&held, f32::NAN);
        assert_eq!(c.position, before);
    }

    #[test]
    fn the_mouse_turns_only_with_the_button_held() {
        let mut c = FlyCamera::at([0.0; 3], Rotator::default());
        c.turn(&Controls::default(), 100.0, 0.0);
        assert_eq!(c.yaw, 0.0);
        let look = Controls {
            look: true,
            ..Controls::default()
        };
        c.turn(&look, 100.0, 10.0);
        assert_eq!((c.yaw, c.pitch), (2000.0, -200.0));
        c.turn(&look, 0.0, -1e9);
        assert_eq!(c.pitch, PITCH_LIMIT);
    }

    #[test]
    fn a_point_in_front_lands_inside_the_view_with_reverse_depth() {
        let c = FlyCamera::at([0.0; 3], Rotator::default());
        let m = c.view_projection(1280, 720).unwrap();
        let near = m.apply([100.0, 0.0, 0.0, 1.0]);
        let far = m.apply([10000.0, 0.0, 0.0, 1.0]);
        let (dn, df) = (near[2] / near[3], far[2] / far[3]);
        assert!(dn > df && df > 0.0 && dn <= 1.0, "{dn} {df}");
        assert!(c.view_projection(0, 720).is_none());
    }
}
