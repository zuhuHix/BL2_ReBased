//! Owner: lane IMPL-AIMOFFSET-PROFILES. The aim-offset profiles layer: the profile names of an aim-offset node whose
//! `Profiles` the store skipped (`ow_defs::DefStore::is_skipped`), read from the node's own export by an installed
//! [`ProfileSource`], cached by the node's identity. AI-assisted (Claude Code). Decision 2026-11-25-NNN.
//!
//! The reader lives in `ow-assets` (`aim_profiles`, a fitted layout checked by structural oracles, **UNVERIFIED**),
//! which this crate may not depend on (the crate edges), so `ow-cli` installs it as the source, as the skeletal mesh
//! layer's source is installed. Every refusal is `NotYet { LayerAbsent }` on [`aim_offset::PROFILES_LAYER`]:
//! - no source is installed (the [`aim_offset::PROFILES`] text, as before the reader);
//! - the node is not a loaded export of the store, or the source finds no `Profiles` tag;
//! - the source's tag walk, from the ordinary prefix 4, does not end where the store's own decode ended (the start of
//!   the store's raw tail), so the prefix is not the store's;
//! - the reader refuses (its oracle's reason in the text).
//!
//! Reads only: no load, no write, nothing interned (names stay text).

use std::rc::Rc;

use ow_core::{DetMap, NotYet};
use ow_object::heap::Origin;
use ow_object::ids::ObjRef;
use ow_package::package::Package;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::world::aim_offset::{PROFILES, PROFILES_LAYER, RECORD};

/// Where an ordinary object's tags start in its export payload (the store's `PropertyPrefix::Ordinary`).
pub const PROPERTY_OFFSET: usize = 4;

/// What a source read from one export: the profile names (`None` without a `Profiles` tag) and where the export's tag
/// stream ended, from its payload start.
pub type SourceProfiles = (Option<Vec<String>>, usize);

/// The reader of a node's binary-serialized `Profiles`, installed by the caller (`ow-cli`).
pub trait ProfileSource {
    /// The profile names of export `index` of `package`, its tags read from `property_offset`. `Err(text)`: the read
    /// was refused (an oracle failed).
    fn profiles_in(&self, package: &Package, index: i32, property_offset: usize) -> Result<SourceProfiles, String>;
}

/// The layer: the source and the names read so far, by node.
pub struct AimProfiles {
    source: Box<dyn ProfileSource>,
    cache: DetMap<ObjRef, Rc<Vec<String>>>,
}

impl AimProfiles {
    /// Installs `source` as the VM's profiles reader (once per VM).
    pub fn install(vm: &mut Vm, source: Box<dyn ProfileSource>) -> Result<(), VmError> {
        vm.add_service(AimProfiles { source, cache: DetMap::default() }).map(|_| ())
    }
}

fn absent(why: Option<String>) -> VmError {
    let what = match why {
        None => PROFILES.to_string(),
        Some(why) => format!("{PROFILES}: {why}"),
    };
    NotYet::layer_absent(what, PROFILES_LAYER, RECORD).into()
}

/// The profile names of `node`, a loaded definition whose `Profiles` the store skipped (module documentation).
pub fn profile_names(vm: &Vm, node: ObjRef) -> Result<Rc<Vec<String>>, VmError> {
    let Some(cell) = vm.service::<AimProfiles>() else {
        return Err(absent(None));
    };
    if let Some(hit) = cell.try_borrow().map_err(|_| VmError::ServiceBorrowed("AimProfiles"))?.cache.get(&node) {
        return Ok(Rc::clone(hit));
    }
    let (Origin::Export { package, index } | Origin::Resource { package, index }) = vm.space.heap.get(node)?.origin else {
        return Err(absent(Some("the node is not a package export".into())));
    };
    let Some(tail) = vm.defs.raw_tail(node) else {
        return Err(absent(Some("the node is not a loaded definition".into())));
    };
    let pkg = vm.defs.packages().get(package);
    let base = pkg.object(index).map_err(|e| absent(Some(e.to_string())))?.offset as usize;
    let store_end = tail.range.start.checked_sub(base).ok_or_else(|| absent(Some("the store's tail is outside the export".into())))?;
    let read = cell.try_borrow().map_err(|_| VmError::ServiceBorrowed("AimProfiles"))?.source.profiles_in(pkg, index, PROPERTY_OFFSET);
    let names = match read {
        Err(why) => return Err(absent(Some(why))),
        Ok((None, _)) => return Err(absent(Some("the node's export has no Profiles tag".into()))),
        Ok((Some(_), end)) if end != store_end => {
            return Err(absent(Some(format!("the tag walk ends at {end}, the store's decode at {store_end}"))));
        }
        Ok((Some(names), _)) => Rc::new(names),
    };
    cell.try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("AimProfiles"))?
        .cache
        .insert(node, Rc::clone(&names));
    Ok(names)
}
