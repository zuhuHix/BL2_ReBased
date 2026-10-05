#include "census.hpp"

#include <algorithm>
#include <map>
#include <set>
#include <sstream>

// Native census, see census.hpp. Nothing here knows about a particular game function: it runs whatever entry list
// it is given and walks whatever bytecode those entries reach.
namespace vm {

using script::Expr;

namespace {

std::string lowerCase(std::string text) {
    for (auto& c : text) c = char(std::tolower(static_cast<unsigned char>(c)));
    return text;
}

std::vector<std::string> tokens(const std::string& line) {
    std::vector<std::string> out;
    std::istringstream stream(line);
    std::string token;
    while (stream >> token) out.push_back(token);
    return out;
}

// ------------------------------------------------------------------------------------------ static closure
// A receiver's inferred class. `iface` marks an interface-typed receiver (implementers are not known statically).
struct Type {
    Class* cls = nullptr;
    bool iface = false;
    explicit operator bool() const { return cls != nullptr; }
};

constexpr size_t MAX_NAME_ONLY = 8;     // most functions of one name accepted as candidates when the receiver class is unknown

bool isTypedSlot(uint8_t op) { return op >= script::EX_Op4C && op <= script::EX_Op50; }

// How a call site reached its target. Primary: the function the call names (a direct call, a numbered native, or the
// version of a virtual method visible at the receiver's static class). Upper: a subclass override or an interface
// implementer that the call might dispatch to. Closure A follows primary edges only, closure B follows both.
enum class Reach { Primary, Upper };

struct Sites { size_t primary = 0, upper = 0; };   // call sites in one caller's body, by how they reach the target

struct Target {
    std::map<Function*, Sites> callers;          // calling script function -> its sites on this target (counted once per body)
};

struct Edges { bool primary = false, upper = false; };

class Closure {
public:
    explicit Closure(Runtime& runtime) : rt(runtime) {}

    std::map<Function*, Target> targets;                   // every function that is called from walked bytecode
    std::map<Function*, std::map<Function*, Edges>> callees;   // walked script function -> what it calls, by edge kind
    std::map<std::string, size_t> unresolvedVirtual;       // call name -> sites whose receiver class was not inferred
    std::map<std::string, size_t> missingMethod;           // "Class.Name" -> sites where the inferred class has no such method
    std::map<uint32_t, size_t> unknownNativeIndex;         // numbered native with no declaration in the loaded packages
    std::set<std::string> undecodable;                     // reachable script functions whose bytecode does not decode
    size_t delegateSites = 0;

    // Walks `entry` and everything it reaches (each function body once, however many entries reach it).
    void analyse(Function& entry) {
        pending.push_back(&entry);
        while (!pending.empty()) {
            Function* function = pending.back();
            pending.pop_back();
            walkFunction(*function);
        }
    }

    size_t scriptsWalked() const { return walked.size(); }   // decoded or attempted script bodies (natives excluded)

    // `entry` and the functions reachable from it through the walked call edges: closure A (primary edges only) or
    // closure B (primary and upper-bound edges).
    std::set<Function*> reachableFrom(Function& entry, bool withUpper) {
        std::set<Function*> seen{&entry};
        std::vector<Function*> stack{&entry};
        while (!stack.empty()) {
            Function* function = stack.back();
            stack.pop_back();
            const auto found = callees.find(function);
            if (found == callees.end()) continue;
            for (const auto& [callee, edge] : found->second)
                if ((edge.primary || (withUpper && edge.upper)) && seen.insert(callee).second) stack.push_back(callee);
        }
        return seen;
    }

private:
    Runtime& rt;
    std::vector<Function*> pending;
    std::set<Function*> walked;
    std::vector<Class*> allClasses;
    bool classesBuilt = false;
    std::map<Class*, std::vector<Class*>> descendantCache;
    std::map<std::string, std::vector<Function*>> byName;  // lower-cased name -> every class-level or state function of that name
    std::map<std::pair<int, int>, Type> slotTypes;         // typed temporaries (Op4C..Op50, index) of the function being walked

    void buildClasses() {
        if (classesBuilt) return;
        classesBuilt = true;
        for (const auto& package : rt.codePackages()) {
            const auto& index = rt.indexOf(*package);
            for (size_t i = 0; i < index.classNames.size(); ++i) {
                if (index.classNames[i] != "Class") continue;
                try { if (Class* cls = rt.classAt(package, int32_t(i) + 1)) allClasses.push_back(cls); }
                catch (const std::exception&) {}
            }
        }
    }

