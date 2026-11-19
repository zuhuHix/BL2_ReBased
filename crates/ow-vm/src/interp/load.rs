//! Owner: lane W3-INTERP. Loading functions and classes from the code packages: the port of `Runtime::findClass`,
//! `findFunction`, `functionAt`, `loadClass` (its function and state half), `findMethod`, `buildNativeIndex` and
//! `nativeByIndex` (`src/vm.cpp`), over the definition store's package store and reflection index
//! (`DefStore::code_mut`).
//!
//! A class's layout is `CodeIndex::define_layout`'s (one index defines every layout in the VM's object space). Its
//! functions are declared in the VM's function table when the class's code loads: every function the class and its
//! states declare (the C++ `loadClass` calls `functionAt` for each), with parameters typed by
//! `CodeIndex::child_declarations`, a native's registry key built as `vm.cpp:600-606` builds it, and the class as
//! the owner (for a state's function, the class that owns the state). A super class's code loads first.
//!
//! A function whose header does not read, or whose declarations do not type, is still declared (as the C++ keeps it)
//! and fails when it is called, with the C++ text `cannot run <path>: <why>`.

use std::collections::BTreeMap;
use std::rc::Rc;

use ow_core::{FixedHashMap, NotYet, ObjectPath};
use ow_defs::store::DefError;
use ow_object::heap::HeapError;
use ow_object::ids::{ClassId, FuncId, NameId, ObjRef};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::reflect::{CODE_PACKAGES, ClassKey, ReflectError};
use ow_package::PackageId;
use ow_script::function::{FunctionInfo, is_function_export, read_function};
use ow_script::ops::flags::{FUNC_NATIVE, FUNC_PRE_OPERATOR, FUNC_STATIC};

use super::{ClassCode, FuncCode, Local};
use crate::api::{FunctionDecl, Vm};
use crate::error::VmError;

/// An error's text and its causes' texts joined with `: ` (the C++ `what()`).
pub(crate) fn chain_text(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(c) = cause {
        text.push_str(": ");
        text.push_str(&c.to_string());
        cause = c.source();
    }
    text
}

/// A C++ `RuntimeError` raised while running or loading `function`.
pub(crate) fn script_error(function: &str, message: impl Into<String>) -> VmError {
    VmError::Script {
        function: function.to_string(),
        message: message.into(),
    }
}

/// A definition store error as the VM reports it (`From<DefError>`: `NotYet` and heap errors keep their own
/// variants, the rest is [`VmError::Def`]). `context` is not needed: the store's error names what failed.
pub(crate) fn def_error(_context: &str, e: DefError) -> VmError {
    VmError::from(e)
}

fn reflect_error(context: &str, e: &ReflectError) -> VmError {
    script_error(context, chain_text(e))
}

/// True when the definition store holds no packages at all (`DefStore::new_empty`): no code layer to load from.
fn no_code_layer(vm: &Vm) -> bool {
    vm.defs.packages().root().as_os_str().is_empty()
}

fn absent(what: String) -> VmError {
    NotYet::layer_absent(
        what,
        "ow-defs code packages",
        "lane W3-INTERP (a VM whose definition store holds no packages)",
    )
    .into()
}

/// [`crate::api::Vm::find_class`]: a class the table has (its code loaded if it came from a package), else the
/// class loaded from the code packages (C++ `findClass`).
/// [`find_class`], with a class that is genuinely absent (no installed package holds it, or no code layer at all) as
/// `None`; every other failure is kept (lane FIX-RUN-BEHAVIORS, review r1 of #439).
pub(crate) fn find_class_if_present(vm: &mut Vm, path: &str) -> Result<Option<ClassId>, VmError> {
    if vm.space.classes.by_path(path).is_some_and(|id| vm.space.classes.get_layout(id).is_some_and(|l| l.is_defined())) {
        return find_class(vm, path).map(Some);
    }
    if no_code_layer(vm) {
        return Ok(None);
    }
    let found = {
        let (code, store) = vm.defs.code_mut();
        code.find_class(store, path)
    };
    match found {
        Err(ReflectError::ClassNotFound(_))
        | Err(ReflectError::Store(ow_package::store::StoreError::PackageNotFound(_))) => Ok(None),
        Err(e) => Err(reflect_error(path, &e)),
        Ok(_) => find_class(vm, path).map(Some),
    }
}

