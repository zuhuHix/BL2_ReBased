#include "vm.hpp"

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <sstream>

namespace vm {

using script::Expr;
using K = Value::Kind;

namespace {
std::string lowerText(std::string_view text) {
    std::string out(text);
    for (auto& c : out) c = char(std::tolower(static_cast<unsigned char>(c)));
    return out;
}
constexpr unsigned MAX_CALL_DEPTH = 200;
} // namespace

// One variable slot. Out parameters alias the caller's storage.
struct Slot {
    Value v;
    Value* alias = nullptr;
    bool omitted = false;
    Value& ref() { return alias ? *alias : v; }
};

struct IterState {
    bool dynamicArray = true;
    Value* array = nullptr;
    Value* item = nullptr;
    Value* index = nullptr;
    size_t position = 0;
    size_t resume = 0;      // statement to run for each element
};

struct Frame {
    Function* function = nullptr;
    ObjectPtr self;
    std::unordered_map<int32_t, Slot> locals;                 // keyed by the property's export index
    std::map<std::pair<int, int>, Slot> typed;                // Op4C..Op50 slots: (opcode, index)
    const script::Code* code = nullptr;
    Value switchValue;
    std::vector<IterState> iterators;
    Value result;
    bool returned = false;
};

struct Interp {
    Runtime& rt;
    unsigned depth = 0;
    std::vector<const Function*> stack;                       // active script functions, innermost last
    Value scratch;                                            // sink for writes through a null reference
    std::map<std::pair<const Package*, int32_t>, PropertyDecl> decls;

    explicit Interp(Runtime& runtime) : rt(runtime) {}

    // ---------------------------------------------------------------------------- reference helpers
    const PropertyDecl& decl(const Frame& f, int32_t ref) {
        const auto resolved = rt.resolveRef(f.function->package, ref);
        if (!resolved) throw RuntimeError("unresolved property reference " + std::to_string(ref) + " in " + f.function->path);
        const auto key = std::make_pair(resolved.package.get(), resolved.index);
        const auto found = decls.find(key);
        if (found != decls.end()) return found->second;
        return decls.emplace(key, rt.readProperty(resolved.package, resolved.index)).first->second;
    }

    Value zeroFor(const Frame& f, int32_t ref) {
        if (!ref) return Value();
        try { return rt.zeroValue(decl(f, ref)); } catch (const std::exception&) { return Value(); }
    }

    void tick() {
        if (++rt.steps > rt.stepLimit) throw RuntimeError("step limit exceeded (runaway script?)");
    }

    // ------------------------------------------------------------------------------------ l-values
    Value* lval(const Expr& e, Frame& f, const ObjectPtr& ctx) {
        tick();
        switch (e.op) {
        case script::EX_LocalVariable: case script::EX_LocalOutVariable: {
            auto found = f.locals.find(e.refs.at(0));
            if (found == f.locals.end()) found = f.locals.emplace(e.refs.at(0), Slot{}).first;
            return &found->second.ref();
        }
        case script::EX_Op4C: case script::EX_Op4D: case script::EX_Op4E: case script::EX_Op4F: case script::EX_Op50:
            return &f.typed[{e.op, e.ints.at(0)}].ref();
        case script::EX_InstanceVariable: case script::EX_Op5E: case script::EX_StateVariable: {
            if (!ctx) return &(scratch = Value());
            const auto& property = decl(f, e.refs.at(0));
            Value* slot = rt.property(*ctx, property.name);
            return slot ? slot : &(scratch = Value());
        }
        case script::EX_DefaultVariable: {
            if (!ctx || !ctx->cls) return &(scratch = Value());
            const auto& property = decl(f, e.refs.at(0));
            ObjectPtr defaults = rt.defaultsOf(ctx->cls);
            Value* slot = defaults ? rt.property(*defaults, property.name) : nullptr;
            return slot ? slot : &(scratch = Value());
        }
        case script::EX_BoolVariable: case script::EX_InterfaceContext:
            return lval(e.kids.at(0), f, ctx);
        case script::EX_StructMember: {
            Value* base = lval(e.kids.at(0), f, ctx);
            Value* field = base->field(decl(f, e.refs.at(0)).name);
            return field ? field : &(scratch = Value());
        }
        case script::EX_ArrayElement: case script::EX_DynArrayElement: {
            const int64_t index = eval(e.kids.at(0), f, ctx).integer();
            Value* array = lval(e.kids.at(1), f, ctx);
            if (array->kind != K::Array) { *array = Value::makeArray(); }
            auto& items = array->elements();
            if (index < 0) { log("accessed array out of bounds: " + std::to_string(index)); return &(scratch = Value()); }
            if (size_t(index) >= items.size()) {
                if (e.op == script::EX_ArrayElement) { log("accessed array out of bounds: " + std::to_string(index)); return &(scratch = Value()); }
                items.resize(size_t(index) + 1);   // a dynamic array grows when written past its end
            }
            return &items[size_t(index)];
        }
        case script::EX_Context: case script::EX_ClassContext: {
            ObjectPtr target = contextTarget(e, f, ctx);
            if (!target) { noteNoneContext(f); return &(scratch = zeroFor(f, e.refs.at(0))); }
            return lval(e.kids.at(1), f, target);
        }
        default:
            scratch = eval(e, f, ctx);          // not a storage location: operate on a temporary
            return &scratch;
        }
    }

