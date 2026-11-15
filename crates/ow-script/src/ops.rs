//! Owner: lane W2-SCRIPT. The opcode set, the function flags and the operand layout table: the port of the
//! `FunctionFlag` and `Op` enums of `src/script.hpp` and of `layoutFor` in `src/script.cpp`.
//!
//! The token values are the public UE3 set. The operand layouts were fitted to this build by the C++ port and the
//! Python prototype (`research/script_disasm.py`) and checked structurally (`docs/verification/SCRIPT_BYTECODE_DISASM.md`):
//! every function of the code packages decodes, sizes meet the header, jumps land on statements. They are copied
//! from `src/script.cpp` as they are, nothing here is re-derived (AGENTS.md rule 4). The layouts that only fit, and
//! could not be told apart from a neighbouring layout by the structural checks, are marked UNVERIFIED as in the C++.

/// `FunctionFlags` bits as read from the packages (UE3 values; `0x22003` is Final | Defined | Public).
pub mod flags {
    pub const FUNC_FINAL: u32 = 0x1;
    pub const FUNC_DEFINED: u32 = 0x2;
    pub const FUNC_ITERATOR: u32 = 0x4;
    pub const FUNC_LATENT: u32 = 0x8;
    pub const FUNC_PRE_OPERATOR: u32 = 0x10;
    pub const FUNC_SINGULAR: u32 = 0x20;
    pub const FUNC_NET: u32 = 0x40;
    pub const FUNC_NET_RELIABLE: u32 = 0x80;
    pub const FUNC_SIMULATED: u32 = 0x100;
    pub const FUNC_EXEC: u32 = 0x200;
    pub const FUNC_NATIVE: u32 = 0x400;
    pub const FUNC_EVENT: u32 = 0x800;
    pub const FUNC_OPERATOR: u32 = 0x1000;
    pub const FUNC_STATIC: u32 = 0x2000;
    pub const FUNC_CONST: u32 = 0x8000;
    pub const FUNC_PUBLIC: u32 = 0x20000;
    pub const FUNC_PRIVATE: u32 = 0x40000;
    pub const FUNC_PROTECTED: u32 = 0x80000;
    pub const FUNC_DELEGATE: u32 = 0x100000;
    pub const FUNC_HAS_OUT_PARMS: u32 = 0x400000;
}