pub(crate) fn find_class(vm: &mut Vm, path: &str) -> Result<ClassId, VmError> {
    if let Some(id) = vm.space.classes.by_path(path)
        && vm
            .space
            .classes
            .get_layout(id)
            .is_some_and(|l| l.is_defined())
    {
        ensure_class_code(vm, id)?;
        return Ok(id);
    }
    if no_code_layer(vm) {
        return Err(absent(format!(
            "class {path}: loading classes from the code packages"
        )));
    }
    let key = {
        let (code, store) = vm.defs.code_mut();
        code.find_class(store, path)
    }
    .map_err(|e| reflect_error(path, &e))?;
    let id = layout_of(vm, key, path)?;
    ensure_class_code(vm, id)?;
    Ok(id)
}

/// [`crate::api::Vm::find_function`] for a path not declared yet (C++ `findFunction`).
pub(crate) fn find_function(vm: &mut Vm, path: &str) -> Result<FuncId, VmError> {
    if no_code_layer(vm) {
        return Err(absent(format!(
            "function {path}: loading functions from the code packages"
        )));
    }
    find_function_if_any(vm, path)?
        .ok_or_else(|| script_error(path, format!("function not found: {path}")))
}

/// [`find_function`] where a confirmed absence is `None`: no code layer, or a package that holds no export at the
/// path. Every other failure (a malformed path, a package that fails to load, an export that is not a function) is
/// the same error `find_function` gives.
pub(crate) fn find_function_if_any(vm: &mut Vm, path: &str) -> Result<Option<FuncId>, VmError> {
    if no_code_layer(vm) {
        return Ok(None);
    }
    let Some((package_name, rest)) = path.split_once('.') else {
        return Err(script_error(
            path,
            format!("function path must be Package.Class.Function: {path}"),
        ));
    };
    let (package, index) = {
        let (code, store) = vm.defs.code_mut();
        let package = code
            .package(store, package_name)
            .map_err(|e| reflect_error(path, &e))?;
        (package, code.find_export(store, package, rest))
    };
    if index == 0 {
        return Ok(None);
    }
    function_at(vm, package, index)?
        .ok_or_else(|| script_error(path, format!("not a function: {path}")))
        .map(Some)
}

/// The layout of loaded class `key` (defined on first use).
fn layout_of(vm: &mut Vm, key: ClassKey, context: &str) -> Result<ClassId, VmError> {
    let (code, store) = vm.defs.code_mut();
    code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key)
        .map_err(|e| reflect_error(context, &e))
}

/// The class `reference` of `package` names (C++ `classAt`), its layout defined; `None` where it resolves to nothing.
pub(crate) fn class_at(
    vm: &mut Vm,
    package: PackageId,
    reference: i32,
    context: &str,
) -> Result<Option<ClassId>, VmError> {
    let key = {
        let (code, store) = vm.defs.code_mut();
        code.class_at(store, package, reference)
    }
    .map_err(|e| reflect_error(context, &e))?;
    match key {
        Some(key) => layout_of(vm, key, context).map(Some),
        None => Ok(None),
    }
}

/// Loads the functions and states of `class` and of its super classes, once (a class the table defined by hand, not
/// from a package, has none to load).
pub(crate) fn ensure_class_code(vm: &mut Vm, class: ClassId) -> Result<(), VmError> {
    if vm.code.classes.contains_key(&class) {
        return Ok(());
    }
    // The chain from `class` up to the first class already loaded; loaded root first.
    let mut chain = Vec::new();
    let mut cursor = Some(class);
    while let Some(c) = cursor {
        if vm.code.classes.contains_key(&c) || chain.contains(&c) {
            break;
        }
        chain.push(c);
        cursor = vm.space.classes.get_layout(c).and_then(|l| l.super_class());
    }
    for c in chain.into_iter().rev() {
        load_class_code(vm, c)?;
    }
    Ok(())
}

