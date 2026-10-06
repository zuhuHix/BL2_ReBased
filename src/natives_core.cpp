#include "vm.hpp"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>

// Core natives of the VM (ROADMAP Phase 2, step 5): the operators and helpers the Core package declares.
// Keys are "Class.Name(type,...)" exactly as Runtime derives them from a native's declaration, so an
// overloaded operator binds to the right implementation by its parameter types. Semantics follow the
// language (32-bit wrapping ints, 32-bit floats); anything the original does differently in an edge case
// (formatting, rounding modes) is noted UNVERIFIED and awaits a golden trace.
namespace vm {
namespace {

std::string lowered(std::string text) {
    for (auto& c : text) c = char(std::tolower(static_cast<unsigned char>(c)));
    return text;
}

double comp(const Value& v, const char* name) {
    const Value* field = v.field(name);
    return field ? field->number() : 0.0;
}

Value makeVector(Runtime& rt, double x, double y, double z) {
    Value v = rt.zeroStruct("Vector");
    auto& a = v.mut();
    a.values[0] = Value::makeFloat(x);
    a.values[1] = Value::makeFloat(y);
    a.values[2] = Value::makeFloat(z);
    return v;
}

Value makeRotator(Runtime& rt, int64_t pitch, int64_t yaw, int64_t roll) {
    Value v = rt.zeroStruct("Rotator");
    auto& a = v.mut();
    a.values[0] = Value::makeInt(pitch);
    a.values[1] = Value::makeInt(yaw);
    a.values[2] = Value::makeInt(roll);
    return v;
}

// A rotator axis wraps into a signed 16-bit turn (65536 units = 360 degrees).
int64_t normalizeAxis(int64_t angle) {
    angle &= 0xFFFF;
    return angle > 32767 ? angle - 65536 : angle;
}

int32_t wrapInt(int64_t v) { return int32_t(uint32_t(uint64_t(v))); }

} // namespace

void Runtime::registerCoreNatives() {
    auto reg = [this](const char* key, NativeFn fn) { registerNative(key, std::move(fn)); };

    // ---------------------------------------------------------------------------------- interfaces
    // Object.QueryInterface (NATIVE_CONTROLLER_HELPERS.md, UNVERIFIED): the object itself when its class implements the interface class, else
    // None. An interface value is valid iff its object half is non-null. "Implements" is the interface table of the class or a super class
    // (Runtime::implements, NATIVE_CLASS_SERIAL_LAYOUT.md, UNVERIFIED in the running game) or a class derived from the interface.
    reg("Object.QueryInterface(class)", [](NativeCall& c) {
        Class* want = c.in(0).cls;
        if (!c.self || !c.self->cls || !want) return Value::makeObject(nullptr);
        return c.self->cls->isChildOf(want) || c.runtime.implements(c.self->cls, want) ? Value::makeObject(c.self) : Value::makeObject(nullptr);
    });

    // ---------------------------------------------------------------------------------- integers
    reg("Object.+(int,int)", [](NativeCall& c) { return Value::makeInt(wrapInt(c.in(0).i + c.in(1).i)); });
    reg("Object.-(int,int)", [](NativeCall& c) { return Value::makeInt(wrapInt(c.in(0).i - c.in(1).i)); });
    reg("Object.*(int,int)", [](NativeCall& c) { return Value::makeInt(wrapInt(c.in(0).i * c.in(1).i)); });
    reg("Object./(int,int)", [](NativeCall& c) {
        if (!c.in(1).i) { c.runtime.log.push_back("Divide by zero"); return Value::makeInt(0); }
        return Value::makeInt(wrapInt(c.in(0).i / c.in(1).i));
    });
    reg("Object.%(int,int)", [](NativeCall& c) {
        if (!c.in(1).i) { c.runtime.log.push_back("Divide by zero"); return Value::makeInt(0); }
        return Value::makeInt(wrapInt(c.in(0).i % c.in(1).i));
    });
    reg("Object.&(int,int)", [](NativeCall& c) { return Value::makeInt(c.in(0).i & c.in(1).i); });
    reg("Object.|(int,int)", [](NativeCall& c) { return Value::makeInt(c.in(0).i | c.in(1).i); });
    reg("Object.^(int,int)", [](NativeCall& c) { return Value::makeInt(c.in(0).i ^ c.in(1).i); });
    reg("Object.<<(int,int)", [](NativeCall& c) { return Value::makeInt(int32_t(uint32_t(c.in(0).i) << (c.in(1).i & 31))); });
    reg("Object.>>(int,int)", [](NativeCall& c) { return Value::makeInt(int32_t(c.in(0).i) >> (c.in(1).i & 31)); });
    reg("Object.>>>(int,int)", [](NativeCall& c) { return Value::makeInt(int32_t(uint32_t(c.in(0).i) >> (c.in(1).i & 31))); });
    reg("Object.~_pre(int)", [](NativeCall& c) { return Value::makeInt(~int32_t(c.in(0).i)); });
    reg("Object.-_pre(int)", [](NativeCall& c) { return Value::makeInt(wrapInt(-c.in(0).i)); });
    reg("Object.<(int,int)", [](NativeCall& c) { return Value::makeBool(c.in(0).i < c.in(1).i); });
    reg("Object.>(int,int)", [](NativeCall& c) { return Value::makeBool(c.in(0).i > c.in(1).i); });
    reg("Object.<=(int,int)", [](NativeCall& c) { return Value::makeBool(c.in(0).i <= c.in(1).i); });
    reg("Object.>=(int,int)", [](NativeCall& c) { return Value::makeBool(c.in(0).i >= c.in(1).i); });
    reg("Object.==(int,int)", [](NativeCall& c) { return Value::makeBool(c.in(0).i == c.in(1).i); });
    reg("Object.!=(int,int)", [](NativeCall& c) { return Value::makeBool(c.in(0).i != c.in(1).i); });
    reg("Object.Min(int,int)", [](NativeCall& c) { return Value::makeInt(std::min(c.in(0).i, c.in(1).i)); });
    reg("Object.Max(int,int)", [](NativeCall& c) { return Value::makeInt(std::max(c.in(0).i, c.in(1).i)); });
    reg("Object.Clamp(int,int,int)", [](NativeCall& c) {
        return Value::makeInt(std::min(std::max(c.in(0).i, c.in(1).i), c.in(2).i));
    });
    reg("Object.NormalizeRotAxis(int)", [](NativeCall& c) { return Value::makeInt(normalizeAxis(c.in(0).i)); });

    // Compound assignment and increment on out parameters.
    reg("Object.+=(int,int)", [](NativeCall& c) { c.out(0) = Value::makeInt(c.out(0).i + c.in(1).i); return c.out(0); });
    reg("Object.-=(int,int)", [](NativeCall& c) { c.out(0) = Value::makeInt(c.out(0).i - c.in(1).i); return c.out(0); });
    reg("Object.*=(int,float)", [](NativeCall& c) { c.out(0) = Value::makeInt(int64_t(double(c.out(0).i) * c.in(1).f)); return c.out(0); });
    reg("Object./=(int,float)", [](NativeCall& c) {
        if (c.in(1).f == 0) { c.runtime.log.push_back("Divide by zero"); return c.out(0); }
        c.out(0) = Value::makeInt(int64_t(double(c.out(0).i) / c.in(1).f));
        return c.out(0);
    });
    reg("Object.++_pre(int)", [](NativeCall& c) { c.out(0) = Value::makeInt(c.out(0).i + 1); return c.out(0); });
    reg("Object.--_pre(int)", [](NativeCall& c) { c.out(0) = Value::makeInt(c.out(0).i - 1); return c.out(0); });
    reg("Object.++(int)", [](NativeCall& c) { Value old = c.out(0); c.out(0) = Value::makeInt(old.i + 1); return old; });
    reg("Object.--(int)", [](NativeCall& c) { Value old = c.out(0); c.out(0) = Value::makeInt(old.i - 1); return old; });

    // ------------------------------------------------------------------------------------- bytes
    reg("Object.+=(byte,byte)", [](NativeCall& c) { c.out(0) = Value::makeByte(c.out(0).i + c.in(1).i); return c.out(0); });
    reg("Object.-=(byte,byte)", [](NativeCall& c) { c.out(0) = Value::makeByte(c.out(0).i - c.in(1).i); return c.out(0); });
    reg("Object.*=(byte,byte)", [](NativeCall& c) { c.out(0) = Value::makeByte(c.out(0).i * c.in(1).i); return c.out(0); });
    reg("Object./=(byte,byte)", [](NativeCall& c) {
        if (!c.in(1).i) { c.runtime.log.push_back("Divide by zero"); return c.out(0); }
        c.out(0) = Value::makeByte(c.out(0).i / c.in(1).i);
        return c.out(0);
    });
    reg("Object.*=(byte,float)", [](NativeCall& c) { c.out(0) = Value::makeByte(int64_t(double(c.out(0).i) * c.in(1).f)); return c.out(0); });
    reg("Object.++_pre(byte)", [](NativeCall& c) { c.out(0) = Value::makeByte(c.out(0).i + 1); return c.out(0); });
    reg("Object.--_pre(byte)", [](NativeCall& c) { c.out(0) = Value::makeByte(c.out(0).i - 1); return c.out(0); });
    reg("Object.++(byte)", [](NativeCall& c) { Value old = c.out(0); c.out(0) = Value::makeByte(old.i + 1); return old; });
    reg("Object.--(byte)", [](NativeCall& c) { Value old = c.out(0); c.out(0) = Value::makeByte(old.i - 1); return old; });

    // ------------------------------------------------------------------------------------ floats
    reg("Object.+(float,float)", [](NativeCall& c) { return Value::makeFloat(c.in(0).f + c.in(1).f); });
    reg("Object.-(float,float)", [](NativeCall& c) { return Value::makeFloat(c.in(0).f - c.in(1).f); });
    reg("Object.*(float,float)", [](NativeCall& c) { return Value::makeFloat(c.in(0).f * c.in(1).f); });
    reg("Object./(float,float)", [](NativeCall& c) {
        if (c.in(1).f == 0) { c.runtime.log.push_back("Divide by zero"); return Value::makeFloat(0); }
        return Value::makeFloat(c.in(0).f / c.in(1).f);
    });
    reg("Object.%(float,float)", [](NativeCall& c) {
        return Value::makeFloat(c.in(1).f == 0 ? 0.0 : std::fmod(c.in(0).f, c.in(1).f));
    });
    reg("Object.**(float,float)", [](NativeCall& c) { return Value::makeFloat(std::pow(c.in(0).f, c.in(1).f)); });
    reg("Object.-_pre(float)", [](NativeCall& c) { return Value::makeFloat(-c.in(0).f); });
    reg("Object.<(float,float)", [](NativeCall& c) { return Value::makeBool(c.in(0).f < c.in(1).f); });
    reg("Object.>(float,float)", [](NativeCall& c) { return Value::makeBool(c.in(0).f > c.in(1).f); });
    reg("Object.<=(float,float)", [](NativeCall& c) { return Value::makeBool(c.in(0).f <= c.in(1).f); });
    reg("Object.>=(float,float)", [](NativeCall& c) { return Value::makeBool(c.in(0).f >= c.in(1).f); });
    reg("Object.==(float,float)", [](NativeCall& c) { return Value::makeBool(c.in(0).f == c.in(1).f); });
    reg("Object.!=(float,float)", [](NativeCall& c) { return Value::makeBool(c.in(0).f != c.in(1).f); });
    reg("Object.~=(float,float)", [](NativeCall& c) { return Value::makeBool(std::fabs(c.in(0).f - c.in(1).f) < 0.0001); });
    reg("Object.+=(float,float)", [](NativeCall& c) { c.out(0) = Value::makeFloat(c.out(0).f + c.in(1).f); return c.out(0); });
    reg("Object.-=(float,float)", [](NativeCall& c) { c.out(0) = Value::makeFloat(c.out(0).f - c.in(1).f); return c.out(0); });
    reg("Object.*=(float,float)", [](NativeCall& c) { c.out(0) = Value::makeFloat(c.out(0).f * c.in(1).f); return c.out(0); });
    reg("Object./=(float,float)", [](NativeCall& c) {
        if (c.in(1).f == 0) { c.runtime.log.push_back("Divide by zero"); return c.out(0); }
        c.out(0) = Value::makeFloat(c.out(0).f / c.in(1).f);
        return c.out(0);
    });
    reg("Object.Abs(float)", [](NativeCall& c) { return Value::makeFloat(std::fabs(c.in(0).f)); });
    reg("Object.Sin(float)", [](NativeCall& c) { return Value::makeFloat(std::sin(c.in(0).f)); });
    reg("Object.Cos(float)", [](NativeCall& c) { return Value::makeFloat(std::cos(c.in(0).f)); });
    reg("Object.Tan(float)", [](NativeCall& c) { return Value::makeFloat(std::tan(c.in(0).f)); });
    reg("Object.Asin(float)", [](NativeCall& c) { return Value::makeFloat(std::asin(c.in(0).f)); });
    reg("Object.Acos(float)", [](NativeCall& c) { return Value::makeFloat(std::acos(c.in(0).f)); });
    reg("Object.Atan(float)", [](NativeCall& c) { return Value::makeFloat(std::atan(c.in(0).f)); });
    reg("Object.Atan2(float,float)", [](NativeCall& c) { return Value::makeFloat(std::atan2(c.in(0).f, c.in(1).f)); });
    reg("Object.Exp(float)", [](NativeCall& c) { return Value::makeFloat(std::exp(c.in(0).f)); });
    reg("Object.Loge(float)", [](NativeCall& c) { return Value::makeFloat(std::log(c.in(0).f)); });
    reg("Object.Sqrt(float)", [](NativeCall& c) { return Value::makeFloat(std::sqrt(c.in(0).f)); });
    reg("Object.Square(float)", [](NativeCall& c) { return Value::makeFloat(c.in(0).f * c.in(0).f); });
    reg("Object.Round(float)", [](NativeCall& c) { return Value::makeInt(int64_t(std::floor(c.in(0).f + 0.5))); });
    reg("Object.FCeil(float)", [](NativeCall& c) { return Value::makeInt(int64_t(std::ceil(c.in(0).f))); });
    reg("Object.FFloor(float)", [](NativeCall& c) { return Value::makeInt(int64_t(std::floor(c.in(0).f))); });
    reg("Object.FMin(float,float)", [](NativeCall& c) { return Value::makeFloat(std::min(c.in(0).f, c.in(1).f)); });
    reg("Object.FMax(float,float)", [](NativeCall& c) { return Value::makeFloat(std::max(c.in(0).f, c.in(1).f)); });
    reg("Object.FClamp(float,float,float)", [](NativeCall& c) {
        return Value::makeFloat(std::min(std::max(c.in(0).f, c.in(1).f), c.in(2).f));
    });
    reg("Object.Lerp(float,float,float)", [](NativeCall& c) {
        return Value::makeFloat(c.in(0).f + c.in(2).f * (c.in(1).f - c.in(0).f));
    });
    reg("Object.FCubicInterp(float,float,float,float,float)", [](NativeCall& c) {
        // Cubic Hermite interpolation of P0/T0 to P1/T1 at A.
        const double p0 = c.in(0).f, t0 = c.in(1).f, p1 = c.in(2).f, t1 = c.in(3).f, a = c.in(4).f;
        const double a2 = a * a, a3 = a2 * a;
        return Value::makeFloat((2 * a3 - 3 * a2 + 1) * p0 + (a3 - 2 * a2 + a) * t0 + (a3 - a2) * t1 + (-2 * a3 + 3 * a2) * p1);
    });
    reg("Object.FInterpTo(float,float,float,float)", [](NativeCall& c) {
        const double current = c.in(0).f, target = c.in(1).f, delta = c.in(2).f, speed = c.in(3).f;
        if (speed <= 0) return Value::makeFloat(target);
        const double distance = target - current;
        if (distance * distance < 1e-8) return Value::makeFloat(target);
        return Value::makeFloat(current + distance * std::clamp(delta * speed, 0.0, 1.0));
    });
    reg("Object.FInterpConstantTo(float,float,float,float)", [](NativeCall& c) {
        const double current = c.in(0).f, target = c.in(1).f, delta = c.in(2).f, speed = c.in(3).f;
        const double distance = target - current;
        if (distance * distance < 1e-8) return Value::makeFloat(target);
        const double step = speed * delta;
        return Value::makeFloat(current + std::clamp(distance, -step, step));
    });
    // Deterministic generator: a real seed source belongs to the engine host, not to this VM.
    reg("Object.Rand(int)", [](NativeCall& c) {
        static uint32_t state = 12345;
        state = state * 1664525u + 1013904223u;
        const int64_t limit = c.in(0).i;
        return Value::makeInt(limit > 0 ? int64_t((state >> 8) % uint32_t(limit)) : 0);
    });
    reg("Object.FRand()", [](NativeCall&) {
        static uint32_t state = 54321;
        state = state * 1664525u + 1013904223u;
        return Value::makeFloat(double(state >> 8) / double(1 << 24));
    });

    // -------------------------------------------------------------------------- booleans and names
    reg("Object.!_pre(bool)", [](NativeCall& c) { return Value::makeBool(!c.in(0).truth()); });
    reg("Object.&&(bool,bool)", [](NativeCall& c) { return Value::makeBool(c.in(0).truth() && c.in(1).truth()); });
    reg("Object.||(bool,bool)", [](NativeCall& c) { return Value::makeBool(c.in(0).truth() || c.in(1).truth()); });
    reg("Object.^^(bool,bool)", [](NativeCall& c) { return Value::makeBool(c.in(0).truth() != c.in(1).truth()); });
    reg("Object.==(bool,bool)", [](NativeCall& c) { return Value::makeBool(c.in(0).truth() == c.in(1).truth()); });
    reg("Object.!=(bool,bool)", [](NativeCall& c) { return Value::makeBool(c.in(0).truth() != c.in(1).truth()); });
    reg("Object.==(name,name)", [](NativeCall& c) { return Value::makeBool(lowered(c.in(0).s) == lowered(c.in(1).s)); });
    reg("Object.!=(name,name)", [](NativeCall& c) { return Value::makeBool(lowered(c.in(0).s) != lowered(c.in(1).s)); });
    reg("Object.==(object,object)", [](NativeCall& c) { return Value::makeBool(sameValue(c.in(0), c.in(1))); });
    reg("Object.!=(object,object)", [](NativeCall& c) { return Value::makeBool(!sameValue(c.in(0), c.in(1))); });
    reg("Object.==(interface,interface)", [](NativeCall& c) { return Value::makeBool(sameValue(c.in(0), c.in(1))); });
    reg("Object.!=(interface,interface)", [](NativeCall& c) { return Value::makeBool(!sameValue(c.in(0), c.in(1))); });

    // ------------------------------------------------------------------------------------ strings
    reg("Object.$(string,string)", [](NativeCall& c) { return Value::makeString(c.in(0).s + c.in(1).s); });
    reg("Object.@(string,string)", [](NativeCall& c) { return Value::makeString(c.in(0).s + " " + c.in(1).s); });
    reg("Object.$=(string,string)", [](NativeCall& c) { c.out(0).s += c.in(1).s; c.out(0).kind = Value::Kind::String; return c.out(0); });
    reg("Object.@=(string,string)", [](NativeCall& c) { c.out(0).s += " " + c.in(1).s; c.out(0).kind = Value::Kind::String; return c.out(0); });
    reg("Object.-=(string,string)", [](NativeCall& c) {
        // Removes every occurrence of the second string (UE3 definition, UNVERIFIED for overlapping matches).
        std::string& text = c.out(0).s;
        const std::string& cut = c.in(1).s;
        if (!cut.empty()) for (size_t at = text.find(cut); at != std::string::npos; at = text.find(cut, at)) text.erase(at, cut.size());
        c.out(0).kind = Value::Kind::String;
        return c.out(0);
    });
    reg("Object.<(string,string)", [](NativeCall& c) { return Value::makeBool(c.in(0).s < c.in(1).s); });
    reg("Object.>(string,string)", [](NativeCall& c) { return Value::makeBool(c.in(0).s > c.in(1).s); });
    reg("Object.<=(string,string)", [](NativeCall& c) { return Value::makeBool(c.in(0).s <= c.in(1).s); });
    reg("Object.>=(string,string)", [](NativeCall& c) { return Value::makeBool(c.in(0).s >= c.in(1).s); });
    reg("Object.==(string,string)", [](NativeCall& c) { return Value::makeBool(c.in(0).s == c.in(1).s); });
    reg("Object.!=(string,string)", [](NativeCall& c) { return Value::makeBool(c.in(0).s != c.in(1).s); });
    reg("Object.~=(string,string)", [](NativeCall& c) { return Value::makeBool(lowered(c.in(0).s) == lowered(c.in(1).s)); });
    reg("Object.Len(string)", [](NativeCall& c) { return Value::makeInt(int64_t(c.in(0).s.size())); });
    reg("Object.Left(string,int)", [](NativeCall& c) {
        const auto n = size_t(std::clamp<int64_t>(c.in(1).i, 0, int64_t(c.in(0).s.size())));
        return Value::makeString(c.in(0).s.substr(0, n));
    });
    reg("Object.Right(string,int)", [](NativeCall& c) {
        const auto& s = c.in(0).s;
        const auto n = size_t(std::clamp<int64_t>(c.in(1).i, 0, int64_t(s.size())));
        return Value::makeString(s.substr(s.size() - n));
    });
    reg("Object.Mid(string,int,int)", [](NativeCall& c) {
        const auto& s = c.in(0).s;
        const auto start = size_t(std::clamp<int64_t>(c.in(1).i, 0, int64_t(s.size())));
        const int64_t count = c.has(2) ? c.in(2).i : int64_t(s.size());
        return Value::makeString(s.substr(start, size_t(std::max<int64_t>(0, count))));
    });
    reg("Object.Caps(string)", [](NativeCall& c) {
        std::string s = c.in(0).s;
        for (auto& ch : s) ch = char(std::toupper(static_cast<unsigned char>(ch)));
        return Value::makeString(s);
    });
    reg("Object.Locs(string)", [](NativeCall& c) { return Value::makeString(lowered(c.in(0).s)); });
    reg("Object.Chr(int)", [](NativeCall& c) { return Value::makeString(std::string(1, char(c.in(0).i))); });
    reg("Object.Asc(string)", [](NativeCall& c) { return Value::makeInt(c.in(0).s.empty() ? 0 : uint8_t(c.in(0).s[0])); });
    reg("Object.InStr(string,string,bool,bool,int)", [](NativeCall& c) {
        std::string hay = c.in(0).s, needle = c.in(1).s;
        const bool fromRight = c.has(2) && c.in(2).truth(), ignoreCase = c.has(3) && c.in(3).truth();
        const int64_t startPos = c.has(4) ? c.in(4).i : 0;
        if (ignoreCase) { hay = lowered(hay); needle = lowered(needle); }
        size_t at;
        if (fromRight) at = hay.rfind(needle);
        else at = hay.find(needle, size_t(std::max<int64_t>(0, startPos)));
        return Value::makeInt(at == std::string::npos ? -1 : int64_t(at));
    });
    reg("Object.ParseStringIntoArray(string,array,string,bool)", [](NativeCall& c) {
        // Splits the text at every delimiter; bCullEmpty drops empty pieces. The result replaces the array.
        const std::string& text = c.in(0).s;
        const std::string& delimiter = c.in(2).s;
        const bool cullEmpty = c.has(3) && c.in(3).truth();
        Value pieces = Value::makeArray();
        const auto add = [&](std::string piece) {
            if (!cullEmpty || !piece.empty()) pieces.elements().push_back(Value::makeString(std::move(piece)));
        };
        if (delimiter.empty()) add(text);
        else {
            size_t from = 0;
            for (size_t at = text.find(delimiter); at != std::string::npos; at = text.find(delimiter, from)) {
                add(text.substr(from, at - from));
                from = at + delimiter.size();
            }
            add(text.substr(from));
        }
        c.out(1) = std::move(pieces);
        return Value::makeInt(int64_t(c.out(1).elements().size()));
    });
    reg("Object.Repl(string,string,string,bool)", [](NativeCall& c) {
        std::string text = c.in(0).s;
        const std::string& match = c.in(1).s;
        const std::string& with = c.in(2).s;
        const bool caseSensitive = !c.has(3) || c.in(3).truth();
        if (match.empty()) return Value::makeString(text);
        const std::string hay = caseSensitive ? text : lowered(text), needle = caseSensitive ? match : lowered(match);
        std::string out;
        size_t from = 0;
        for (size_t at = hay.find(needle); at != std::string::npos; at = hay.find(needle, from)) {
            out += text.substr(from, at - from) + with;
            from = at + needle.size();
        }
        out += text.substr(from);
        return Value::makeString(out);
    });

    // ------------------------------------------------------------------------------------ vectors
    reg("Object.+(vector,vector)", [](NativeCall& c) {
        return makeVector(c.runtime, comp(c.in(0), "X") + comp(c.in(1), "X"), comp(c.in(0), "Y") + comp(c.in(1), "Y"),
                          comp(c.in(0), "Z") + comp(c.in(1), "Z"));
    });
    reg("Object.-(vector,vector)", [](NativeCall& c) {
        return makeVector(c.runtime, comp(c.in(0), "X") - comp(c.in(1), "X"), comp(c.in(0), "Y") - comp(c.in(1), "Y"),
                          comp(c.in(0), "Z") - comp(c.in(1), "Z"));
    });
    reg("Object.*(vector,vector)", [](NativeCall& c) {
        return makeVector(c.runtime, comp(c.in(0), "X") * comp(c.in(1), "X"), comp(c.in(0), "Y") * comp(c.in(1), "Y"),
                          comp(c.in(0), "Z") * comp(c.in(1), "Z"));
    });
    reg("Object.*(vector,float)", [](NativeCall& c) {
        const double s = c.in(1).f;
        return makeVector(c.runtime, comp(c.in(0), "X") * s, comp(c.in(0), "Y") * s, comp(c.in(0), "Z") * s);
    });
    reg("Object.*(float,vector)", [](NativeCall& c) {
        const double s = c.in(0).f;
        return makeVector(c.runtime, comp(c.in(1), "X") * s, comp(c.in(1), "Y") * s, comp(c.in(1), "Z") * s);
    });
    reg("Object./(vector,float)", [](NativeCall& c) {
        const double s = c.in(1).f == 0 ? 0.0 : 1.0 / c.in(1).f;
        return makeVector(c.runtime, comp(c.in(0), "X") * s, comp(c.in(0), "Y") * s, comp(c.in(0), "Z") * s);
    });
    reg("Object.-_pre(vector)", [](NativeCall& c) {
        return makeVector(c.runtime, -comp(c.in(0), "X"), -comp(c.in(0), "Y"), -comp(c.in(0), "Z"));
    });
    reg("Object.==(vector,vector)", [](NativeCall& c) { return Value::makeBool(sameValue(c.in(0), c.in(1))); });
    reg("Object.!=(vector,vector)", [](NativeCall& c) { return Value::makeBool(!sameValue(c.in(0), c.in(1))); });
    reg("Object.Dot(vector,vector)", [](NativeCall& c) {
        return Value::makeFloat(comp(c.in(0), "X") * comp(c.in(1), "X") + comp(c.in(0), "Y") * comp(c.in(1), "Y") +
                                comp(c.in(0), "Z") * comp(c.in(1), "Z"));
    });
    reg("Object.Cross(vector,vector)", [](NativeCall& c) {
        const double ax = comp(c.in(0), "X"), ay = comp(c.in(0), "Y"), az = comp(c.in(0), "Z");
        const double bx = comp(c.in(1), "X"), by = comp(c.in(1), "Y"), bz = comp(c.in(1), "Z");
        return makeVector(c.runtime, ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx);
    });
    reg("Object.VSize(vector)", [](NativeCall& c) {
        const double x = comp(c.in(0), "X"), y = comp(c.in(0), "Y"), z = comp(c.in(0), "Z");
        return Value::makeFloat(std::sqrt(x * x + y * y + z * z));
    });
    reg("Object.VSizeSq(vector)", [](NativeCall& c) {
        const double x = comp(c.in(0), "X"), y = comp(c.in(0), "Y"), z = comp(c.in(0), "Z");
        return Value::makeFloat(x * x + y * y + z * z);
    });
    reg("Object.Normal(vector)", [](NativeCall& c) {
        const double x = comp(c.in(0), "X"), y = comp(c.in(0), "Y"), z = comp(c.in(0), "Z");
        const double length = std::sqrt(x * x + y * y + z * z);
        if (length <= 1e-8) return makeVector(c.runtime, 0, 0, 0);
        return makeVector(c.runtime, x / length, y / length, z / length);
    });
    reg("Object.IsZero(vector)", [](NativeCall& c) {
        return Value::makeBool(comp(c.in(0), "X") == 0 && comp(c.in(0), "Y") == 0 && comp(c.in(0), "Z") == 0);
    });
    reg("Object.+=(vector,vector)", [](NativeCall& c) {
        Value& v = c.out(0);
        v = makeVector(c.runtime, comp(v, "X") + comp(c.in(1), "X"), comp(v, "Y") + comp(c.in(1), "Y"), comp(v, "Z") + comp(c.in(1), "Z"));
        return v;
    });
    reg("Object.-=(vector,vector)", [](NativeCall& c) {
        Value& v = c.out(0);
        v = makeVector(c.runtime, comp(v, "X") - comp(c.in(1), "X"), comp(v, "Y") - comp(c.in(1), "Y"), comp(v, "Z") - comp(c.in(1), "Z"));
        return v;
    });
    reg("Object.*=(vector,float)", [](NativeCall& c) {
        Value& v = c.out(0);
        const double s = c.in(1).f;
        v = makeVector(c.runtime, comp(v, "X") * s, comp(v, "Y") * s, comp(v, "Z") * s);
        return v;
    });
    reg("Object./=(vector,float)", [](NativeCall& c) {
        Value& v = c.out(0);
        const double s = c.in(1).f == 0 ? 0.0 : 1.0 / c.in(1).f;
        v = makeVector(c.runtime, comp(v, "X") * s, comp(v, "Y") * s, comp(v, "Z") * s);
        return v;
    });

    // ----------------------------------------------------------------------------------- rotators
    reg("Object.==(rotator,rotator)", [](NativeCall& c) {
        const auto same = [&](const char* n) { return (int64_t(comp(c.in(0), n)) & 0xFFFF) == (int64_t(comp(c.in(1), n)) & 0xFFFF); };
        return Value::makeBool(same("Pitch") && same("Yaw") && same("Roll"));
    });
    reg("Object.!=(rotator,rotator)", [](NativeCall& c) {
        const auto same = [&](const char* n) { return (int64_t(comp(c.in(0), n)) & 0xFFFF) == (int64_t(comp(c.in(1), n)) & 0xFFFF); };
        return Value::makeBool(!(same("Pitch") && same("Yaw") && same("Roll")));
    });
    reg("Object.+(rotator,rotator)", [](NativeCall& c) {
        return makeRotator(c.runtime, int64_t(comp(c.in(0), "Pitch")) + int64_t(comp(c.in(1), "Pitch")),
                           int64_t(comp(c.in(0), "Yaw")) + int64_t(comp(c.in(1), "Yaw")),
                           int64_t(comp(c.in(0), "Roll")) + int64_t(comp(c.in(1), "Roll")));
    });
    reg("Object.-(rotator,rotator)", [](NativeCall& c) {
        return makeRotator(c.runtime, int64_t(comp(c.in(0), "Pitch")) - int64_t(comp(c.in(1), "Pitch")),
                           int64_t(comp(c.in(0), "Yaw")) - int64_t(comp(c.in(1), "Yaw")),
                           int64_t(comp(c.in(0), "Roll")) - int64_t(comp(c.in(1), "Roll")));
    });
    reg("Object.*(rotator,float)", [](NativeCall& c) {
        const double s = c.in(1).f;
        return makeRotator(c.runtime, int64_t(comp(c.in(0), "Pitch") * s), int64_t(comp(c.in(0), "Yaw") * s), int64_t(comp(c.in(0), "Roll") * s));
    });
    reg("Object.*(float,rotator)", [](NativeCall& c) {
        const double s = c.in(0).f;
        return makeRotator(c.runtime, int64_t(comp(c.in(1), "Pitch") * s), int64_t(comp(c.in(1), "Yaw") * s), int64_t(comp(c.in(1), "Roll") * s));
    });
    reg("Object.Normalize(rotator)", [](NativeCall& c) {
        return makeRotator(c.runtime, normalizeAxis(int64_t(comp(c.in(0), "Pitch"))), normalizeAxis(int64_t(comp(c.in(0), "Yaw"))),
                           normalizeAxis(int64_t(comp(c.in(0), "Roll"))));
    });

    // GetAxes / GetUnAxes: the rotator's three axes (X forward, Y right, Z up) and their inverse transform.
    // The rotation matrix is the standard UE3 one (UNVERIFIED against the game; yaw-only results are exact).
    const auto axes = [](NativeCall& c, bool inverse) {
        const double k = 3.14159265358979323846 / 32768.0;
        const double pitch = comp(c.in(0), "Pitch") * k, yaw = comp(c.in(0), "Yaw") * k, roll = comp(c.in(0), "Roll") * k;
        const double cp = std::cos(pitch), sp = std::sin(pitch), cy = std::cos(yaw), sy = std::sin(yaw), cr = std::cos(roll), sr = std::sin(roll);
        double m[3][3] = {{cp * cy, cp * sy, sp},
                          {sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp},
                          {-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp}};
        if (inverse) std::swap(m[0][1], m[1][0]), std::swap(m[0][2], m[2][0]), std::swap(m[1][2], m[2][1]);
        for (int row = 0; row < 3; ++row) c.out(size_t(row) + 1) = makeVector(c.runtime, m[row][0], m[row][1], m[row][2]);
        return Value();
    };
    reg("Object.GetAxes(rotator,vector,vector,vector)", [axes](NativeCall& c) { return axes(c, false); });
    reg("Object.GetUnAxes(rotator,vector,vector,vector)", [axes](NativeCall& c) { return axes(c, true); });

    // ----------------------------------------------------------------------- objects, classes, state
    reg("Object.IsA(name)", [](NativeCall& c) {
        if (!c.self || !c.self->cls) return Value::makeBool(false);
        const auto wanted = lowered(c.in(0).s);
        for (const Class* cursor = c.self->cls; cursor; cursor = cursor->super)
            if (lowered(cursor->name) == wanted) return Value::makeBool(true);
        return Value::makeBool(false);
    });
    reg("Object.ClassIsChildOf(class,class)", [](NativeCall& c) {
        const Class* a = c.in(0).cls;
        const Class* b = c.in(1).cls;
        return Value::makeBool(a && b && a->isChildOf(b));
    });
    reg("Object.GotoState(name,name,bool,bool)", [](NativeCall& c) {
        // Latent state code is not run yet (UNVERIFIED); the state name is tracked so state-specific
        // function overrides are selected.
        if (c.self && c.has(0)) {
            const std::string state = c.in(0).s;
            c.self->state = lowered(state) == "none" ? std::string() : state;
        }
        return Value();
    });
    reg("Object.GetStateName()", [](NativeCall& c) {
        return Value::makeName(c.self && !c.self->state.empty() ? c.self->state : "None");
    });
    reg("Object.IsInState(name,bool)", [](NativeCall& c) {
        return Value::makeBool(c.self && lowered(c.self->state) == lowered(c.in(0).s));
    });
    reg("Object.Enable(name)", [](NativeCall&) { return Value(); });
    reg("Object.Disable(name)", [](NativeCall&) { return Value(); });
    reg("Object.LogInternal(string,name)", [](NativeCall& c) {
        c.runtime.log.push_back("Log: " + c.in(0).s);
        return Value();
    });
    reg("Object.WarnInternal(string)", [](NativeCall& c) {
        c.runtime.log.push_back("Warning: " + c.in(0).s);
        return Value();
    });
}

} // namespace vm
