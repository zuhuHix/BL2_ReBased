//! Owner: lane W4-ROLL. The weighted pick of a part slot, of a manufacturer and of a definition: `weapon_recipe.pick`
//! (read from native code for weapons, NATIVE_WEAPON_RULES.md section 3; the weights and the pick's shape are
//! verified for two common weapon balances at stage 30, 400 game-rolled guns each; UNVERIFIED for items).
//!
//! The rule, as the oracle writes it: entries weighing at or below [`MIN_WEIGHT`] (or with no weight) are left out,
//! and such an entry does not remove an earlier one of the same key; a key listed twice keeps its first position with
//! its later weight; the total is Python's `sum()` of the kept weights ([`py_sum`]); `r = total * rand() / 32767`
//! ([`ow_core::rand::weighted_draw`]); the first entry whose interval `[running, running + weight]` holds `r` wins,
//! and when rounding leaves `r` past every interval the last entry wins. No candidate: no draw and no pick.
//!
//! The seeding (one `CrtRand(seed)` per roll) is the tools', not the game's: the game's own random sequence is not
//! reproduced (UNVERIFIED).

use ow_core::rand::{Rand15, weighted_draw};

/// Entries at or below this weight never become candidates (`weapon_recipe.MIN_WEIGHT`).
pub const MIN_WEIGHT: f64 = 1e-8;

/// CPython 3.12's `sum()` of floats starting from the integer 0: the Neumaier-compensated sum it has used since
/// 3.12, with the compensation added at the end when it is non-zero and finite. The oracle tools run on 3.12
/// (`weapon_recipe.pick`'s `sum(candidates.values())`), and a plain left-to-right sum differs from it in the last bit
/// for some weight lists, which moves a pick that lands on a boundary.
pub fn py_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = 0.0f64;
    let mut c = 0.0f64;
    for x in values {
        let t = sum + x;
        if sum.abs() >= x.abs() {
            c += (sum - t) + x;
        } else {
            c += (x - t) + sum;
        }
        sum = t;
    }
    if c != 0.0 && c.is_finite() {
        sum += c;
    }
    sum
}

/// What one pick saw and chose.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Picked<K> {
    /// The chosen key, `None` when no entry weighed above [`MIN_WEIGHT`] (then nothing was drawn).
    pub choice: Option<K>,
    /// The candidates the pick drew among, in pick order, with their weights (`weapon_recipe.pick`'s second result).
    pub candidates: Vec<(K, f64)>,
}

impl<K> Picked<K> {
    /// The chosen key's weight.
    pub fn weight(&self) -> Option<f64>
    where
        K: PartialEq,
    {
        let choice = self.choice.as_ref()?;
        self.candidates
            .iter()
            .find(|(k, _)| k == choice)
            .map(|(_, w)| *w)
    }
}

/// The candidate list of `entries` as the pick builds it (module documentation): a weight of `None` or at or below
/// [`MIN_WEIGHT`] is left out, a repeated key keeps its first position and takes the later weight.
pub fn candidates<K: PartialEq>(
    entries: impl IntoIterator<Item = (K, Option<f64>)>,
) -> Vec<(K, f64)> {
    let mut out: Vec<(K, f64)> = Vec::new();
    for (key, weight) in entries {
        let Some(weight) = weight.filter(|w| *w > MIN_WEIGHT) else {
            continue;
        };
        match out.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = weight,
            None => out.push((key, weight)),
        }
    }
    out
}

/// `weapon_recipe.pick` (module documentation): one draw from `rng` when at least one entry is a candidate.
pub fn pick<K: PartialEq + Clone>(
    entries: impl IntoIterator<Item = (K, Option<f64>)>,
    rng: &mut dyn Rand15,
) -> Picked<K> {
    let candidates = candidates(entries);
    if candidates.is_empty() {
        return Picked {
            choice: None,
            candidates,
        };
    }
    let total = py_sum(candidates.iter().map(|(_, w)| *w));
    let r = weighted_draw(rng, total);
    let mut running = 0.0f64;
    let mut choice = None;
    for (key, weight) in &candidates {
        if running <= r && r <= running + weight {
            choice = Some(key.clone());
            break;
        }
        running += weight;
    }
    // r == total after rounding: the last entry, as the oracle's fall-through returns it.
    let choice = choice.or_else(|| candidates.last().map(|(k, _)| k.clone()));
    Picked { choice, candidates }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ow_core::rand::CrtRand;

    /// A generator that returns the given values in turn.
    struct Fixed(Vec<u16>, usize);

    impl Rand15 for Fixed {
        fn rand15(&mut self) -> u16 {
            let v = self.0[self.1];
            self.1 += 1;
            v
        }
    }

    #[test]
    fn the_sum_is_cpythons_compensated_sum() {
        // Invented values chosen so the plain sum and the compensated one differ.
        let values = [0.1, 0.2, 0.3];
        let plain: f64 = values.iter().fold(0.0, |a, b| a + b);
        assert_eq!(plain, 0.6000000000000001);
        assert_eq!(py_sum(values), 0.6);
        assert_eq!(py_sum([1e16, 1.0, -1e16]), 1.0);
        assert_eq!(py_sum(std::iter::empty()), 0.0);
        assert_eq!(py_sum([100.0, 100.0]), 200.0);
    }

    #[test]
    fn a_repeated_key_keeps_its_first_position_and_the_later_weight() {
        let c = candidates([
            ("a", Some(1.0)),
            ("b", Some(2.0)),
            ("a", Some(5.0)),
            ("b", Some(0.0)),
            ("c", None),
            ("d", Some(1e-9)),
        ]);
        // A zero-weight repeat does not remove an earlier entry; no weight and tiny weights are not candidates.
        assert_eq!(c, vec![("a", 5.0), ("b", 2.0)]);
    }

    #[test]
    fn the_pick_walks_closed_intervals_and_draws_once() {
        // total 3: r = 3 * u / 32767. u = 0 lands on the first entry's lower bound.
        let entries = || [("x", Some(1.0)), ("y", Some(2.0))];
        let mut rng = Fixed(vec![0, 10922, 32767], 0);
        assert_eq!(pick(entries(), &mut rng).choice, Some("x"));
        // 3 * 10922 / 32767 = 0.99997 is inside [0, 1].
        assert_eq!(pick(entries(), &mut rng).choice, Some("x"));
        assert_eq!(pick(entries(), &mut rng).choice, Some("y"));
        assert_eq!(rng.1, 3);
    }

    #[test]
    fn no_candidate_means_no_draw() {
        let mut rng = Fixed(vec![], 0);
        let p = pick([("x", Some(0.0)), ("y", None)], &mut rng);
        assert_eq!(p.choice, None);
        assert!(p.candidates.is_empty());
        assert_eq!(rng.1, 0);
    }

    #[test]
    fn the_crt_stream_gives_a_fixed_sequence_of_picks() {
        // Invented weights; the expected picks follow from CrtRand(7)'s first draws (7, then the LCG).
        let mut rng = CrtRand::new(7);
        let mut r2 = CrtRand::new(7);
        let first = f64::from(r2.rand15());
        let p = pick([("a", Some(100.0)), ("b", Some(100.0))], &mut rng);
        let expected = if 200.0 * first / 32767.0 <= 100.0 {
            "a"
        } else {
            "b"
        };
        assert_eq!(p.choice, Some(expected));
        assert_eq!(p.weight(), Some(100.0));
    }
}