    // The object a Context expression's member runs against; null when the object expression is None.
    ObjectPtr contextTarget(const Expr& e, Frame& f, const ObjectPtr& ctx) {
        Value target = eval(e.kids.at(0), f, ctx);
        if (target.kind == K::Class) return rt.defaultsOf(target.cls);   // Class.default / static calls
        if (target.kind == K::Object) return target.o;
        return nullptr;
    }

    void log(const std::string& text) { rt.log.push_back(text); }

    // Census only: a Context expression whose object was None silently yields a zero value; remember where.
    void noteNoneContext(const Frame& f) {
        if (rt.countCalls) ++rt.noneContexts[f.function->path];
    }

    // ---------------------------------------------------------------------------------- expressions
    Value constant(const Expr& e) {
        switch (e.op) {
        case script::EX_IntConst: return Value::makeInt(e.ints.at(0));
        case script::EX_FloatConst: return Value::makeFloat(e.floats.at(0));
        case script::EX_StringConst: case script::EX_UnicodeStringConst: return Value::makeString(e.text);
        case script::EX_NameConst: return Value::makeName(e.names.at(0));
        case script::EX_IntZero: return Value::makeInt(0);
        case script::EX_IntOne: return Value::makeInt(1);
        case script::EX_True: return Value::makeBool(true);
        case script::EX_False: return Value::makeBool(false);
        case script::EX_NoObject: return Value::makeObject(nullptr);
        case script::EX_VectorConst: {
            Value v = rt.zeroStruct("Vector");
            for (size_t i = 0; i < 3; ++i) v.mut().values[i] = Value::makeFloat(e.floats.at(i));
            return v;
        }
        case script::EX_RotationConst: {
            Value v = rt.zeroStruct("Rotator");
            for (size_t i = 0; i < 3; ++i) v.mut().values[i] = Value::makeInt(e.ints.at(i));
            return v;
        }
        default: return Value();
        }
    }

    Value castPrimitive(uint8_t code, const Value& v) {
        auto str = [&](const Value& x) -> std::string { return describeForString(x); };
        switch (code) {
        case 0x3A: case 0x41: case 0x44: return Value::makeInt(v.integer());
        case 0x3B: case 0x3E: case 0x45: case 0x47: case 0x48: return Value::makeBool(v.truth());
        case 0x3C: case 0x3F: case 0x42: return Value::makeFloat(v.number());
        case 0x3D: case 0x40: case 0x43: return Value::makeByte(v.integer());
        case 0x49: case 0x4A: return Value::makeInt(std::strtol(v.s.c_str(), nullptr, 10));
        case 0x4B: { const auto t = lowerText(v.s); return Value::makeBool(t == "true" || t == "1"); }
        case 0x4C: return Value::makeFloat(std::strtod(v.s.c_str(), nullptr));
        case 0x52: case 0x53: case 0x54: case 0x55: case 0x56: case 0x57: case 0x58: case 0x59: case 0x5A: return Value::makeString(str(v));
        case 0x60: return Value::makeName(v.s);
        case 0x39: return rotatorToVector(v);
        case 0x50: return vectorToRotator(v);
        case 0x36: case 0x37: case 0x46: return v;                     // object <-> interface views share a reference
        default:
            log("UNIMPLEMENTED primitive cast 0x" + hex(code));
            return v;
        }
    }

