#pragma once

#include "package.hpp"

#include <cstdint>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <unordered_map>
#include <vector>

// Phase 2 (UnrealScript VM), step 2: the bytecode loader.
//
// Reads the script of a `UFunction` export into an expression tree. Nothing is executed here.
// The function layout and every operand layout were recovered from the packages and are checked
// structurally (see docs/verification/SCRIPT_BYTECODE_DISASM.md); operand layouts that only
// "fit" are marked UNVERIFIED in script.cpp. The Python prototype research/script_disasm.py is
// the independent oracle this port is compared against (tools/verify_scripts.py).
namespace script {

struct DecodeError : std::runtime_error {
    using std::runtime_error::runtime_error;
};

// FunctionFlags bits as read from the packages (UE3 values; 0x22003 = Final|Defined|Public).
enum FunctionFlag : uint32_t {
    FUNC_Final = 0x1, FUNC_Defined = 0x2, FUNC_Iterator = 0x4, FUNC_Latent = 0x8, FUNC_PreOperator = 0x10,
    FUNC_Singular = 0x20, FUNC_Net = 0x40, FUNC_NetReliable = 0x80, FUNC_Simulated = 0x100, FUNC_Exec = 0x200,
    FUNC_Native = 0x400, FUNC_Event = 0x800, FUNC_Operator = 0x1000, FUNC_Static = 0x2000, FUNC_Const = 0x8000,
    FUNC_Public = 0x20000, FUNC_Private = 0x40000, FUNC_Protected = 0x80000, FUNC_Delegate = 0x100000,
    FUNC_HasOutParms = 0x400000,
};

// Expression tokens (public UE3 set; layouts fitted to this build, UNVERIFIED ones noted in script.cpp).
enum Op : uint8_t {
    EX_LocalVariable = 0x00, EX_InstanceVariable = 0x01, EX_DefaultVariable = 0x02, EX_StateVariable = 0x03,
    EX_Return = 0x04, EX_Switch = 0x05, EX_Jump = 0x06, EX_JumpIfNot = 0x07, EX_Stop = 0x08, EX_Assert = 0x09,
    EX_Case = 0x0A, EX_Nothing = 0x0B, EX_LabelTable = 0x0C, EX_GotoLabel = 0x0D, EX_EatReturnValue = 0x0E,
    EX_Let = 0x0F, EX_DynArrayElement = 0x10, EX_New = 0x11, EX_ClassContext = 0x12, EX_MetaCast = 0x13,
    EX_LetBool = 0x14, EX_EndParmValue = 0x15, EX_EndFunctionParms = 0x16, EX_Self = 0x17, EX_Skip = 0x18,
    EX_Context = 0x19, EX_ArrayElement = 0x1A, EX_VirtualFunction = 0x1B, EX_FinalFunction = 0x1C,
    EX_IntConst = 0x1D, EX_FloatConst = 0x1E, EX_StringConst = 0x1F, EX_ObjectConst = 0x20, EX_NameConst = 0x21,
    EX_RotationConst = 0x22, EX_VectorConst = 0x23, EX_ByteConst = 0x24, EX_IntZero = 0x25, EX_IntOne = 0x26,
    EX_True = 0x27, EX_False = 0x28, EX_NativeParm = 0x29, EX_NoObject = 0x2A, EX_IntConstByte = 0x2C,
    EX_BoolVariable = 0x2D, EX_DynamicCast = 0x2E, EX_Iterator = 0x2F, EX_IteratorPop = 0x30,
    EX_IteratorNext = 0x31, EX_StructCmpEq = 0x32, EX_StructCmpNe = 0x33, EX_UnicodeStringConst = 0x34,
    EX_StructMember = 0x35, EX_DynArrayLength = 0x36, EX_GlobalFunction = 0x37, EX_PrimitiveCast = 0x38,
    EX_DynArrayInsert = 0x39, EX_ReturnNothing = 0x3A, EX_EqualEqual_DelDel = 0x3B, EX_NotEqual_DelDel = 0x3C,
    EX_EqualEqual_DelFunc = 0x3D, EX_NotEqual_DelFunc = 0x3E, EX_EmptyDelegate = 0x3F, EX_DynArrayRemove = 0x40,
    EX_DebugInfo = 0x41, EX_DelegateFunction = 0x42, EX_DelegateProperty = 0x43, EX_LetDelegate = 0x44,
    EX_Conditional = 0x45, EX_DynArrayFind = 0x46, EX_DynArrayFindStruct = 0x47, EX_LocalOutVariable = 0x48,
    EX_DefaultParmValue = 0x49, EX_EmptyParmValue = 0x4A, EX_InstanceDelegate = 0x4B,
    // 0x4C-0x50: present in this build, one plain i32 each. Meaning UNVERIFIED (they behave like
    // typed local-variable slots in the listings; see the verification record).
    EX_Op4C = 0x4C, EX_Op4D = 0x4D, EX_Op4E = 0x4E, EX_Op4F = 0x4F, EX_Op50 = 0x50,
    EX_InterfaceContext = 0x51, EX_InterfaceCast = 0x52, EX_EndOfScript = 0x53, EX_DynArrayAdd = 0x54,
    EX_DynArrayAddItem = 0x55, EX_DynArrayRemoveItem = 0x56, EX_DynArrayInsertItem = 0x57,
    EX_DynArrayIterator = 0x58, EX_DynArraySort = 0x59, EX_FilterEditorOnly = 0x5A,
    EX_Op5E = 0x5E,  // one object reference, UNVERIFIED meaning (a property reference variant)
    EX_Op5F = 0x5F,  // sits where Let does: lhs, rhs (a typed Let), UNVERIFIED
    // 0x60-0x6F: extended native call (two bytes, index = (op-0x60)*256 + next byte); 0x70+: native call.
    EX_ExtendedNative = 0x60, EX_FirstNative = 0x70,
};

// One decoded expression. Operands are stored by kind in the order the opcode lists them; see the
// layout table in script.cpp (and the accessors the interpreter uses, e.g. Let = kids[0], kids[1]).
struct Expr {
    uint8_t op = 0;
    uint32_t native = 0;          // native index for call tokens (op >= 0x60 and op < 0x80 or op >= 0x70)
    uint32_t file = 0;            // offset of the token in the stored script
    uint32_t mem = 0;             // the same offset in the in-memory script (what jumps count in)
    std::vector<int32_t> refs;    // object references, relative to the function's package
    std::vector<std::string> names;
    std::vector<int32_t> ints;
    std::vector<float> floats;
    std::vector<uint8_t> bytes;
    std::vector<uint16_t> words;  // u16 operands (jump/skip offsets are in-memory offsets)
    std::string text;             // StringConst / UnicodeStringConst
    std::vector<Expr> kids;       // sub-expressions; for calls, the parameters
    std::vector<std::pair<std::string, int32_t>> labels;  // LabelTable entries (name, offset)
    bool isNativeCall() const { return op >= EX_FirstNative || (op >= EX_ExtendedNative && op < EX_FirstNative); }
};

// One function's decoded script.
struct Code {
    std::vector<Expr> statements;
    std::unordered_map<uint32_t, size_t> statementAt;  // in-memory offset -> statement index
    uint32_t memorySize = 0;
};

struct FunctionInfo {
    int32_t index = 0;            // export index in its package
    std::string name;             // export name
    std::string friendlyName;     // FriendlyName from the tail (operator symbol for operators)
    uint32_t flags = 0;
    uint16_t native = 0;          // iNative (0 for script functions)
    uint8_t operatorPrecedence = 0;
    uint32_t memorySize = 0;      // ScriptBytecodeSize
    size_t scriptOffset = 0;      // offset of the script bytes inside the package data
    size_t scriptSize = 0;        // ScriptSize
    bool hasScript() const { return scriptSize != 0; }
};

// Is this export a UFunction (its class is the Core class named "Function")?
bool isFunctionExport(const Package& package, int32_t index);

// Reads the header and tail of a function export. Throws DecodeError when the layout does not hold
// (the script size must end exactly at the function tail).
FunctionInfo readFunction(const Package& package, int32_t index);

// Decodes a function's script. Throws DecodeError on any structural violation: unknown opcode,
// reading past the script, the script not ending in EndOfScript, the in-memory size disagreeing
// with the header, a jump/case target that is not a statement start, or nesting deeper than 256.
Code decode(const Package& package, const FunctionInfo& function);

// Readable pseudo-code for one expression / a whole function (for --disasm and diagnostics).
std::string format(const Package& package, const Expr& expr);
std::string disassemble(const Package& package, const FunctionInfo& function, const Code& code);

// Whole-package structural check, the C++ counterpart of research/script_disasm.py --check.
struct CheckResult {
    size_t functions = 0;
    size_t native = 0;
    size_t decoded = 0;
    std::vector<std::string> failures;  // "Class.Function: reason"
};
CheckResult checkPackage(const Package& package);

} // namespace script