fn load_class_code(vm: &mut Vm, class: ClassId) -> Result<(), VmError> {
    let Some(key) = vm.defs.code().key_of(class) else {
        return Ok(());
    };
    let info = vm.defs.code().class(key);
    let package = info.package;
    let functions: Vec<(String, i32)> = info
        .functions
        .iter()
        .map(|(k, &v)| (k.clone(), v))
        .collect();
    let states: Vec<(String, Vec<(String, i32)>)> = info
        .states()
        .iter()
        .map(|(k, s)| {
            (
                k.clone(),
                s.functions.iter().map(|(n, &f)| (n.clone(), f)).collect(),
            )
        })
        .collect();
    let mut loaded = ClassCode::default();
    for (name, export) in functions {
        let f = declare_package_function(vm, package, export, Some(class))?;
        loaded.functions.insert(name, f);
    }
    for (key, list) in states {
        let mut in_state = BTreeMap::new();
        for (name, export) in list {
            let f = declare_package_function(vm, package, export, Some(class))?;
            in_state.insert(name, f);
        }
        loaded.states.insert(key, in_state);
    }
    vm.code.classes.insert(class, Rc::new(loaded));
    Ok(())
}

/// The function `reference` of `package` names (C++ `functionAt`), loading its class's code; `None` where it
/// resolves to nothing or to an export that is not a function.
pub(crate) fn function_at(
    vm: &mut Vm,
    package: PackageId,
    reference: i32,
) -> Result<Option<FuncId>, VmError> {
    let (target, owner_ref) = {
        let (code, store) = vm.defs.code_mut();
        let Some((p, i)) = code.resolve_ref(store, package, reference) else {
            return Ok(None);
        };
        if !is_function_export(store.get(p), i) {
            return Ok(None);
        }
        if let Some(&f) = vm.code.functions.get(&(p, i)) {
            return Ok(Some(f));
        }
        // The owner: the class, or the class owning the state the function is declared in.
        let pkg = store.shared(p);
        let mut outer = pkg.exports[i as usize - 1].outer;
        if outer > 0 && code.class_name(store, p, outer) == "State" {
            outer = pkg.exports[outer as usize - 1].outer;
        }
        let owner = (outer > 0 && code.class_name(store, p, outer) == "Class").then_some(outer);
        ((p, i), owner)
    };
    let owner = match owner_ref {
        Some(outer) => {
            let class = class_at(vm, target.0, outer, "function owner")?;
            if let Some(c) = class {
                ensure_class_code(vm, c)?;
                if let Some(&f) = vm.code.functions.get(&target) {
                    return Ok(Some(f));
                }
            }
            class
        }
        None => None,
    };
    declare_package_function(vm, target.0, target.1, owner).map(Some)
}

/// The C++ `typeTag` of a parameter, as the native key spells it.
fn type_tag(vm: &Vm, kind: &PropKind) -> String {
    match kind {
        PropKind::Int => "int".into(),
        PropKind::Float => "float".into(),
        PropKind::Byte { .. } => "byte".into(),
        PropKind::Bool => "bool".into(),
        PropKind::Str => "string".into(),
        PropKind::Name => "name".into(),
        PropKind::Object { .. } => "object".into(),
        PropKind::Interface { .. } => "interface".into(),
        PropKind::Class { .. } => "class".into(),
        PropKind::Array(_) => "array".into(),
        PropKind::Delegate => "delegate".into(),
        PropKind::Struct(s) => vm
            .space
            .classes
            .struct_layout(*s)
            .map_or_else(|| "struct".into(), |l| l.path().name().to_ascii_lowercase()),
        PropKind::FloatAttribute => "floatattribute".into(),
        PropKind::IntAttribute => "intattribute".into(),
        PropKind::ByteAttribute { .. } => "byteattribute".into(),
        PropKind::Map { .. } => "map".into(),
        other => other.kind_name().to_ascii_lowercase(),
    }
}