/// Expression tokens (C++ `script::Op`). 0x2B, 0x5B-0x5D and 0x5E/0x5F aside, the names are the public UE3 ones.
pub mod op {
    pub const LOCAL_VARIABLE: u8 = 0x00;
    pub const INSTANCE_VARIABLE: u8 = 0x01;
    pub const DEFAULT_VARIABLE: u8 = 0x02;
    pub const STATE_VARIABLE: u8 = 0x03;
    pub const RETURN: u8 = 0x04;
    pub const SWITCH: u8 = 0x05;
    pub const JUMP: u8 = 0x06;
    pub const JUMP_IF_NOT: u8 = 0x07;
    pub const STOP: u8 = 0x08;
    pub const ASSERT: u8 = 0x09;
    pub const CASE: u8 = 0x0A;
    pub const NOTHING: u8 = 0x0B;
    pub const LABEL_TABLE: u8 = 0x0C;
    pub const GOTO_LABEL: u8 = 0x0D;
    pub const EAT_RETURN_VALUE: u8 = 0x0E;
    pub const LET: u8 = 0x0F;
    pub const DYN_ARRAY_ELEMENT: u8 = 0x10;
    pub const NEW: u8 = 0x11;
    pub const CLASS_CONTEXT: u8 = 0x12;
    pub const META_CAST: u8 = 0x13;
    pub const LET_BOOL: u8 = 0x14;
    pub const END_PARM_VALUE: u8 = 0x15;
    pub const END_FUNCTION_PARMS: u8 = 0x16;
    pub const SELF: u8 = 0x17;
    pub const SKIP: u8 = 0x18;
    pub const CONTEXT: u8 = 0x19;
    pub const ARRAY_ELEMENT: u8 = 0x1A;
    pub const VIRTUAL_FUNCTION: u8 = 0x1B;
    pub const FINAL_FUNCTION: u8 = 0x1C;
    pub const INT_CONST: u8 = 0x1D;
    pub const FLOAT_CONST: u8 = 0x1E;
    pub const STRING_CONST: u8 = 0x1F;
    pub const OBJECT_CONST: u8 = 0x20;
    pub const NAME_CONST: u8 = 0x21;
    pub const ROTATION_CONST: u8 = 0x22;
    pub const VECTOR_CONST: u8 = 0x23;
    pub const BYTE_CONST: u8 = 0x24;
    pub const INT_ZERO: u8 = 0x25;
    pub const INT_ONE: u8 = 0x26;
    pub const TRUE: u8 = 0x27;
    pub const FALSE: u8 = 0x28;
    pub const NATIVE_PARM: u8 = 0x29;
    pub const NO_OBJECT: u8 = 0x2A;
    pub const INT_CONST_BYTE: u8 = 0x2C;
    pub const BOOL_VARIABLE: u8 = 0x2D;
    pub const DYNAMIC_CAST: u8 = 0x2E;
    pub const ITERATOR: u8 = 0x2F;
    pub const ITERATOR_POP: u8 = 0x30;
    pub const ITERATOR_NEXT: u8 = 0x31;
    pub const STRUCT_CMP_EQ: u8 = 0x32;
    pub const STRUCT_CMP_NE: u8 = 0x33;
    pub const UNICODE_STRING_CONST: u8 = 0x34;
    pub const STRUCT_MEMBER: u8 = 0x35;
    pub const DYN_ARRAY_LENGTH: u8 = 0x36;
    pub const GLOBAL_FUNCTION: u8 = 0x37;
    pub const PRIMITIVE_CAST: u8 = 0x38;
    pub const DYN_ARRAY_INSERT: u8 = 0x39;
    pub const RETURN_NOTHING: u8 = 0x3A;
    pub const EQUAL_EQUAL_DEL_DEL: u8 = 0x3B;
    pub const NOT_EQUAL_DEL_DEL: u8 = 0x3C;
    pub const EQUAL_EQUAL_DEL_FUNC: u8 = 0x3D;
    pub const NOT_EQUAL_DEL_FUNC: u8 = 0x3E;
    pub const EMPTY_DELEGATE: u8 = 0x3F;
    pub const DYN_ARRAY_REMOVE: u8 = 0x40;
    pub const DEBUG_INFO: u8 = 0x41;
    pub const DELEGATE_FUNCTION: u8 = 0x42;
    pub const DELEGATE_PROPERTY: u8 = 0x43;
    pub const LET_DELEGATE: u8 = 0x44;
    pub const CONDITIONAL: u8 = 0x45;
    pub const DYN_ARRAY_FIND: u8 = 0x46;
    pub const DYN_ARRAY_FIND_STRUCT: u8 = 0x47;
    pub const LOCAL_OUT_VARIABLE: u8 = 0x48;
    pub const DEFAULT_PARM_VALUE: u8 = 0x49;
    pub const EMPTY_PARM_VALUE: u8 = 0x4A;
    pub const INSTANCE_DELEGATE: u8 = 0x4B;
    /// 0x4C-0x50: hidden typed temporaries (int, float, byte, bool, object). The i32 operand indexes the function's
    /// local-variable array ([`crate::function::FunctionInfo::hidden_locals`]). Meaning from
    /// `NATIVE_BYTECODE_OPCODES.md`, UNVERIFIED in the running game.
    pub const OP_4C: u8 = 0x4C;
    pub const OP_4D: u8 = 0x4D;
    pub const OP_4E: u8 = 0x4E;
    pub const OP_4F: u8 = 0x4F;
    pub const OP_50: u8 = 0x50;
    pub const INTERFACE_CONTEXT: u8 = 0x51;
    pub const INTERFACE_CAST: u8 = 0x52;
    pub const END_OF_SCRIPT: u8 = 0x53;
    pub const DYN_ARRAY_ADD: u8 = 0x54;
    pub const DYN_ARRAY_ADD_ITEM: u8 = 0x55;
    pub const DYN_ARRAY_REMOVE_ITEM: u8 = 0x56;
    pub const DYN_ARRAY_INSERT_ITEM: u8 = 0x57;
    pub const DYN_ARRAY_ITERATOR: u8 = 0x58;
    pub const DYN_ARRAY_SORT: u8 = 0x59;
    pub const FILTER_EDITOR_ONLY: u8 = 0x5A;
    /// An attribute value property reference: the same handler as [`INSTANCE_VARIABLE`] (UNVERIFIED in game).
    pub const OP_5E: u8 = 0x5E;
    /// "Let attribute": the left side is a 5E (or a Context around one), the right side goes into the base
    /// companion (UNVERIFIED in game).
    pub const OP_5F: u8 = 0x5F;
    /// 0x60-0x6F: extended native call (two bytes, index `(op - 0x60) * 256 + next byte`).
    pub const EXTENDED_NATIVE: u8 = 0x60;
    /// 0x70 and above: a native call whose index is the token itself.
    pub const FIRST_NATIVE: u8 = 0x70;
}