    // Rotator units are 1/65536 of a turn. vector(rot) is the rotator's forward axis; rotation(vec) inverts it
    // (yaw and pitch only, roll 0). Standard UE3 conventions, UNVERIFIED against the game.
    Value rotatorToVector(const Value& r) {
        const double k = 3.14159265358979323846 / 32768.0;
        const Value* p = r.field("Pitch");
        const Value* y = r.field("Yaw");
        const double pitch = (p ? p->number() : 0) * k, yaw = (y ? y->number() : 0) * k;
        Value v = rt.zeroStruct("Vector");
        v.mut().values[0] = Value::makeFloat(std::cos(pitch) * std::cos(yaw));
        v.mut().values[1] = Value::makeFloat(std::cos(pitch) * std::sin(yaw));
        v.mut().values[2] = Value::makeFloat(std::sin(pitch));
        return v;
    }
    Value vectorToRotator(const Value& v) {
        const double k = 32768.0 / 3.14159265358979323846;
        const Value* x = v.field("X");
        const Value* y = v.field("Y");
        const Value* z = v.field("Z");
        const double vx = x ? x->number() : 0, vy = y ? y->number() : 0, vz = z ? z->number() : 0;
        Value r = rt.zeroStruct("Rotator");
        r.mut().values[1] = Value::makeInt(int64_t(std::atan2(vy, vx) * k));
        r.mut().values[0] = Value::makeInt(int64_t(std::atan2(vz, std::sqrt(vx * vx + vy * vy)) * k));
        return r;
    }

    static std::string hex(unsigned v) { char b[8]; std::snprintf(b, sizeof b, "%02x", v); return b; }

    // The text UnrealScript produces when a value is converted to a string. Float formatting follows the
    // usual "%f" of the engine's string conversion (UNVERIFIED against the game).
    static std::string describeForString(const Value& v) {
        char buffer[64];
        switch (v.kind) {
        case K::Int: case K::Byte: return std::to_string(v.i);
        case K::Bool: return v.i ? "True" : "False";
        case K::Float: std::snprintf(buffer, sizeof buffer, "%f", v.f); return buffer;
        case K::String: case K::Name: return v.s;
        case K::Object: return v.o ? v.o->name : "None";
        case K::Class: return v.cls ? v.cls->name : "None";
        case K::Struct: {
            std::string out;
            if (v.agg) for (size_t i = 0; i < v.agg->names.size(); ++i) {
                const auto& f = v.agg->values[i];
                out += (i ? " " : "") + v.agg->names[i] + "=" + describeForString(f);
            }
            return out;
        }
        default: return "";
        }
    }