    // Interface receivers: the implementers are not known statically, so every function with the name counts.
    const std::vector<Function*>& declaredNamed(const std::string& name) {
        if (byName.empty()) {
            buildClasses();
            for (Class* cls : allClasses) {
                for (const auto& [key, function] : cls->functions) byName[key].push_back(function);
                for (const auto& [_, state] : cls->states)
                    for (const auto& [key, function] : state.functions) byName[key].push_back(function);
            }
        }
        static const std::vector<Function*> none;
        const auto found = byName.find(lowerCase(name));
        return found == byName.end() ? none : found->second;
    }

    const std::vector<Class*>& descendants(Class* base) {
        const auto cached = descendantCache.find(base);
        if (cached != descendantCache.end()) return cached->second;
        buildClasses();
        std::vector<Class*> found;
        for (Class* cls : allClasses)
            if (cls != base && cls->isChildOf(base)) found.push_back(cls);
        return descendantCache.emplace(base, std::move(found)).first->second;
    }

    // ------------------------------------------------------------------------------------------ typing
    Type typeOfDecl(const PropertyDecl& decl) {
        try {
            if (decl.type == "ObjectProperty" || decl.type == "ComponentProperty") return {rt.classAt(decl.package, decl.typeRef), false};
            if (decl.type == "ClassProperty") return {rt.classAt(decl.package, decl.typeRef2), false};
            if (decl.type == "InterfaceProperty") return {rt.classAt(decl.package, decl.typeRef), true};
        } catch (const std::exception&) {}
        return {};
    }

    // The declaration an expression reads or writes, for variables, struct members and array elements.
    std::optional<PropertyDecl> declOf(const Expr& e, Function& f) {
        try {
            switch (e.op) {
            case script::EX_LocalVariable: case script::EX_LocalOutVariable: case script::EX_InstanceVariable:
            case script::EX_DefaultVariable: case script::EX_StateVariable: case script::EX_Op5E: case script::EX_StructMember:
                return rt.declAt(f.package, e.refs.at(0));
            case script::EX_ArrayElement: case script::EX_DynArrayElement: {
                auto array = declOf(e.kids.at(1), f);
                if (array && array->type == "ArrayProperty") return rt.declAt(array->package, array->typeRef);
                return array;
            }
            case script::EX_Context: case script::EX_ClassContext: return declOf(e.kids.at(1), f);
            case script::EX_BoolVariable: case script::EX_InterfaceContext: return declOf(e.kids.at(0), f);
            default: break;
            }
        } catch (const std::exception&) {}
        return std::nullopt;
    }

    Type typeOf(const Expr& e, Function& f, const Type& recv) {
        switch (e.op) {
        case script::EX_Self: return {f.owner, false};
        case script::EX_DynamicCast: case script::EX_InterfaceCast:
            try { return {rt.classAt(f.package, e.refs.at(0)), e.op == script::EX_InterfaceCast}; } catch (const std::exception&) { return {}; }
        case script::EX_ObjectConst:
            try {
                const auto resolved = rt.resolveRef(f.package, e.refs.at(0));
                // A class export has no class reference of its own (0 stands for Class).
                if (resolved && (resolved.package->exports[size_t(resolved.index) - 1].cls == 0 ||
                                 resolved.package->object(resolved.package->exports[size_t(resolved.index) - 1].cls).name == "Class"))
                    return {rt.classAt(resolved.package, resolved.index), false};
            } catch (const std::exception&) {}
            return {};
        case script::EX_Context: case script::EX_ClassContext: return typeOf(e.kids.at(1), f, typeOf(e.kids.at(0), f, recv));
        case script::EX_Conditional: return typeOf(e.kids.at(1), f, recv);
        case script::EX_Op4C: case script::EX_Op4D: case script::EX_Op4E: case script::EX_Op4F: case script::EX_Op50: {
            const auto found = slotTypes.find({e.op, e.ints.at(0)});
            return found == slotTypes.end() ? Type() : found->second;
        }
        case script::EX_FinalFunction: case script::EX_VirtualFunction: case script::EX_GlobalFunction: {
            Function* target = nullptr;
            try {
                if (e.op == script::EX_FinalFunction) target = rt.functionAt(f.package, e.refs.at(0));
                else if (recv) target = rt.findMethod(recv.cls, e.names.at(0));
            } catch (const std::exception&) {}
            return target && target->result ? typeOfDecl(*target->result) : Type();
        }
        default: break;
        }
        const auto decl = declOf(e, f);
        return decl ? typeOfDecl(*decl) : Type();
    }