/// The operand letters of one token, in order:
///
/// | Letter | Operand |
/// |---|---|
/// | `r` | object reference (i32; 4 bytes in the file, 8 in the in-memory script) |
/// | `n` | FName (two i32) |
/// | `i` | i32 |
/// | `f` | f32 |
/// | `b` | u8 |
/// | `w` | u16 (jump and skip offsets are in-memory offsets) |
/// | `E` | one sub-expression |
/// | `P` | call parameters: expressions up to `EndFunctionParms` |
///
/// Strings, the label table, `Case`, `Conditional` and native calls need logic and have no entry; the decoder
/// handles them itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub name: &'static str,
    pub operands: &'static str,
}

const fn l(name: &'static str, operands: &'static str) -> Option<Layout> {
    Some(Layout { name, operands })
}

/// The table of `layoutFor` in `src/script.cpp`, entry for entry.
static LAYOUTS: [Option<Layout>; 256] = {
    let mut t: [Option<Layout>; 256] = [None; 256];
    t[0x00] = l("LocalVariable", "r");
    t[0x01] = l("InstanceVariable", "r");
    t[0x02] = l("DefaultVariable", "r");
    t[0x03] = l("StateVariable", "r");
    t[0x48] = l("LocalOutVariable", "r");
    t[0x04] = l("Return", "E");
    t[0x05] = l("Switch", "rbE");
    t[0x06] = l("Jump", "w");
    t[0x07] = l("JumpIfNot", "wE");
    t[0x08] = l("Stop", "");
    t[0x09] = l("Assert", "wbE");
    t[0x0B] = l("Nothing", "");
    t[0x0D] = l("GotoLabel", "E");
    t[0x0E] = l("EatReturnValue", "r");
    t[0x0F] = l("Let", "EE");
    t[0x10] = l("DynArrayElement", "EE");
    t[0x11] = l("New", "EEEE");
    t[0x13] = l("MetaCast", "rE");
    t[0x14] = l("LetBool", "EE");
    t[0x15] = l("EndParmValue", "");
    t[0x16] = l("EndFunctionParms", "");
    t[0x17] = l("Self", "");
    t[0x18] = l("Skip", "wE");
    t[0x1A] = l("ArrayElement", "EE");
    t[0x1D] = l("IntConst", "i");
    t[0x1E] = l("FloatConst", "f");
    t[0x20] = l("ObjectConst", "r");
    t[0x21] = l("NameConst", "n");
    t[0x22] = l("RotationConst", "iii");
    t[0x23] = l("VectorConst", "fff");
    t[0x24] = l("ByteConst", "b");
    t[0x25] = l("IntZero", "");
    t[0x26] = l("IntOne", "");
    t[0x27] = l("True", "");
    t[0x28] = l("False", "");
    t[0x29] = l("NativeParm", "r");
    t[0x2A] = l("NoObject", "");
    t[0x2C] = l("IntConstByte", "b");
    t[0x2D] = l("BoolVariable", "E");
    t[0x2E] = l("DynamicCast", "rE");
    t[0x2F] = l("Iterator", "Ew");
    t[0x30] = l("IteratorPop", "");
    t[0x31] = l("IteratorNext", "");
    t[0x32] = l("StructCmpEq", "rEE");
    t[0x33] = l("StructCmpNe", "rEE");
    t[0x35] = l("StructMember", "rrbbE");
    t[0x36] = l("DynArrayLength", "E");
    t[0x38] = l("PrimitiveCast", "bE");
    t[0x39] = l("DynArrayInsert", "EP");
    t[0x3A] = l("ReturnNothing", "r");
    t[0x3B] = l("EqualEqual_DelDel", "EP");
    t[0x3C] = l("NotEqual_DelDel", "EP");
    t[0x3D] = l("EqualEqual_DelFunc", "EP");
    t[0x3E] = l("NotEqual_DelFunc", "EP");
    t[0x3F] = l("EmptyDelegate", "");
    t[0x40] = l("DynArrayRemove", "EP");
    t[0x41] = l("DebugInfo", "iiib");
    t[0x43] = l("DelegateProperty", "nr");
    t[0x44] = l("LetDelegate", "EE");
    // Array, u16, item, EndFunctionParms.
    t[0x46] = l("DynArrayFind", "EwP");
    // UNVERIFIED (array, u16, parameters up to 0x16).
    t[0x47] = l("DynArrayFindStruct", "EwP");
    t[0x49] = l("DefaultParmValue", "wE");
    t[0x4A] = l("EmptyParmValue", "");
    t[0x4B] = l("InstanceDelegate", "n");
    // Typed temporaries 4C-50: a plain i32 index (not a reference).
    t[0x4C] = l("Op4C", "i");
    t[0x4D] = l("Op4D", "i");
    t[0x4E] = l("Op4E", "i");
    t[0x4F] = l("Op4F", "i");
    t[0x50] = l("Op50", "i");
    t[0x51] = l("InterfaceContext", "E");
    t[0x52] = l("InterfaceCast", "rE");
    t[0x53] = l("EndOfScript", "");
    t[0x54] = l("DynArrayAdd", "EP");
    // UNVERIFIED: the parameters end at 0x16.
    t[0x55] = l("DynArrayAddItem", "EwP");
    t[0x56] = l("DynArrayRemoveItem", "EwP");
    t[0x57] = l("DynArrayInsertItem", "EP");
    t[0x58] = l("DynArrayIterator", "EEbEw");
    t[0x59] = l("DynArraySort", "EP");
    t[0x5A] = l("FilterEditorOnly", "w");
    // Attribute property reference (shares the 0x01 handler).
    t[0x5E] = l("Op5E", "r");
    // Let attribute (NATIVE_BYTECODE_OPCODES.md, UNVERIFIED in game).
    t[0x5F] = l("Op5F", "EE");
    t[0x1B] = l("VirtualFunction", "nP");
    t[0x1C] = l("FinalFunction", "rP");
    t[0x37] = l("GlobalFunction", "nP");
    t[0x42] = l("DelegateFunction", "brnP");
    t[0x19] = l("Context", "EwrbE");
    t[0x12] = l("ClassContext", "EwrbE");
    t
};