/// Declares export `export` of `package` (a Function export) in the VM's function table, once.
fn declare_package_function(
    vm: &mut Vm,
    package: PackageId,
    export: i32,
    owner: Option<ClassId>,
) -> Result<FuncId, VmError> {
    if let Some(&f) = vm.code.functions.get(&(package, export)) {
        return Ok(f);
    }
    let (name, path, info, decls, failure) = {
        let (code, store) = vm.defs.code_mut();
        let pkg = store.shared(package);
        let name = pkg.exports[export as usize - 1].name.clone();
        let path = format!(
            "{}.{}",
            pkg.name,
            pkg.path_of(export)
                .map_err(|e| script_error(&name, chain_text(&e)))?
        );
        let mut failure = None;
        let info = match read_function(&pkg, export) {
            Ok(info) => info,
            Err(e) => {
                failure = Some(e.to_string());
                FunctionInfo::default()
            }
        };
        let decls = match code.child_declarations(
            store,
            &mut vm.space.names,
            &mut vm.space.classes,
            package,
            export,
        ) {
            Ok(d) => d,
            Err(e) => {
                failure.get_or_insert_with(|| chain_text(&e));
                Vec::new()
            }
        };
        (name, path, info, decls, failure)
    };
    let mut locals = Vec::with_capacity(decls.len());
    let mut by_export = FixedHashMap::default();
    let (mut params, mut result) = (Vec::new(), None);
    let mut slot = 0usize;
    for (at, decl) in decls.into_iter().enumerate() {
        if decl.is_return() {
            result = Some(at);
        } else if decl.is_param() {
            params.push(at);
        }
        if let Some((_, index)) = decl.source {
            by_export.insert(index, at);
        }
        let width = usize::try_from(decl.array_dim.max(1)).unwrap_or(1);
        locals.push(Local { decl, slot });
        slot += width;
    }
    let param_decls: Vec<PropertyDecl> = params.iter().map(|&i| locals[i].decl.clone()).collect();
    let object_path = ObjectPath::parse(&path).map_err(|e| script_error(&path, e.to_string()))?;
    let mut decl = if info.flags & FUNC_NATIVE != 0 {
        let owner_name = owner
            .and_then(|c| vm.space.classes.get_layout(c))
            .map_or_else(|| "?".to_string(), |l| l.path().name().to_string());
        let shown = if info.friendly_name.is_empty() {
            name.clone()
        } else {
            info.friendly_name.clone()
        };
        let pre = if info.flags & FUNC_PRE_OPERATOR != 0 {
            "_pre"
        } else {
            ""
        };
        let types: Vec<String> = param_decls.iter().map(|p| type_tag(vm, &p.kind)).collect();
        let key = format!("{owner_name}.{shown}{pre}({})", types.join(","));
        FunctionDecl::native(object_path, key)
    } else {
        FunctionDecl::script(object_path)
    };
    if let Some(c) = owner {
        decl = decl.with_owner(c);
    }
    decl = decl
        .with_static(info.flags & FUNC_STATIC != 0)
        .with_params(param_decls);
    if let Some(r) = result {
        decl = decl.with_result(locals[r].decl.clone());
    }
    let id = vm.declare_function(decl)?;
    vm.code.functions.insert((package, export), id);
    vm.code.code.insert(
        id,
        Rc::new(FuncCode {
            package,
            path,
            info,
            owner,
            locals,
            by_export,
            params,
            result,
            slot_count: slot,
            failure,
            script: Default::default(),
            state_code: false,
        }),
    );
    Ok(id)
}

/// W5-STATES' seam: declares the code of a State export as a function the interpreter runs as state code
/// (`crate::states`): `path` (not a package path, so no lookup finds it), `info` the script's place in `package`
/// (from `states::read_state_body`), no parameters or locals.
pub(crate) fn declare_state_code(
    vm: &mut Vm,
    package: PackageId,
    owner: ClassId,
    path: &str,
    info: FunctionInfo,
) -> Result<FuncId, VmError> {
    let object_path = ObjectPath::parse(path).map_err(|e| script_error(path, e.to_string()))?;
    let id = vm.declare_function(FunctionDecl::script(object_path).with_owner(owner))?;
    vm.code.code.insert(
        id,
        Rc::new(FuncCode {
            package,
            path: path.to_string(),
            info,
            owner: Some(owner),
            locals: Vec::new(),
            by_export: FixedHashMap::default(),
            params: Vec::new(),
            result: None,
            slot_count: 0,
            failure: None,
            script: Default::default(),
            state_code: true,
        }),
    );
    Ok(id)
}