    Value eval(const Expr& e, Frame& f, const ObjectPtr& ctx) {
        tick();
        if (e.isNativeCall()) return callNative(e, f, ctx);
        switch (e.op) {
        case script::EX_IntConst: case script::EX_FloatConst: case script::EX_StringConst:
        case script::EX_UnicodeStringConst: case script::EX_NameConst: case script::EX_IntZero: case script::EX_IntOne:
        case script::EX_True: case script::EX_False: case script::EX_NoObject: case script::EX_VectorConst:
        case script::EX_RotationConst:
            return constant(e);
        case script::EX_ByteConst: case script::EX_IntConstByte: return Value::makeByte(e.bytes.at(0));
        case script::EX_Self: return Value::makeObject(ctx);
        case script::EX_Nothing: case script::EX_EmptyParmValue: case script::EX_EndParmValue: case script::EX_DebugInfo:
        case script::EX_LabelTable: case script::EX_FilterEditorOnly:
            return Value();
        case script::EX_ObjectConst: {
            const auto resolved = rt.resolveRef(f.function->package, e.refs.at(0));
            if (resolved) {
                const auto& exportObject = resolved.package->exports[size_t(resolved.index) - 1];
                try {
                    if (resolved.package->object(exportObject.cls).name == "Class")
                        return Value::makeClass(rt.classAt(resolved.package, resolved.index));
                } catch (const std::exception&) {}
            }
            return Value::makeObject(resolved ? rt.resource(resolved.package, resolved.index)
                                              : rt.resource(f.function->package, e.refs.at(0)));
        }
        case script::EX_LocalVariable: case script::EX_LocalOutVariable: case script::EX_InstanceVariable:
        case script::EX_DefaultVariable: case script::EX_StateVariable: case script::EX_Op4C: case script::EX_Op4D:
        case script::EX_Op4E: case script::EX_Op4F: case script::EX_Op50: case script::EX_Op5E:
        case script::EX_BoolVariable: case script::EX_StructMember: case script::EX_ArrayElement:
        case script::EX_DynArrayElement: {
            if (e.op == script::EX_ArrayElement || e.op == script::EX_DynArrayElement) {
                // Reading past the end yields a zero value and does not grow a dynamic array.
                const int64_t index = eval(e.kids.at(0), f, ctx).integer();
                Value* array = lval(e.kids.at(1), f, ctx);
                if (array->kind != K::Array || index < 0 || size_t(index) >= array->elements().size()) {
                    log("accessed array out of bounds: " + std::to_string(index));
                    return Value();
                }
                return array->elements()[size_t(index)];
            }
            return *lval(e, f, ctx);
        }
        case script::EX_Context: case script::EX_ClassContext: {
            ObjectPtr target = contextTarget(e, f, ctx);
            if (!target) { noteNoneContext(f); return zeroFor(f, e.refs.at(0)); }
            return eval(e.kids.at(1), f, target);
        }
        case script::EX_InterfaceContext: return eval(e.kids.at(0), f, ctx);
        case script::EX_Skip: return eval(e.kids.at(0), f, ctx);
        case script::EX_Let: case script::EX_LetBool: case script::EX_LetDelegate: case script::EX_Op5F: {
            const Expr& lhs = e.kids.at(0);
            if (lhs.op == script::EX_DynArrayLength) {
                // Assigning a dynamic array's length resizes it.
                Value* array = lval(lhs.kids.at(0), f, ctx);
                const int64_t length = eval(e.kids.at(1), f, ctx).integer();
                if (array->kind != K::Array) *array = Value::makeArray();
                auto& items = array->elements();
                const Value like = items.empty() ? Value() : zeroLike(items.back());
                items.resize(size_t(std::max<int64_t>(0, length)), like);
                return Value::makeInt(length);
            }
            Value* target = lval(lhs, f, ctx);
            Value value = eval(e.kids.at(1), f, ctx);
            assign(*target, std::move(value));
            return *lval(lhs, f, ctx);
        }
        case script::EX_Conditional: {
            return eval(e.kids.at(0), f, ctx).truth() ? eval(e.kids.at(1), f, ctx) : eval(e.kids.at(2), f, ctx);
        }
        case script::EX_VirtualFunction: return callVirtual(e, f, ctx, false);
        case script::EX_GlobalFunction: return callVirtual(e, f, ctx, true);
        case script::EX_FinalFunction: {
            Function* function = rt.functionAt(f.function->package, e.refs.at(0));
            if (!function) throw RuntimeError("call to a missing function (reference " + std::to_string(e.refs.at(0)) + ")");
            return callFunction(*function, ctx, e, f, 0);
        }
        case script::EX_DelegateFunction: return callDelegate(e, f, ctx);
        case script::EX_PrimitiveCast: return castPrimitive(e.bytes.at(0), eval(e.kids.at(0), f, ctx));
        case script::EX_DynamicCast: case script::EX_InterfaceCast: {
            Value v = eval(e.kids.at(0), f, ctx);
            Class* want = rt.classAt(f.function->package, e.refs.at(0));
            if (v.kind == K::Object && v.o && v.o->cls && want && v.o->cls->isChildOf(want)) return v;
            return Value::makeObject(nullptr);
        }
        case script::EX_MetaCast: {
            Value v = eval(e.kids.at(0), f, ctx);
            Class* want = rt.classAt(f.function->package, e.refs.at(0));
            if (v.kind == K::Class && v.cls && want && v.cls->isChildOf(want)) return v;
            return Value::makeClass(nullptr);
        }
        case script::EX_StructCmpEq: case script::EX_StructCmpNe: {
            const bool same = sameValue(eval(e.kids.at(0), f, ctx), eval(e.kids.at(1), f, ctx));
            return Value::makeBool(e.op == script::EX_StructCmpEq ? same : !same);
        }
        case script::EX_EqualEqual_DelDel: case script::EX_EqualEqual_DelFunc: case script::EX_NotEqual_DelDel:
        case script::EX_NotEqual_DelFunc: {
            const Value a = eval(e.kids.at(0), f, ctx), b = eval(e.kids.at(1), f, ctx);
            const bool same = a.o == b.o && a.s == b.s;
            return Value::makeBool(e.op == script::EX_EqualEqual_DelDel || e.op == script::EX_EqualEqual_DelFunc ? same : !same);
        }
        case script::EX_EmptyDelegate: { Value v; v.kind = K::Delegate; return v; }
        case script::EX_DelegateProperty: case script::EX_InstanceDelegate: {
            Value v;
            v.kind = K::Delegate;
            v.o = ctx;
            v.s = e.names.at(0);
            return v;
        }
        case script::EX_DynArrayLength: {
            const Value array = eval(e.kids.at(0), f, ctx);
            return Value::makeInt(array.kind == K::Array ? int64_t(array.elements().size()) : 0);
        }
        case script::EX_DynArrayAdd: {
            Value* array = lval(e.kids.at(0), f, ctx);
            const int64_t count = eval(e.kids.at(1), f, ctx).integer();
            if (array->kind != K::Array) *array = Value::makeArray();
            auto& items = array->elements();
            const size_t first = items.size();
            const Value like = items.empty() ? Value() : zeroLike(items.back());
            items.resize(items.size() + size_t(std::max<int64_t>(0, count)), like);
            return Value::makeInt(int64_t(first));
        }
        case script::EX_DynArrayAddItem: {
            Value* array = lval(e.kids.at(0), f, ctx);
            Value item = eval(e.kids.at(1), f, ctx);
            if (array->kind != K::Array) *array = Value::makeArray();
            array->elements().push_back(std::move(item));
            return Value::makeInt(int64_t(array->elements().size()) - 1);
        }
        case script::EX_DynArrayInsert: {
            Value* array = lval(e.kids.at(0), f, ctx);
            const int64_t at = eval(e.kids.at(1), f, ctx).integer(), count = eval(e.kids.at(2), f, ctx).integer();
            if (array->kind != K::Array) *array = Value::makeArray();
            auto& items = array->elements();
            const size_t position = size_t(std::clamp<int64_t>(at, 0, int64_t(items.size())));
            const Value like = items.empty() ? Value() : zeroLike(items.back());
            items.insert(items.begin() + position, size_t(std::max<int64_t>(0, count)), like);
            return Value();
        }
        case script::EX_DynArrayInsertItem: {
            Value* array = lval(e.kids.at(0), f, ctx);
            const int64_t at = eval(e.kids.at(1), f, ctx).integer();
            Value item = eval(e.kids.at(2), f, ctx);
            if (array->kind != K::Array) *array = Value::makeArray();
            auto& items = array->elements();
            items.insert(items.begin() + size_t(std::clamp<int64_t>(at, 0, int64_t(items.size()))), std::move(item));
            return Value();
        }
        case script::EX_DynArrayRemove: {
            Value* array = lval(e.kids.at(0), f, ctx);
            const int64_t at = eval(e.kids.at(1), f, ctx).integer(), count = eval(e.kids.at(2), f, ctx).integer();
            if (array->kind == K::Array) {
                auto& items = array->elements();
                if (at >= 0 && size_t(at) <= items.size()) {
                    const size_t last = std::min(items.size(), size_t(at) + size_t(std::max<int64_t>(0, count)));
                    items.erase(items.begin() + size_t(at), items.begin() + last);
                }
            }
            return Value();
        }
        case script::EX_DynArrayRemoveItem: {
            Value* array = lval(e.kids.at(0), f, ctx);
            const Value item = eval(e.kids.at(1), f, ctx);
            if (array->kind == K::Array) {
                auto& items = array->elements();
                items.erase(std::remove_if(items.begin(), items.end(), [&](const Value& v) { return sameValue(v, item); }), items.end());
            }
            return Value();
        }
        case script::EX_DynArrayFind: {
            const Value array = eval(e.kids.at(0), f, ctx);
            const Value item = eval(e.kids.at(1), f, ctx);
            const auto& items = array.elements();
            for (size_t i = 0; i < items.size(); ++i)
                if (sameValue(items[i], item)) return Value::makeInt(int64_t(i));
            return Value::makeInt(-1);
        }
        case script::EX_DynArrayFindStruct: {
            // Params after the array: the member name (as a name/string constant) and the value to match.
            const Value array = eval(e.kids.at(0), f, ctx);
            const Value member = eval(e.kids.at(1), f, ctx);
            const Value wanted = eval(e.kids.at(2), f, ctx);
            const auto& items = array.elements();
            for (size_t i = 0; i < items.size(); ++i)
                if (const Value* field = items[i].field(member.s))
                    if (sameValue(*field, wanted)) return Value::makeInt(int64_t(i));
            return Value::makeInt(-1);
        }
        case script::EX_DynArraySort:
            log("UNIMPLEMENTED DynArraySort");
            return Value();
        case script::EX_New: {
            Value cls = eval(e.kids.at(3), f, ctx);
            if (cls.kind != K::Class || !cls.cls) return Value::makeObject(nullptr);
            ObjectPtr object = rt.instantiate(cls.cls);
            Value outer = eval(e.kids.at(0), f, ctx);
            if (outer.kind == K::Object) object->outer = outer.o;
            return Value::makeObject(object);
        }
        case script::EX_Assert: {
            if (!eval(e.kids.at(0), f, ctx).truth()) log("Assertion failed at line " + std::to_string(e.words.at(0)));
            return Value();
        }
        case script::EX_EatReturnValue: return Value();
        case script::EX_ReturnNothing: return zeroFor(f, e.refs.at(0));
        case script::EX_NativeParm: return Value();
        default:
            throw RuntimeError("expression 0x" + hex(e.op) + " is not evaluable");
        }
    }

