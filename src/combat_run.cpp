#include "combat.hpp"
#include "behavior.hpp"
#include "mission_script.hpp"

#include <chrono>
#include <cmath>
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
// What a swing step can name instead of a definition path (the stock bullymong data of Ice_Dynamic): the melee definitions, and the Slinger's thrown rock as the throw
// behavior plus the projectile's Behavior_CauseDamage. The path of the throw is the one of the Slinger's first throw animation (UNVERIFIED that it is the one used live).
struct Swing { const char* name; const char* definition; const char* causeDamage; };
const Swing swings[] = {
    {"melee", "GD_PrimalBeastShared.Melee.MeleeDef_PrimalBeast_Melee", nullptr},
    {"leap", "GD_PrimalBeastShared.Melee.MeleeDef_PrimalBeast_RadiusLeap", nullptr},
    {"badass", "GD_PrimalBeastShared.Melee.MeleeDef_PrimalBeast_RadiusBadass", nullptr},
    {"grab", "GD_PrimalBeastShared.Melee.MeleeDef_PrimalBeast_MeleeGrabBaby", nullptr},
    {"charge", "GD_PrimalBeastShared.Melee.MeleeDef_Melee_Charge", nullptr},
    {"slinger", "GD_PrimalBeastShared.Anims.AnimMotion_SlingerThrow1.BehaviorProviderDefinition_43.Behavior_AIThrowProjectileAtTarget_142",
     "GD_PrimalBeastShared.Projectiles.Projectile_IceRock.BehaviorProviderDefinition_0.Behavior_CauseDamage_13"},
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
    // `script`: the mission bridge a run with a `maya` step is built over (its controller is the player, its experience pool and level-up chain take the kill credit);
    // without it the run has its own combat system and the player of the `player` step.
    Run(Runtime& runtime, MissionScript* script)
        : runtime_(runtime), evaluator_(runtime), ownCombat_(script ? nullptr : std::make_unique<CombatSystem>(runtime, evaluator_)),
          combat_(script ? script->combat() : *ownCombat_), script_(script) {
        try { runtime.package("Startup"); } catch (const std::exception&) {}
        combat_.setTakeDamageHandler([this](const CombatSystem::TakeDamageEvent& event) { raiseTakeDamage(event); });
        combat_.setBehaviorEventHandler([this](const CombatSystem::BehaviorEvent& event) { raiseBehaviorEvent(event); });
        combat_.enableDeaths();
    }

    void apply(const std::string& step);
    std::string report(double elapsed);
    bool failed() const { return !errors_.empty(); }

private:
    Runtime& runtime_;
    AttributeEvaluator evaluator_;
    std::unique_ptr<CombatSystem> ownCombat_;
    CombatSystem& combat_;
    MissionScript* script_;
    std::vector<Spawned> pawns_;
    std::vector<CombatSystem::Player> players_;
    std::map<const Object*, std::unique_ptr<BehaviorProvider>> providers_;    // one class provider per pawn that was hit or whose sequences were set
    std::map<const BehaviorProvider*, size_t> reported_;       // how many trace lines of a death-chain provider the hit's event list already has
    std::map<const Object*, std::unique_ptr<BehaviorProvider>> deathProviders_;   // the provider of each pawn's death definition (BodyClassDeathDefinition), made when it raises its first event
    std::map<std::string, int> hitCounts_;
    std::map<std::string, double> rows_;
    // A kill's row (kill.<class>.L<level>.p<player level>[.<tag>]): the award CalculateExperiencePointsForKill returned and the experience the player's pool gained.
    struct KillRow { std::string id; double award = 0; bool credited = false; int credit = 0; };
    std::vector<KillRow> killRows_;
    std::string killTag_;
    size_t killsSeen_ = 0;
    std::vector<std::string> results_, errors_;

    Spawned& pawn(const std::string& index);
    Value call(const ObjectPtr& self, const char* name, std::vector<Value> args = {}, std::vector<Value>* outs = nullptr);
    void row(const std::string& id, double value);
    Value managerOf(const Spawned& spawned);
    BehaviorProvider& providerFor(const ObjectPtr& pawn);
    std::unique_ptr<BehaviorProvider> makeProvider(const Value& definition);
    BehaviorProvider& deathProviderFor(const ObjectPtr& pawn, const ObjectPtr& deathDefinition);
    std::vector<int> runScriptBehavior(BehaviorProvider& provider, BehaviorProvider::Behavior& behavior, const ObjectPtr& pawn);
    void raiseBehaviorEvent(const CombatSystem::BehaviorEvent& event);
    void raiseTakeDamage(const CombatSystem::TakeDamageEvent& event);
    std::string hitJson(const CombatSystem::HitReport& report);
    std::string playerJson();
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
        } else if (verb == "maya") {
            // maya:<level>[:<experience> | :short<n>]: the bridge's player at that level with that experience (default 0; shortN = N points short of the next level), health pool
            // full for the level. May be given again between kills to change the player's level (the level is an input).
            if (!script_) throw RuntimeError("combat-run: maya needs the mission bridge (it is built when a maya step is among the steps)");
            const int level = std::stoi(parts.at(1));
            script_->setPlayerExperience(level, 0);
            if (parts.size() > 2) {
                const std::string& amount = parts[2];
                int64_t experience = 0;
                if (amount.rfind("short", 0) == 0) {
                    const Value* next = runtime_.property(*runtime_.property(*script_->playerController(), "PlayerReplicationInfo")->o, "ExpPointsNextLevelAt");
                    experience = next->integer() - std::stoll(amount.substr(5));
                } else {
                    experience = std::stoll(amount);
                }
                script_->setPlayerExperience(level, experience);
            }
            result = "{\"player\":" + playerJson() + "}";
        } else if (verb == "tick") {
            // tick:<seconds>: game time passes and the experience pool's per-frame update runs (ApplyExpPointsToExpLevel: the installed ExpLevelUp while the pool has reached the
            // next level), as the host's tick does. The level-up's gains are listed.
            if (!script_) throw RuntimeError("combat-run: tick needs the mission bridge");
            script_->advanceTime(std::stod(parts.at(1)));
            script_->updateExperiencePool();
            std::string gains;
            const char* gainKinds[] = {"experience", "level", "skill_points", "max_health", "health", "weapon_damage_scale", "action_skill_cooldown"};
            for (const auto& gain : script_->takeGains())
                gains += std::string(gains.empty() ? "" : ",") + "{\"kind\":\"" + gainKinds[int(gain.kind)] + "\",\"amount\":" + std::to_string(gain.amount) + ",\"health\":" + number(gain.health)
                         + ",\"maximum\":" + number(gain.maximum) + "}";
            result = "{\"gains\":[" + gains + "],\"player\":" + playerJson() + "}";
        } else if (verb == "players") {
            // players:<n>: the game's EffectiveNumPlayers, which picks the row of the party-weight table (the VM has one local player; a second is for the synthetic tables).
            const Value* game = runtime_.property(*combat_.world(), "Game");
            Value* effective = game && game->kind == Value::Kind::Object && game->o ? runtime_.property(*game->o, "EffectiveNumPlayers") : nullptr;
            if (!effective) throw RuntimeError("combat-run: the world has no game with EffectiveNumPlayers");
            *effective = Value::makeInt(std::stoi(parts.at(1)));
        } else if (verb == "killtag") {
            killTag_ = parts.size() > 1 ? parts[1] : "";
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
        } else if (verb == "swing") {
            // swing:<attacker>:<kind | definition path>[:<cause damage path>]: one blow of pawn n at the player (Maya's VM pawn when the run has a maya step, else player 0; plan step S-e, a fixture: it always connects). The definition is
            // a kind of the table above, or the path of a MeleeDefinition (or of a Behavior_AIThrowProjectileAtTarget with the projectile's Behavior_CauseDamage as the third part) in the
            // attacker's package. The damage is evaluated with the attacker's mind, then goes through the player's stock TakeDamage with the mind as the instigator. Rows
            // arrival.<class>.L<level> (the damage as it arrives) and ratio.<class>.L<level> (what reached the player's health over the arrival).
            if (parts.size() < 3 || parts.size() > 4) throw RuntimeError("combat-run: swing:<attacker>:<kind or definition path>[:<cause damage path>]");
            if (!script_ && players_.empty()) throw RuntimeError("combat-run: a swing needs a player to hit (the step maya:<level>, Maya's VM pawn, or player:<level>)");
            Spawned& attacker = pawn(parts.at(1));
            std::string definition = parts[2], causeDamage = parts.size() == 4 ? parts[3] : "";
            for (const Swing& kind : swings)
                if (definition == kind.name) { definition = kind.definition; causeDamage = kind.causeDamage ? kind.causeDamage : ""; }
            const auto package = runtime_.package(attacker.package);
            const auto instantiate = [&](const std::string& path) {
                const int32_t index = runtime_.findExport(*package, path);
                if (index <= 0) throw RuntimeError("combat-run: " + attacker.package + " has no " + path);
                return runtime_.instantiateExport(package, index, 4);
            };
            const auto delivery = combat_.deliveryOf(instantiate(definition), causeDamage.empty() ? nullptr : instantiate(causeDamage));
            const auto swing = script_ ? script_->swingAtPlayer(attacker.enemy.mind, delivery) : combat_.swing(attacker.enemy.mind, players_[0].pawn, delivery);
            const std::string id = attacker.rowClass + ".L" + std::to_string(attacker.level);
            row("arrival." + id, swing.arrival);
            if (swing.arrival > 0) row("ratio." + id, swing.hit.applied / swing.arrival);
            std::ostringstream out;
            out << "{\"definition\":" << quote(delivery.what) << ",\"source\":" << quote(swing.source) << ",\"damage_type\":" << quote(swing.damageType)
                << ",\"arrival\":" << number(swing.arrival) << ",\"applied\":" << number(swing.hit.applied) << ",\"ratio\":" << number(swing.arrival > 0 ? swing.hit.applied / swing.arrival : 0)
                << ",\"shield_damage\":" << number(swing.hit.shieldDamage) << ",\"health_lost\":" << number(swing.hit.healthBefore - swing.hit.healthAfter)
                << ",\"hit\":" << hitJson(swing.hit) << ",\"player\":" << (script_ ? playerJson() : "null") << "}";
            result = out.str();
        } else if (verb == "hit") {
            // hit:<pawn>:<damage>:<damage type definition path>[:<source>] by player 0; source is a short name (Pistol), a class name or a class path.
            if (parts.size() < 4 || parts.size() > 5) throw RuntimeError("combat-run: hit:<pawn>:<damage>:<damage type path>[:<source>]");
            if (players_.empty() && !script_) throw RuntimeError("combat-run: a hit needs a player (the step player:<level> or maya:<level>) first");
            Spawned& spawned = pawn(parts.at(1));
            std::string source = parts.size() == 5 ? parts[4] : "Bullet";
            if (source.find('.') == std::string::npos) source = "WillowGame." + std::string(source.rfind("WillowDmgSource_", 0) == 0 ? "" : "WillowDmgSource_") + source;
            const float poolBefore = script_ ? script_->experiencePool() : 0.f;
            const int playerLevel = script_ ? script_->playerLevel() : 0;
            const auto report = script_ ? script_->hitFromPlayer(spawned.enemy.pawn, std::stod(parts.at(2)), parts.at(3), source)
                                        : combat_.hit(spawned.enemy.pawn, players_[0].controller, std::stod(parts.at(2)), parts.at(3), runtime_.findClass(source));
            // The kills this hit made (the experience natives record them): one row per kill, the credit being what the player's pool gained.
            for (; killsSeen_ < combat_.kills().size(); ++killsSeen_) {
                const auto& kill = combat_.kills()[killsSeen_];
                if (kill.killed != spawned.enemy.pawn) continue;
                KillRow killRow;
                killRow.id = "kill." + spawned.rowClass + ".L" + std::to_string(spawned.level) + ".p" + std::to_string(kill.credits.empty() ? playerLevel : kill.credits[0].playerLevel);
                if (!killTag_.empty()) killRow.id += "." + killTag_;
                killRow.award = kill.award;
                if (script_ && !kill.credits.empty()) { killRow.credited = true; killRow.credit = int(std::lround(script_->experiencePool() - poolBefore)); }
                killRows_.push_back(killRow);
            }
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
        if (parts.size() > 1 && parts[0] != "spawn" && parts[0] != "base" && parts[0] != "player" && parts[0] != "attr" && parts[0] != "maya" && parts[0] != "tick" && parts[0] != "killtag" && parts[0] != "players" && std::stoul(parts[1]) < pawns_.size()) {
            result.pop_back();
            result += (result.size() > 1 ? "," : "") + std::string("\"state\":{") + stateJson(std::stoul(parts[1])) + "}}";
        }
    } catch (const std::exception&) {}
    results_.push_back("{\"step\":" + quote(step) + ",\"result\":" + result + "}");
}