    // Typed temporaries are assigned once near the top of a function; remember what each is assigned.
    void learnSlots(const Expr& e, Function& f, const Type& self) {
        if ((e.op == script::EX_Let || e.op == script::EX_Op5F) && e.kids.size() == 2 && isTypedSlot(e.kids[0].op)) {
            const std::pair<int, int> key{e.kids[0].op, e.kids[0].ints.at(0)};
            if (!slotTypes.count(key))
                if (const Type type = typeOf(e.kids[1], f, self)) slotTypes[key] = type;
        }
        for (const auto& kid : e.kids) learnSlots(kid, f, self);
    }

    // ------------------------------------------------------------------------------------------ walking
    void record(Function& caller, Function* target, Reach how) {
        if (!target) return;
        Sites& sites = targets[target].callers[&caller];
        Edges& edge = callees[&caller][target];
        if (how == Reach::Primary) { ++sites.primary; edge.primary = true; }
        else { ++sites.upper; edge.upper = true; }
        if (!target->isNative() && !walked.count(target)) pending.push_back(target);
    }

    // The methods a call by name may run when the receiver's static class is `recv`: the version visible at that
    // class, plus every override declared by a subclass (also inside states). An upper bound.
    std::vector<Function*> resolveByName(const Type& recv, const std::string& name) {
        std::vector<Function*> found;
        const auto key = lowerCase(name);
        const auto add = [&](Function* function) {
            if (function && std::find(found.begin(), found.end(), function) == found.end()) found.push_back(function);
        };
        add(rt.findMethod(recv.cls, name));
        const auto declared = [&](Class* cls) {
            const auto method = cls->functions.find(key);
            if (method != cls->functions.end()) add(method->second);
            for (const auto& [_, state] : cls->states) {
                const auto inState = state.functions.find(key);
                if (inState != state.functions.end()) add(inState->second);
            }
        };
        declared(recv.cls);
        for (Class* cls : descendants(recv.cls)) declared(cls);
        return found;
    }

    // A Context expression names the return property of the function it calls; that property's outer is the function
    // as the compiler resolved it, which gives a receiver class when none can be inferred from the expression.
    Function* declaredFunction(const Expr& context, Function& f) {
        if (context.refs.empty() || !context.refs[0]) return nullptr;
        try {
            const auto property = rt.resolveRef(f.package, context.refs[0]);
            if (!property) return nullptr;
            const int32_t outer = property.package->exports[size_t(property.index) - 1].outer;
            return outer > 0 && script::isFunctionExport(*property.package, outer) ? rt.functionAt(property.package, outer) : nullptr;
        } catch (const std::exception&) { return nullptr; }
    }

    void callByName(Function& caller, Type recv, const std::string& name, const Function* declared) {
        if (name == "None") return;                  // an empty delegate
        if (!recv && declared && declared->owner && lowerCase(declared->name) == lowerCase(name)) recv = {declared->owner, false};
        if (!recv) {
            // No class could be inferred: a name that few classes declare is taken as upper-bound candidates, others stay unresolved.
            const auto& candidates = declaredNamed(name);
            if (candidates.empty() || candidates.size() > MAX_NAME_ONLY) { ++unresolvedVirtual[name]; return; }
            for (Function* function : candidates) record(caller, function, Reach::Upper);
            return;
        }
        if (recv.iface) {
            const auto& candidates = declaredNamed(name);
            if (candidates.empty()) ++missingMethod[recv.cls->name + "." + name];
            for (Function* function : candidates) record(caller, function, Reach::Upper);
            return;
        }
        const auto found = resolveByName(recv, name);
        if (found.empty()) { ++missingMethod[recv.cls->name + "." + name]; return; }
        const Function* visible = rt.findMethod(recv.cls, name);
        for (Function* function : found) record(caller, function, function == visible ? Reach::Primary : Reach::Upper);
    }

