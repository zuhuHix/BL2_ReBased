//! The `--combat-run` parity with the C++ binary on invented packages (AGENTS.md rule 9): the packages of
//! `ow-combat`'s fixture (`crates/ow-combat/src/run/fixture.rs`, included below), sixteen step lists of
//! `tests/combat_test.py`'s scenarios plus the armored pawn's `AdjustDamage` ones.
//!
//! **Where the expected digests come from.** Each is the FNV-1a 64 of the C++ binary's stdout line (its newline read as `\n`) for that step list on
//! the same package bytes (`cargo test -p ow-combat -- --ignored write_fixture` wrote them; `build/Release/
//! ow-package.exe <dir>/ToyPawns.upk --combat-run --cooked <dir> <steps>` ran on them, 2026-11-16, pc-hummer; re-run on the r2 bytes at 881ea3c, unchanged), after
//! [`normalize`]: the two timing fields removed (`milliseconds`, `elapsed_ms`: each binary's own clock) and every
//! `"SetHealth <value>"` event line removed. Those lines are the C++ pool half's log, which the Rust `SetHealth`
//! (W5-POOLS) does not write (`ow_combat::run`, module documentation). Nothing else is changed: the rest of each
//! report, rows, events, call order, state, stubs, notes and errors, is byte-identical.
//!
//! Off mode equals the C++ on the scenarios that reach no stand-in. The armored scenarios read attributes the toy
//! classes do not declare, which the C++ reads as the stage's identity (a Reference stand-in): they equal the C++ in
//! Reference mode, and in Off mode each such hit is `NotYet` (pinned below).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_core::ParityMode;

use super::world;

#[allow(dead_code)]
#[path = "../../../../ow-combat/src/run/fixture.rs"]
mod fixture;

/// The scenarios: name, steps, the C++ digest, and whether Off mode equals it (else only Reference does).
const CASES: [(&str, &str, u64, bool); 16] = [
    (
        "pools",
        "spawn:ToyPawns/Pawn_Toy:4 sethealth:0:0.5 sethealth:0:-3 sethealth:0:1.5 sethealth:0:500 setmax:0:99 pool:0:Pool_Health_Copy pool:0:Pool_Ammo pool:0:Pool_Ammo_Copy copyref:0:1 valid:0:3 unpool:0:1 valid:0:3 pool:0:Pool_Ammo valid:0:3 valid:0:4 destroyhealth:0 sethealth:0:7 pool:0:Pool_Health",
        0x7d03_6655_4536_18f2,
        true,
    ),
    (
        "hit",
        "spawn:ToyPawns/Pawn_Toy:4 player:8 hit:0:10::Bullet sethealth:0:40.5 hold:0:Safe hit:0:10::Bullet sethealth:0:40.5 hold:0:Other release:0:Safe hit:0:10::Bullet release:0:Other sethealth:0:40.5 hit:0:10::Bullet",
        0xe5f4_8be8_9c0b_a963,
        true,
    ),
    (
        "death",
        "spawn:ToyPawns/Pawn_Doomed:4 player:8 hit:0:10::Bullet destroyhealth:0",
        0x86ec_ba2d_c2ea_3bf0,
        true,
    ),
    (
        "xp1",
        "spawn:ToyPawns/Pawn_Doomed:5 player:4 players:1 hit:0:10::Bullet",
        0xc207_d47b_66fa_077e,
        true,
    ),
    (
        "xp2",
        "spawn:ToyPawns/Pawn_Doomed:5 player:4 player:7 players:2 hit:0:10::Bullet",
        0xf106_ebb9_b6f1_aa37,
        true,
    ),
    (
        "xp3",
        "spawn:ToyPawns/Pawn_Doomed:5 player:1 players:1 hit:0:10::Bullet",
        0xb4ad_0f14_88e9_6ec6,
        true,
    ),
    (
        "xp4",
        "spawn:ToyPawns/Pawn_Doomed:5 player:5 players:1 hit:0:10::Bullet",
        0x4365_a8f4_6ee9_eed7,
        true,
    ),
    (
        "xp5",
        "spawn:ToyPawns/Pawn_Doomed:5 player:4 hit:0:10::Bullet",
        0x2dac_9831_a0f8_c17c,
        true,
    ),
    (
        "swing",
        "spawn:ToyPawns/Pawn_Toy:4 player:8 swing:0:Melee_Toy swing:0:Throw_Toy:Cause_Toy swing:0:Throw_Toy swing:0:Pawn_Toy swing:0:NoSuchObject",
        0xeb15_1147_8b46_72da,
        true,
    ),
    (
        "swing2",
        "spawn:ToyPawns/Pawn_Toy:4 swing:0:Melee_Toy",
        0x5ed0_b100_bb5e_fd73,
        true,
    ),
    (
        "down",
        "spawn:ToyPawns/Pawn_Down:4 player:8 hit:0:10::Bullet",
        0x991a_9371_6809_5c9f,
        true,
    ),
    (
        "down2",
        "spawn:ToyPawns/Pawn_Down:4 bleed:1",
        0xa387_af0e_fc77_419f,
        true,
    ),
    (
        "down3",
        "spawn:ToyPawns/Pawn_Down:4 cash:100",
        0xad02_e542_28e8_011d,
        true,
    ),
    (
        "armor",
        "spawn:ToyPawns/Pawn_Armored:4 player:8 attr:0:InstigatedBulletDamageModifier:1.2 hit:0:100:DamageType_Toy:Bullet hit:0:100::Bullet hit:0:100:DamageType_Toy:Melee",
        0x1e8e_9241_fc06_215e,
        false,
    ),
    (
        "armor2",
        "spawn:ToyPawns/Pawn_Armored:8 player:8 hit:0:50:DamageType_Toy:Grenade",
        0xa1fa_abcd_e08d_0894,
        false,
    ),
    (
        "bodied",
        "spawn:ToyPawns/Pawn_Bodied:4 player:8 attr:0:InstigatedBulletDamageModifier:1.2 hit:0:100:DamageType_Toy:Bullet hit:0:100:DamageType_Resist:Bullet",
        0x069b_83b6_24fe_fc8a,
        false,
    ),
];