// A provider over a BehaviorProviderDefinition value (an export, or an import the VM resolves by the same path in Startup, as instantiateReference does).
std::unique_ptr<BehaviorProvider> Run::makeProvider(const Value& definition) {
    if (definition.kind != Value::Kind::Object || !definition.o || !definition.o->resourcePackage) throw RuntimeError("combat-run: no behavior provider definition");
    auto package = definition.o->resourcePackage;
    int32_t index = definition.o->resourceIndex;
    if (index < 0) {
        const std::string path = package->path(index);
        package = runtime_.package("Startup");
        index = runtime_.findExport(*package, path);
    }
    return std::make_unique<BehaviorProvider>(runtime_, package, index);
}

// A behavior that is installed script runs its ApplyBehaviorToContext on the VM, once per context object the kernel resolves for it; the output links its script
// selects (BehaviorKernel.ActivateBehaviorOutputLink) are the ids the provider follows.
std::vector<int> Run::runScriptBehavior(BehaviorProvider& provider, BehaviorProvider::Behavior& behavior, const ObjectPtr& pawn) {
    std::vector<int> ids;
    std::vector<int>* const outer = combat_.selectLinks(&ids);
    Function* apply = runtime_.findMethod(behavior.object->cls, "ApplyBehaviorToContext");
    if (!apply) throw RuntimeError("combat-run: no ApplyBehaviorToContext on " + behavior.cls);
    try {
        for (const ObjectPtr& context : provider.contexts(behavior, pawn)) {
            std::vector<Value> args(apply->params.size());
            for (size_t i = 0; i < args.size(); ++i) {
                if (apply->params[i].name == "ContextObject") args[i] = Value::makeObject(context);
                else if (apply->params[i].name == "SelfObject") args[i] = Value::makeObject(pawn);
            }
            runtime_.call(*apply, behavior.object, std::move(args));
        }
    } catch (...) { combat_.selectLinks(outer); throw; }
    combat_.selectLinks(outer);
    return ids;
}

