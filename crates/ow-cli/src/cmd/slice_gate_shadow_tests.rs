//! The installed census of silent native shadows (lane PLACEHOLDER-SHADOWS): every world the slice gate builds, built
//! with the VM's shadow audit on (`ow_vm::api::audit_new_vms`), reports each key that one pack bound and another gave
//! up (a keep-existing check, an `is_bound` guard, or a refused registration a caller swallowed). Ignored: it reads the
//! installed game (`OW_GAME`, else the default Steam folder). It prints keys and source files only. AI-assisted
//! (Claude Code).

use std::path::PathBuf;

use ow_core::ParityMode;
use ow_vm::api::{NativeShadow, Vm, audit_new_vms};

fn cooked() -> PathBuf {
    let root = std::env::var_os("OW_GAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2"));
    ow_core::install::cooked_or_join(&root).unwrap()
}

fn report(world: &str, vm: &Vm, all: &mut Vec<(String, NativeShadow)>) {
    let shadows: Vec<_> = vm.native_shadows().into_iter().filter(|s| !super::combat::shared_wrapper_shadow(s)).collect();
    println!("{world}: {} shadow(s)", shadows.len());
    for s in shadows {
        println!("  {}  bound by {}  given up in {}", s.key, s.bound_by, s.checked_by);
        all.push((world.to_string(), s));
    }
}

#[test]
#[ignore = "reads the installed game"]
fn installed_slice_worlds_have_no_silent_native_shadow() {
    let cooked = cooked();
    audit_new_vms(true);
    let mut all = Vec::new();
    match super::combat::world(&cooked, ParityMode::Off) {
        Ok(w) => report("combat-run (links 1, 3, 4, 5, 13, 14)", &w.vm, &mut all),
        Err(e) => println!("combat-run: set-up failed: {e:?}"),
    }
    match crate::cmd::inventory::equip_run::equip_world(&cooked) {
        Ok(w) => report("equip (link 8)", &w.vm, &mut all),
        Err(e) => println!("equip: set-up failed: {e:?}"),
    }
    match crate::cmd::inventory::equip_run::pickup_world(&cooked) {
        Ok((w, _)) => report("pickup (links 6, 7)", &w.vm, &mut all),
        Err(e) => println!("pickup: set-up failed: {e:?}"),
    }
    let mut harness = Vec::new();
    match super::phaselock_world(&cooked, &mut harness) {
        Ok(w) => report("phaselock (link 9)", &w.vm, &mut all),
        Err(e) => println!("phaselock: set-up failed: {e:?}"),
    }
    let cooked_text = cooked.to_string_lossy().into_owned();
    match crate::cmd::slice::open(&cooked_text, ParityMode::Off, ow_defs::identity::IdentityPolicy::FirstInScanOrder) {
        Ok(vm) => match ow_mission::bridge::world(vm) {
            Ok(w) => report("mission (links 10, 11, 12)", &w.vm, &mut all),
            Err(e) => println!("mission: set-up failed: {e:?}"),
        },
        Err(e) => println!("mission: open failed: {e:?}"),
    }
    audit_new_vms(false);
    assert!(all.is_empty(), "{} silent shadow(s): {all:#?}", all.len());
}