    // `declared`: the function a Context expression resolved for the call directly under it, when it names one.
    void walk(const Expr& e, Function& f, const Type& self, const Type& recv, const Function* declared = nullptr) {
        if (e.isNativeCall()) {
            Function* native = rt.nativeByIndex(e.native);
            if (native) record(f, native, Reach::Primary);
            else ++unknownNativeIndex[e.native];
            for (const auto& kid : e.kids) walk(kid, f, self, self);
            return;
        }
        switch (e.op) {
        case script::EX_FinalFunction: {
            Function* target = nullptr;
            try { target = rt.functionAt(f.package, e.refs.at(0)); } catch (const std::exception&) {}
            if (target) record(f, target, Reach::Primary); else ++unresolvedVirtual["<unresolved final reference>"];
            for (const auto& kid : e.kids) walk(kid, f, self, self);
            return;
        }
        case script::EX_VirtualFunction: case script::EX_GlobalFunction:
            callByName(f, recv, e.names.at(0), declared);
            for (const auto& kid : e.kids) walk(kid, f, self, self);
            return;
        case script::EX_DelegateFunction: {
            // The delegate's own declaration; what it is bound to at run time is not known statically.
            Function* target = nullptr;
            try { target = rt.functionAt(f.package, e.refs.at(0)); } catch (const std::exception&) {}
            if (target) record(f, target, Reach::Primary);
            ++delegateSites;
            for (const auto& kid : e.kids) walk(kid, f, self, self);
            return;
        }
        case script::EX_DelegateProperty: case script::EX_InstanceDelegate:
            // A function name stored into a delegate: treated as reachable (it is called when the delegate fires).
            callByName(f, recv, e.names.at(0), nullptr);
            ++delegateSites;
            return;
        case script::EX_Context: case script::EX_ClassContext: {
            walk(e.kids.at(0), f, self, recv);
            const Function* resolved = declaredFunction(e, f);
            Type inner = typeOf(e.kids.at(0), f, recv);
            if (!inner && resolved && resolved->owner && e.kids.at(0).op == script::EX_InterfaceContext) inner = {resolved->owner, true};
            walk(e.kids.at(1), f, self, inner, resolved);
            return;
        }
        default:
            for (const auto& kid : e.kids) walk(kid, f, self, recv);
        }
    }

