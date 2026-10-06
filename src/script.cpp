#include "script.hpp"

#include <bit>
#include <cstring>
#include <cstdio>
#include <sstream>

namespace script {
namespace {

// ---------------------------------------------------------------------------------------------
// Operand layouts. Letters: r = object reference (i32), n = FName (two i32), i = i32, f = f32,
// b = u8, w = u16, E = a sub-expression, P = call parameters (expressions up to EndFunctionParms).
// Everything that needs logic (strings, label table, case, conditional, native calls) is handled in
// Decoder::expression(). Layouts marked UNVERIFIED decode the whole build exactly but could not be
// told apart from a neighbouring layout by the structural checks.
// ---------------------------------------------------------------------------------------------
struct Layout { const char* name; const char* operands; };

const Layout* layoutFor(uint8_t op) {
    static Layout table[256] = {};
    static bool ready = false;
    if (!ready) {
        auto set = [&](uint8_t code, const char* name, const char* operands) { table[code] = {name, operands}; };
        set(0x00, "LocalVariable", "r"); set(0x01, "InstanceVariable", "r"); set(0x02, "DefaultVariable", "r");
        set(0x03, "StateVariable", "r"); set(0x48, "LocalOutVariable", "r");
        set(0x04, "Return", "E"); set(0x05, "Switch", "rbE"); set(0x06, "Jump", "w"); set(0x07, "JumpIfNot", "wE");
        set(0x08, "Stop", ""); set(0x09, "Assert", "wbE"); set(0x0B, "Nothing", ""); set(0x0D, "GotoLabel", "E");
        set(0x0E, "EatReturnValue", "r"); set(0x0F, "Let", "EE"); set(0x10, "DynArrayElement", "EE");
        set(0x11, "New", "EEEE"); set(0x13, "MetaCast", "rE"); set(0x14, "LetBool", "EE");
        set(0x15, "EndParmValue", ""); set(0x16, "EndFunctionParms", ""); set(0x17, "Self", "");
        set(0x18, "Skip", "wE"); set(0x1A, "ArrayElement", "EE"); set(0x1D, "IntConst", "i");
        set(0x1E, "FloatConst", "f"); set(0x20, "ObjectConst", "r"); set(0x21, "NameConst", "n");
        set(0x22, "RotationConst", "iii"); set(0x23, "VectorConst", "fff"); set(0x24, "ByteConst", "b");
        set(0x25, "IntZero", ""); set(0x26, "IntOne", ""); set(0x27, "True", ""); set(0x28, "False", "");
        set(0x29, "NativeParm", "r"); set(0x2A, "NoObject", ""); set(0x2C, "IntConstByte", "b");
        set(0x2D, "BoolVariable", "E"); set(0x2E, "DynamicCast", "rE"); set(0x2F, "Iterator", "Ew");
        set(0x30, "IteratorPop", ""); set(0x31, "IteratorNext", ""); set(0x32, "StructCmpEq", "rEE");
        set(0x33, "StructCmpNe", "rEE"); set(0x35, "StructMember", "rrbbE"); set(0x36, "DynArrayLength", "E");
        set(0x38, "PrimitiveCast", "bE"); set(0x39, "DynArrayInsert", "EP"); set(0x3A, "ReturnNothing", "r");
        set(0x3B, "EqualEqual_DelDel", "EP"); set(0x3C, "NotEqual_DelDel", "EP");
        set(0x3D, "EqualEqual_DelFunc", "EP"); set(0x3E, "NotEqual_DelFunc", "EP"); set(0x3F, "EmptyDelegate", "");
        set(0x40, "DynArrayRemove", "EP"); set(0x41, "DebugInfo", "iiib"); set(0x43, "DelegateProperty", "nr");
        set(0x44, "LetDelegate", "EE");
        set(0x46, "DynArrayFind", "EwP");                   // array, u16, item, EndFunctionParms
        set(0x47, "DynArrayFindStruct", "EwP");             // UNVERIFIED (array, u16, params up to 0x16)
        set(0x49, "DefaultParmValue", "wE"); set(0x4A, "EmptyParmValue", ""); set(0x4B, "InstanceDelegate", "n");
        set(0x4C, "Op4C", "i"); set(0x4D, "Op4D", "i"); set(0x4E, "Op4E", "i"); set(0x4F, "Op4F", "i");
        set(0x50, "Op50", "i");                             // typed temporaries 4C-50: plain i32 index (not a reference)
        set(0x51, "InterfaceContext", "E"); set(0x52, "InterfaceCast", "rE"); set(0x53, "EndOfScript", "");
        set(0x54, "DynArrayAdd", "EP");
        set(0x55, "DynArrayAddItem", "EwP"); set(0x56, "DynArrayRemoveItem", "EwP");  // UNVERIFIED: P ends at 0x16
        set(0x57, "DynArrayInsertItem", "EP"); set(0x58, "DynArrayIterator", "EEbEw");
        set(0x59, "DynArraySort", "EP"); set(0x5A, "FilterEditorOnly", "w");
        set(0x5E, "Op5E", "r");                             // attribute property reference (shares the 0x01 handler)
        set(0x5F, "Op5F", "EE");                            // let attribute (NATIVE_BYTECODE_OPCODES.md, UNVERIFIED in game)
        set(0x1B, "VirtualFunction", "nP"); set(0x1C, "FinalFunction", "rP"); set(0x37, "GlobalFunction", "nP");
        set(0x42, "DelegateFunction", "brnP");
        set(0x19, "Context", "EwrbE"); set(0x12, "ClassContext", "EwrbE");
        ready = true;
    }
    return table[op].name ? &table[op] : nullptr;
}

constexpr unsigned MAX_DEPTH = 256;

struct Decoder {
    const Package& package;
    const Bytes& data;
    size_t pos;
    size_t end;
    size_t base;
    uint32_t refs = 0;  // object references so far: 4 bytes in the file, 8 in the in-memory script

