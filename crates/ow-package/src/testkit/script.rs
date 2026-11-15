//! Owner: lane W2-SCRIPT. Function exports and hand-assembled UnrealScript bodies for synthetic packages, built on
//! [`super::PackageBuilder`] (invented values only, AGENTS.md rule 9). They re-express these Python helpers byte for
//! byte (the tests at the end pin the bytes against them):
//!
//! | Python helper | Here |
//! |---|---|
//! | `Asm` (`tests/combat_test.py:138`, `tests/mission_script_test.py:161`, `tests/vm_test.py:63`) | [`Asm`] |
//! | the function payload of `func()` / `make_function()` (`tests/vm_test.py`) and `Toy.function()` | [`function_payload`] with [`FunctionSpec`] |
//! | `Toy` of `tests/mission_script_test.py` (and of `tests/combat_test.py`): `cls`, `prop`, `function` | [`ScriptToy`] |
//!
//! The function layout is the one `src/script.cpp` (`readFunction`) reads: `[count u16][count u16 values]`, ten
//! `i32`, the in-memory size, the stored size, the script, then the tail `[iNative u16][OperPrecedence u8]
//! [FunctionFlags u32][RepOffset u16 if FUNC_Net][FriendlyName FName]`. The Python helpers never write a RepOffset;
//! [`FunctionSpec::rep_offset`] adds one for the `FUNC_Net` tail.
//!
//! `vm_test.py`'s `make_function` differs from `Toy.function` in two ways this file does not mirror: its properties
//! omit the type word when the type reference is 0, and locals are exported among the parameters in reverse. A
//! lane that needs those packages adds that form here.

use super::{Export, PackageBuilder, ints};

/// Object references are 4 bytes in the file and 8 in the in-memory script that jump targets count in.
pub const REFERENCE_MEMORY_EXTRA: u32 = 4;

/// Assembles one function body (the Python `Asm`). Object references are 4 bytes in the file and 8 in memory;
/// [`Asm::here`] is the in-memory offset that jump targets use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Asm {
    /// The stored script bytes.
    pub bytes: Vec<u8>,
    /// Object references written so far.
    pub refs: u32,
    /// In-memory offsets recorded by [`Asm::stmt`] (and by [`Asm::end`]).
    pub statements: Vec<u32>,
}

impl Asm {
    /// An empty body.
    pub fn new() -> Self {
        Asm::default()
    }

    /// The in-memory offset of the next byte: stored length plus 4 per reference.
    pub fn here(&self) -> u32 {
        u32::try_from(self.bytes.len()).expect("script fits u32") + REFERENCE_MEMORY_EXTRA * self.refs
    }

    /// The in-memory size of the body so far (what the function header's `ScriptBytecodeSize` holds).
    pub fn memory_size(&self) -> u32 {
        self.here()
    }

    /// Records the start of a statement.
    pub fn stmt(&mut self) -> &mut Self {
        let here = self.here();
        self.statements.push(here);
        self
    }

    /// Raw bytes (opcodes, byte operands).
    pub fn raw(&mut self, bytes: &[u8]) -> &mut Self {
        self.bytes.extend_from_slice(bytes);
        self
    }

    /// An object reference: an `i32`, counted as 8 bytes in memory.
    pub fn reference(&mut self, reference: i32) -> &mut Self {
        self.bytes.extend_from_slice(&reference.to_le_bytes());
        self.refs += 1;
        self
    }

    /// An `i32` operand.
    pub fn i32(&mut self, value: i32) -> &mut Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// A `u16` operand (`w()` in Python).
    pub fn w(&mut self, value: u16) -> &mut Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// An `f32` operand.
    pub fn f32(&mut self, value: f32) -> &mut Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// An FName operand (two `i32`), the name added to `p` if absent.
    pub fn fname(&mut self, p: &mut PackageBuilder, text: &str, number: i32) -> &mut Self {
        let bytes = p.fname(text, number);
        self.raw(&bytes)
    }

    /// `EndOfScript` (0x53), recorded as a statement (`vm_test.py`'s `end()`; the other helpers do not record it,
    /// which changes nothing in the bytes).
    pub fn end(&mut self) -> &mut Self {
        self.stmt().raw(&[0x53])
    }