/// FNV-1a 64.
fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Removes `,"<key>":<number>` (a number as the reports print it).
fn drop_number_field(text: &str, key: &str) -> String {
    let pattern = format!(",\"{key}\":");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(&pattern) {
        out.push_str(&rest[..at]);
        let after = &rest[at + pattern.len()..];
        let end = after
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e')))
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// The normalization of the module documentation.
fn normalize(text: &str) -> String {
    let text = drop_number_field(text, "milliseconds");
    let text = drop_number_field(&text, "elapsed_ms");
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(at) = rest.find("\"SetHealth ") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let close = after.find('"').map_or(after.len(), |i| i + 1);
        let after = &after[close..];
        rest = after.strip_prefix(',').unwrap_or(after);
    }
    out.push_str(rest);
    out
}

static NEXT: AtomicU32 = AtomicU32::new(0);

/// The fixture written to a fresh folder (removed on drop).
struct Install(PathBuf);

impl Install {
    fn new() -> Install {
        let dir = std::env::temp_dir().join(format!(
            "ow-cli-combat-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in fixture::files() {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        Install(dir)
    }
}

impl Drop for Install {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The line the command prints for `steps` in `mode` (its newline included, as the digests hash it).
fn report(install: &Install, steps: &str, mode: ParityMode) -> (String, bool) {
    let w = world(&install.0, mode).unwrap();
    let steps: Vec<String> = steps.split(' ').map(str::to_string).collect();
    let (text, failed) = ow_combat::run::combat_run(w, &steps).unwrap();
    (
        format!(
            "{text}
"
        ),
        failed,
    )
}

#[test]
fn the_normalization_removes_only_timing_fields_and_set_health_lines() {
    let text = r#"{"a":1,"milliseconds":0.25,"events":["TakeDamage x","SetHealth -5.000000","NotifyTakeHit"],"elapsed_ms":3e-05}"#;
    assert_eq!(
        normalize(text),
        r#"{"a":1,"events":["TakeDamage x","NotifyTakeHit"]}"#
    );
}

#[test]
fn combat_run_equals_the_cxx_binary_on_the_invented_packages() {
    let install = Install::new();
    for (name, steps, cxx, off_equal) in CASES {
        let (reference, _) = report(&install, steps, ParityMode::Reference);
        assert_eq!(
            fnv1a64(normalize(&reference).as_bytes()),
            cxx,
            "{name} (Reference): {reference}"
        );
        let (off, _) = report(&install, steps, ParityMode::Off);
        let same = fnv1a64(normalize(&off).as_bytes()) == cxx;
        assert_eq!(same, off_equal, "{name} (Off): {off}");
    }
}

/// Replay determinism (lane W6-COMBAT-RUN2): each list run twice in the same process, in each mode, prints the same
/// line once the two timing fields are removed.
#[test]
fn every_list_replays_to_the_same_report_in_the_same_process() {
    let install = Install::new();
    for (name, steps, _, _) in CASES {
        for mode in [ParityMode::Reference, ParityMode::Off] {
            let (first, a) = report(&install, steps, mode);
            let (second, b) = report(&install, steps, mode);
            assert_eq!(a, b, "{name} ({mode:?})");
            assert_eq!(normalize(&first), normalize(&second), "{name} ({mode:?})");
        }
    }
}

#[test]
fn off_mode_refuses_each_attribute_the_toy_classes_do_not_declare() {
    let install = Install::new();
    let (off, failed) = report(&install, CASES[13].1, ParityMode::Off);
    assert!(failed);
    for text in [
        "the attribute WillowMind.ReceivedBulletDamageModifier: the VM object's class does not declare it",
        "the attribute WillowPlayerController.InstigatedMeleeDamageModifier: the VM object's class does not declare it",
    ] {
        assert!(off.contains(text), "{off}");
    }
    let (reference, failed) = report(&install, CASES[13].1, ParityMode::Reference);
    assert!(!failed, "{reference}");
    assert!(
        reference.contains("\"hit.Armored.L4.1.applied\":60"),
        "{reference}"
    );
}

#[test]
fn a_run_with_a_maya_step_is_built_over_the_mission_bridge() {
    let args: Vec<String> = ["ToyPawns.upk", "--combat-run", "--cooked", "x", "spawn:dummy:1", "maya:8"]
        .map(String::from)
        .to_vec();
    assert!(ow_combat::run::wants_bridge(&args[4..]));
    assert!(!ow_combat::run::wants_bridge(&args[4..5]));
}
