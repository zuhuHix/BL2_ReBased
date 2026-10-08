#include "combat.hpp"
#include "behavior.hpp"

#include <chrono>
#include <cstdio>
#include <iomanip>
#include <memory>
#include <map>
#include <sstream>

namespace vm {
namespace {
// The stock archetypes the CLI names: kind -> package and archetype path. Anything else is "<Package>/<archetype path>".
struct Kind { const char* name; const char* package; const char* path; };
const Kind kinds[] = {
    {"dummy", "Sanctuary_Dynamic", "GD_TargetDummy.Character.Pawn_TargetDummy"},
    {"regular", "Ice_Dynamic", "GD_PrimalBeast.Character.Pawn_PrimalBeast"},
    {"adult", "Ice_Dynamic", "GD_PrimalBeastAdult.Character.Pawn_PrimalBeastAdult"},
    {"ranged", "Ice_Dynamic", "GD_PrimalBeast_Ranged.Character.Pawn_PrimalBeast_Ranged"},
    {"baby", "Ice_Dynamic", "GD_PrimalBeastBaby.Character.Pawn_PrimalBeastBaby"},
};
// Where the per-level base of the enemy health formula sits (the same data is in the packages that hold an enemy: Sanctuary_Dynamic, Ice_Dynamic).
constexpr const char* baseHealthPath = "GD_Balance_HealthAndDamage.HealthAndDamage.Init_BaseEnemyHealth";

struct Spawned {
    std::string package, path, rowClass;
    int level = 0, stage = 0;
    double milliseconds = 0;
    CombatSystem::Enemy enemy;
    std::vector<Value> pools;          // the references the pool step made, in order
};

std::string quote(const std::string& text) {
    std::string out = "\"";
    for (const unsigned char ch : text) {
        if (ch == '"' || ch == '\\') { out += '\\'; out += char(ch); }
        else if (ch < 0x20) { char buffer[8]; std::snprintf(buffer, sizeof buffer, "\\u%04x", ch); out += buffer; }
        else out += char(ch);
    }
    return out + "\"";
}
std::string number(double value) {
    std::ostringstream out;
    out << std::setprecision(9) << value;
    return out.str();
}
std::string objectPath(const ObjectPtr& object) {
    return object && object->resourcePackage ? object->resourcePackage->path(object->resourceIndex) : "";
}
std::vector<std::string> split(const std::string& text) {
    std::vector<std::string> parts;
    std::stringstream stream(text);
    std::string part;
    while (std::getline(stream, part, ':')) parts.push_back(part);
    return parts;
}

class Run {
public:
    // Startup is always loaded in the game; the shared resource and attribute definitions (D_Resources.Health, D_Attributes.*) live there, and a reference
    // from another package resolves to them only while it is loaded. A package set without it (a synthetic one) has none.
    explicit Run(Runtime& runtime) : runtime_(runtime), evaluator_(runtime), combat_(runtime, evaluator_) {
        try { runtime.package("Startup"); } catch (const std::exception&) {}
        combat_.setTakeDamageHandler([this](const CombatSystem::TakeDamageEvent& event) { raiseTakeDamage(event); });
    }

    void apply(const std::string& step);
    std::string report(double elapsed);
    bool failed() const { return !errors_.empty(); }

private:
    Runtime& runtime_;
    AttributeEvaluator evaluator_;
    CombatSystem combat_;
    std::vector<Spawned> pawns_;
    std::vector<CombatSystem::Player> players_;
    std::map<const Object*, std::unique_ptr<BehaviorProvider>> providers_;    // one class provider per pawn that was hit or whose sequences were set
    std::map<std::string, int> hitCounts_;
    std::map<std::string, double> rows_;
    std::vector<std::string> results_, errors_;