    /// `LocalVariable(reference)`.
    pub fn local(&mut self, reference: i32) -> &mut Self {
        self.raw(&[0x00]).reference(reference)
    }

    /// `InstanceVariable(reference)`.
    pub fn instance(&mut self, reference: i32) -> &mut Self {
        self.raw(&[0x01]).reference(reference)
    }

    /// `Context(object, skip 0, return property 0, flags 0, body)`.
    pub fn context(&mut self, object: impl FnOnce(&mut Asm), body: impl FnOnce(&mut Asm)) -> &mut Self {
        self.raw(&[0x19]);
        object(self);
        self.w(0).reference(0).raw(&[0]);
        body(self);
        self
    }

    /// `ClassContext(ObjectConst(class), skip 0, return property 0, flags 0, body)`: a static call.
    pub fn class_context(&mut self, class: i32, body: impl FnOnce(&mut Asm)) -> &mut Self {
        self.raw(&[0x12, 0x20]).reference(class).w(0).reference(0).raw(&[0]);
        body(self);
        self
    }

    /// `FinalFunction(function, args..., EndFunctionParms)` (`final()` / `call()` of the Python helpers).
    pub fn final_function(&mut self, function: i32, args: impl FnOnce(&mut Asm)) -> &mut Self {
        self.raw(&[0x1C]).reference(function);
        args(self);
        self.raw(&[0x16])
    }

    /// A numbered native call: the token is the native index (0x70 and above), then the arguments and
    /// `EndFunctionParms` (`call()` of `vm_test.py`).
    pub fn native(&mut self, index: u8, args: impl FnOnce(&mut Asm)) -> &mut Self {
        self.raw(&[index]);
        args(self);
        self.raw(&[0x16])
    }

    /// `IntConst(value)`.
    pub fn int_const(&mut self, value: i32) -> &mut Self {
        self.raw(&[0x1D]).i32(value)
    }

    /// `FloatConst(value)`.
    pub fn float_const(&mut self, value: f32) -> &mut Self {
        self.raw(&[0x1E]).f32(value)
    }

    /// `ByteConst(value)`.
    pub fn byte_const(&mut self, value: u8) -> &mut Self {
        self.raw(&[0x24, value])
    }

    /// `Self`.
    pub fn self_(&mut self) -> &mut Self {
        self.raw(&[0x17])
    }

    /// `False`.
    pub fn false_(&mut self) -> &mut Self {
        self.raw(&[0x28])
    }

    /// `Return(Nothing)`.
    pub fn return_nothing(&mut self) -> &mut Self {
        self.raw(&[0x04, 0x0B])
    }

    /// `StructMember(property, struct, 0, 0, expr)`.
    pub fn struct_member(&mut self, property: i32, structure: i32, expr: impl FnOnce(&mut Asm)) -> &mut Self {
        self.raw(&[0x35]).reference(property).reference(structure).raw(&[0, 0]);
        expr(self);
        self
    }

    /// `Jump(<target>)`; returns the position of the target word for [`Asm::patch`].
    pub fn jump(&mut self) -> usize {
        self.raw(&[0x06]);
        let at = self.bytes.len();
        self.w(0);
        at
    }

    /// `JumpIfNot(<target>, cond)`; returns the position of the target word for [`Asm::patch`].
    pub fn jump_if_not(&mut self, cond: impl FnOnce(&mut Asm)) -> usize {
        self.raw(&[0x07]);
        let at = self.bytes.len();
        self.w(0);
        cond(self);
        at
    }

    /// Writes the current in-memory offset (the next statement) into the target word at `at`.
    pub fn patch(&mut self, at: usize) {
        let here = u16::try_from(self.here()).expect("jump target fits u16");
        self.bytes[at..at + 2].copy_from_slice(&here.to_le_bytes());
    }
}

