//! Owner: lane FIX-PHASELOCK-CHAIN (decision 2026-11-16-69). `Object.PathName(object)`, the stock UE3 Core native that
//! answers an object's full path name. AI-assisted (Claude Code).
//!
//! **The rule** (public UE3 semantics, not a Gearbox rule): the names of the object's outer chain, outermost first,
//! each joined to the one before it by `.`, except that `:` goes before an object whose outer is not a package while
//! that outer's outer is a package (the subobject delimiter). So a definition in a group reads
//! `GD_Siren_Skills.Misc.Att_Phaselock_Duration`, and a level's actor reads `<Map>.TheWorld:PersistentLevel.<Actor>`.
//! The `:` case is what the recorded engine dump shows for level actors (BLCMM_DUMP_CROSSCHECK.md); its use for a data
//! subobject (an attribute's resolver, `...Att_X:Resolver_0`) follows the same rule and is **UNVERIFIED** (no live
//! read of one). Whether an object is a package is its class: `Core.Package`.
//!
//! **Which chain.** An object read from a package (an export, or a reference resolved through the store) uses that
//! package's export and import tables, with `ow_defs::identity::full_path`'s rule for the file (review r1 of PR #368):
//! an export whose outermost export is a `Package` (a package cooked into another file, as `Startup.upk` holds the
//! `GD_*` packages) is named by its path in the file; any other export starts with the file's own package, the file
//! stem (a map's `<Map>.TheWorld:PersistentLevel.<Actor>`); an import's chain already ends at its package. A class
//! default object is `<class package>.Default__<class>`. An object made
//! at run time uses the VM's outer links; a package with no outer is its name, any other object with no outer is
//! `NotYet { NotRead }`, since in the game its path starts at the transient package or a level, which the VM does not
//! model. A None object is `NotYet { NotRead }` too (the
//! answer for None is not read). The callers this lane needs (`WillowPawn.GetInstancedDesignerAttribute`,
//! `InstancedDesignerAttribute.Initialize`) pass a definition, store the string and compare it with the same call on
//! the same definition.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::heap::Origin;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_package::PackageId;

use crate::api::{NativeCall, Vm};
use crate::error::VmError;

/// The record of the rule.
pub const RECORD: &str = "public UE3 Object.PathName; BLCMM_DUMP_CROSSCHECK.md (the ':' of a level actor's path)";

/// The class of a package object.
const PACKAGE_CLASS: &str = "Package";

/// The longest outer chain followed (a Rust-only bound against cycles in data).
const MAX_DEPTH: usize = 64;

/// One link of an outer chain: the name and whether that object is a package.
type Link = (String, bool);

/// The chain of export or import `index` of `package`, outermost first.
fn package_chain(vm: &Vm, package: PackageId, index: i32) -> Result<Vec<Link>, VmError> {
    let p = vm.defs.packages().get(package);
    let mut links = Vec::new();
    let mut at = index;
    while at != 0 {
        if links.len() >= MAX_DEPTH {
            return Err(VmError::Script {
                function: "Object.PathName".into(),
                message: format!("an outer chain longer than {MAX_DEPTH} links"),
            });
        }
        let entry = p.object(at).map_err(|e| VmError::Script {
            function: "Object.PathName".into(),
            message: e.to_string(),
        })?;
        let is_package = if at < 0 {
            entry.class_name == PACKAGE_CLASS
        } else {
            entry.class != 0
                && p.object(entry.class)
                    .map(|c| c.name == PACKAGE_CLASS)
                    .unwrap_or(false)
        };
        links.push((entry.name.clone(), is_package));
        at = entry.outer;
    }
    links.reverse();
    // The file's own package leads when the rule of ow-defs' full_path adds it (module documentation).
    let full = ow_defs::identity::full_path(p, index).map_err(|e| VmError::Script {
        function: "Object.PathName".into(),
        message: e.to_string(),
    })?;
    let in_file = p.path_of(index).map_err(|e| VmError::Script {
        function: "Object.PathName".into(),
        message: e.to_string(),
    })?;
    if full != in_file {
        links.insert(0, (p.name.clone(), true));
    }
    Ok(links)
}