/// The operand layout of `op`, or `None` for a token with no table entry (the native calls, the strings, the label
/// table, `Case`, `Conditional`, and the unknown tokens 0x2B and 0x5B-0x5D).
pub fn layout_for(op: u8) -> Option<&'static Layout> {
    LAYOUTS[usize::from(op)].as_ref()
}

/// Is `op` a native call token (0x60 and above)?
pub fn is_native_call(op: u8) -> bool {
    op >= op::EXTENDED_NATIVE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_has_the_cxx_entries_and_gaps() {
        let known = (0u8..=255).filter(|&o| layout_for(o).is_some()).count();
        // 0x00-0x5F holds 96 tokens; 0x0A (Case), 0x0C (LabelTable), 0x1F/0x34 (strings), 0x45 (Conditional), and the
        // unknown 0x2B, 0x5B, 0x5C, 0x5D have no entry.
        assert_eq!(known, 96 - 9);
        for gap in [0x0A, 0x0C, 0x1F, 0x2B, 0x34, 0x45, 0x5B, 0x5C, 0x5D, 0x60, 0x70, 0xFF] {
            assert!(layout_for(gap).is_none(), "0x{gap:02x}");
        }
        assert_eq!(layout_for(op::CONTEXT), Some(&Layout { name: "Context", operands: "EwrbE" }));
        assert_eq!(layout_for(op::DELEGATE_FUNCTION).map(|l| l.operands), Some("brnP"));
        assert!(is_native_call(0x60) && is_native_call(0xFF) && !is_native_call(0x5F));
    }
}