    static Value zeroLike(const Value& v) {
        switch (v.kind) {
        case K::Int: return Value::makeInt(0);
        case K::Float: return Value::makeFloat(0);
        case K::Bool: return Value::makeBool(false);
        case K::Byte: return Value::makeByte(0);
        case K::String: return Value::makeString("");
        case K::Name: return Value::makeName("None");
        case K::Object: return Value::makeObject(nullptr);
        case K::Class: return Value::makeClass(nullptr);
        case K::Struct: {
            Value z = v;
            for (auto& field : z.mut().values) field = zeroLike(field);
            return z;
        }
        case K::Array: return Value::makeArray();
        default: return Value();
        }
    }

    // Assignment converts between the numeric kinds the compiler treats as compatible and keeps the
    // destination's kind when it already has one (a byte slot receiving an int keeps wrapping to a byte).
    void assign(Value& target, Value value) {
        if (target.kind != value.kind && target.kind != K::None) {
            switch (target.kind) {
            case K::Int: if (value.kind == K::Byte || value.kind == K::Bool || value.kind == K::Float) value = Value::makeInt(value.integer()); break;
            case K::Float: if (value.kind == K::Int || value.kind == K::Byte) value = Value::makeFloat(value.number()); break;
            case K::Byte: if (value.kind == K::Int || value.kind == K::Bool) value = Value::makeByte(value.integer()); break;
            case K::Bool: if (value.kind == K::Int || value.kind == K::Byte) value = Value::makeBool(value.truth()); break;
            case K::Name: if (value.kind == K::String) value = Value::makeName(value.s); break;
            default: break;
            }
        }
        target = std::move(value);
    }

