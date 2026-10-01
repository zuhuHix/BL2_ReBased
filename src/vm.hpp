#pragma once

#include "script.hpp"

#include <functional>
#include <map>
#include <memory>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

// Phase 2 (UnrealScript VM): values, the object model and the interpreter.
//
// Scope: enough to run Gearbox's own script functions on the classes Sanctuary and the first Vault Hunter
// need (see ROADMAP.md), not the whole engine. Objects keep their properties in a name-keyed bag (no memory
// layout), class defaults come from the packages' default objects, and every native function that has no
// implementation is a logged stub that returns a zero value (Runtime::log).
//
// Nothing here has been compared against the real game yet: behaviour is checked against hand-computed
// results on synthetic packages and on Core's script functions (tests/vm_test.py). Anything marked UNVERIFIED
// is an assumption about UE3 semantics that still needs a golden trace.
namespace vm {

struct Class;
struct Object;
struct Function;
struct Aggregate;
class Runtime;
using ObjectPtr = std::shared_ptr<Object>;

struct RuntimeError : std::runtime_error {
    using std::runtime_error::runtime_error;
};

// A script value. Structs and dynamic arrays are values (copied on assignment); objects are references.
struct Value {
    enum class Kind : uint8_t { None, Int, Float, Bool, Byte, String, Name, Object, Class, Struct, Array, Delegate };
    Kind kind = Kind::None;
    int64_t i = 0;                      // Int (32-bit), Bool, Byte
    double f = 0;                       // Float (kept at 32-bit precision)
    std::string s;                      // String, Name, Delegate function name
    ObjectPtr o;                        // Object, Delegate target
    Class* cls = nullptr;               // Class
    std::shared_ptr<Aggregate> agg;     // Struct fields / Array elements (copy on write)

    static Value makeInt(int64_t v);
    static Value makeFloat(double v);
    static Value makeBool(bool v);
    static Value makeByte(int64_t v);
    static Value makeString(std::string v);
    static Value makeName(std::string v);
    static Value makeObject(ObjectPtr v);
    static Value makeClass(Class* v);
    static Value makeArray();
    static Value makeStruct(std::string typeName);