// The class provider of a pawn (its AI class's BehaviorProviderDefinition), registered as the stock spawn would: made once. The behavior classes the dummy and
// the bullymongs reach that act on the world are listed at the boundary (reported, not run), the hold behavior and the conditional run their installed script.
BehaviorProvider& Run::providerFor(const ObjectPtr& pawn) {
    const auto found = providers_.find(pawn.get());
    if (found != providers_.end()) return *found->second;
    const Value* aiClass = runtime_.property(*pawn, "AIClass");
    const Value* definition = aiClass && aiClass->o ? runtime_.property(*aiClass->o, "BehaviorProviderDefinition") : nullptr;
    if (!definition) throw RuntimeError("combat-run: the pawn's AI class has no behavior provider");
    auto provider = makeProvider(*definition);
    BehaviorProvider& made = *provider;
    providers_[pawn.get()] = std::move(provider);
    made.handle("GearboxFramework.Behavior_AIHold", [this, pawn](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
        for (const ObjectPtr& context : p.contexts(b, pawn))
            runtime_.callByName(b.object, "ApplyBehaviorToContext", {Value::makeObject(combat_.aiInterfaceFor(context))});
        return std::vector<int>();
    });
    // Behavior_Conditional is installed script: its conditions are expression trees over the pawn (src/combat_death.cpp binds their Evaluate natives from the data).
    made.handle("WillowGame.Behavior_Conditional", [this, pawn](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
        if (Value* conditions = runtime_.property(*b.object, "Conditions"))       // the expression trees are references to data objects: built before the script calls Evaluate
            for (Value& entry : conditions->elements())
                if (Value* tree = entry.field("Condition"); tree && tree->kind == Value::Kind::Object && tree->o) *tree = Value::makeObject(combat_.built(*tree));
        return runScriptBehavior(p, b, pawn);
    });
    for (const char* cls : {"WillowGame.Behavior_Transform", "WillowGame.Behavior_AttemptStatusEffect", "WillowGame.Behavior_UpdateMissionObjective",
                            "WillowGame.Behavior_RegisterTargetable", "GearboxFramework.Behavior_ChangeRemoteBehaviorSequenceState", "Engine.Behavior_RemoteEvent"})
        made.reportAtBoundary(cls);
    made.reportAtBoundary("GearboxFramework.Behavior_SpecialMove", nullptr, {0});
    made.registerConsumer();
    reported_[&made] = made.trace.size();
    return made;
}