/// Whether `f` is a state's code (W5-STATES' `declare_state_code`), not a function.
pub(crate) fn is_state_code(vm: &Vm, f: FuncId) -> bool {
    vm.code.code.get(&f).is_some_and(|c| c.state_code)
}

/// The loader's record of `f`, if a package export is behind it.
pub(crate) fn code_of(vm: &Vm, f: FuncId) -> Option<Rc<FuncCode>> {
    vm.code.code.get(&f).cloned()
}

/// The numbered native `index` (C++ `nativeByIndex`): the index is built on first use over the code packages the
/// store holds, in the C++ order; the first native with an index keeps it.
pub(crate) fn native_by_index(vm: &mut Vm, index: u32) -> Result<Option<FuncId>, VmError> {
    let table = match &vm.code.natives {
        Some(t) => t.clone(),
        None => {
            let built = Rc::new(build_native_index(vm)?);
            vm.code.natives = Some(built.clone());
            built
        }
    };
    match table.get(&index) {
        None => Ok(None),
        Some(Ok(f)) => Ok(Some(*f)),
        Some(Err(why)) => Err(script_error(&format!("native index {index}"), why.clone())),
    }
}

fn build_native_index(vm: &mut Vm) -> Result<FixedHashMap<u32, Result<FuncId, String>>, VmError> {
    let mut table = FixedHashMap::default();
    if no_code_layer(vm) {
        return Ok(table);
    }
    for name in CODE_PACKAGES {
        // A code package the store does not hold has no natives here (synthetic stores), as the C++ catches it.
        let package = {
            let (code, store) = vm.defs.code_mut();
            match code.package(store, name) {
                Ok(p) => p,
                Err(_) => continue,
            }
        };
        let pkg = vm.defs.packages().shared(package);
        for index in 1..=pkg.exports.len() as i32 {
            if !is_function_export(&pkg, index) {
                continue;
            }
            let Ok(info) = read_function(&pkg, index) else {
                continue;
            };
            let number = u32::from(info.native);
            if info.flags & FUNC_NATIVE == 0 || number == 0 || table.contains_key(&number) {
                continue;
            }
            let entry = match function_at(vm, package, index) {
                Ok(Some(f)) => Ok(f),
                Ok(None) => continue,
                Err(e) => Err(e.to_string()),
            };
            table.insert(number, entry);
        }
    }
    Ok(table)
}

/// The typed declaration of the property export `reference` of `package` names (C++ `decl(f, ref)`), cached by its
/// outer export.
pub(crate) fn decl_at(
    vm: &mut Vm,
    package: PackageId,
    reference: i32,
    function: &str,
) -> Result<PropertyDecl, VmError> {
    let unresolved = || {
        script_error(
            function,
            format!("unresolved property reference {reference} in {function}"),
        )
    };
    let (target, outer) = {
        let (code, store) = vm.defs.code_mut();
        let Some((p, i)) = code.resolve_ref(store, package, reference) else {
            return Err(unresolved());
        };
        ((p, i), store.get(p).exports[i as usize - 1].outer)
    };
    let key = (target.0, outer);
    if !vm.code.children.contains_key(&key) {
        let (code, store) = vm.defs.code_mut();
        let typed = code
            .child_declarations(
                store,
                &mut vm.space.names,
                &mut vm.space.classes,
                target.0,
                outer,
            )
            .map(Rc::new)
            .map_err(|e| chain_text(&e));
        vm.code.children.insert(key, typed);
    }
    match &vm.code.children[&key] {
        Ok(list) => list
            .iter()
            .find(|d| d.source == Some(target))
            .cloned()
            .ok_or_else(unresolved),
        Err(why) => Err(script_error(function, why.clone())),
    }
}