    bool truth() const;                 // bool coercion used by conditions
    double number() const;              // Int/Float/Byte/Bool as a double
    int64_t integer() const;            // same as an integer (floats truncate)
    Aggregate& mut();                   // detaches a shared aggregate before it is modified
    const Aggregate* aggregate() const { return agg.get(); }
    Value* field(const std::string& name);              // struct field (case-insensitive), nullptr if absent
    const Value* field(const std::string& name) const;
    std::vector<Value>& elements();                     // array elements, detaching first
    const std::vector<Value>& elements() const;
    std::string describe() const;       // readable form for logs and tests
};

struct Aggregate {
    std::string typeName;               // struct type; empty for arrays
    std::vector<std::string> names;     // struct field names (empty for arrays)
    std::vector<Value> values;
};

bool sameValue(const Value& a, const Value& b);   // UnrealScript == on two values of the same type

struct PropertyDecl {
    std::string name;
    std::string type;                   // export class, e.g. "IntProperty"
    uint64_t flags = 0;
    int32_t arrayDim = 1;
    std::shared_ptr<const Package> package;
    int32_t index = 0;                  // export index of the property
    int32_t typeRef = 0;                // struct / class / enum / inner property / function, package-relative
    int32_t typeRef2 = 0;               // metaclass (ClassProperty), source delegate (DelegateProperty)
    bool isParam() const { return flags & 0x80; }
    bool isOut() const { return flags & 0x100; }
    bool isOptional() const { return flags & 0x10; }
    bool isReturn() const { return flags & 0x400; }
};

// A package-relative reference resolved to the package that really holds the object.
struct Resolved {
    std::shared_ptr<const Package> package;
    int32_t index = 0;
    explicit operator bool() const { return package && index > 0; }
};

// Lookup tables over one package's exports, built once.
struct PackageIndex {
    std::vector<std::string> classNames;                               // class name per export (index - 1)
    std::unordered_map<int32_t, std::vector<int32_t>> children;       // outer export -> child exports, ascending
    std::unordered_map<std::string, std::vector<int32_t>> byName;     // lower-cased export name -> exports
    std::unordered_map<std::string, int32_t> byPath;                  // lower-cased object path -> first export
};

struct StructDef {
    std::string name;
    std::vector<PropertyDecl> fields;
    std::shared_ptr<const Package> package;
    int32_t index = 0;
};

// Native implementation: reads its arguments from `c`, returns the result (None for void).
struct NativeCall;
using NativeFn = std::function<Value(NativeCall&)>;

struct Function {
    Class* owner = nullptr;             // class (or the class owning the state)
    std::string state;                  // non-empty when declared inside a state
    std::shared_ptr<const Package> package;
    script::FunctionInfo info;
    std::string name;
    std::string path;                   // Package.Outer.Name
    std::vector<PropertyDecl> params;   // declaration order, return value excluded
    std::vector<PropertyDecl> locals;   // every other child property
    std::optional<PropertyDecl> result; // return value
    std::shared_ptr<script::Code> code; // lazily decoded
    bool decodeFailed = false;
    std::string decodeError;
    NativeFn native;                    // bound implementation, or empty
    std::string nativeKey;              // "Class.Name(type,type)" used to look implementations up
    bool isNative() const { return info.flags & script::FUNC_Native; }
    bool isStatic() const { return info.flags & script::FUNC_Static; }
};

struct State {
    std::string name;
    std::unordered_map<std::string, Function*> functions;   // lower-cased name
};

struct Class {
    std::string name;
    std::string path;                   // Package.Name
    Class* super = nullptr;
    std::shared_ptr<const Package> package;
    int32_t index = 0;
    std::vector<PropertyDecl> properties;                   // declared by this class only
    std::unordered_map<std::string, Function*> functions;   // lower-cased name, declared by this class
    std::unordered_map<std::string, State> states;          // lower-cased name
    ObjectPtr defaults;                 // the class default object (Default__Name), built on first use
    bool defaultsBuilt = false;
    bool isChildOf(const Class* other) const;
};

struct Object {
    Class* cls = nullptr;
    std::string name;
    std::unordered_map<std::string, Value> props;           // property name (lower-cased) -> value
    ObjectPtr outer;
    std::string state;                  // current state name ("" = default)
    // A reference to an exported asset that has not been instantiated (textures, meshes, archetypes...).
    std::shared_ptr<const Package> resourcePackage;
    int32_t resourceIndex = 0;
    bool destroyed = false;
    Value* find(const std::string& property);
};

struct Frame;

// One native call as the implementation sees it.
struct NativeCall {
    Runtime& runtime;
    Function& function;
    ObjectPtr self;
    struct Arg {
        Value value;
        Value* alias = nullptr;         // out / inout parameters write through this
        bool supplied = true;           // false for an omitted optional parameter
    };
    std::vector<Arg> args;
    size_t count() const { return args.size(); }
    Value& in(size_t i) { return args.at(i).value; }
    Value& out(size_t i) { auto& a = args.at(i); return a.alias ? *a.alias : a.value; }
    bool has(size_t i) const { return i < args.size() && args[i].supplied; }
};

class Runtime {
public:
    explicit Runtime(PackageStore& store);

    PackageStore& store() { return store_; }
    std::vector<std::string> log;       // Log() output and every "UNIMPLEMENTED ..." native stub call
    size_t stepLimit = 5'000'000;       // executed expressions per top-level call (guards runaway scripts)
    size_t steps = 0;