/// The header and tail fields of a function export other than its script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSpec {
    /// `FunctionFlags`.
    pub flags: u32,
    /// `iNative`.
    pub native: u16,
    /// `OperPrecedence`.
    pub precedence: u8,
    /// The FriendlyName (number 0).
    pub friendly: String,
    /// The Gearbox local-variable array (`hidden` of `make_function`).
    pub hidden_locals: Vec<u16>,
    /// The ten `i32` between the array and the sizes (all 0 in the Python helpers).
    pub header_words: [i32; 10],
    /// The in-memory size to write instead of the body's own (for size-mismatch tests).
    pub memory_size: Option<i32>,
    /// A RepOffset to write between the flags and the FriendlyName (the `FUNC_Net` tail).
    pub rep_offset: Option<u16>,
}

/// `FUNC_Final | FUNC_Defined | FUNC_Public`, the Python helpers' default flags.
pub const DEFAULT_FLAGS: u32 = 0x1 | 0x2 | 0x20000;

impl FunctionSpec {
    /// The Python helpers' defaults: `flags`, no native index, precedence 0, no local array, no RepOffset.
    pub fn new(flags: u32, friendly: &str) -> Self {
        FunctionSpec {
            flags,
            native: 0,
            precedence: 0,
            friendly: friendly.to_owned(),
            hidden_locals: Vec::new(),
            header_words: [0; 10],
            memory_size: None,
            rep_offset: None,
        }
    }
}

/// A Function export's payload: the header, `body`'s script (none for a native) and the tail. The FriendlyName is
/// added to `p`'s names if absent.
pub fn function_payload(p: &mut PackageBuilder, body: Option<&Asm>, spec: &FunctionSpec) -> Vec<u8> {
    let script: &[u8] = body.map_or(&[], |a| &a.bytes);
    let memory = spec
        .memory_size
        .unwrap_or_else(|| body.map_or(0, |a| i32::try_from(a.memory_size()).expect("size fits i32")));
    let count = u16::try_from(spec.hidden_locals.len()).expect("local array fits u16");
    let mut out = count.to_le_bytes().to_vec();
    for v in &spec.hidden_locals {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&ints(&spec.header_words));
    out.extend_from_slice(&ints(&[memory, i32::try_from(script.len()).expect("script fits i32")]));
    out.extend_from_slice(script);
    out.extend_from_slice(&spec.native.to_le_bytes());
    out.push(spec.precedence);
    out.extend_from_slice(&spec.flags.to_le_bytes());
    if let Some(rep) = spec.rep_offset {
        out.extend_from_slice(&rep.to_le_bytes());
    }
    out.extend_from_slice(&p.fname(&spec.friendly, 0));
    out
}

/// One declared parameter of [`ScriptToy::function`]: its property kind (without `Property`), name, extra property
/// flags (`CPF_Parm` is added) and type reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub kind: String,
    pub name: String,
    pub flags: u64,
    pub type_ref: i32,
}

impl Param {
    /// A parameter with no extra flags and no type reference.
    pub fn new(kind: &str, name: &str) -> Self {
        Param {
            kind: kind.to_owned(),
            name: name.to_owned(),
            flags: 0,
            type_ref: 0,
        }
    }
}

/// `CPF_Parm`, `CPF_OutParm`, `CPF_OptionalParm`, `CPF_ReturnParm` as the Python helpers write them.
pub const CPF_PARM: u64 = 0x80;
pub const CPF_OUT: u64 = 0x100;
pub const CPF_OPT: u64 = 0x10;
pub const CPF_RET: u64 = 0x400;

/// The exports [`ScriptToy::function`] added: the function and its children by name (`ids` of the Python
/// helpers; `__self__` is [`FunctionIds::function`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionIds {
    pub function: i32,
    pub children: Vec<(String, i32)>,
}

impl FunctionIds {
    /// The export of child `name`; panics (a test bug) when there is none.
    pub fn get(&self, name: &str) -> i32 {
        self.children
            .iter()
            .find(|(n, _)| n == name)
            .map(|&(_, r)| r)
            .unwrap_or_else(|| panic!("no child {name}"))
    }
}

/// Reflection helpers over one package (the Python `Toy` of `mission_script_test.py`): Core class imports,
/// classes, properties, functions with their parameter exports.
#[derive(Debug, Clone)]
pub struct ScriptToy {
    pub p: PackageBuilder,
    /// The `None` FName (added first, as `Toy.__init__` does).
    pub none: [u8; 8],
    imports: Vec<(String, i32)>,
}