    void walkFunction(Function& f) {
        if (f.isNative() || !walked.insert(&f).second) return;
        if (f.decodeFailed) { undecodable.insert(f.path); return; }
        if (!f.code) {
            try { f.code = std::make_shared<script::Code>(script::decode(*f.package, f.info)); }
            catch (const std::exception& error) {
                f.decodeFailed = true;
                f.decodeError = error.what();
                undecodable.insert(f.path);
                return;
            }
        }
        const Type self{f.owner, false};
        slotTypes.clear();
        for (const auto& statement : f.code->statements) learnSlots(statement, f, self);
        for (const auto& statement : f.code->statements) walk(statement, f, self, self);
    }
};

// ------------------------------------------------------------------------------------------------ JSON
std::string jsonCountMap(const std::map<std::string, size_t>& counts) {
    std::ostringstream out;
    out << '{';
    bool first = true;
    for (const auto& [key, count] : counts) { out << (first ? "" : ",") << quote(key) << ':' << count; first = false; }
    out << '}';
    return out.str();
}

std::string describeBrief(const Value& value) {
    if (value.kind == Value::Kind::Struct || value.kind == Value::Kind::Array) return value.kind == Value::Kind::Array ? "array" : "struct";
    return value.describe();
}

// The "Package.Class" of a function path "Package.Class.Function" (states add a level; the class is the second part).
std::string packageOf(const Function& function) { return function.package ? function.package->packageName : std::string(); }

struct NativeInfo {
    std::string path, key, package, owner, name;
    bool op = false, implemented = false;
    uint32_t index = 0;
};

NativeInfo nativeInfo(const Function& function) {
    NativeInfo info;
    info.path = function.path;
    info.key = function.nativeKey;
    info.package = packageOf(function);
    info.owner = function.owner ? function.owner->name : "?";
    info.name = function.name;
    info.op = (function.info.flags & (script::FUNC_Operator | script::FUNC_PreOperator)) != 0;
    info.implemented = bool(function.native);
    info.index = function.info.native;
    return info;
}

struct Command {
    std::vector<std::string> words;
    size_t line = 0;
};

struct Run {
    Function* function = nullptr;
    std::string self;
    std::vector<std::string> args;
    std::string status = "completed", reason, error, result;
    size_t steps = 0;
    std::map<std::string, size_t> scripts, natives, stubs, noneContexts, logCategories;
    std::vector<std::string> logHead;
};

} // namespace

std::string nativeCensus(Runtime& runtime, const std::string& entryFile, const CensusOptions& options) {
    std::vector<Command> commands;
    {
        std::istringstream lines(entryFile);
        std::string line;
        size_t number = 0;
        while (std::getline(lines, line)) {
            ++number;
            if (const auto hash = line.find('#'); hash != std::string::npos) line.resize(hash);
            auto words = tokens(line);
            if (!words.empty()) commands.push_back({std::move(words), number});
        }
    }

    std::map<std::string, Value> labels;
    const auto fail = [](const Command& command, const std::string& why) {
        return RuntimeError("entry file line " + std::to_string(command.line) + ": " + why);
    };
    const auto object = [&](const Command& command, const std::string& token) -> ObjectPtr {
        if (token.empty() || token[0] != '$') throw fail(command, "expected $label, got " + token);
        const auto found = labels.find(token.substr(1));
        if (found == labels.end() || found->second.kind != Value::Kind::Object || !found->second.o) throw fail(command, "unknown label " + token);
        return found->second.o;
    };
    const auto value = [&](const Command& command, const std::string& token) -> Value {
        if (token == "none") return Value::makeObject(nullptr);
        if (!token.empty() && token[0] == '$') return Value::makeObject(object(command, token));
        const auto colon = token.find(':');
        if (colon == std::string::npos) throw fail(command, "unknown value " + token);
        const auto kind = token.substr(0, colon), body = token.substr(colon + 1);
        try {
            if (kind == "i") return Value::makeInt(std::stoll(body));
            if (kind == "f") return Value::makeFloat(std::stod(body));
            if (kind == "b") return Value::makeBool(body == "1" || body == "true");
            if (kind == "y") return Value::makeByte(std::stoll(body));
            if (kind == "s") return Value::makeString(body);
            if (kind == "n") return Value::makeName(body);
            if (kind == "class") return Value::makeClass(runtime.findClass(body));
            if (kind == "struct") return runtime.newStruct(body);
        } catch (const RuntimeError&) { throw; }
        catch (const std::exception&) { throw fail(command, "bad value " + token); }
        throw fail(command, "unknown value kind " + kind);
    };
    // $label.Property, then any number of .Field or [index] steps into structs and arrays (the element must exist).
    const auto property = [&](const Command& command, const std::string& target) -> Value* {
        const auto dot = target.find('.');
        if (dot == std::string::npos) throw fail(command, "expected $label.Property, got " + target);
        const auto owner = object(command, target.substr(0, dot));
        Value* slot = nullptr;
        size_t at = dot + 1;
        while (at < target.size()) {
            size_t end = target.find_first_of(".[", at);
            if (end == std::string::npos) end = target.size();
            if (target[at] == '[') {
                const auto close = target.find(']', at);
                if (!slot || close == std::string::npos) throw fail(command, "bad index in " + target);
                const size_t index = std::stoul(target.substr(at + 1, close - at - 1));
                if (slot->kind != Value::Kind::Array || index >= slot->elements().size()) throw fail(command, "no element in " + target);
                slot = &slot->elements()[index];
                at = close + 1;
                if (at < target.size() && target[at] == '.') ++at;
                continue;
            }
            const std::string name = target.substr(at, end - at);
            slot = slot ? slot->field(name) : runtime.property(*owner, name);
            if (!slot) throw fail(command, "no property " + name + " in " + target);
            at = end < target.size() && target[end] == '.' ? end + 1 : end;
        }
        if (!slot) throw fail(command, "no property in " + target);
        return slot;
    };

    runtime.countCalls = true;
    runtime.stepLimit = options.stepLimit;
    std::vector<Run> runs;
    // Runs one entry on the VM and files what the call counters and the log recorded.
    const auto execute = [&](Run& run, const ObjectPtr& self, std::vector<Value> arguments) {
        runtime.log.clear();
        runtime.scriptCalls.clear(); runtime.nativeCalls.clear(); runtime.stubCalls.clear(); runtime.noneContexts.clear();
        try {
            run.result = describeBrief(runtime.call(*run.function, self, std::move(arguments)));
        } catch (const std::exception& problem) {
            run.status = "stopped";
            run.error = problem.what();
            run.reason = run.error.find("step limit") != std::string::npos ? "step limit"
                       : run.error.find("call depth") != std::string::npos ? "call depth limit" : "exception";
        }
        run.steps = runtime.steps;
        run.scripts = runtime.scriptCalls; run.natives = runtime.nativeCalls; run.stubs = runtime.stubCalls;
        run.noneContexts = runtime.noneContexts;
        // Log lines other than the stub lines the counters already cover, grouped with digits dropped.
        std::set<std::string> stubLines;
        for (const auto& [path, _] : run.stubs) {
            const Function* stub = runtime.countedFunctions.at(path);
            stubLines.insert("UNIMPLEMENTED " + (stub->nativeKey.empty() ? stub->path : stub->nativeKey));
        }
        for (const auto& line : runtime.log) {
            if (stubLines.count(line)) continue;
            std::string category;
            for (const char c : line.substr(0, 90)) category += (c >= '0' && c <= '9') ? '#' : c;
            ++run.logCategories[category];
            if (run.logHead.size() < 12) run.logHead.push_back(line.substr(0, 200));
        }
        runs.push_back(std::move(run));
    };
    for (const auto& command : commands) {
        const auto& w = command.words;
        if (w[0] == "new" && w.size() == 3) {
            labels[w[1]] = Value::makeObject(runtime.instantiate(runtime.findClass(w[2]), w[1]));
        } else if (w[0] == "export" && w.size() == 4) {
            auto package = runtime.package(w[2]);
            const int32_t index = runtime.findExport(*package, w[3]);
            if (index <= 0) throw fail(command, "export not found: " + w[2] + " " + w[3]);
            labels[w[1]] = Value::makeObject(runtime.instantiateExport(package, index, 4));
        } else if (w[0] == "set" && w.size() == 3) {
            *property(command, w[1]) = value(command, w[2]);
        } else if (w[0] == "add" && w.size() == 3) {
            Value* slot = property(command, w[1]);
            if (slot->kind != Value::Kind::Array) *slot = Value::makeArray();
            slot->elements().push_back(value(command, w[2]));
        } else if (w[0] == "run" && w.size() >= 3) {
            Run run;
            run.function = runtime.findFunction(w[1]);
            run.self = w[2];
            run.args.assign(w.begin() + 3, w.end());
            ObjectPtr self = w[2] == "-" ? nullptr : object(command, w[2]);
            std::vector<Value> arguments;
            for (const auto& token : run.args) arguments.push_back(value(command, token));
            execute(run, self, std::move(arguments));
        } else if (w[0] == "runclass" && w.size() == 2) {
            // Every script function the class itself declares, once each on one object built from its defaults, with no
            // arguments (omitted parameters read as zero). Natives and inherited functions are not entries.
            Class* cls = runtime.findClass(w[1]);
            ObjectPtr self = runtime.instantiate(cls);
            std::map<std::string, Function*> declared;
            for (const auto& [_, function] : cls->functions)
                if (!function->isNative()) declared[function->name] = function;
            for (const auto& [_, function] : declared) {
                Run run;
                run.function = function;
                run.self = "new " + cls->path;
                execute(run, self, {});
            }
        } else {
            throw fail(command, "cannot parse: " + w[0]);
        }
    }
    runtime.countCalls = false;

    // ------------------------------------------------------------------------------------ static closure
    Closure closure(runtime);
    std::vector<std::set<Function*>> reachA(runs.size()), reachB(runs.size());   // per entry, closures A and B
    std::set<Function*> unionA, unionB;
    if (options.staticClosure) {
        for (size_t i = 0; i < runs.size(); ++i) {
            closure.analyse(*runs[i].function);
            reachA[i] = closure.reachableFrom(*runs[i].function, false);
            reachB[i] = closure.reachableFrom(*runs[i].function, true);
            unionA.insert(reachA[i].begin(), reachA[i].end());
            unionB.insert(reachB[i].begin(), reachB[i].end());
        }
    }

    // ----------------------------------------------------------------------------------------- report
    std::map<std::string, NativeInfo> natives;                 // every native seen, dynamic or static
    std::map<std::string, Function*> nativeFunctions;          // the same, by path
    std::map<std::string, std::map<size_t, size_t>> dynamicBy;  // native path -> entry -> calls
    std::map<std::string, size_t> scriptDynamic;
    for (size_t i = 0; i < runs.size(); ++i) {
        for (const auto* map : {&runs[i].natives, &runs[i].stubs})
            for (const auto& [path, count] : *map) {
                Function* function = runtime.countedFunctions.at(path);
                natives.emplace(path, nativeInfo(*function));
                nativeFunctions.emplace(path, function);
                dynamicBy[path][i] += count;
            }
        for (const auto& [path, count] : runs[i].scripts) scriptDynamic[path] += count;
    }
    for (const auto& [function, _] : closure.targets)
        if (function->isNative()) {
            natives.emplace(function->path, nativeInfo(*function));
            nativeFunctions.emplace(function->path, function);
        }
    std::map<std::string, char> scriptClosure;                  // every script function seen: 'A', 'B' or '-' (dynamic only)
    for (const auto& [path, _] : scriptDynamic) scriptClosure[path] = '-';
    for (const Function* function : unionB) if (!function->isNative()) scriptClosure[function->path] = 'B';
    for (const Function* function : unionA) if (!function->isNative()) scriptClosure[function->path] = 'A';

    std::ostringstream out;
    out << "{\"entries\":[";
    for (size_t i = 0; i < runs.size(); ++i) {
        const Run& run = runs[i];
        size_t implemented = 0, stubbed = 0, scriptTotal = 0;
        for (const auto& [_, n] : run.natives) implemented += n;
        for (const auto& [_, n] : run.stubs) stubbed += n;
        for (const auto& [_, n] : run.scripts) scriptTotal += n;
        out << (i ? "," : "") << "{\"index\":" << i << ",\"function\":" << quote(run.function->path) << ",\"self\":" << quote(run.self)
            << ",\"args\":[";
        for (size_t a = 0; a < run.args.size(); ++a) out << (a ? "," : "") << quote(run.args[a]);
        out << "],\"params\":[";
        for (size_t a = 0; a < run.function->params.size(); ++a) out << (a ? "," : "") << quote(run.function->params[a].name);
        out << "],\"status\":" << quote(run.status) << ",\"stop_reason\":" << (run.reason.empty() ? "null" : quote(run.reason))
            << ",\"error\":" << (run.error.empty() ? "null" : quote(run.error)) << ",\"result\":" << (run.status == "completed" ? quote(run.result) : "null")
            << ",\"steps\":" << run.steps << ",\"script_functions_entered\":" << run.scripts.size() << ",\"script_calls\":" << scriptTotal
            << ",\"native_calls_implemented\":" << implemented << ",\"native_calls_stub\":" << stubbed
            << ",\"distinct_natives_implemented\":" << run.natives.size() << ",\"distinct_natives_stub\":" << run.stubs.size()
            << ",\"none_contexts\":" << jsonCountMap(run.noneContexts) << ",\"log_categories\":" << jsonCountMap(run.logCategories)
            << ",\"log_head\":[";
        for (size_t l = 0; l < run.logHead.size(); ++l) out << (l ? "," : "") << quote(run.logHead[l]);
        out << "]";
        if (options.staticClosure) {
            for (const bool upper : {false, true}) {
                const auto& reach = upper ? reachB[i] : reachA[i];
                const char* name = upper ? "b" : "a";
                size_t scripts = 0, nativeCount = 0, implementedStatic = 0;
                for (Function* function : reach) {
                    if (!function->isNative()) ++scripts;
                    else { ++nativeCount; if (function->native) ++implementedStatic; }
                }
                out << ",\"static_" << name << "_script_functions\":" << scripts << ",\"static_" << name << "_natives\":" << nativeCount
                    << ",\"static_" << name << "_natives_implemented\":" << implementedStatic;
            }
        }
        out << "}";
    }

    out << "],\"script_functions\":{";
    bool first = true;
    for (const auto& [path, closureName] : scriptClosure) {
        const auto dynamic = scriptDynamic.find(path);
        out << (first ? "" : ",") << quote(path) << ":{\"dynamic_calls\":" << (dynamic == scriptDynamic.end() ? 0 : dynamic->second)
            << ",\"static_closure\":" << quote(std::string(1, closureName)) << "}";
        first = false;
    }

    out << "},\"natives\":{";
    first = true;
    for (const auto& [path, info] : natives) {
        size_t dynamicTotal = 0;
        std::string byEntry = "{";
        if (const auto found = dynamicBy.find(path); found != dynamicBy.end()) {
            bool firstEntry = true;
            for (const auto& [entry, count] : found->second) { byEntry += (firstEntry ? "" : ",") + quote(std::to_string(entry)) + ":" + std::to_string(count); firstEntry = false; dynamicTotal += count; }
        }
        byEntry += "}";
        size_t sitesA = 0, sitesB = 0;
        std::vector<std::pair<size_t, std::string>> callers;
        std::string entriesA = "[", entriesB = "[";
        if (options.staticClosure) {
            Function* function = nativeFunctions.at(path);
            if (const auto data = closure.targets.find(function); data != closure.targets.end()) {
                for (const auto& [caller, sites] : data->second.callers) {
                    if (unionA.count(caller)) sitesA += sites.primary;
                    if (unionB.count(caller)) {
                        sitesB += sites.primary + sites.upper;
                        callers.emplace_back(sites.primary + sites.upper, caller->path);
                    }
                }
            }
            bool firstA = true, firstB = true;
            for (size_t i = 0; i < runs.size(); ++i) {
                if (reachA[i].count(function)) { entriesA += (firstA ? "" : ",") + std::to_string(i); firstA = false; }
                if (reachB[i].count(function)) { entriesB += (firstB ? "" : ",") + std::to_string(i); firstB = false; }
            }
        }
        entriesA += "]";
        entriesB += "]";
        std::sort(callers.begin(), callers.end(), [](const auto& a, const auto& b) { return a.first != b.first ? a.first > b.first : a.second < b.second; });
        out << (first ? "" : ",") << quote(path) << ":{\"key\":" << quote(info.key) << ",\"package\":" << quote(info.package)
            << ",\"owner\":" << quote(info.owner) << ",\"name\":" << quote(info.name) << ",\"native_index\":" << info.index
            << ",\"operator\":" << (info.op ? "true" : "false") << ",\"implemented\":" << (info.implemented ? "true" : "false")
            << ",\"dynamic_calls\":" << dynamicTotal << ",\"dynamic_by_entry\":" << byEntry
            << ",\"static_sites_a\":" << sitesA << ",\"static_sites_b\":" << sitesB
            << ",\"static_entries_a\":" << entriesA << ",\"static_entries_b\":" << entriesB << ",\"top_callers\":[";
        for (size_t c = 0; c < callers.size() && c < 3; ++c) out << (c ? "," : "") << quote(callers[c].second);
        out << "]}";
        first = false;
    }
    out << "}";

    if (options.staticClosure) {
        size_t scriptTargets = 0, nativeTargets = 0;
        for (const auto& [function, _] : closure.targets) (function->isNative() ? nativeTargets : scriptTargets)++;
        out << ",\"static\":{\"script_functions_walked\":" << closure.scriptsWalked() << ",\"called_script_functions\":" << scriptTargets
            << ",\"called_natives\":" << nativeTargets << ",\"delegate_sites\":" << closure.delegateSites
            << ",\"unresolved_virtual\":" << jsonCountMap(closure.unresolvedVirtual) << ",\"missing_method\":" << jsonCountMap(closure.missingMethod)
            << ",\"unknown_native_index\":{";
        bool firstIndex = true;
        for (const auto& [index, count] : closure.unknownNativeIndex) { out << (firstIndex ? "" : ",") << quote(std::to_string(index)) << ':' << count; firstIndex = false; }
        out << "},\"undecodable\":[";
        bool firstPath = true;
        for (const auto& path : closure.undecodable) { out << (firstPath ? "" : ",") << quote(path); firstPath = false; }
        // Script functions the dynamic runs entered that closure B does not contain: a check on the walker.
        out << "],\"dynamic_outside_static\":[";
        firstPath = true;
        for (const auto& [path, closureName] : scriptClosure)
            if (closureName == '-') { out << (firstPath ? "" : ",") << quote(path); firstPath = false; }
        out << "]}";
    }
    out << "}";
    return out.str();
}

} // namespace vm