    // --------------------------------------------------------------------------------------- calls
    // Parameter expressions are evaluated in the *caller's* context (the frame's own object) even when the
    // call target was chosen by a Context expression (UE3's Stack.Object versus Context; UNVERIFIED).
    struct Bound { std::vector<NativeCall::Arg> args; };

    Bound bindArguments(Function& function, const Expr& e, Frame& f, size_t firstKid) {
        Bound bound;
        const size_t given = e.kids.size() > firstKid ? e.kids.size() - firstKid : 0;
        for (size_t i = 0; i < function.params.size(); ++i) {
            NativeCall::Arg arg;
            if (i >= given) { arg.supplied = false; bound.args.push_back(std::move(arg)); continue; }
            const Expr& kid = e.kids[firstKid + i];
            if (kid.op == script::EX_EmptyParmValue || (kid.op == script::EX_Nothing && function.params[i].isOptional())) {
                arg.supplied = false;
            } else if (function.params[i].isOut()) {
                arg.alias = lval(kid, f, f.self);
                arg.value = *arg.alias;
            } else {
                arg.value = eval(kid, f, f.self);
            }
            bound.args.push_back(std::move(arg));
        }
        // Surplus arguments (a call through a function with a different shape) are evaluated and dropped.
        for (size_t i = function.params.size(); i < given; ++i) eval(e.kids[firstKid + i], f, f.self);
        return bound;
    }

    Value callFunction(Function& function, const ObjectPtr& target, const Expr& e, Frame& f, size_t firstKid) {
        Bound bound = bindArguments(function, e, f, firstKid);
        return invoke(function, target, std::move(bound.args));
    }

    Value callVirtual(const Expr& e, Frame& f, const ObjectPtr& ctx, bool global) {
        const std::string& name = e.names.at(0);
        if (!ctx) { log("call of " + name + " on a None object"); return Value(); }
        Function* function = rt.findMethod(ctx->cls, name, global ? std::string() : ctx->state);
        if (!function) {
            log("UNIMPLEMENTED function " + (ctx->cls ? ctx->cls->name : std::string("?")) + "." + name);
            for (const auto& kid : e.kids) eval(kid, f, f.self);
            return Value();
        }
        return callFunction(*function, ctx, e, f, 0);
    }

    Value callDelegate(const Expr& e, Frame& f, const ObjectPtr& ctx) {
        // [isLocal byte][delegate function ref][delegate property name] then the arguments.
        const std::string& property = e.names.at(0);
        Value delegateValue;
        if (!e.bytes.at(0) && ctx) {   // a delegate held in a local variable is not supported yet (UNVERIFIED)
            if (Value* slot = rt.property(*ctx, property)) delegateValue = *slot;
        }
        Function* function = nullptr;
        ObjectPtr target = ctx;
        if (delegateValue.kind == K::Delegate && delegateValue.o && !delegateValue.s.empty()) {
            target = delegateValue.o;
            function = rt.findMethod(target->cls, delegateValue.s, target->state);
        } else {
            function = rt.functionAt(f.function->package, e.refs.at(0));   // the delegate's own declaration/default
        }
        if (!function) { log("delegate " + property + " is not bound"); return Value(); }
        return callFunction(*function, target, e, f, 0);
    }