    // Loading. Class paths are "Package.Class"; function paths "Package.Class.Function".
    std::shared_ptr<const Package> package(const std::string& name);
    Class* findClass(const std::string& path);
    Function* findFunction(const std::string& path);
    Function* functionAt(const std::shared_ptr<const Package>& package, int32_t index);
    Class* classAt(const std::shared_ptr<const Package>& package, int32_t index);
    Function* findMethod(Class* cls, const std::string& name, const std::string& state = "");
    Function* nativeByIndex(uint32_t index);        // numbered natives come from the loaded code packages
    const StructDef* structAt(const std::shared_ptr<const Package>& package, int32_t ref);
    // Resolves a reference (import or export) to the object's real package, cached. Empty when the target does not
    // exist in the installed packages (some imports name engine-native objects that have no export).
    Resolved resolveRef(const std::shared_ptr<const Package>& package, int32_t ref);
    const PackageIndex& indexOf(const Package& package);
    int32_t findExport(const Package& package, const std::string& objectPath);   // fast Package::findExport

    // Objects and values.
    ObjectPtr instantiate(Class* cls, const std::string& name = "");
    // Explicit observed prefix, supplied by the caller (4 ordinary objects,
    // 26 placed actors, 8 components). No offset scanning or native-tail decode.
    ObjectPtr instantiateExport(const std::shared_ptr<const Package>& package, int32_t index, size_t prefix);
    Value zeroValue(const PropertyDecl& decl);
    Value zeroStruct(const std::string& name);
    Value newStruct(const std::string& path);        // "Package.Struct": zero value with the declared fields
    Value* property(Object& object, const std::string& name);       // walks the class chain for defaults
    ObjectPtr defaultsOf(Class* cls);
    ObjectPtr resource(const std::shared_ptr<const Package>& package, int32_t ref);   // stand-in for an asset export

    // Running script. `self` may be null for static functions.
    // `outs`, when given, receives every argument as it stands after the call (out parameters updated).
    Value call(Function& function, ObjectPtr self, std::vector<Value> args = {}, std::vector<Value>* outs = nullptr);
    Value callByName(ObjectPtr self, const std::string& name, std::vector<Value> args = {});

    // Native registry. Key: "Class.Name(type,type,...)", see Function::nativeKey.
    void registerNative(const std::string& key, NativeFn fn);
    void registerCoreNatives();

private:
    friend struct Interp;
    PackageStore& store_;
    std::map<std::string, std::unique_ptr<Class>> classes_;
    std::map<std::pair<const Package*, int32_t>, std::unique_ptr<Function>> functions_;
    std::map<std::pair<const Package*, int32_t>, Class*> classByExport_;
    std::map<std::pair<const Package*, int32_t>, std::unique_ptr<StructDef>> structs_;
    std::unordered_map<std::string, NativeFn> natives_;
    std::unordered_map<uint32_t, Function*> nativeIndex_;
    bool nativeIndexBuilt_ = false;
    std::vector<std::shared_ptr<const Package>> codePackages_;
    std::map<std::pair<const Package*, int32_t>, ObjectPtr> resources_;
    std::map<std::pair<const Package*, int32_t>, Resolved> refCache_;
    std::unordered_map<const Package*, std::shared_ptr<PackageIndex>> indexes_;

    std::vector<PropertyDecl> childProperties(const std::shared_ptr<const Package>& package, int32_t outer);
    PropertyDecl readProperty(const std::shared_ptr<const Package>& package, int32_t index);
    Value zeroOfDef(const StructDef* def);
    void buildNativeIndex();
    Class* loadClass(const std::shared_ptr<const Package>& package, int32_t index);
    void buildDefaults(Class* cls);
    void applyTaggedDefaults(Object& object, Class* cls, const std::shared_ptr<const Package>& package, int32_t exportIndex, size_t prefix = 4);
};

struct SweepOptions {
    std::string classFilter;            // only functions of this class ("" = all)
    size_t limit = 0;                   // stop after this many functions (0 = all)
    size_t stepLimit = 200000;          // per function
    size_t top = 25;                    // entries kept per ranked list
};
// Robustness sweep over a package's script functions, see vm_sweep.cpp. Returns a JSON summary.
std::string sweepPackage(Runtime& runtime, const std::shared_ptr<const Package>& package, const SweepOptions& options);

} // namespace vm
