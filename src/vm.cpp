#include "vm.hpp"

#include <algorithm>
#include <bit>
#include <cctype>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <sstream>
#include <unordered_set>

namespace vm {
namespace {

std::string lower(std::string_view text) {
    std::string out(text);
    for (auto& c : out) c = char(std::tolower(static_cast<unsigned char>(c)));
    return out;
}

bool endsWith(const std::string& text, const char* suffix) {
    const size_t n = std::strlen(suffix);
    return text.size() >= n && text.compare(text.size() - n, n, suffix) == 0;
}

uint32_t u32At(const Package& package, size_t at) {
    if (at + 4 > package.data.size()) throw RuntimeError("read past the package data");
    uint32_t v = 0;
    for (unsigned i = 0; i < 4; ++i) v |= uint32_t(package.data[at + i]) << (8 * i);
    return v;
}

// Native layouts of the structs the package format stores without property tags (see Package::structure).
struct StructLayout { std::vector<const char*> fields; char kind; };  // f = float, i = int32, b = byte
const StructLayout* nativeStruct(const std::string& name) {
    static const std::map<std::string, StructLayout> table = {
        {"Vector", {{"X", "Y", "Z"}, 'f'}}, {"Vector2D", {{"X", "Y"}, 'f'}},
        {"Rotator", {{"Pitch", "Yaw", "Roll"}, 'i'}}, {"Guid", {{"A", "B", "C", "D"}, 'i'}},
        {"LinearColor", {{"R", "G", "B", "A"}, 'f'}}, {"Color", {{"B", "G", "R", "A"}, 'b'}},
        {"Quat", {{"X", "Y", "Z", "W"}, 'f'}}, {"Plane", {{"W", "X", "Y", "Z"}, 'f'}},
    };
    const auto found = table.find(name);
    return found == table.end() ? nullptr : &found->second;
}

} // namespace

// ------------------------------------------------------------------------------------------ Value
Value Value::makeInt(int64_t v) { Value r; r.kind = Kind::Int; r.i = int32_t(v); return r; }
Value Value::makeFloat(double v) { Value r; r.kind = Kind::Float; r.f = double(float(v)); return r; }
Value Value::makeBool(bool v) { Value r; r.kind = Kind::Bool; r.i = v ? 1 : 0; return r; }
Value Value::makeByte(int64_t v) { Value r; r.kind = Kind::Byte; r.i = uint8_t(v); return r; }
Value Value::makeString(std::string v) { Value r; r.kind = Kind::String; r.s = std::move(v); return r; }
Value Value::makeName(std::string v) { Value r; r.kind = Kind::Name; r.s = std::move(v); return r; }
Value Value::makeObject(ObjectPtr v) { Value r; r.kind = Kind::Object; r.o = std::move(v); return r; }
Value Value::makeClass(Class* v) { Value r; r.kind = Kind::Class; r.cls = v; return r; }
Value Value::makeArray() { Value r; r.kind = Kind::Array; r.agg = std::make_shared<Aggregate>(); return r; }
Value Value::makeStruct(std::string typeName) {
    Value r;
    r.kind = Kind::Struct;
    r.agg = std::make_shared<Aggregate>();
    r.agg->typeName = std::move(typeName);
    return r;
}

bool Value::truth() const {
    switch (kind) {
    case Kind::Int: case Kind::Bool: case Kind::Byte: return i != 0;
    case Kind::Float: return f != 0;
    case Kind::Object: return bool(o);
    case Kind::Class: return cls != nullptr;
    case Kind::String: return !s.empty();
    case Kind::Name: return !s.empty() && lower(s) != "none";
    default: return false;
    }
}

double Value::number() const { return kind == Kind::Float ? f : double(i); }
int64_t Value::integer() const { return kind == Kind::Float ? int64_t(f) : i; }

Aggregate& Value::mut() {
    if (!agg) agg = std::make_shared<Aggregate>();
    else if (agg.use_count() > 1) agg = std::make_shared<Aggregate>(*agg);
    return *agg;
}

Value* Value::field(const std::string& name) {
    if (!agg) return nullptr;
    const auto key = lower(name);
    for (size_t index = 0; index < agg->names.size(); ++index)
        if (lower(agg->names[index]) == key) return &mut().values[index];
    return nullptr;
}

const Value* Value::field(const std::string& name) const {
    if (!agg) return nullptr;
    const auto key = lower(name);
    for (size_t index = 0; index < agg->names.size(); ++index)
        if (lower(agg->names[index]) == key) return &agg->values[index];
    return nullptr;
}

std::vector<Value>& Value::elements() { return mut().values; }
const std::vector<Value>& Value::elements() const {
    static const std::vector<Value> empty;
    return agg ? agg->values : empty;
}

std::string Value::describe() const {
    std::ostringstream out;
    switch (kind) {
    case Kind::None: return "None";
    case Kind::Int: case Kind::Byte: return std::to_string(i);
    case Kind::Bool: return i ? "true" : "false";
    case Kind::Float: out << f; return out.str();
    case Kind::String: return "\"" + s + "\"";
    case Kind::Name: return "'" + s + "'";
    case Kind::Object: return o ? (o->cls ? o->cls->name : "Object") + "'" + o->name + "'" : "None";
    case Kind::Class: return cls ? "Class'" + cls->name + "'" : "None";
    case Kind::Delegate: return "Delegate(" + (o ? o->name : std::string("None")) + "." + s + ")";
    case Kind::Array:
        out << '[';
        for (size_t index = 0; index < elements().size(); ++index)
            out << (index ? "," : "") << elements()[index].describe();
        out << ']';
        return out.str();
    case Kind::Struct:
        out << (agg ? agg->typeName : std::string()) << '{';
        if (agg)
            for (size_t index = 0; index < agg->names.size(); ++index)
                out << (index ? "," : "") << agg->names[index] << '=' << agg->values[index].describe();
        out << '}';
        return out.str();
    }
    return "?";
}

bool sameValue(const Value& a, const Value& b) {
    using K = Value::Kind;
    if (a.kind == K::Float || b.kind == K::Float) return a.number() == b.number();
    switch (a.kind) {
    case K::Int: case K::Bool: case K::Byte: return a.i == b.i;
    case K::String: return a.s == b.s;
    case K::Name: return lower(a.s) == lower(b.s);
    case K::Object: return a.o == b.o;
    case K::Class: return a.cls == b.cls;
    case K::Struct: case K::Array: {
        const auto& x = a.elements();
        const auto& y = b.elements();
        if (a.kind == K::Struct) {
            if (!a.agg || !b.agg || a.agg->values.size() != b.agg->values.size()) return false;
            for (size_t index = 0; index < a.agg->values.size(); ++index)
                if (!sameValue(a.agg->values[index], b.agg->values[index])) return false;
            return true;
        }
        if (x.size() != y.size()) return false;
        for (size_t index = 0; index < x.size(); ++index)
            if (!sameValue(x[index], y[index])) return false;
        return true;
    }
    case K::None: return b.kind == K::None || (b.kind == K::Object && !b.o);
    default: return false;
    }
}

bool Class::isChildOf(const Class* other) const {
    for (const Class* cursor = this; cursor; cursor = cursor->super)
        if (cursor == other) return true;
    return false;
}

Value* Object::find(const std::string& property) {
    const auto found = props.find(lower(property));
    return found == props.end() ? nullptr : &found->second;
}

// ---------------------------------------------------------------------------------------- Runtime
Runtime::Runtime(PackageStore& store) : store_(store) {}

std::shared_ptr<const Package> Runtime::package(const std::string& name) { return store_.loadPackage(name); }

namespace {
struct Resolved { std::shared_ptr<const Package> package; int32_t index = 0; };

Resolved resolveRef(PackageStore& store, const std::shared_ptr<const Package>& package, int32_t ref) {
    if (!ref) return {};
    if (ref > 0) {
        package->object(ref);
        return {package, ref};
    }
    const auto resolved = store.resolve(package, ref);
    return {resolved.package, resolved.index};
}

const ::Object* exportOf(const Package& package, int32_t index) {
    return index > 0 && size_t(index) <= package.exports.size() ? &package.exports[size_t(index) - 1] : nullptr;
}

std::string classNameOf(const Package& package, int32_t index) {
    const auto* object = exportOf(package, index);
    if (!object || !object->cls) return "Class";
    try { return package.object(object->cls).name; } catch (const std::exception&) { return "?"; }
}
} // namespace

PropertyDecl Runtime::readProperty(const std::shared_ptr<const Package>& package, int32_t index) {
    const auto* object = exportOf(*package, index);
    if (!object || object->size < 40) throw RuntimeError("not a property export");
    PropertyDecl decl;
    decl.name = object->name;
    decl.type = classNameOf(*package, index);
    decl.package = package;
    decl.index = index;
    const size_t at = size_t(object->offset);
    decl.arrayDim = std::max<int32_t>(1, int32_t(u32At(*package, at + 16) & 0xFFFF));
    decl.flags = uint64_t(u32At(*package, at + 20)) | (uint64_t(u32At(*package, at + 24)) << 32);
    const size_t typeAt = at + 40 + ((decl.flags & 0x20) ? 2 : 0);
    if (typeAt + 4 <= at + size_t(object->size)) {
        decl.typeRef = int32_t(u32At(*package, typeAt));
        if (typeAt + 8 <= at + size_t(object->size)) decl.typeRef2 = int32_t(u32At(*package, typeAt + 4));
    }
    return decl;
}

// A function's (or class's, or struct's) child properties in declaration order. Children are exported in
// reverse declaration order and linked by Next; descending export index reproduces the chain (checked over
// every function of Core and Engine: 5,980 of 5,984).
std::vector<PropertyDecl> Runtime::childProperties(const std::shared_ptr<const Package>& package, int32_t outer) {
    std::vector<int32_t> found;
    for (int32_t index = 1; size_t(index) <= package->exports.size(); ++index) {
        const auto& object = package->exports[size_t(index) - 1];
        if (object.outer == outer && endsWith(classNameOf(*package, index), "Property")) found.push_back(index);
    }
    std::sort(found.begin(), found.end(), std::greater<>());
    std::vector<PropertyDecl> result;
    for (const auto index : found) {
        try { result.push_back(readProperty(package, index)); }
        catch (const std::exception&) { /* a property-class default object, not a declaration */ }
    }
    return result;
}

const StructDef* Runtime::structAt(const std::shared_ptr<const Package>& package, int32_t ref) {
    const auto target = resolveRef(store_, package, ref);
    if (!target.package) return nullptr;
    const auto key = std::make_pair(target.package.get(), target.index);
    const auto cached = structs_.find(key);
    if (cached != structs_.end()) return cached->second.get();
    auto def = std::make_unique<StructDef>();
    def->package = target.package;
    def->index = target.index;
    def->name = target.package->exports[size_t(target.index) - 1].name;
    // A struct's fields are its child properties; a struct with a parent struct inherits its fields first.
    const auto& exportObject = target.package->exports[size_t(target.index) - 1];
    if (exportObject.super) {
        const auto* parent = structAt(target.package, exportObject.super);
        if (parent) def->fields = parent->fields;
    }
    for (auto& field : childProperties(target.package, target.index)) def->fields.push_back(std::move(field));
    const auto* result = def.get();
    structs_[key] = std::move(def);
    return result;
}

Value Runtime::zeroStruct(const std::string& name) {
    Value value = Value::makeStruct(name);
    if (const auto* layout = nativeStruct(name)) {
        auto& aggregate = value.mut();
        for (const char* field : layout->fields) {
            aggregate.names.push_back(field);
            aggregate.values.push_back(layout->kind == 'f' ? Value::makeFloat(0) : layout->kind == 'i' ? Value::makeInt(0)
                                                                                                     : Value::makeByte(0));
        }
    }
    return value;
}

Value Runtime::zeroOfDef(const StructDef* def) {
    Value value = zeroStruct(def ? def->name : "");
    if (def && !nativeStruct(def->name)) {
        auto& aggregate = value.mut();
        for (const auto& field : def->fields) {
            aggregate.names.push_back(field.name);
            aggregate.values.push_back(zeroValue(field));
        }
    }
    return value;
}

Value Runtime::newStruct(const std::string& path) {
    const auto dot = path.find('.');
    if (dot == std::string::npos) throw RuntimeError("struct path must be Package.Struct: " + path);
    auto pkg = package(path.substr(0, dot));
    const auto index = pkg->findExport(path.substr(dot + 1));
    if (!index) throw RuntimeError("struct not found: " + path);
    return zeroOfDef(structAt(pkg, index));
}

Value Runtime::zeroValue(const PropertyDecl& decl) {
    const auto scalar = [&]() -> Value {
        const auto& t = decl.type;
        if (t == "IntProperty" || t == "IntAttributeProperty") return Value::makeInt(0);
        if (t == "FloatProperty" || t == "FloatAttributeProperty") return Value::makeFloat(0);
        if (t == "BoolProperty") return Value::makeBool(false);
        if (t == "ByteProperty" || t == "ByteAttributeProperty") return Value::makeByte(0);
        if (t == "StrProperty") return Value::makeString("");
        if (t == "NameProperty") return Value::makeName("None");
        if (t == "ArrayProperty") return Value::makeArray();
        if (t == "ClassProperty") return Value::makeClass(nullptr);
        if (t == "StructProperty") return zeroOfDef(structAt(decl.package, decl.typeRef));
        if (t == "ObjectProperty" || t == "ComponentProperty" || t == "InterfaceProperty") return Value::makeObject(nullptr);
        if (t == "DelegateProperty") { Value v; v.kind = Value::Kind::Delegate; return v; }
        return Value();
    };
    Value value = scalar();
    if (decl.arrayDim > 1 && decl.type != "ArrayProperty") {
        Value array = Value::makeArray();
        for (int32_t index = 0; index < decl.arrayDim; ++index) array.elements().push_back(value);
        return array;
    }
    return value;
}

Class* Runtime::classAt(const std::shared_ptr<const Package>& package, int32_t index) {
    const auto target = resolveRef(store_, package, index);
    if (!target.package) return nullptr;
    const auto key = std::make_pair(target.package.get(), target.index);
    const auto cached = classByExport_.find(key);
    if (cached != classByExport_.end()) return cached->second;
    return loadClass(target.package, target.index);
}

Class* Runtime::findClass(const std::string& path) {
    const auto dot = path.find('.');
    if (dot == std::string::npos) throw RuntimeError("class path must be Package.Class: " + path);
    auto pkg = package(path.substr(0, dot));
    const auto index = pkg->findExport(path.substr(dot + 1));
    if (!index) throw RuntimeError("class not found: " + path);
    return classAt(pkg, index);
}

Class* Runtime::loadClass(const std::shared_ptr<const Package>& package, int32_t index) {
    const auto key = std::make_pair(package.get(), index);
    const auto& exportObject = package->exports[size_t(index) - 1];
    if (classNameOf(*package, index) != "Class") throw RuntimeError("export is not a class: " + package->path(index));
    auto cls = std::make_unique<Class>();
    Class* raw = cls.get();
    cls->name = exportObject.name;
    cls->path = package->packageName + "." + exportObject.name;
    cls->package = package;
    cls->index = index;
    classByExport_[key] = raw;          // registered first so a cyclic reference resolves
    classes_[lower(cls->path)] = std::move(cls);
    if (exportObject.super) raw->super = classAt(package, exportObject.super);
    raw->properties = childProperties(package, index);
    for (int32_t child = 1; size_t(child) <= package->exports.size(); ++child) {
        const auto& object = package->exports[size_t(child) - 1];
        if (object.outer != index) continue;
        const auto kind = classNameOf(*package, child);
        if (kind == "Function") {
            if (auto* function = functionAt(package, child)) raw->functions[lower(function->name)] = function;
        } else if (kind == "State") {
            State state;
            state.name = object.name;
            for (int32_t inner = 1; size_t(inner) <= package->exports.size(); ++inner) {
                const auto& member = package->exports[size_t(inner) - 1];
                if (member.outer == child && classNameOf(*package, inner) == "Function")
                    if (auto* function = functionAt(package, inner)) {
                        function->state = object.name;
                        state.functions[lower(function->name)] = function;
                    }
            }
            raw->states[lower(object.name)] = std::move(state);
        }
    }
    return raw;
}

namespace {
std::string typeTag(Runtime& runtime, const PropertyDecl& decl) {
    const auto& t = decl.type;
    if (t == "IntProperty") return "int";
    if (t == "FloatProperty") return "float";
    if (t == "ByteProperty") return "byte";
    if (t == "BoolProperty") return "bool";
    if (t == "StrProperty") return "string";
    if (t == "NameProperty") return "name";
    if (t == "ObjectProperty" || t == "ComponentProperty") return "object";
    if (t == "InterfaceProperty") return "interface";
    if (t == "ClassProperty") return "class";
    if (t == "ArrayProperty") return "array";
    if (t == "DelegateProperty") return "delegate";
    if (t == "StructProperty") {
        const auto* def = runtime.structAt(decl.package, decl.typeRef);
        return def ? lower(def->name) : "struct";
    }
    return lower(t.substr(0, t.size() - (endsWith(t, "Property") ? 8 : 0)));
}
} // namespace

Function* Runtime::functionAt(const std::shared_ptr<const Package>& package, int32_t index) {
    const auto target = resolveRef(store_, package, index);
    if (!target.package || !script::isFunctionExport(*target.package, target.index)) return nullptr;
    const auto key = std::make_pair(target.package.get(), target.index);
    const auto cached = functions_.find(key);
    if (cached != functions_.end()) return cached->second.get();

    auto function = std::make_unique<Function>();
    Function* raw = function.get();
    functions_[key] = std::move(function);
    const auto& pkg = target.package;
    const auto& exportObject = pkg->exports[size_t(target.index) - 1];
    raw->package = pkg;
    raw->name = exportObject.name;
    raw->path = pkg->packageName + "." + pkg->path(target.index);
    try {
        raw->info = script::readFunction(*pkg, target.index);
    } catch (const std::exception& error) {
        raw->decodeFailed = true;
        raw->decodeError = error.what();
    }
    // Owner: the class, or the class that owns the state the function is declared in.
    int32_t outer = exportObject.outer;
    if (outer > 0 && classNameOf(*pkg, outer) == "State") {
        raw->state = pkg->exports[size_t(outer) - 1].name;
        outer = pkg->exports[size_t(outer) - 1].outer;
    }
    if (outer > 0 && classNameOf(*pkg, outer) == "Class") raw->owner = classAt(pkg, outer);
    for (auto& decl : childProperties(pkg, target.index)) {
        if (decl.isReturn()) raw->result = std::move(decl);
        else if (decl.isParam()) raw->params.push_back(std::move(decl));
        else raw->locals.push_back(std::move(decl));
    }
    if (raw->isNative()) {
        std::string key2 = (raw->owner ? raw->owner->name : std::string("?")) + "." +
                           (raw->info.friendlyName.empty() ? raw->name : raw->info.friendlyName) +
                           ((raw->info.flags & script::FUNC_PreOperator) ? "_pre" : "") + "(";
        for (size_t i = 0; i < raw->params.size(); ++i) key2 += (i ? "," : "") + typeTag(*this, raw->params[i]);
        key2 += ")";
        raw->nativeKey = key2;
        const auto bound = natives_.find(key2);
        if (bound != natives_.end()) raw->native = bound->second;
    }
    return raw;
}

Function* Runtime::findFunction(const std::string& path) {
    const auto dot = path.find('.');
    if (dot == std::string::npos) throw RuntimeError("function path must be Package.Class.Function: " + path);
    auto pkg = package(path.substr(0, dot));
    const auto index = pkg->findExport(path.substr(dot + 1));
    if (!index) throw RuntimeError("function not found: " + path);
    auto* function = functionAt(pkg, index);
    if (!function) throw RuntimeError("not a function: " + path);
    return function;
}

Function* Runtime::findMethod(Class* cls, const std::string& name, const std::string& state) {
    const auto key = lower(name);
    const auto stateKey = lower(state);
    for (Class* cursor = cls; cursor; cursor = cursor->super) {
        if (!stateKey.empty()) {
            const auto s = cursor->states.find(stateKey);
            if (s != cursor->states.end()) {
                const auto f = s->second.functions.find(key);
                if (f != s->second.functions.end()) return f->second;
            }
        }
        const auto f = cursor->functions.find(key);
        if (f != cursor->functions.end()) return f->second;
    }
    return nullptr;
}

void Runtime::buildNativeIndex() {
    if (nativeIndexBuilt_) return;
    nativeIndexBuilt_ = true;
    for (const char* name : {"Core", "Engine", "GameFramework", "GearboxFramework", "WillowGame", "GFxUI", "IpDrv",
                             "OnlineSubsystemSteamworks", "AkAudio"}) {
        try {
            auto pkg = package(name);
            codePackages_.push_back(pkg);
            for (int32_t index = 1; size_t(index) <= pkg->exports.size(); ++index) {
                if (!script::isFunctionExport(*pkg, index)) continue;
                script::FunctionInfo info;
                try { info = script::readFunction(*pkg, index); } catch (const std::exception&) { continue; }
                if ((info.flags & script::FUNC_Native) && info.native) {
                    if (auto* function = functionAt(pkg, index)) nativeIndex_.emplace(info.native, function);
                }
            }
        } catch (const std::exception&) {
            // The package is not in this store (synthetic tests): its natives are simply unavailable.
        }
    }
}

Function* Runtime::nativeByIndex(uint32_t index) {
    buildNativeIndex();
    const auto found = nativeIndex_.find(index);
    return found == nativeIndex_.end() ? nullptr : found->second;
}

void Runtime::registerNative(const std::string& key, NativeFn fn) {
    if (!natives_.emplace(key, fn).second) throw RuntimeError("duplicate native registration: " + key);
    // A function loaded before its implementation was registered binds now.
    for (auto& [_, function] : functions_)
        if (function->nativeKey == key) function->native = fn;
}

// ------------------------------------------------------------------------------ defaults and objects
namespace {

struct TagReader {
    Runtime& runtime;
    const std::shared_ptr<const Package>& package;
    Reader reader;