/// The class default object of `class` (C++ `defaultsOf`), through the definition store; its decode log joins the
/// VM's log in order.
pub(crate) fn class_default(vm: &mut Vm, class: ClassId) -> Result<ObjRef, VmError> {
    let path = vm
        .space
        .classes
        .get_layout(class)
        .ok_or(HeapError::Layout(
            ow_object::layout::LayoutError::UnknownClass(class),
        ))?
        .path()
        .as_str()
        .to_string();
    let made = vm.defs.class_default(&mut vm.space, &path);
    vm.log.extend(vm.defs.take_log());
    made.map_err(|e| def_error(&path, e))
}

/// A new object of `class` that starts from the class default object's values, as the C++ `Runtime::instantiate`
/// copies `defaults->props` (for `New`, `--self` and `--run-batch`). Its name is [`Vm::instantiate`]'s.
pub fn instantiate_from_defaults(vm: &mut Vm, class: ClassId) -> Result<ObjRef, VmError> {
    let defaults = class_default(vm, class)?;
    let object = vm.instantiate(class, "")?;
    let slots = vm.space.heap.get(defaults)?.slots.clone();
    vm.space.heap.get_mut(object)?.slots = slots;
    Ok(object)
}

/// Whether `object` can be read (C++ `property()` finds something): one of the definition store's unloaded handles
/// is loaded first (`DefStore::ensure_loaded`); `false` for a stand-in that stays one (an object with no slots of a
/// class that has some, as the C++ resource stand-ins hold no properties).
pub(crate) fn loaded_for_read(vm: &mut Vm, object: ObjRef, context: &str) -> Result<bool, VmError> {
    if vm.defs.is_unloaded(&vm.space, object) {
        let loaded = vm.defs.ensure_loaded(&mut vm.space, object);
        vm.log.extend(vm.defs.take_log());
        loaded.map_err(|e| def_error(context, e))?;
    }
    let o = vm.space.heap.get(object)?;
    let stand_in = o.slots.is_empty()
        && vm
            .space
            .classes
            .get_layout(o.class)
            .is_some_and(|l| l.slot_count() > 0);
    Ok(!stand_in)
}

impl Vm {
    /// The function `name` (any case) as an object of `class` in `state` sees it (C++ `Runtime::findMethod`): from
    /// `class` up, the state's own function first, then the class's. `state` `None` is the default state. The code
    /// of the class and its super classes is loaded first if it is not (a class defined by hand, with no package
    /// behind it, declares nothing).
    pub fn find_method(
        &mut self,
        class: ClassId,
        name: &str,
        state: Option<NameId>,
    ) -> Result<Option<FuncId>, VmError> {
        ensure_class_code(self, class)?;
        let key = name.to_ascii_lowercase();
        let state_key = state.map(|s| self.space.names.text(s).to_ascii_lowercase());
        for c in self.class_chain(class) {
            let Some(code) = self.code.classes.get(&c) else {
                continue;
            };
            if let Some(sk) = &state_key
                && let Some(f) = code.states.get(sk).and_then(|s| s.get(&key))
            {
                return Ok(Some(*f));
            }
            if let Some(f) = code.functions.get(&key) {
                return Ok(Some(*f));
            }
        }
        Ok(None)
    }

    /// Whether `class` or a super class declares the state `state` (any case; C++ `Class::states` along the chain).
    /// Loads the class's code first, as [`Vm::find_method`] does.
    pub fn class_declares_state(&mut self, class: ClassId, state: NameId) -> Result<bool, VmError> {
        ensure_class_code(self, class)?;
        let key = self.space.names.text(state).to_ascii_lowercase();
        Ok(self.class_chain(class).iter().any(|c| {
            self.code
                .classes
                .get(c)
                .is_some_and(|code| code.states.contains_key(&key))
        }))
    }

    fn class_chain(&self, class: ClassId) -> Vec<ClassId> {
        let mut chain = Vec::new();
        let mut cursor = Some(class);
        while let Some(c) = cursor {
            if chain.contains(&c) {
                break;
            }
            chain.push(c);
            cursor = self
                .space
                .classes
                .get_layout(c)
                .and_then(|l| l.super_class());
        }
        chain
    }
}