// The provider of a death definition (BodyClassDeathDefinition.BehaviorProviderDefinition), made once per pawn and registered like the class provider. Its
// script behaviors run on the VM; the ones that spawn effects, projectiles, sounds and dialog lines are listed at the boundary.
BehaviorProvider& Run::deathProviderFor(const ObjectPtr& pawn, const ObjectPtr& deathDefinition) {
    const auto found = deathProviders_.find(pawn.get());
    if (found != deathProviders_.end()) return *found->second;
    const Value* definition = deathDefinition ? runtime_.property(*deathDefinition, "BehaviorProviderDefinition") : nullptr;
    if (!definition) throw RuntimeError("combat-run: the death definition has no behavior provider");
    auto provider = makeProvider(*definition);
    BehaviorProvider& made = *provider;
    deathProviders_[pawn.get()] = std::move(provider);
    for (const char* cls : {"GearboxFramework.Behavior_CustomEvent", "GearboxFramework.Behavior_CompareBool"})
        made.handle(cls, [this, pawn](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) { return runScriptBehavior(p, b, pawn); });
    for (const char* cls : {"WillowGame.Behavior_CoordinatedEffect", "GearboxFramework.Behavior_TriggerDialogEvent", "WillowGame.Behavior_ChangeInstanceDataSwitch",
                            "WillowGame.PlayerBehavior_UnlockAchievementForAllPlayers", "WillowGame.Behavior_SpawnProjectile", "GearboxFramework.Behavior_PostAkEvent",
                            "WillowGame.Behavior_SpawnParticleSystem", "WillowGame.Behavior_CreateImpactEffect", "Engine.Behavior_Destroy",
                            "GearboxFramework.Behavior_ChangeRemoteBehaviorSequenceState", "WillowGame.Behavior_MissionCustomEvent", "WillowGame.Behavior_SpawnItems",
                            "WillowGame.Behavior_DamageSourceSwitch", "WillowGame.Behavior_AddObjectInstanceData"})
        made.reportAtBoundary(cls);
    made.registerConsumer();
    reported_[&made] = made.trace.size();
    return made;
}