    TagReader(Runtime& r, const std::shared_ptr<const Package>& p) : runtime(r), package(p), reader(p->data) {}

    std::string name() { return package->name(reader); }
    float floating() { return std::bit_cast<float>(reader.u32()); }

    // An object reference value: a class, or a lightweight stand-in for an exported resource.
    Value reference(bool wantClass) {
        const int32_t ref = reader.i32();
        if (!ref) return wantClass ? Value::makeClass(nullptr) : Value::makeObject(nullptr);
        if (wantClass) {
            try { return Value::makeClass(runtime.classAt(package, ref)); }
            catch (const std::exception&) { return Value::makeClass(nullptr); }
        }
        return Value::makeObject(runtime.resource(package, ref));
    }

    int byteFromEnum(const std::string& enumName, const std::string& valueName) {
        // Enum exports hold [NetIndex][None][Next][count][FName * count]; a value's index is its position.
        for (int32_t index = 1; size_t(index) <= package->exports.size(); ++index) {
            const auto& object = package->exports[size_t(index) - 1];
            if (object.name != enumName || classNameOf(*package, index) != "Enum") continue;
            size_t at = size_t(object.offset) + 16;
            if (at + 4 > size_t(object.offset) + size_t(object.size)) return 0;
            const int32_t count = int32_t(u32At(*package, at));
            at += 4;
            for (int32_t i = 0; i < count && at + 8 <= size_t(object.offset) + size_t(object.size); ++i, at += 8) {
                const int32_t nameIndex = int32_t(u32At(*package, at));
                if (nameIndex >= 0 && size_t(nameIndex) < package->names.size() && package->names[size_t(nameIndex)] == valueName)
                    return i;
            }
            return 0;
        }
        return 0;  // enum declared in another package: UNVERIFIED, resolved as 0
    }