/// The chain of `o`, outermost first ([module documentation](self)).
fn chain(vm: &Vm, o: ObjRef, depth: usize) -> Result<Vec<Link>, VmError> {
    if depth > MAX_DEPTH {
        return Err(VmError::Script {
            function: "Object.PathName".into(),
            message: format!("an outer chain longer than {MAX_DEPTH} links"),
        });
    }
    let object = vm.space.heap.get(o)?;
    match object.origin {
        Origin::Export { package, index } | Origin::Resource { package, index } => {
            package_chain(vm, package, index)
        }
        // A class default object: `<class package>.Default__<class>` (its outer is the class's package).
        Origin::Default { class } => {
            let path = vm
                .space
                .classes
                .get_layout(class)
                .map(|l| l.path().as_str().to_string())
                .unwrap_or_default();
            match path.split_once('.') {
                Some((package, name)) => Ok(vec![
                    (package.to_string(), true),
                    (format!("Default__{name}"), false),
                ]),
                None => Err(NotYet::not_read(
                    format!("Object.PathName of the class default object of {path}: its class has no package"),
                    RECORD,
                )
                .into()),
            }
        }
        _ => {
            let is_package = vm
                .space
                .classes
                .get_layout(object.class)
                .is_some_and(|l| l.path().as_str() == "Core.Package");
            let name = vm.space.names.text(object.name).into_owned();
            let Some(outer) = object.outer else {
                // A top-level package has no outer: its path is its name.
                if is_package {
                    return Ok(vec![(name, true)]);
                }
                return Err(NotYet::not_read(
                    format!(
                        "Object.PathName of {name}, made at run time with no outer (the game's transient package or \
                         level is not modelled)"
                    ),
                    RECORD,
                )
                .into());
            };
            let mut links = chain(vm, outer, depth + 1)?;
            links.push((name, is_package));
            Ok(links)
        }
    }
}

/// The full path name of `o` ([module documentation](self)).
pub fn path_name(vm: &Vm, o: ObjRef) -> Result<String, VmError> {
    let links = chain(vm, o, 0)?;
    let mut out = String::new();
    for (i, (name, _)) in links.iter().enumerate() {
        if i > 0 {
            let outer_is_package = links[i - 1].1;
            let outer_outer_is_package = i >= 2 && links[i - 2].1;
            out.push(if !outer_is_package && outer_outer_is_package { ':' } else { '.' });
        }
        out.push_str(name);
    }
    Ok(out)
}