// A behavior event of the death chain (BodyClassDeathDefinition.OnKilledBy and its kin, or a custom event a script behavior broadcast): raised on the death
// definition's provider, a broadcast on the pawn's class provider too. The behaviors that ran and what the provider could not run are added to the hit's event list.
void Run::raiseBehaviorEvent(const CombatSystem::BehaviorEvent& event) {
    std::vector<BehaviorProvider*> targets;
    const ObjectPtr definition = event.definition ? event.definition : (runtime_.property(*event.pawn, "MyDeathDef") ? runtime_.property(*event.pawn, "MyDeathDef")->o : nullptr);
    if (definition) targets.push_back(&deathProviderFor(event.pawn, definition));
    if (event.broadcast) {
        try { targets.push_back(&providerFor(event.pawn)); } catch (const std::exception&) {}
    }
    for (BehaviorProvider* provider : targets) {
        provider->fireEvent(event.name);
        size_t& reported = reported_[provider];           // an event a behavior raised on the same provider inside this one has already listed its lines
        for (; reported < provider->trace.size(); ++reported) combat_.note("behavior " + provider->trace[reported]);
        for (const auto& error : provider->errors) combat_.note("provider error: " + error);
        provider->errors.clear();
    }
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
    reported_[&provider] = provider.trace.size();
    for (const auto& error : provider.errors) combat_.note("provider error: " + error);
    provider.errors.clear();
}