    Value structure(const std::string& type, unsigned depth) {
        if (depth > 16) throw RuntimeError("struct nesting too deep");
        if (const auto* layout = nativeStruct(type)) {
            Value value = Value::makeStruct(type);
            auto& aggregate = value.mut();
            for (const char* field : layout->fields) {
                aggregate.names.push_back(field);
                if (layout->kind == 'f') aggregate.values.push_back(Value::makeFloat(floating()));
                else if (layout->kind == 'i') aggregate.values.push_back(Value::makeInt(reader.i32()));
                else { reader.require(1); aggregate.values.push_back(Value::makeByte(reader.bytes()[reader.pos++])); }
            }
            return value;
        }
        // Other structs are a nested run of tagged properties ended by "None".
        Value value = Value::makeStruct(type);
        std::unordered_map<std::string, PropertyDecl> declared;
        if (const StructDef* def = findStruct(type))
            for (const auto& field : def->fields) declared[lower(field.name)] = field;
        // Start from the struct's zero value so untouched fields read as defaults.
        if (const StructDef* def = findStruct(type)) {
            auto& aggregate = value.mut();
            for (const auto& field : def->fields) {
                aggregate.names.push_back(field.name);
                aggregate.values.push_back(runtime.zeroValue(field));
            }
        }
        tagged(value, declared, depth + 1);
        return value;
    }