/// Binds `Object.PathName(object)`.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    vm.register_native(
        "Object.PathName(object)",
        Rc::new(|c: &mut NativeCall<'_>| match c.arg(0)? {
            Value::Object(Some(o)) | Value::Interface(Some(o)) => {
                let o = *o;
                Ok(Value::Str(path_name(c.vm, o)?.into()))
            }
            _ => Err(NotYet::not_read("Object.PathName of None", RECORD).into()),
        }),
    )
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use ow_attr::AttributeSystem;
    use ow_core::{Missing, ParityMode};
    use ow_defs::DefStore;
    use ow_object::heap::{ObjectSpace, Origin};
    use ow_object::layout::{PropKind, PropertyDecl};
    use ow_object::value::Value;

    use super::*;

    /// Invented objects made at run time: a package `Pkg`, a group `Grp` in it, a thing `Thing` in the group, a part
    /// `Part` of the thing (the subobject delimiter), a piece `Piece` of the part, and a loose object with no outer.
    #[test]
    fn the_path_name_joins_the_outer_chain_with_the_subobject_delimiter() {
        let mut space = ObjectSpace::new();
        let object = space.classes.define("Core.Object", None, vec![]).unwrap();
        let package = space.classes.define("Core.Package", Some(object), vec![]).unwrap();
        let thing = space.classes.define("Test.Thing", Some(object), vec![]).unwrap();
        let pkg = space.alloc(package, "Pkg", None, Origin::Runtime).unwrap();
        let grp = space.alloc(package, "Grp", Some(pkg), Origin::Runtime).unwrap();
        let t = space.alloc(thing, "Thing", Some(grp), Origin::Runtime).unwrap();
        let part = space.alloc(thing, "Part", Some(t), Origin::Runtime).unwrap();
        let piece = space.alloc(thing, "Piece", Some(part), Origin::Runtime).unwrap();
        let loose = space.alloc(thing, "Loose", None, Origin::Runtime).unwrap();
        for mode in [ParityMode::Off, ParityMode::Reference] {
            let mut vm = Vm::new(
                space.clone(),
                DefStore::new_empty(),
                Rc::new(AttributeSystem::new_empty()),
                mode,
            );
            assert_eq!(path_name(&vm, pkg).unwrap(), "Pkg");
            assert_eq!(path_name(&vm, grp).unwrap(), "Pkg.Grp");
            assert_eq!(path_name(&vm, t).unwrap(), "Pkg.Grp.Thing");
            // Part's outer (Thing) is no package and Thing's outer (Grp) is one: the subobject delimiter.
            assert_eq!(path_name(&vm, part).unwrap(), "Pkg.Grp.Thing:Part");
            // Piece's outer (Part) is no package and neither is Part's outer: a dot again.
            assert_eq!(path_name(&vm, piece).unwrap(), "Pkg.Grp.Thing:Part.Piece");
            match path_name(&vm, loose) {
                Err(VmError::NotYet(n)) => assert_eq!(n.missing, Missing::NotRead),
                other => panic!("{other:?}"),
            }
            // The native: a string for an object, NotYet for None.
            register(&mut vm).unwrap();
            let arg = vm.space.names.intern("CheckObject");
            let ret = vm.space.names.intern("ReturnValue");
            let decl = crate::api::FunctionDecl::native(
                ow_core::ObjectPath::parse("Core.Object.PathName").unwrap(),
                "Object.PathName(object)",
            )
            .with_params(vec![
                PropertyDecl::new(arg, PropKind::Object { class: object }).with_flags(PropertyDecl::PARAM),
            ])
            .with_result(PropertyDecl::new(ret, PropKind::Str).with_flags(PropertyDecl::RETURN));
            let f = vm.declare_function(decl).unwrap();
            let r = vm.call(f, None, vec![Value::Object(Some(part))]).unwrap();
            assert_eq!(r.ret, Value::Str("Pkg.Grp.Thing:Part".into()), "{mode:?}");
            assert!(matches!(
                vm.call(f, None, vec![Value::Object(None)]),
                Err(VmError::NotYet(_))
            ));
        }
    }

    /// Review r1 of PR #368: paths from a package's tables, in an invented map-shaped file `Sanctuary_Toy.upk` (a world,
    /// its level, an actor in it), plus a group package cooked into the same file with a definition and its
    /// subobject, an import, and a class default object.
    #[test]
    fn package_paths_lead_with_the_file_stem_unless_the_top_export_is_a_package() {
        use ow_defs::identity::IdentityPolicy;
        use ow_package::testkit::{Export, PackageBuilder};

        let mut b = PackageBuilder::new_empty();
        let core = b.add_import("Core", "Package", 0, "Core");
        let engine = b.add_import("Core", "Package", 0, "Engine");
        let package_class = b.add_import("Core", "Class", core, "Package");
        let world_class = b.add_import("Core", "Class", engine, "World");
        let level_class = b.add_import("Core", "Class", engine, "Level");
        let actor_class = b.add_import("Core", "Class", engine, "Actor");
        let world = b.add_export(world_class, "TheWorld", vec![0u8; 4]);
        let mut e = Export::new(level_class, "PersistentLevel", vec![0u8; 4]);
        e.outer = world;
        let level = b.add_export_with(e);
        let mut e = Export::new(actor_class, "Actor_1", vec![0u8; 4]);
        e.outer = level;
        let actor = b.add_export_with(e);
        let group = b.add_export(package_class, "GD_Toy", vec![0u8; 4]);
        let mut e = Export::new(actor_class, "Def", vec![0u8; 4]);
        e.outer = group;
        let def = b.add_export_with(e);
        let mut e = Export::new(actor_class, "Sub", vec![0u8; 4]);
        e.outer = def;
        let sub = b.add_export_with(e);
        let bytes = b.build();

        let dir = std::env::temp_dir().join(format!("ow-vm-path-name-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for mode in [ParityMode::Off, ParityMode::Reference] {
            let defs = DefStore::from_synthetic(
                &dir,
                &[("Sanctuary_Toy.upk", bytes.as_slice())],
                IdentityPolicy::FirstInScanOrder,
                mode,
            )
            .unwrap();
            let mut space = ObjectSpace::new();
            let object = space.classes.define("Core.Object", None, vec![]).unwrap();
            let thing = space.classes.define("Engine.Thing", Some(object), vec![]).unwrap();
            let mut vm = Vm::new(space, defs, Rc::new(AttributeSystem::new_empty()), mode);
            let name = ow_core::PackageName::parse("Sanctuary_Toy").unwrap();
            let id = vm.defs.code_mut().1.load(&name).unwrap();
            let at = |vm: &mut Vm, index: i32, origin: fn(PackageId, i32) -> Origin| {
                vm.space.alloc(thing, "X", None, origin(id, index)).unwrap()
            };
            let export = |package, index| Origin::Export { package, index };
            let resource = |package, index| Origin::Resource { package, index };
            let a = at(&mut vm, actor, export);
            assert_eq!(path_name(&vm, a).unwrap(), "Sanctuary_Toy.TheWorld:PersistentLevel.Actor_1", "{mode:?}");
            let l = at(&mut vm, level, resource);
            assert_eq!(path_name(&vm, l).unwrap(), "Sanctuary_Toy.TheWorld:PersistentLevel", "{mode:?}");
            let w = at(&mut vm, world, export);
            assert_eq!(path_name(&vm, w).unwrap(), "Sanctuary_Toy.TheWorld", "{mode:?}");
            // A package cooked into the file: named by its path in the file; its definition's subobject takes ':'.
            let d = at(&mut vm, def, export);
            assert_eq!(path_name(&vm, d).unwrap(), "GD_Toy.Def", "{mode:?}");
            let s = at(&mut vm, sub, export);
            assert_eq!(path_name(&vm, s).unwrap(), "GD_Toy.Def:Sub", "{mode:?}");
            // An import's chain ends at its own package.
            let i = at(&mut vm, actor_class, resource);
            assert_eq!(path_name(&vm, i).unwrap(), "Engine.Actor", "{mode:?}");
            // A class default object (review r1, L2).
            let cdo = vm
                .space
                .alloc(thing, "Default__Thing", None, Origin::Default { class: thing })
                .unwrap();
            assert_eq!(path_name(&vm, cdo).unwrap(), "Engine.Default__Thing", "{mode:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

}