    Spawned& pawn(const std::string& index);
    Value call(const ObjectPtr& self, const char* name, std::vector<Value> args = {}, std::vector<Value>* outs = nullptr);
    void row(const std::string& id, double value);
    Value managerOf(const Spawned& spawned);
    BehaviorProvider& providerFor(const ObjectPtr& pawn);
    void raiseTakeDamage(const CombatSystem::TakeDamageEvent& event);
    std::string hitJson(const CombatSystem::HitReport& report);
    std::string stateJson(size_t index);
    std::string pawnJson(size_t index);
};

Spawned& Run::pawn(const std::string& index) {
    const size_t at = std::stoul(index);
    if (at >= pawns_.size()) throw RuntimeError("combat-run: no pawn " + index);
    return pawns_[at];
}

// A script or native method by name, dispatched on the object's class (virtual lookup).
Value Run::call(const ObjectPtr& self, const char* name, std::vector<Value> args, std::vector<Value>* outs) {
    Function* function = runtime_.findMethod(self->cls, name);
    if (!function) throw RuntimeError(std::string("combat-run: ") + self->cls->name + " has no function " + name);
    return runtime_.call(*function, self, std::move(args), outs);
}

void Run::row(const std::string& id, double value) {
    std::string key = id;
    for (int copy = 2; rows_.count(key); ++copy) key = id + "#" + std::to_string(copy);
    rows_[key] = value;
}

Value Run::managerOf(const Spawned& spawned) {
    const Value* manager = runtime_.property(*spawned.enemy.mind, "ResourcePoolManager");
    return manager ? *manager : Value();
}

void Run::apply(const std::string& step) {
    std::string result = "{}";
    try {
        const auto parts = split(step);
        const std::string& verb = parts.at(0);
        if (verb == "spawn") {
            if (parts.size() < 3 || parts.size() > 4) throw RuntimeError("combat-run: spawn:<kind>:<level>[:<stage>]");
            Spawned spawned;
            const auto slash = parts[1].find('/');
            if (slash != std::string::npos) { spawned.package = parts[1].substr(0, slash); spawned.path = parts[1].substr(slash + 1); }
            for (const Kind& kind : kinds)
                if (parts[1] == kind.name) { spawned.package = kind.package; spawned.path = kind.path; }
            if (spawned.path.empty()) throw RuntimeError("combat-run: unknown pawn kind " + parts[1]);
            spawned.level = std::stoi(parts[2]);
            spawned.stage = parts.size() == 4 ? std::stoi(parts[3]) : spawned.level;
            const auto leaf = spawned.path.substr(spawned.path.rfind('.') + 1);
            spawned.rowClass = leaf.rfind("Pawn_", 0) == 0 ? leaf.substr(5) : leaf;
            const auto started = std::chrono::steady_clock::now();
            spawned.enemy = combat_.spawnEnemy(runtime_.package(spawned.package), spawned.path, spawned.level, spawned.stage);
            spawned.milliseconds = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - started).count();
            const std::string id = spawned.rowClass + ".L" + std::to_string(spawned.level);
            row("health." + id, call(spawned.enemy.pawn, "GetHealth").number());
            row("max_health." + id, call(spawned.enemy.pawn, "GetMaxHealth").number());
            pawns_.push_back(std::move(spawned));
            result = "{\"pawn\":" + std::to_string(pawns_.size() - 1) + ",\"milliseconds\":" + number(pawns_.back().milliseconds) + "}";
        } else if (verb == "base") {
            Spawned& spawned = pawn(parts.at(1));
            const auto package = runtime_.package(spawned.package);
            const int32_t index = runtime_.findExport(*package, baseHealthPath);
            if (index <= 0) throw RuntimeError(std::string("combat-run: the package has no ") + baseHealthPath);
            AttributeContext context;
            combat_.describe(spawned.enemy.pawn, context);
            const double value = evaluator_.evaluateDefinition(*runtime_.instantiateExport(package, index, 4), context);
            row("base.L" + std::to_string(spawned.level), value);
            result = "{\"value\":" + number(value) + "}";
        } else if (verb == "sethealth" || verb == "setmax") {
            Spawned& spawned = pawn(parts.at(1));
            call(spawned.enemy.pawn, verb == "sethealth" ? "SetHealth" : "SetMaxHealth", {Value::makeFloat(std::stod(parts.at(2)))});
        } else if (verb == "pool") {
            Spawned& spawned = pawn(parts.at(1));
            const auto package = runtime_.package(spawned.package);
            const int32_t index = runtime_.findExport(*package, parts.at(2));
            if (index <= 0) throw RuntimeError("combat-run: no such pool definition " + parts.at(2));
            const Value definition = Value::makeObject(runtime_.resource(package, index));
            // CreatePool is static on the manager class; the mind's manager variable is the out argument and is written back.
            Function* create = runtime_.findMethod(runtime_.findClass("Engine.ResourcePoolManager"), "CreatePool");
            if (!create) throw RuntimeError("combat-run: no CreatePool");
            std::vector<Value> outs;
            const Value reference = runtime_.call(*create, nullptr, {managerOf(spawned), Value::makeObject(spawned.enemy.mind), definition, Value::makeBool(false)}, &outs);
            *runtime_.property(*spawned.enemy.mind, "ResourcePoolManager") = outs.at(0);
            spawned.pools.push_back(reference);
            std::vector<Value> checked;
            const bool valid = call(spawned.enemy.pawn, "IsResourcePoolValid", {reference}, &checked).truth();
            result = "{\"valid\":" + std::string(valid ? "true" : "false") + ",\"slot\":" + std::to_string(reference.field("PoolIndexInManager")->integer())
                     + ",\"guid\":" + std::to_string(reference.field("PoolGUID")->integer()) + "}";
        } else if (verb == "copyref") {
            Spawned& spawned = pawn(parts.at(1));
            spawned.pools.push_back(spawned.pools.at(std::stoul(parts.at(2))));
        } else if (verb == "valid") {
            Spawned& spawned = pawn(parts.at(1));
            std::vector<Value> checked;
            const bool valid = call(spawned.enemy.pawn, "IsResourcePoolValid", {spawned.pools.at(std::stoul(parts.at(2)))}, &checked).truth();
            result = "{\"valid\":" + std::string(valid ? "true" : "false") + "}";
        } else if (verb == "unpool" || verb == "destroyhealth") {
            Spawned& spawned = pawn(parts.at(1));
            Value* held = verb == "destroyhealth" ? runtime_.property(*spawned.enemy.pawn, "HealthPool") : &spawned.pools.at(std::stoul(parts.at(2)));
            const Value manager = held->field("PoolManager") ? *held->field("PoolManager") : Value();
            if (manager.kind != Value::Kind::Object || !manager.o) throw RuntimeError("combat-run: the reference has no manager");
            Function* destroy = runtime_.findMethod(manager.o->cls, "DeletePoolByReference");
            if (!destroy) throw RuntimeError("combat-run: no DeletePoolByReference");
            std::vector<Value> outs;
            runtime_.call(*destroy, manager.o, {*held}, &outs);
            *held = outs.at(0);
        } else if (verb == "player") {
            players_.push_back(combat_.spawnPlayer(std::stoi(parts.at(1))));
            result = "{\"player\":" + std::to_string(players_.size() - 1) + "}";
        } else if (verb == "hold" || verb == "release") {
            // hold:<n>:<reason> / release:<n>:<reason>: the demigod hold of pawn n's AI component, as the AIHold behavior's script asks for it (HoldDemigod / ReleaseDemigod).
            Spawned& spawned = pawn(parts.at(1));
            const Value* component = runtime_.property(*spawned.enemy.mind, "AIComponent");
            if (!component || component->kind != Value::Kind::Object || !component->o) throw RuntimeError("combat-run: the pawn's mind has no AI component");
            call(component->o, verb == "hold" ? "HoldDemigod" : "ReleaseDemigod", {Value::makeName(parts.at(2)), Value::makeObject(nullptr)});
        } else if (verb == "attr") {
            // attr:<player>:<name>:<value>: a float attribute of player n's controller set as a state input (Maya's skills are not in the VM, so a live attribute
            // value, e.g. InstigatedGrenadeDamageModifier from a snapshot, is given here); the controller's modifier stack stays empty.
            if (players_.size() <= std::stoul(parts.at(1))) throw RuntimeError("combat-run: no such player " + parts.at(1));
            Value* attribute = runtime_.property(*players_[std::stoul(parts.at(1))].controller, parts.at(2));
            if (!attribute) throw RuntimeError("combat-run: the controller has no attribute " + parts.at(2));
            *attribute = Value::makeFloat(std::stod(parts.at(3)));
        } else if (verb == "sequence") {
            // sequence:<pawn>:<name>:on|off: the enable condition the mission would deliver (BehaviorSequenceEnableByMission is not in this driver).
            Spawned& spawned = pawn(parts.at(1));
            const bool on = parts.at(3) == "on";
            if (!on && parts.at(3) != "off") throw RuntimeError("combat-run: sequence:<pawn>:<name>:on|off");
            result = std::string("{\"changed\":") + (providerFor(spawned.enemy.pawn).setSequenceEnabled(parts.at(2), on) ? "true" : "false") + "}";
        } else if (verb == "hit") {
            // hit:<pawn>:<damage>:<damage type definition path>[:<source>] by player 0; source is a short name (Pistol), a class name or a class path.
            if (parts.size() < 4 || parts.size() > 5) throw RuntimeError("combat-run: hit:<pawn>:<damage>:<damage type path>[:<source>]");
            if (players_.empty()) throw RuntimeError("combat-run: a hit needs a player (the step player:<level>) first");
            Spawned& spawned = pawn(parts.at(1));
            std::string source = parts.size() == 5 ? parts[4] : "Bullet";
            if (source.find('.') == std::string::npos) source = "WillowGame." + std::string(source.rfind("WillowDmgSource_", 0) == 0 ? "" : "WillowDmgSource_") + source;
            const auto report = combat_.hit(spawned.enemy.pawn, players_[0].controller, std::stod(parts.at(2)), parts.at(3), runtime_.findClass(source));
            const std::string id = "hit." + spawned.rowClass + ".L" + std::to_string(spawned.level) + "." + std::to_string(++hitCounts_[spawned.rowClass + std::to_string(spawned.level)]);
            row(id + ".arriving", report.arriving);
            row(id + ".applied", report.applied);
            row(id + ".health_after", report.healthAfter);
            result = hitJson(report);
        } else {
            throw RuntimeError("combat-run: unknown step " + step);
        }
    } catch (const std::exception& error) {
        errors_.push_back(step + ": " + error.what());
        result = "{\"error\":" + quote(error.what()) + "}";
    }
    // A step that names a pawn also reports that pawn's state after it.
    try {
        const auto parts = split(step);
        if (parts.size() > 1 && parts[0] != "spawn" && parts[0] != "base" && parts[0] != "player" && parts[0] != "attr" && std::stoul(parts[1]) < pawns_.size()) {
            result.pop_back();
            result += (result.size() > 1 ? "," : "") + std::string("\"state\":{") + stateJson(std::stoul(parts[1])) + "}}";
        }
    } catch (const std::exception&) {}
    results_.push_back("{\"step\":" + quote(step) + ",\"result\":" + result + "}");
}

// The class provider of a pawn (its AI class's BehaviorProviderDefinition), registered as the stock spawn would: made once. The behavior classes the dummy and
// the bullymongs reach that act on the world are listed at the boundary (reported, not run), the hold behavior runs its installed script on the pawn.
BehaviorProvider& Run::providerFor(const ObjectPtr& pawn) {
    const auto found = providers_.find(pawn.get());
    if (found != providers_.end()) return *found->second;
    const Value* aiClass = runtime_.property(*pawn, "AIClass");
    const Value* definition = aiClass && aiClass->o ? runtime_.property(*aiClass->o, "BehaviorProviderDefinition") : nullptr;
    if (!definition || definition->kind != Value::Kind::Object || !definition->o || !definition->o->resourcePackage) throw RuntimeError("combat-run: the pawn's AI class has no behavior provider");
    auto package = definition->o->resourcePackage;
    int32_t index = definition->o->resourceIndex;
    if (index < 0) {                         // an import: the same export by path in Startup (as instantiateReference does)
        const std::string path = package->path(index);
        package = runtime_.package("Startup");
        index = runtime_.findExport(*package, path);
    }
    auto provider = std::make_unique<BehaviorProvider>(runtime_, package, index);
    BehaviorProvider& made = *provider;
    providers_[pawn.get()] = std::move(provider);
    made.handle("GearboxFramework.Behavior_AIHold", [this, pawn](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
        for (const ObjectPtr& context : p.contexts(b, pawn))
            runtime_.callByName(b.object, "ApplyBehaviorToContext", {Value::makeObject(combat_.aiInterfaceFor(context))});
        return std::vector<int>();
    });
    for (const char* cls : {"WillowGame.Behavior_Transform", "WillowGame.Behavior_AttemptStatusEffect", "WillowGame.Behavior_UpdateMissionObjective",
                            "WillowGame.Behavior_RegisterTargetable", "GearboxFramework.Behavior_ChangeRemoteBehaviorSequenceState", "Engine.Behavior_RemoteEvent"})
        made.reportAtBoundary(cls);
    made.reportAtBoundary("GearboxFramework.Behavior_SpecialMove", nullptr, {0});
    made.registerConsumer();
    return made;
}

// NotifyTakeHit's raise (NATIVE_DAMAGE_DEATH.md section 5): OnTakeDamage on the pawn's class provider with the damage type definition and source as the
// event's DamageType and DamageSource outputs (stock paths); the behaviors that ran are added to the hit's event list.
void Run::raiseTakeDamage(const CombatSystem::TakeDamageEvent& event) {
    combat_.note("OnTakeDamage raised (damage=" + number(event.damage) + ")");
    const Value* aiClass = runtime_.property(*event.pawn, "AIClass");
    const Value* definition = aiClass && aiClass->o ? runtime_.property(*aiClass->o, "BehaviorProviderDefinition") : nullptr;
    if (!definition || definition->kind != Value::Kind::Object || !definition->o) {         // a class with no behavior provider has nothing to raise it to
        combat_.note("the pawn's AI class has no behavior provider");
        return;
    }
    BehaviorProvider& provider = providerFor(event.pawn);
    const size_t before = provider.trace.size();
    provider.fireEvent("OnTakeDamage", {{"DamageType", objectPath(event.damageType)}, {"DamageSource", event.source ? event.source->path : ""}});
    for (size_t i = before; i < provider.trace.size(); ++i) combat_.note("behavior " + provider.trace[i]);
    for (const auto& error : provider.errors) combat_.note("provider error: " + error);
    provider.errors.clear();
}

std::string Run::hitJson(const CombatSystem::HitReport& report) {
    std::ostringstream out;
    out << "{\"health_before\":" << number(report.healthBefore) << ",\"health_after\":" << number(report.healthAfter) << ",\"arriving\":" << number(report.arriving)
        << ",\"applied\":" << number(report.applied) << ",\"shield_damage\":" << number(report.shieldDamage) << ",\"demigod_before\":" << (report.demigodBefore ? "true" : "false")
        << ",\"demigod_after\":" << (report.demigodAfter ? "true" : "false") << ",\"events\":[";
    for (size_t i = 0; i < report.events.size(); ++i) out << (i ? "," : "") << quote(report.events[i]);
    out << "]}";
    return out.str();
}

// A pawn's state: the natives' answers, the plain variables, the health reference and every occupied slot of the mind's manager.
std::string Run::stateJson(size_t index) {
    Spawned& spawned = pawns_[index];
    const ObjectPtr& pawnObject = spawned.enemy.pawn;
    std::ostringstream out;
    out << "\"demigod\":" << (combat_.demigod(pawnObject) ? "true" : "false") << ",\"health\":" << number(call(pawnObject, "GetHealth").number()) << ",\"max_health\":" << number(call(pawnObject, "GetMaxHealth").number())
        << ",\"health_var\":" << number(runtime_.property(*pawnObject, "HealthVar")->number())
        << ",\"health_max_var\":" << number(runtime_.property(*pawnObject, "HealthMaxVar")->number());
    Value reference = *runtime_.property(*pawnObject, "HealthPool");
    std::vector<Value> outs;
    const bool valid = call(pawnObject, "IsResourcePoolValid", {reference}, &outs).truth();
    out << ",\"pool_valid\":" << (valid ? "true" : "false") << ",\"pool_slot\":" << reference.field("PoolIndexInManager")->integer()
        << ",\"pool_guid\":" << reference.field("PoolGUID")->integer() << ",\"slots\":[";
    const Value manager = managerOf(spawned);
    bool first = true;
    if (manager.kind == Value::Kind::Object && manager.o) {
        const Value* slots = runtime_.property(*manager.o, "ResourcePools");
        for (size_t i = 0; slots && i < slots->elements().size(); ++i) {
            const Value& slot = slots->elements()[i];
            if (slot.kind != Value::Kind::Object || !slot.o) continue;
            const Value* definition = runtime_.property(*slot.o, "Definition");
            const Value* resource = definition && definition->o ? runtime_.property(*definition->o, "Resource") : nullptr;
            const Value* provider = runtime_.property(*slot.o, "AssociatedProvider");
            out << (first ? "" : ",") << "{\"slot\":" << i << ",\"guid\":" << runtime_.property(*slot.o, "PoolGUID")->integer()
                << ",\"resource\":" << quote(resource && resource->o ? objectPath(resource->o) : "")
                << ",\"current\":" << number(runtime_.property(*slot.o, "CurrentValue")->number())
                << ",\"max\":" << number(runtime_.property(*slot.o, "MaxValue")->number())
                << ",\"provider_is_mind\":" << (provider && provider->o == spawned.enemy.mind ? "true" : "false") << "}";
            first = false;
        }
    }
    out << "]";
    return out.str();
}

std::string Run::pawnJson(size_t index) {
    const Spawned& spawned = pawns_[index];
    std::ostringstream out;
    out << "{\"index\":" << index << ",\"archetype\":" << quote(spawned.package + "/" + spawned.path) << ",\"level\":" << spawned.level
        << ",\"stage\":" << spawned.stage << ",\"milliseconds\":" << number(spawned.milliseconds) << ',' << stateJson(index) << '}';
    return out.str();
}

std::string Run::report(double elapsed) {
    std::ostringstream out;
    out << "{\"rows\":{";
    bool first = true;
    for (const auto& [id, value] : rows_) { out << (first ? "" : ",") << quote(id) << ':' << number(value); first = false; }
    out << "},\"pawns\":[";
    for (size_t i = 0; i < pawns_.size(); ++i) {
        try { out << (i ? "," : "") << pawnJson(i); }
        catch (const std::exception& error) { errors_.push_back("report pawn " + std::to_string(i) + ": " + error.what()); out << (i ? "," : "") << "null"; }
    }
    out << "],\"steps\":[";
    for (size_t i = 0; i < results_.size(); ++i) out << (i ? "," : "") << results_[i];
    // The VM's log: natives without an implementation (counted) and every other line.
    const std::string prefix = "UNIMPLEMENTED ";
    std::map<std::string, size_t> stubs;
    std::vector<std::string> notes;
    for (const auto& line : runtime_.log) {
        if (line.rfind(prefix, 0) == 0) ++stubs[line.substr(prefix.size())];
        else notes.push_back(line);
    }
    out << "],\"stubs\":{";
    first = true;
    for (const auto& [name, count] : stubs) { out << (first ? "" : ",") << quote(name) << ':' << count; first = false; }
    out << "},\"notes\":[";
    for (size_t i = 0; i < notes.size(); ++i) out << (i ? "," : "") << quote(notes[i]);
    out << "],\"errors\":[";
    for (size_t i = 0; i < errors_.size(); ++i) out << (i ? "," : "") << quote(errors_[i]);
    out << "],\"elapsed_ms\":" << number(elapsed) << "}";
    return out.str();
}
} // namespace

std::string combatRun(Runtime& runtime, const std::vector<std::string>& steps, bool* failed) {
    const auto started = std::chrono::steady_clock::now();
    Run run(runtime);
    for (const auto& step : steps) run.apply(step);
    const std::string text = run.report(std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - started).count());
    if (failed) *failed = run.failed();
    return text;
}

} // namespace vm