std::string Run::hitJson(const CombatSystem::HitReport& report) {
    std::ostringstream out;
    out << "{\"health_before\":" << number(report.healthBefore) << ",\"health_after\":" << number(report.healthAfter) << ",\"arriving\":" << number(report.arriving)
        << ",\"applied\":" << number(report.applied) << ",\"shield_damage\":" << number(report.shieldDamage) << ",\"demigod_before\":" << (report.demigodBefore ? "true" : "false")
        << ",\"demigod_after\":" << (report.demigodAfter ? "true" : "false") << ",\"events\":[";
    for (size_t i = 0; i < report.events.size(); ++i) out << (i ? "," : "") << quote(report.events[i]);
    out << "],\"order\":[";
    bool firstCall = true;
    for (const std::string& call : report.order) {          // the operators (Core.Object) are left out: they are the arithmetic of the scripts
        if (call.rfind("Core.Object.", 0) == 0) continue;
        out << (firstCall ? "" : ",") << quote(call);
        firstCall = false;
    }
    out << "]}";
    return out.str();
}

// The bridge's player: level, experience pool, the next level's requirement, health and maximum (the VM pawn's natives over its health pool).
std::string Run::playerJson() {
    const ObjectPtr controller = script_->playerController();
    const ObjectPtr pawnObject = script_->playerPawnObject();
    const Value* info = runtime_.property(*controller, "PlayerReplicationInfo");
    std::ostringstream out;
    out << "{\"level\":" << script_->playerLevel() << ",\"experience\":" << number(script_->experiencePool())
        << ",\"next_level_at\":" << runtime_.property(*info->o, "ExpPointsNextLevelAt")->integer()
        << ",\"health\":" << number(call(pawnObject, "GetHealth").number()) << ",\"max_health\":" << number(call(pawnObject, "GetMaxHealth").number()) << "}";
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
    // The kill rows: {"award", "credit"} (credit only when the experience went into the bridge's pool). The same id more than once in a run (the same class, levels and tag)
    // becomes <id>.a, <id>.b, ... in kill order, so that repeated kills are separate rows.
    std::map<std::string, int> total, seen;
    for (const KillRow& kill : killRows_) ++total[kill.id];
    for (const KillRow& kill : killRows_) {
        const std::string id = total[kill.id] > 1 ? kill.id + "." + char('a' + seen[kill.id]++) : kill.id;
        out << (first ? "" : ",") << quote(id) << ":{\"award\":" << number(kill.award);
        if (kill.credited) out << ",\"credit\":" << kill.credit;
        out << "}";
        first = false;
    }
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
    // A run with a `maya` step is built over the mission bridge (src/mission_script.*): the player is its controller, whose experience pool and level-up chain take the kill
    // credit. The mission is the stock Fire mission the slice uses; it only gives the bridge its curve and globals, nothing of it is played.
    std::unique_ptr<MissionSystem> mission;
    std::unique_ptr<MissionScript> script;
    for (const auto& step : steps)
        if (step.rfind("maya:", 0) == 0) {
            mission = std::make_unique<MissionSystem>(runtime, "Startup", "GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire");
            script = std::make_unique<MissionScript>(runtime, *mission);
            break;
        }
    Run run(runtime, script.get());
    for (const auto& step : steps) run.apply(step);
    const std::string text = run.report(std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - started).count());
    if (failed) *failed = run.failed();
    return text;
}

} // namespace vm