    Decoder(const Package& p, size_t begin, size_t finish)
        : package(p), data(p.data), pos(begin), end(finish), base(begin) {}

    void need(size_t count) const {
        if (pos > end || count > end - pos) throw DecodeError("read past the end of the script");
    }
    uint8_t u8() { need(1); return data[pos++]; }
    uint16_t u16() { need(2); const uint16_t v = uint16_t(data[pos] | (data[pos + 1] << 8)); pos += 2; return v; }
    int32_t i32() {
        need(4);
        uint32_t v = 0;
        for (unsigned i = 0; i < 4; ++i) v |= uint32_t(data[pos + i]) << (8 * i);
        pos += 4;
        return static_cast<int32_t>(v);
    }
    float f32() { return std::bit_cast<float>(static_cast<uint32_t>(i32())); }
    std::string fname() {
        const int32_t index = i32(), number = i32();
        if (index < 0 || size_t(index) >= package.names.size()) throw DecodeError("FName index out of range");
        if (number < 0) throw DecodeError("negative FName number");
        return package.names[size_t(index)] + (number ? "_" + std::to_string(number - 1) : "");
    }
    int32_t ref() {
        const int32_t value = i32();
        if (value < -int64_t(package.imports.size()) || value > int64_t(package.exports.size()))
            throw DecodeError("object reference out of range");
        ++refs;
        return value;
    }
    uint32_t memoryOffset() const { return uint32_t(pos - base) + 4 * refs; }

    void parameters(Expr& out, unsigned depth) {
        while (true) {
            if (pos >= end) throw DecodeError("call parameters run past the script end");
            if (data[pos] == EX_EndFunctionParms) { ++pos; return; }
            out.kids.push_back(expression(depth + 1));
        }
    }

    Expr expression(unsigned depth) {
        if (depth > MAX_DEPTH) throw DecodeError("expression nesting too deep");
        Expr e;
        e.file = uint32_t(pos - base);
        e.mem = memoryOffset();
        e.op = u8();
        if (e.op >= EX_FirstNative) {
            e.native = e.op;
            parameters(e, depth);
            return e;
        }
        if (e.op >= EX_ExtendedNative) {
            e.native = uint32_t(e.op - EX_ExtendedNative) * 256u + u8();
            parameters(e, depth);
            return e;
        }
        switch (e.op) {
        case EX_StringConst: {
            while (true) { const auto c = u8(); if (!c) break; e.text.push_back(char(c)); }
            return e;
        }
        case EX_UnicodeStringConst: {
            while (true) {
                const auto c = u16();
                if (!c) break;
                utf8(e.text, c);
            }
            return e;
        }
        case EX_LabelTable: {
            while (true) {
                auto name = fname();
                const int32_t offset = i32();
                if (name == "None") break;
                e.labels.emplace_back(std::move(name), offset);
                if (e.labels.size() > 4096) throw DecodeError("runaway label table");
            }
            return e;
        }
        case EX_Case: {
            const uint16_t next = u16();
            e.words.push_back(next);
            if (next == 0xFFFF) return e;  // default
            e.kids.push_back(expression(depth + 1));
            return e;
        }
        case EX_Conditional: {
            e.kids.push_back(expression(depth + 1));
            e.words.push_back(u16());
            e.kids.push_back(expression(depth + 1));
            e.words.push_back(u16());
            e.kids.push_back(expression(depth + 1));
            return e;
        }
        default: break;
        }
        const Layout* layout = layoutFor(e.op);
        if (!layout) throw DecodeError("unknown opcode 0x" + hex(e.op));
        for (const char* letter = layout->operands; *letter; ++letter) {
            switch (*letter) {
            case 'r': e.refs.push_back(ref()); break;
            case 'n': e.names.push_back(fname()); break;
            case 'i': e.ints.push_back(i32()); break;
            case 'f': e.floats.push_back(f32()); break;
            case 'b': e.bytes.push_back(u8()); break;
            case 'w': e.words.push_back(u16()); break;
            case 'E': e.kids.push_back(expression(depth + 1)); break;
            case 'P': parameters(e, depth); break;
            default: throw DecodeError("bad operand letter");
            }
        }
        return e;
    }