    Value callNative(const Expr& e, Frame& f, const ObjectPtr& ctx) {
        Function* function = rt.nativeByIndex(e.native);
        if (!function) {
            log("UNIMPLEMENTED native index " + std::to_string(e.native));
            for (const auto& kid : e.kids) if (kid.op != script::EX_EmptyParmValue) eval(kid, f, f.self);
            return Value();
        }
        // && and || skip their second operand when the first decides the result.
        const std::string& op = function->info.friendlyName;
        if ((op == "&&" || op == "||") && e.kids.size() == 2) {
            const bool first = eval(e.kids[0], f, f.self).truth();
            if (op == "&&" ? !first : first) return Value::makeBool(first);
            return Value::makeBool(eval(e.kids[1], f, f.self).truth());
        }
        return callFunction(*function, ctx, e, f, 0);
    }

    // Runs `function` with bound arguments; natives call their implementation, script functions get a frame.
    Value invoke(Function& function, const ObjectPtr& target, std::vector<NativeCall::Arg> args) {
        tick();
        if (rt.countCalls) {
            rt.countedFunctions.emplace(function.path, &function);
            if (!function.isNative()) ++rt.scriptCalls[function.path];
            else if (function.native) ++rt.nativeCalls[function.path];
            else ++rt.stubCalls[function.path];
        }
        if (function.isNative()) {
            if (function.native) {
                NativeCall call{rt, function, target, std::move(args)};
                return function.native(call);
            }
            rt.log.push_back("UNIMPLEMENTED " + (function.nativeKey.empty() ? function.path : function.nativeKey));
            // Out parameters keep their values; the result is the return type's zero value.
            if (function.result) return rt.zeroValue(*function.result);
            return Value();
        }
        if (function.decodeFailed) throw RuntimeError("cannot run " + function.path + ": " + function.decodeError);
        if (!function.code) {
            try { function.code = std::make_shared<script::Code>(script::decode(*function.package, function.info)); }
            catch (const std::exception& error) {
                function.decodeFailed = true;
                function.decodeError = error.what();
                throw RuntimeError("cannot run " + function.path + ": " + error.what());
            }
        }
        if (depth >= MAX_CALL_DEPTH) {
            std::string trace;
            for (size_t i = stack.size() > 4 ? stack.size() - 4 : 0; i < stack.size(); ++i) trace += " <- " + stack[i]->path;
            throw RuntimeError("call depth limit reached in " + function.path + trace);
        }
        struct DepthGuard {
            Interp& in;
            DepthGuard(Interp& x, const Function* f) : in(x) { ++in.depth; in.stack.push_back(f); }
            ~DepthGuard() { --in.depth; in.stack.pop_back(); }
        } guard(*this, &function);

        Frame frame;
        frame.function = &function;
        // A call with no object (static functions, or a top-level call) runs against the class default object.
        frame.self = target ? target : (function.owner ? rt.defaultsOf(function.owner) : nullptr);
        frame.code = function.code.get();
        for (size_t i = 0; i < function.params.size(); ++i) {
            Slot slot;
            if (i < args.size() && args[i].supplied) {
                slot.v = std::move(args[i].value);
                slot.alias = function.params[i].isOut() ? args[i].alias : nullptr;
            } else {
                slot.v = rt.zeroValue(function.params[i]);
                slot.omitted = true;
            }
            frame.locals[function.params[i].index] = std::move(slot);
        }
        for (const auto& local : function.locals) frame.locals[local.index].v = rt.zeroValue(local);
        if (function.result) frame.locals[function.result->index].v = rt.zeroValue(*function.result);
        run(frame);
        // Copy-out for out parameters is by alias; the result is the returned value.
        return std::move(frame.result);
    }

    // ------------------------------------------------------------------------------------ statements
    size_t target(const Frame& f, uint32_t memoryOffset) {
        const auto found = f.code->statementAt.find(memoryOffset);
        if (found == f.code->statementAt.end()) throw RuntimeError("jump to an unknown offset in " + f.function->path);
        return found->second;
    }