impl ScriptToy {
    /// `Toy(kinds)`: imports `Class`, `Function`, `ScriptStruct`, `Enum`, then `<kind>Property` for each kind, all
    /// of class `Core.Class`.
    pub fn new(kinds: &[&str]) -> Self {
        let mut names: Vec<String> = ["Class", "Function", "ScriptStruct", "Enum"].map(String::from).to_vec();
        names.extend(kinds.iter().map(|k| format!("{k}Property")));
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        ScriptToy::with_imports(&names)
    }

    /// A toy whose Core class imports are exactly `names`, in order (`build_package()` of `vm_test.py` uses its own
    /// list).
    pub fn with_imports(names: &[&str]) -> Self {
        let mut p = PackageBuilder::new_empty();
        let none = p.fname("None", 0);
        let imports = names
            .iter()
            .map(|&n| (n.to_owned(), p.add_import("Core", "Class", 0, n)))
            .collect();
        ScriptToy { p, none, imports }
    }

    /// The import reference of class `name`; panics (a test bug) when it was not imported.
    pub fn import(&self, name: &str) -> i32 {
        self.imports
            .iter()
            .find(|(n, _)| n == name)
            .map(|&(_, r)| r)
            .unwrap_or_else(|| panic!("no import {name}"))
    }

    /// `cls(name, super_ref)`: a Class export with a 16-byte zero body.
    pub fn class(&mut self, name: &str, super_ref: i32) -> i32 {
        let class = self.import("Class");
        let mut export = Export::new(class, name, vec![0u8; 16]);
        export.super_ = super_ref;
        self.p.add_export_with(export)
    }

    /// `prop(kind, owner, name, flags, type_ref)`: a `<kind>Property` export in `owner` whose payload ends in
    /// `type_ref` (`tail` of the Python helper replaces that word when given).
    pub fn prop(&mut self, kind: &str, owner: i32, name: &str, flags: u64, type_ref: i32) -> i32 {
        self.prop_tail(kind, owner, name, flags, &ints(&[type_ref]))
    }

    /// [`ScriptToy::prop`] with `tail` in place of the type word.
    pub fn prop_tail(&mut self, kind: &str, owner: i32, name: &str, flags: u64, tail: &[u8]) -> i32 {
        let mut payload = ints(&[0]);
        payload.extend_from_slice(&self.none);
        payload.extend_from_slice(&ints(&[0, 1]));
        payload.extend_from_slice(&flags.to_le_bytes());
        payload.extend_from_slice(&self.none);
        payload.extend_from_slice(&ints(&[0]));
        payload.extend_from_slice(tail);
        let class = self.import(&format!("{kind}Property"));
        let mut export = Export::new(class, name, payload);
        export.outer = owner;
        self.p.add_export_with(export)
    }

    /// `function(owner, name, declared, body, flags, result, native, friendly, locals_)`: the return value (kind,
    /// type reference) first, then the declared parameters in reverse declaration order, then `locals`, each a
    /// property export in the function; then the Function export, whose index is reserved before its children are
    /// added. `body` gets the ids and returns the script (`None` for a native). The FriendlyName is `spec.friendly`.
    pub fn function(
        &mut self,
        owner: i32,
        name: &str,
        declared: &[Param],
        result: Option<(&str, i32)>,
        locals: &[(&str, &str)],
        spec: &FunctionSpec,
        body: impl FnOnce(&FunctionIds) -> Option<Asm>,
    ) -> FunctionIds {
        let mut ordered: Vec<(String, String, u64, i32)> = Vec::new();
        if let Some((kind, type_ref)) = result {
            ordered.push((kind.to_owned(), "ReturnValue".to_owned(), CPF_PARM | CPF_OUT | CPF_RET, type_ref));
        }
        for d in declared.iter().rev() {
            ordered.push((d.kind.clone(), d.name.clone(), CPF_PARM | d.flags, d.type_ref));
        }
        for &(kind, local) in locals {
            ordered.push((kind.to_owned(), local.to_owned(), 0, 0));
        }
        let function = self.p.next_export() + i32::try_from(ordered.len()).expect("child count fits i32");
        let mut ids = FunctionIds {
            function,
            children: Vec::new(),
        };
        for (kind, child, flags, type_ref) in &ordered {
            let reference = self.prop(kind, function, child, *flags, *type_ref);
            ids.children.push((child.clone(), reference));
        }
        let asm = body(&ids);
        let payload = function_payload(&mut self.p, asm.as_ref(), spec);
        let mut export = Export::new(self.import("Function"), name, payload);
        export.outer = owner;
        let added = self.p.add_export_with(export);
        assert_eq!(added, function, "the function's reserved index");
        ids
    }