    static std::string hex(unsigned value) {
        char buffer[8];
        std::snprintf(buffer, sizeof buffer, "%02x", value);
        return buffer;
    }
};

std::string qualifiedName(const Package& package, int32_t reference) {
    if (!reference) return "None";
    try { return package.path(reference); }
    catch (const std::exception&) { return "<bad ref " + std::to_string(reference) + ">"; }
}

void formatExpr(const Package& package, const Expr& e, std::string& out);

void formatList(const Package& package, const std::vector<Expr>& list, std::string& out) {
    for (size_t i = 0; i < list.size(); ++i) {
        if (i) out += ", ";
        formatExpr(package, list[i], out);
    }
}

void formatExpr(const Package& package, const Expr& e, std::string& out) {
    if (e.isNativeCall()) {
        out += "native_" + std::to_string(e.native) + "(";
        formatList(package, e.kids, out);
        out += ")";
        return;
    }
    if (e.op == EX_StringConst || e.op == EX_UnicodeStringConst) { out += quote(e.text); return; }
    if (e.op == EX_LabelTable) {
        out += "LabelTable[";
        for (size_t i = 0; i < e.labels.size(); ++i)
            out += (i ? ", " : "") + e.labels[i].first + "@" + std::to_string(e.labels[i].second);
        out += "]";
        return;
    }
    const Layout* layout = layoutFor(e.op);
    std::string name = e.op == EX_Case ? "Case" : e.op == EX_Conditional ? "Conditional" : layout ? layout->name : "?";
    out += name;
    std::vector<std::string> parts;
    for (const auto r : e.refs) parts.push_back(qualifiedName(package, r));
    for (const auto& n : e.names) parts.push_back(n);
    for (const auto v : e.ints) parts.push_back(std::to_string(v));
    for (const auto v : e.floats) { std::ostringstream s; s << v; parts.push_back(s.str()); }
    for (const auto v : e.bytes) parts.push_back(std::to_string(unsigned(v)));
    for (const auto v : e.words) parts.push_back(std::to_string(unsigned(v)));
    if (parts.empty() && e.kids.empty()) return;
    out += "(";
    bool first = true;
    for (const auto& part : parts) { if (!first) out += ", "; out += part; first = false; }
    for (const auto& kid : e.kids) { if (!first) out += ", "; formatExpr(package, kid, out); first = false; }
    out += ")";
}

// Function exports: the export's class is the Core class "Function".
bool classNamed(const Package& package, int32_t reference, const char* wanted) {
    if (!reference) return false;
    try { return package.object(reference).name == wanted; }
    catch (const std::exception&) { return false; }
}

} // namespace

bool isFunctionExport(const Package& package, int32_t index) {
    if (index <= 0 || size_t(index) > package.exports.size()) return false;
    return classNamed(package, package.exports[size_t(index) - 1].cls, "Function");
}

FunctionInfo readFunction(const Package& package, int32_t index) {
    if (!isFunctionExport(package, index)) throw DecodeError("export is not a function");
    const auto& exportObject = package.exports[size_t(index) - 1];
    const size_t start = size_t(exportObject.offset), finish = start + size_t(exportObject.size);
    if (exportObject.size < 40) throw DecodeError("function export too small");
    auto u16At = [&](size_t at) { return uint16_t(package.data[at] | (package.data[at + 1] << 8)); };
    auto u32At = [&](size_t at) {
        uint32_t v = 0;
        for (unsigned i = 0; i < 4; ++i) v |= uint32_t(package.data[at + i]) << (8 * i);
        return v;
    };
    // The tail is [iNative u16][OperPrecedence u8][FunctionFlags u32][RepOffset u16 if FUNC_Net][FriendlyName FName].
    // FunctionFlags sits at a different distance from the end depending on FUNC_Net, which is itself
    // one of the flags; the FriendlyName must be a valid name to accept a candidate.
    const auto validName = [&](size_t at) {
        const int32_t nameIndex = int32_t(u32At(at)), number = int32_t(u32At(at + 4));
        return nameIndex >= 0 && size_t(nameIndex) < package.names.size() && number >= 0 && number < 100000;
    };
    if (!validName(finish - 8)) throw DecodeError("function tail has no FriendlyName");
    FunctionInfo info;
    info.index = index;
    info.name = exportObject.name;
    uint32_t flags = u32At(finish - 12);
    size_t tail = 15;
    if (u32At(finish - 14) & FUNC_Net) { flags = u32At(finish - 14); tail = 17; }
    info.flags = flags;
    info.friendlyName = package.names[size_t(int32_t(u32At(finish - 8)))];
    info.native = u16At(finish - tail);
    info.operatorPrecedence = package.data[finish - tail + 2];
    const size_t tailStart = finish - tail;
    // Natives have no script: only the tail (flags, iNative, FriendlyName) is meaningful.
    if (flags & FUNC_Native) return info;

    const size_t count = u16At(start);
    const size_t sizeAt = start + 2 + 2 * count + 44;
    if (sizeAt + 4 > tailStart) throw DecodeError("header runs into the tail");
    for (size_t i = 0; i < count; ++i) info.hiddenLocals.push_back(u16At(start + 2 + 2 * i));
    const int32_t memory = int32_t(u32At(sizeAt - 4)), size = int32_t(u32At(sizeAt));
    if (size < 0 || memory < 0) throw DecodeError("negative script size");
    if (sizeAt + 4 + size_t(size) != tailStart)
        throw DecodeError("ScriptSize " + std::to_string(size) + " does not end at the function tail");
    info.memorySize = uint32_t(memory);
    info.scriptOffset = sizeAt + 4;
    info.scriptSize = size_t(size);
    return info;
}

Code decode(const Package& package, const FunctionInfo& function) {
    if (!function.scriptSize) throw DecodeError("function has no script");
    Decoder decoder(package, function.scriptOffset, function.scriptOffset + function.scriptSize);
    Code code;
    while (decoder.pos < decoder.end) {
        const uint32_t memory = decoder.memoryOffset();
        // A statement never starts with EndFunctionParms: one there means a call consumed too few operands
        // (the dynamic-array tokens carry their own terminator; this check found the layouts that missed it).
        if (decoder.data[decoder.pos] == EX_EndFunctionParms) throw DecodeError("stray EndFunctionParms at statement level");
        code.statements.push_back(decoder.expression(0));
        code.statementAt.emplace(memory, code.statements.size() - 1);
        if (code.statements.back().op == EX_EndOfScript) break;
    }
    if (decoder.pos != decoder.end) throw DecodeError("script ended before its stored size");
    if (code.statements.back().op != EX_EndOfScript) throw DecodeError("last token is not EndOfScript");
    code.memorySize = decoder.memoryOffset();
    code.statementAt.emplace(code.memorySize, code.statements.size());
    if (code.memorySize != function.memorySize)
        throw DecodeError("memory size " + std::to_string(code.memorySize) + " != header ScriptBytecodeSize " +
                          std::to_string(function.memorySize));
    // Every Jump / JumpIfNot / Case target must be the start of a statement.
    std::vector<const Expr*> stack;
    for (const auto& statement : code.statements) stack.push_back(&statement);
    while (!stack.empty()) {
        const Expr* e = stack.back();
        stack.pop_back();
        if ((e->op == EX_Jump || e->op == EX_JumpIfNot || (e->op == EX_Case && e->words[0] != 0xFFFF)) &&
            !code.statementAt.contains(e->words[0]))
            throw DecodeError("jump target " + std::to_string(e->words[0]) + " is not a statement start");
        for (const auto& kid : e->kids) stack.push_back(&kid);
    }
    return code;
}

std::string format(const Package& package, const Expr& expr) {
    std::string out;
    formatExpr(package, expr, out);
    return out;
}

std::string disassemble(const Package& package, const FunctionInfo& function, const Code& code) {
    std::ostringstream out;
    out << "// " << package.path(function.index) << "  flags 0x" << std::hex << function.flags << std::dec << '\n';
    for (const auto& statement : code.statements)
        out << statement.file << ": " << format(package, statement) << '\n';
    return out.str();
}

CheckResult checkPackage(const Package& package) {
    CheckResult result;
    for (int32_t index = 1; size_t(index) <= package.exports.size(); ++index) {
        if (!isFunctionExport(package, index)) continue;
        FunctionInfo info;
        try {
            // Native functions have no script; their tail is still valid.
            const auto& exportObject = package.exports[size_t(index) - 1];
            if (exportObject.size < 40) continue;
            info = readFunction(package, index);
        } catch (const DecodeError&) {
            // A function export whose tail cannot be read is reported like the Python oracle: skipped.
            continue;
        }
        ++result.functions;
        if (info.flags & FUNC_Native) { ++result.native; continue; }
        try {
            decode(package, info);
            ++result.decoded;
        } catch (const std::exception& error) {
            result.failures.push_back(package.path(index) + ": " + error.what());
        }
    }
    return result;
}

} // namespace script