    void run(Frame& f) {
        const auto& statements = f.code->statements;
        size_t pc = 0;
        // Prologue: one entry per optional parameter, either Nothing or DefaultParmValue ... EndParmValue.
        std::vector<const PropertyDecl*> optional;
        for (const auto& param : f.function->params) if (param.isOptional()) optional.push_back(&param);
        for (const auto* param : optional) {
            if (pc >= statements.size()) break;
            const Expr& s = statements[pc];
            if (s.op == script::EX_DefaultParmValue) {
                Slot& slot = f.locals[param->index];
                if (slot.omitted) slot.ref() = eval(s.kids.at(0), f, f.self);
                ++pc;
                if (pc < statements.size() && statements[pc].op == script::EX_EndParmValue) ++pc;
            } else if (s.op == script::EX_Nothing) {
                ++pc;
            } else break;
        }
        while (pc < statements.size() && !f.returned) {
            const Expr& s = statements[pc];
            tick();
            switch (s.op) {
            case script::EX_EndOfScript: return;
            case script::EX_Stop: return;
            case script::EX_Return: {
                Value value = eval(s.kids.at(0), f, f.self);
                if (s.kids.at(0).op == script::EX_ReturnNothing || s.kids.at(0).op == script::EX_Nothing) {
                    if (f.function->result) value = f.locals[f.function->result->index].ref();
                }
                f.result = std::move(value);
                f.returned = true;
                return;
            }
            case script::EX_Jump: pc = target(f, s.words.at(0)); continue;
            case script::EX_JumpIfNot:
                if (!eval(s.kids.at(0), f, f.self).truth()) { pc = target(f, s.words.at(0)); continue; }
                ++pc;
                continue;
            case script::EX_Switch:
                f.switchValue = eval(s.kids.at(0), f, f.self);
                ++pc;
                continue;
            case script::EX_Case: {
                if (s.words.at(0) == 0xFFFF) { ++pc; continue; }          // default: always matches
                if (sameValue(f.switchValue, eval(s.kids.at(0), f, f.self))) { ++pc; continue; }
                pc = target(f, s.words.at(0));
                continue;
            }
            case script::EX_DynArrayIterator: {
                // [array][item][hasIndex byte][index][u16 end]: loop over a copy-free view of the array.
                IterState it;
                it.array = lval(s.kids.at(0), f, f.self);
                it.item = lval(s.kids.at(1), f, f.self);
                it.index = s.bytes.at(0) ? lval(s.kids.at(2), f, f.self) : nullptr;
                it.resume = pc + 1;
                if (it.array->kind != K::Array || it.array->elements().empty()) { pc = target(f, s.words.at(0)); continue; }
                *it.item = it.array->elements()[0];
                if (it.index) *it.index = Value::makeInt(0);
                f.iterators.push_back(it);
                ++pc;
                continue;
            }
            case script::EX_Iterator: {
                log("UNIMPLEMENTED native iterator (foreach over an engine iterator)");
                pc = target(f, s.words.at(0));
                continue;
            }
            case script::EX_IteratorNext: {
                if (f.iterators.empty()) { ++pc; continue; }
                IterState& it = f.iterators.back();
                ++it.position;
                if (it.array->kind == K::Array && it.position < it.array->elements().size()) {
                    *it.item = it.array->elements()[it.position];
                    if (it.index) *it.index = Value::makeInt(int64_t(it.position));
                    pc = it.resume;
                } else ++pc;
                continue;
            }
            case script::EX_IteratorPop:
                if (!f.iterators.empty()) f.iterators.pop_back();
                ++pc;
                continue;
            case script::EX_LabelTable: case script::EX_GotoLabel: ++pc; continue;
            default:
                eval(s, f, f.self);
                ++pc;
            }
        }
    }
};

// ------------------------------------------------------------------------------------ Runtime::call
Value Runtime::call(Function& function, ObjectPtr self, std::vector<Value> args, std::vector<Value>* outs) {
    Interp interp(*this);
    steps = 0;
    if (args.size() > function.params.size()) throw RuntimeError("too many arguments for " + function.path);
    const size_t given = args.size();
    args.resize(function.params.size());    // omitted optional parameters stay None and read as zero
    std::vector<NativeCall::Arg> bound;
    for (size_t i = 0; i < args.size(); ++i) {
        NativeCall::Arg arg;
        arg.supplied = i < given;
        if (arg.supplied) arg.value = args[i];
        if (function.params[i].isOut()) arg.alias = &args[i];   // out parameters write back into `args`
        bound.push_back(std::move(arg));
    }
    Value result = interp.invoke(function, self, std::move(bound));
    if (outs) *outs = std::move(args);
    return result;
}

Value Runtime::callByName(ObjectPtr self, const std::string& name, std::vector<Value> args) {
    if (!self || !self->cls) throw RuntimeError("callByName needs an object with a class");
    Function* function = findMethod(self->cls, name, self->state);
    if (!function) throw RuntimeError("no function " + name + " on " + self->cls->name);
    return call(*function, self, std::move(args));
}

} // namespace vm