    const StructDef* findStruct(const std::string& type) {
        for (int32_t index = 1; size_t(index) <= package->exports.size(); ++index) {
            const auto& object = package->exports[size_t(index) - 1];
            if (object.name == type && classNameOf(*package, index) == "ScriptStruct") return runtime.structAt(package, index);
        }
        for (int32_t index = 1; size_t(index) <= package->imports.size(); ++index) {
            const auto& object = package->imports[size_t(index) - 1];
            if (object.name == type && object.className == "ScriptStruct") {
                try { return runtime.structAt(package, -index); } catch (const std::exception&) { return nullptr; }
            }
        }
        return nullptr;
    }

    // Reads one tagged property run into `target`: a struct Value (fields by name), or an object's props.
    template <typename Store>
    void tagged(Store& target, const std::unordered_map<std::string, PropertyDecl>& declared, unsigned depth);

    Value element(const std::string& innerType, const PropertyDecl* innerDecl, unsigned depth) {
        if (innerType == "IntProperty") return Value::makeInt(reader.i32());
        if (innerType == "FloatProperty") return Value::makeFloat(floating());
        if (innerType == "NameProperty") return Value::makeName(name());
        if (innerType == "StrProperty") return Value::makeString(reader.string());
        if (innerType == "ObjectProperty" || innerType == "ComponentProperty" || innerType == "InterfaceProperty") return reference(false);
        if (innerType == "ClassProperty") return reference(true);
        if (innerType == "ByteProperty") { reader.require(1); return Value::makeByte(reader.bytes()[reader.pos++]); }
        if (innerType == "BoolProperty") return Value::makeBool(reader.u32() != 0);
        if (innerType == "StructProperty" && innerDecl) {
            const auto* def = runtime.structAt(innerDecl->package, innerDecl->typeRef);
            return structure(def ? def->name : "", depth + 1);
        }
        throw RuntimeError("unsupported array element type " + innerType);
    }
};

} // namespace

// Applies one object's tagged defaults. Unsupported property kinds are skipped by their stored size, which
// the tag always records, so a property this runtime cannot decode never desynchronises the rest.
void Runtime::applyTaggedDefaults(Object& object, Class* cls, const std::shared_ptr<const Package>& pkg, int32_t exportIndex) {
    const auto& exportObject = pkg->exports[size_t(exportIndex) - 1];
    if (exportObject.size < 12) return;
    TagReader tags(*this, pkg);
    tags.reader.pos = size_t(exportObject.offset) + 4;           // after the net index
    tags.reader.limit = size_t(exportObject.offset) + size_t(exportObject.size);
    std::unordered_map<std::string, const PropertyDecl*> decls;
    for (Class* cursor = cls; cursor; cursor = cursor->super)
        for (const auto& decl : cursor->properties) decls.emplace(lower(decl.name), &decl);
    while (true) {
        const auto propertyName = tags.name();
        if (propertyName == "None") break;
        const auto type = tags.name();
        const int32_t size = tags.reader.i32();
        const int32_t arrayIndex = tags.reader.i32();
        if (size < 0 || arrayIndex < 0) throw RuntimeError("negative property size or index");
        std::string detail;
        int boolean = 0;
        if (type == "StructProperty" || type == "ByteProperty") detail = tags.name();
        if (type == "BoolProperty") { tags.reader.require(1); boolean = tags.reader.bytes()[tags.reader.pos++]; }
        tags.reader.require(size_t(size));
        const size_t end = tags.reader.pos + size_t(size);
        const auto savedLimit = tags.reader.limit;
        tags.reader.limit = end;
        Value value;
        bool ok = true;
        const auto declared = decls.find(lower(propertyName));
        const PropertyDecl* decl = declared == decls.end() ? nullptr : declared->second;
        try {
            if (type == "IntProperty") value = Value::makeInt(tags.reader.i32());
            else if (type == "FloatProperty") value = Value::makeFloat(tags.floating());
            else if (type == "BoolProperty") value = Value::makeBool(boolean != 0);
            else if (type == "NameProperty") value = Value::makeName(tags.name());
            else if (type == "StrProperty") value = Value::makeString(tags.reader.string());
            else if (type == "ObjectProperty" || type == "ComponentProperty" || type == "InterfaceProperty") value = tags.reference(false);
            else if (type == "ClassProperty") value = tags.reference(true);
            else if (type == "ByteProperty" && detail == "None") { tags.reader.require(1); value = Value::makeByte(tags.reader.bytes()[tags.reader.pos++]); }
            else if (type == "ByteProperty") value = Value::makeByte(tags.byteFromEnum(detail, tags.name()));
            else if (type == "StructProperty") value = tags.structure(detail, 0);
            else if (type == "ArrayProperty") {
                const int32_t length = tags.reader.i32();
                if (length < 0 || length > 1'000'000) throw RuntimeError("invalid array length");
                value = Value::makeArray();
                PropertyDecl inner;
                const PropertyDecl* innerDecl = nullptr;
                std::string innerType;
                if (decl) {
                    const auto target = resolveRef(store_, decl->package, decl->typeRef);
                    if (target.package) {
                        inner = readProperty(target.package, target.index);
                        innerDecl = &inner;
                        innerType = inner.type;
                    }
                }
                for (int32_t i = 0; i < length; ++i) {
                    if (!innerDecl) throw RuntimeError("array element type unknown");
                    value.elements().push_back(tags.element(innerType, innerDecl, 0));
                }
            } else ok = false;
            if (ok && tags.reader.pos != end) ok = false;
        } catch (const std::exception&) {
            ok = false;
        }
        tags.reader.limit = savedLimit;
        tags.reader.pos = end;
        if (!ok) { log.push_back("defaults: skipped " + cls->name + "." + propertyName + " (" + type + ")"); continue; }
        auto& slot = object.props[lower(propertyName)];
        if (decl && decl->arrayDim > 1 && decl->type != "ArrayProperty") {
            if (slot.kind != Value::Kind::Array) slot = zeroValue(*decl);
            if (size_t(arrayIndex) < slot.elements().size()) slot.elements()[size_t(arrayIndex)] = std::move(value);
        } else {
            slot = std::move(value);
        }
    }
}

template <typename Store>
void TagReader::tagged(Store& target, const std::unordered_map<std::string, PropertyDecl>& declared, unsigned depth) {
    if (depth > 16) throw RuntimeError("struct nesting too deep");
    while (true) {
        const auto propertyName = name();
        if (propertyName == "None") break;
        const auto type = name();
        const int32_t size = reader.i32();
        const int32_t arrayIndex = reader.i32();
        if (size < 0 || arrayIndex < 0) throw RuntimeError("negative property size or index");
        std::string detail;
        int boolean = 0;
        if (type == "StructProperty" || type == "ByteProperty") detail = name();
        if (type == "BoolProperty") { reader.require(1); boolean = reader.bytes()[reader.pos++]; }
        reader.require(size_t(size));
        const size_t end = reader.pos + size_t(size);
        const auto savedLimit = reader.limit;
        reader.limit = end;
        Value value;
        bool ok = true;
        const auto found = declared.find(lower(propertyName));
        try {
            if (type == "IntProperty") value = Value::makeInt(reader.i32());
            else if (type == "FloatProperty") value = Value::makeFloat(floating());
            else if (type == "BoolProperty") value = Value::makeBool(boolean != 0);
            else if (type == "NameProperty") value = Value::makeName(name());
            else if (type == "StrProperty") value = Value::makeString(reader.string());
            else if (type == "ObjectProperty" || type == "ComponentProperty" || type == "InterfaceProperty") value = reference(false);
            else if (type == "ClassProperty") value = reference(true);
            else if (type == "ByteProperty" && detail == "None") { reader.require(1); value = Value::makeByte(reader.bytes()[reader.pos++]); }
            else if (type == "ByteProperty") value = Value::makeByte(byteFromEnum(detail, name()));
            else if (type == "StructProperty") value = structure(detail, depth + 1);
            else ok = false;
            if (ok && reader.pos != end) ok = false;
        } catch (const std::exception&) { ok = false; }
        reader.limit = savedLimit;
        reader.pos = end;
        if (!ok) continue;
        (void)found;
        if (Value* slot = target.field(propertyName)) *slot = std::move(value);
    }
}

ObjectPtr Runtime::resource(const std::shared_ptr<const Package>& pkg, int32_t ref) {
    if (!pkg || !ref) return nullptr;
    std::shared_ptr<const Package> owner = pkg;
    int32_t index = ref;
    if (ref < 0) {
        try {
            const auto resolved = store_.resolve(pkg, ref);
            if (resolved.package) { owner = resolved.package; index = resolved.index; }
        } catch (const std::exception&) { /* keep the unresolved reference */ }
    }
    const auto key = std::make_pair(owner.get(), index);
    const auto found = resources_.find(key);
    if (found != resources_.end()) return found->second;
    auto object = std::make_shared<Object>();
    try { object->name = owner->object(index).name; } catch (const std::exception&) { object->name = "?"; }
    object->resourcePackage = owner;
    object->resourceIndex = index;
    resources_[key] = object;
    return object;
}

ObjectPtr Runtime::defaultsOf(Class* cls) {
    if (!cls) return nullptr;
    if (!cls->defaultsBuilt) buildDefaults(cls);
    return cls->defaults;
}

void Runtime::buildDefaults(Class* cls) {
    cls->defaultsBuilt = true;
    auto object = std::make_shared<Object>();
    object->cls = cls;
    object->name = "Default__" + cls->name;
    std::vector<Class*> chain;
    for (Class* cursor = cls; cursor; cursor = cursor->super) chain.push_back(cursor);
    std::reverse(chain.begin(), chain.end());
    for (Class* cursor : chain)
        for (const auto& decl : cursor->properties) object->props[lower(decl.name)] = zeroValue(decl);
    // Each class's default object records only the values it changes; apply root to leaf.
    for (Class* cursor : chain) {
        const auto wanted = "Default__" + cursor->name;
        for (int32_t index = 1; size_t(index) <= cursor->package->exports.size(); ++index) {
            const auto& exportObject = cursor->package->exports[size_t(index) - 1];
            if (exportObject.name != wanted || exportObject.cls != cursor->index) continue;
            try { applyTaggedDefaults(*object, cursor, cursor->package, index); }
            catch (const std::exception& error) { log.push_back("defaults: " + wanted + ": " + error.what()); }
            break;
        }
    }
    cls->defaults = std::move(object);
}

ObjectPtr Runtime::instantiate(Class* cls, const std::string& name) {
    if (!cls) throw RuntimeError("cannot instantiate a null class");
    auto defaults = defaultsOf(cls);
    auto object = std::make_shared<Object>();
    object->cls = cls;
    object->name = name.empty() ? cls->name + "_0" : name;
    object->props = defaults->props;
    return object;
}

Value* Runtime::property(Object& object, const std::string& name) {
    if (Value* found = object.find(name)) return found;
    // A property declared after the object was built (should not happen): start it at zero.
    for (Class* cursor = object.cls; cursor; cursor = cursor->super)
        for (const auto& decl : cursor->properties)
            if (lower(decl.name) == lower(name)) return &(object.props[lower(name)] = zeroValue(decl));
    return nullptr;
}

} // namespace vm