    /// The package bytes.
    pub fn build(&mut self) -> Vec<u8> {
        self.p.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FNV-1a 64 of the bytes, for pins too long to write out.
    fn fnv1a64(bytes: &[u8]) -> u64 {
        bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
    }

    #[test]
    fn asm_counts_references_as_eight_bytes_in_memory() {
        let mut a = Asm::new();
        a.stmt().raw(&[0x0F]).local(1).local(2);
        assert_eq!((a.bytes.len(), a.here()), (11, 19));
        let at = a.jump();
        a.patch(at);
        assert_eq!(&a.bytes[at..at + 2], &22u16.to_le_bytes());
        a.end();
        assert_eq!(a.statements, vec![0, 22]);
        assert_eq!(a.memory_size(), 23);
    }

    /// The same calls on the Python `Toy` and `Asm` of `mission_script_test.py` (its lines 1-251 run on their own,
    /// local run 2026-11-15) give the bytes pinned in [`PYTHON_TOY`]: `Toy(('Int', 'Float'))`, `cls('Thing')`,
    /// `prop('Int', c, 'Count')`, `function(c, 'Bump', [('Int', 'A', 0), ('Float', 'B', 0x10)], body, result='Int',
    /// locals_=[('Int', 'Tmp')])` with the body below written call for call, then the native `Add_IntInt`
    /// (`native=146, friendly='+'`). The ids printed there were ReturnValue 3, B 4, A 5, Tmp 6, the function 7, and
    /// 11 for the native.
    #[test]
    fn toy_function_matches_the_python_toy() {
        let mut t = ScriptToy::new(&["Int", "Float"]);
        let c = t.class("Thing", 0);
        let count = t.prop("Int", c, "Count", 0, 0);
        let ids = t.function(
            c,
            "Bump",
            &[Param::new("Int", "A"), Param { flags: CPF_OPT, ..Param::new("Float", "B") }],
            Some(("Int", 0)),
            &[("Int", "Tmp")],
            &FunctionSpec::new(DEFAULT_FLAGS, "Bump"),
            |ids| {
                let mut a = Asm::new();
                a.raw(&[0x0F]).local(ids.get("Tmp")).local(ids.get("A"));
                let at = a.jump_if_not(|a| {
                    a.false_();
                });
                a.raw(&[0x0F]).instance(count).int_const(7);
                a.patch(at);
                a.raw(&[0x04]).final_function(ids.function, |a| {
                    a.local(ids.get("A"));
                    a.float_const(1.5);
                });
                a.return_nothing().end();
                Some(a)
            },
        );
        assert_eq!((ids.function, ids.get("ReturnValue"), ids.get("B"), ids.get("A"), ids.get("Tmp")), (7, 3, 4, 5, 6));
        let native = t.function(
            c,
            "Add_IntInt",
            &[Param::new("Int", "P0"), Param::new("Int", "P1")],
            Some(("Int", 0)),
            &[],
            &FunctionSpec { native: 146, ..FunctionSpec::new(0x400 | 0x1000 | 0x23000, "+") },
            |_| None,
        );
        assert_eq!(native.function, 11);
        let bytes = t.build();
        assert_eq!((bytes.len(), fnv1a64(&bytes)), PYTHON_TOY);
    }

    /// Length and FNV-1a 64 of the Python bytes (see the test above).
    const PYTHON_TOY: (usize, u64) = (1861, 0x35e3_3a70_9ee2_93c7);
}
