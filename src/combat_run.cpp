#include "combat.hpp"
#include "behavior.hpp"
#include "mission_script.hpp"

#include <algorithm>
#include <cctype>
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
// `balance` is the pawn's AIPawnBalanceDefinition in the same package (the loot step reads the pool lists and the item game stage from it); empty for a pawn that has none here.
struct Kind { const char* name; const char* package; const char* path; const char* balance; };
const Kind kinds[] = {
    {"dummy", "Sanctuary_Dynamic", "GD_TargetDummy.Character.Pawn_TargetDummy", ""},
    {"regular", "Ice_Dynamic", "GD_PrimalBeast.Character.Pawn_PrimalBeast", "GD_Population_PrimalBeast.Balance.PawnBalance_PrimalBeast"},
    {"adult", "Ice_Dynamic", "GD_PrimalBeastAdult.Character.Pawn_PrimalBeastAdult", "GD_Population_PrimalBeast.Balance.PawnBalance_PrimalBeastAdult"},
    {"ranged", "Ice_Dynamic", "GD_PrimalBeast_Ranged.Character.Pawn_PrimalBeast_Ranged", "GD_Population_PrimalBeast.Balance.PawnBalance_PrimalBeast_Slinger"},
    {"baby", "Ice_Dynamic", "GD_PrimalBeastBaby.Character.Pawn_PrimalBeastBaby", "GD_Population_PrimalBeast.Balance.PawnBalance_PrimalBeast_Monglet"},
};
constexpr const char* killDamageType = "GD_Impact.DamageType.DmgType_Normal";   // what the loot batch kills with
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
// Where the game times a step of the player's down state by an animation or a station's Kismet, neither of which the VM has, the bleed step plays the stock script's callback
// itself after the delay the live game showed (COMBAT_GOLDEN_TRACE.md section D steps 11 to 16; UNVERIFIED stand-ins, not derived from data): the injured special move's end
// (SwitchToSidearm), the injured-death animation's end (InjuredDeathAnimDone) and the respawn dialog (ShowRespawnDialog, scheduled by the station on the live path; its caller
// was not traced). The death camera is not one of them: the game's camera asks the pawn for its view every frame (CalcInjuredDeadCamera), and the installed script ends the
// camera itself after the delays of its data, so the bleed step makes that same call each frame while the camera is active.
constexpr double specialMoveEnd = 2.28, ragdollDelay = 0.84, respawnDialogDelay = 11.8;
// Where the per-level base of the enemy health formula sits (the same data is in the packages that hold an enemy: Sanctuary_Dynamic, Ice_Dynamic).
constexpr const char* baseHealthPath = "GD_Balance_HealthAndDamage.HealthAndDamage.Init_BaseEnemyHealth";

struct Spawned {
    std::string package, path, rowClass, balance;
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
    size_t dropsSeen_ = 0;                 // how many of the combat system's death rolls the steps have already reported
    // The player's down state (plan step S-f): the injury being played, with the times of what the callbacks above were played at. Everything about it is read from the
    // pawn's script variables; the number counts the injuries of the run.
    struct Episode {
        bool active = false, targeted = false, ragdolled = false, cameraEnded = false;
        int number = 0;
        double downAt = 0, targetedAt = -1, deadAt = -1, total = 0, successive = 0;
    };
    Episode episode_;
    int injuries_ = 0;
    double respawnAt_ = -1;       // when the respawn happened, while its dialog (the fee) is still due
    struct Fee { std::string injury; int64_t cash = 0, charged = 0; };
    std::vector<Fee> feeRows_;     // one per respawn dialog: the cash before and what ShowRespawnDialog charged
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
    std::string lootJson(const CombatSystem::LootDrop& drop);
    std::string lootAfter(const Spawned& spawned, const std::string& key);
    std::string lootBatch(const std::vector<std::string>& parts);
    std::string rollPoolSteps(const std::vector<std::string>& parts);
    std::string playerJson();
    Value callState(const ObjectPtr& self, const char* name, std::vector<Value> args = {});
    double worldNow();
    void trackDown();
    void bleed(double seconds, double dt);
    std::string downJson();
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
            if (parts.size() < 3 || parts.size() > 5) throw RuntimeError("combat-run: spawn:<kind>:<level>[:<stage>[:<balance path>]]");
            Spawned spawned;
            const auto slash = parts[1].find('/');
            if (slash != std::string::npos) { spawned.package = parts[1].substr(0, slash); spawned.path = parts[1].substr(slash + 1); }
            for (const Kind& kind : kinds)
                if (parts[1] == kind.name) { spawned.package = kind.package; spawned.path = kind.path; spawned.balance = kind.balance; }
            if (spawned.path.empty()) throw RuntimeError("combat-run: unknown pawn kind " + parts[1]);
            spawned.level = std::stoi(parts[2]);
            spawned.stage = parts.size() >= 4 ? std::stoi(parts[3]) : spawned.level;
            if (parts.size() == 5) spawned.balance = parts[4];       // the pawn balance of an archetype that is not one of the named kinds
            const auto leaf = spawned.path.substr(spawned.path.rfind('.') + 1);
            spawned.rowClass = leaf.rfind("Pawn_", 0) == 0 ? leaf.substr(5) : leaf;
            std::string lootSetup;
            const auto started = std::chrono::steady_clock::now();
            spawned.enemy = combat_.spawnEnemy(runtime_.package(spawned.package), spawned.path, spawned.level, spawned.stage);
            if (!spawned.balance.empty()) {         // the stock spawn's loot setup: the item game stage and the pawn's pool list (plan step S-g)
                const auto package = runtime_.package(spawned.package);
                const int32_t balance = runtime_.findExport(*package, spawned.balance);
                if (balance <= 0) throw RuntimeError("combat-run: " + spawned.package + " has no " + spawned.balance);
                combat_.setupLoot(spawned.enemy.pawn, runtime_.instantiateExport(package, balance, 4));
                lootSetup = ",\"loot_setup\":{\"item_stage\":" + std::to_string(runtime_.property(*spawned.enemy.pawn, "GameStageForSpawnedInventory")->integer()) + ",\"pools\":[";
                const Value* list = runtime_.property(*spawned.enemy.pawn, "ItemPoolList");
                for (size_t i = 0; list && i < list->elements().size(); ++i)
                    lootSetup += (i ? "," : "") + quote(objectPath(list->elements()[i].field("ItemPool")->o));
                lootSetup += "]}";
            }
            spawned.milliseconds = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - started).count();
            const std::string id = spawned.rowClass + ".L" + std::to_string(spawned.level);
            row("health." + id, call(spawned.enemy.pawn, "GetHealth").number());
            row("max_health." + id, call(spawned.enemy.pawn, "GetMaxHealth").number());
            pawns_.push_back(std::move(spawned));
            result = "{\"pawn\":" + std::to_string(pawns_.size() - 1) + ",\"milliseconds\":" + number(pawns_.back().milliseconds) + lootSetup + "}";
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
            combat_.preparePlayerPawn(script_->playerPawnObject());
            for (const char* name : {"DigistructCoordinatedEffect", "DigistructCoordinatedEffectKillVolume"}) script_->loadGlobalsReference(name);   // the death camera's end time
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
        } else if (verb == "cash") {
            // cash:<amount>: the player's credits on hand (a state input: the respawn fee is a share of it).
            if (!script_) throw RuntimeError("combat-run: cash needs the mission bridge (the step maya:<level>)");
            combat_.setCurrencyOnHand(*runtime_.property(*script_->playerController(), "PlayerReplicationInfo")->o, 0, std::stoll(parts.at(1)));
        } else if (verb == "bleed") {
            // bleed:<seconds>[:<dt>] (plan step S-f): game time passes in frames of dt seconds (default 1/60, about the live frame time) and, while Maya's pawn is in the injured state, her installed script Tick
            // runs each frame; the callbacks the game's animations, death camera and station time are played at the live delays (the constants above). Rows down.* per injury.
            if (!script_) throw RuntimeError("combat-run: bleed needs the mission bridge (the step maya:<level>)");
            if (parts.size() < 2 || parts.size() > 3) throw RuntimeError("combat-run: bleed:<seconds>[:<dt>]");
            const size_t logBefore = runtime_.log.size();
            bleed(std::stod(parts.at(1)), parts.size() == 3 ? std::stod(parts[2]) : 1.0 / 60.0);
            std::ostringstream out;
            out << "{\"time\":" << number(worldNow()) << ",\"down\":" << downJson() << ",\"player\":" << playerJson() << ",\"log\":[";
            bool first = true;
            for (size_t i = logBefore; i < runtime_.log.size(); ++i)
                if (runtime_.log[i].rfind("UNIMPLEMENTED ", 0) != 0) { out << (first ? "" : ",") << quote(runtime_.log[i]); first = false; }
            out << "]}";
            result = out.str();
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
                << ",\"hit\":" << hitJson(swing.hit) << ",\"player\":" << (script_ ? playerJson() : "null");
            if (script_) { trackDown(); if (episode_.active) out << ",\"down\":" << downJson(); }
            out << "}";
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
            if (const std::string lootText = lootAfter(spawned, id.substr(4)); !lootText.empty()) { result.pop_back(); result += ",\"loot\":" + lootText + "}"; }
            if (script_ && (episode_.active || injuries_ > 0)) { trackDown(); result.pop_back(); result += ",\"down\":" + downJson() + "}"; }
        } else if (verb == "seed") {
            // seed:<n>: the loot's random (C rand() stand-in, src/combat_loot.cpp) restarts from n; the default is fixed, so a run repeats.
            combat_.seedRandom(uint32_t(std::stoul(parts.at(1))));
        } else if (verb == "mayahealth") {
            // mayahealth:<share>: Maya's health set to that share of her maximum (a state input: the health drop weight looks at it).
            if (!script_) throw RuntimeError("combat-run: mayahealth needs the mission bridge (the step maya:<level>)");
            script_->setPlayerHealth(float(std::stod(parts.at(1)) * call(script_->playerPawnObject(), "GetMaxHealth").number()));
        } else if (verb == "rollpool") {
            result = rollPoolSteps(parts);
        } else if (verb == "lootbatch") {
            result = lootBatch(parts);
        } else {
            throw RuntimeError("combat-run: unknown step " + step);
        }
        if (script_) {          // a script run the bridge's guard caught (an exception inside the VM): the step failed
            for (const std::string& error : script_->errors) errors_.push_back(step + ": " + error);
            script_->errors.clear();
        }
    } catch (const std::exception& error) {
        errors_.push_back(step + ": " + error.what());
        result = "{\"error\":" + quote(error.what()) + "}";
    }
    // A step that names a pawn also reports that pawn's state after it.
    try {
        const auto parts = split(step);
        if (parts.size() > 1 && parts[0] != "spawn" && parts[0] != "base" && parts[0] != "player" && parts[0] != "attr" && parts[0] != "maya" && parts[0] != "tick" && parts[0] != "killtag" && parts[0] != "players" && parts[0] != "bleed" && parts[0] != "cash" && parts[0] != "seed" && parts[0] != "mayahealth" && parts[0] != "lootbatch" && std::stoul(parts[1]) < pawns_.size()) {
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

// A script method by name, dispatched on the object's class and its current state (the state's function wins, as the game's own calls resolve it).
Value Run::callState(const ObjectPtr& self, const char* name, std::vector<Value> args) {
    Function* function = runtime_.findMethod(self->cls, name, self->state);
    if (!function) throw RuntimeError(std::string("combat-run: ") + self->cls->name + " has no function " + name + " in state " + self->state);
    return runtime_.call(*function, self, std::move(args), nullptr);
}

double Run::worldNow() {
    const Value* time = runtime_.property(*combat_.world(), "TimeSeconds");
    return time ? time->number() : 0.0;
}

namespace {
bool isInjuredState(const ObjectPtr& pawn) {
    std::string state = pawn->state;
    for (char& ch : state) ch = char(std::tolower((unsigned char)ch));
    return state == "injured";
}
double numberOf(Runtime& runtime, Object& object, const char* name) {
    const Value* value = runtime.property(object, name);
    return value ? value->number() : 0.0;
}
}

// Starts the record of an injury when Maya's pawn has just entered the injured state (BeginState ran inside the hit), reading what the script worked out.
void Run::trackDown() {
    const ObjectPtr pawn = script_->playerPawnObject();
    if (episode_.active || !isInjuredState(pawn)) return;
    episode_ = Episode();
    episode_.active = true;
    episode_.number = ++injuries_;
    episode_.downAt = worldNow();
    episode_.total = numberOf(runtime_, *pawn, "TotalBleedoutTime");
    episode_.successive = numberOf(runtime_, *pawn, "NumSuccessiveInjuries");
    row("down.total." + std::to_string(episode_.number), episode_.total);
    row("down.successive." + std::to_string(episode_.number), episode_.successive);
}

// Game time in frames; see the bleed step. One frame: time moves on, then, in the injured state, the callbacks that are due and the script Tick.
void Run::bleed(double seconds, double dt) {
    if (dt <= 0) throw RuntimeError("combat-run: bleed: the frame time must be positive");
    for (long frame = 0, frames = std::lround(seconds / dt); frame < frames; ++frame) {
        script_->advanceTime(dt);
        const double now = worldNow();
        trackDown();
        const std::string n = std::to_string(episode_.number);
        script_->withPlayerPawn([&](const ObjectPtr& pawn) {
            if (episode_.active && isInjuredState(pawn)) {
                if (!episode_.targeted && now - episode_.downAt >= specialMoveEnd - 1e-9) {
                    callState(pawn, "SwitchToSidearm");
                    episode_.targeted = true;
                    episode_.targetedAt = now;
                }
                callState(pawn, "Tick", {Value::makeFloat(dt)});
                if (episode_.deadAt < 0 && numberOf(runtime_, *pawn, "InjuredDeadState") >= 1) {
                    episode_.deadAt = now;
                    // The script's own timer at that frame: the bleed-out total plus the part of the last frame past it (the game's frames were about 1/60 s).
                    row("down.bleed_over." + n, numberOf(runtime_, *pawn, "flInjuredTargetedTime") - episode_.total);
                    if (episode_.targetedAt >= 0) row("down.bleed_wall." + n, now - episode_.targetedAt);
                }
                if (episode_.deadAt >= 0 && !episode_.ragdolled && isInjuredState(pawn) && now - episode_.deadAt >= ragdollDelay - 1e-9) {
                    callState(pawn, "InjuredDeathAnimDone");
                    episode_.ragdolled = true;
                }
                if (episode_.deadAt >= 0 && !episode_.cameraEnded && isInjuredState(pawn) && numberOf(runtime_, *pawn, "bInjuredDeadCameraActive") != 0) {
                    // The camera's per-frame view request (out: location, rotation, field of view); the script ends the camera (EndInjuredDeadCamera -> InjuredRespawn) when its time is up.
                    callState(pawn, "CalcInjuredDeadCamera", {Value::makeFloat(dt), Value(), Value(), Value()});
                    if (numberOf(runtime_, *pawn, "bInjuredDeadCameraActive") == 0) {
                        episode_.cameraEnded = true;
                        row("down.camera." + n, now - episode_.deadAt);
                    }
                }
            }
            if (episode_.active && !isInjuredState(pawn)) {         // the state ended inside this frame: a second wind, or the respawn
                episode_.active = false;
                const double ratio = call(pawn, "GetHealth").number() / std::max(1e-9, call(pawn, "GetMaxHealth").number());
                row(std::string(episode_.cameraEnded ? "down.respawn_health." : "down.second_wind_health.") + n, ratio);
                if (episode_.cameraEnded) respawnAt_ = now;
            }
            if (respawnAt_ >= 0 && now - respawnAt_ >= respawnDialogDelay - 1e-9) {
                respawnAt_ = -1;
                Object& info = *runtime_.property(*script_->playerController(), "PlayerReplicationInfo")->o;
                const int64_t cash = combat_.currencyOnHand(info, 0);
                callState(pawn, "ShowRespawnDialog");
                feeRows_.push_back({n, cash, cash - combat_.currencyOnHand(info, 0)});
            }
        });
    }
}

std::string Run::downJson() {
    const ObjectPtr pawn = script_->playerPawnObject();
    std::ostringstream out;
    out << "{\"state\":" << quote(pawn->state) << ",\"injury\":" << episode_.number << ",\"injured_state\":" << number(numberOf(runtime_, *pawn, "InjuredState"))
        << ",\"dead_state\":" << number(numberOf(runtime_, *pawn, "InjuredDeadState")) << ",\"second_wind_reason\":" << number(numberOf(runtime_, *pawn, "SecondWindReason"))
        << ",\"total_bleedout\":" << number(numberOf(runtime_, *pawn, "TotalBleedoutTime")) << ",\"successive\":" << number(numberOf(runtime_, *pawn, "NumSuccessiveInjuries"))
        << ",\"targeted_time\":" << number(numberOf(runtime_, *pawn, "flInjuredTargetedTime")) << ",\"last_injury_time\":" << number(numberOf(runtime_, *pawn, "LastInjuryTime"))
        << ",\"since_down\":" << number(worldNow() - episode_.downAt) << "}";
    return out.str();
}

// A death roll as JSON (plan step S-g): the item game stage and awesome level the pools were called with, and for every pool the script's loop rolled its chance, draw, whether it was
// tried (the call the live hook saw), what the call returned and the items (records: balance, manufacturer, inventory class, level, drop flags).
std::string Run::lootJson(const CombatSystem::LootDrop& drop) {
    std::ostringstream out;
    out << "{\"item_stage\":" << drop.stage << ",\"awesome\":" << drop.awesome << ",\"below_kill_z\":" << (drop.belowKillZ ? "true" : "false") << ",\"pools\":[";
    bool first = true;
    for (const auto& pool : drop.pools) {
        out << (first ? "" : ",") << "{\"pool\":" << quote(pool.pool) << ",\"chance\":" << number(pool.chance) << ",\"roll\":" << number(pool.roll) << ",\"tried\":" << (pool.tried ? "true" : "false")
            << ",\"returned\":" << (pool.returned ? "true" : "false");
        if (!pool.error.empty()) out << ",\"error\":" << quote(pool.error);
        out << ",\"items\":[";
        bool firstItem = true;
        for (const auto& item : pool.items) {
            out << (firstItem ? "" : ",") << "{\"balance\":" << quote(item.balance) << ",\"manufacturer\":" << quote(item.manufacturer) << ",\"class\":" << quote(item.inventoryClass) << ",\"level\":" << item.level
                << ",\"drop_on_death\":" << (item.dropOnDeath ? "true" : "false") << ",\"dropped\":" << (item.dropped ? "true" : "false") << "}";
            firstItem = false;
        }
        out << "]}";
        first = false;
    }
    out << "]}";
    return out.str();
}

// The death roll(s) a hit on `spawned` made since the last report: rows loot.<key>.* for the tried pools (returned 0/1 and the number of items) and the whole roll as JSON.
std::string Run::lootAfter(const Spawned& spawned, const std::string& key) {
    std::string text;
    for (; dropsSeen_ < combat_.drops().size(); ++dropsSeen_) {
        const auto& drop = combat_.drops()[dropsSeen_];
        if (drop.pawn != spawned.enemy.pawn) continue;
        size_t tried = 0, dropped = 0;
        for (const auto& pool : drop.pools) {
            if (!pool.tried) continue;
            ++tried;
            for (const auto& item : pool.items) dropped += item.dropped;
            const std::string name = pool.pool.substr(pool.pool.rfind('.') + 1);
            row("loot." + key + "." + name + ".returned", pool.returned ? 1 : 0);
            row("loot." + key + "." + name + ".items", double(pool.items.size()));
        }
        row("loot." + key + ".item_stage", drop.stage);
        row("loot." + key + ".pools_tried", double(tried));
        row("loot." + key + ".items_dropped", double(dropped));
        text = lootJson(drop);
    }
    return text;
}

// rollpool:<pawn>:<pool path>[:<stage>[:<count>]]: ItemPool.SpawnBalancedInventoryFromPool called through the VM (as the script calls it, optional arguments omitted) with pawn n as the context
// source, at the given item game stage (default the pawn's), <count> times. Rows loot.roll.<pool>.stage<s>.n / .true / .items (the calls, those that returned true, the inventories made);
// the result lists the inventories of the last call (stand-in objects: their class and bDropOnDeath). The pool path is looked up in Startup and in the pawn's package.
std::string Run::rollPoolSteps(const std::vector<std::string>& parts) {
    if (parts.size() < 3 || parts.size() > 5) throw RuntimeError("combat-run: rollpool:<pawn>:<pool path>[:<stage>[:<count>]]");
    Spawned& spawned = pawn(parts.at(1));
    ObjectPtr pool;
    for (const std::string& name : {std::string("Startup"), spawned.package}) {
        const auto package = runtime_.package(name);
        if (const int32_t at = runtime_.findExport(*package, parts.at(2)); at > 0) { pool = runtime_.instantiateExport(package, at, 4); break; }
    }
    if (!pool) throw RuntimeError("combat-run: no such item pool " + parts.at(2));
    const int stage = parts.size() > 3 ? std::stoi(parts[3]) : int(runtime_.property(*spawned.enemy.pawn, "GameStageForSpawnedInventory")->integer());
    const int count = parts.size() > 4 ? std::stoi(parts[4]) : 1;
    Function* spawn = runtime_.findFunction("WillowGame.ItemPool.SpawnBalancedInventoryFromPool");
    int returned = 0, items = 0;
    std::vector<Value> last;
    for (int i = 0; i < count; ++i) {
        std::vector<Value> outs;
        std::vector<Value> arguments{Value::makeObject(pool), Value::makeInt(stage), Value::makeInt(0), Value::makeObject(spawned.enemy.pawn), Value::makeArray()};   // the optional ones omitted
        returned += runtime_.call(*spawn, nullptr, arguments, &outs).truth();
        last = outs.at(4).elements();
        items += int(last.size());
    }
    const std::string name = parts.at(2).substr(parts.at(2).rfind('.') + 1);
    const std::string key = "loot.roll." + name + ".stage" + std::to_string(stage);
    row(key + ".n", count);
    row(key + ".true", returned);
    row(key + ".items", items);
    std::ostringstream out;
    out << "{\"stage\":" << stage << ",\"calls\":" << count << ",\"returned_true\":" << returned << ",\"inventories\":[";
    for (size_t i = 0; i < last.size(); ++i) {
        const Value* flag = runtime_.property(*last[i].o, "bDropOnDeath");
        out << (i ? "," : "") << "{\"class\":" << quote(last[i].o->cls->name) << ",\"drop_on_death\":" << (flag && flag->truth() ? "true" : "false") << "}";
    }
    out << "]}";
    return out.str();
}

// lootbatch:<kind>:<level>:<stage>:<count>[:<tag>] (plan step S-g): <count> kills of a fresh pawn each, through the same spawn and lethal hit steps (a shotgun hit of 9999 by the player, one
// pawn after the other), with the death rolls summed. Nothing of the kills is kept except the sums (the pawns, their rows and results are dropped as they finish). Rows
// loot.batch.<tag>.n, .stage.<s> (rolls at that item game stage), .stage_min / .stage_max, .level_mismatch (items not at the stage's level), and per pool: .<pool>.chance (the first kill's) / .chance_max, .tried, .true (returned true), .items, .dropped; and for
// Pool_Money_1or2 .qty.<k> (tried rolls that made k items). The random is seeded by the seed step, so the batch repeats.
std::string Run::lootBatch(const std::vector<std::string>& parts) {
    if (parts.size() < 5 || parts.size() > 6) throw RuntimeError("combat-run: lootbatch:<kind>:<level>:<stage>:<count>[:<tag>]");
    if (!script_ && players_.empty()) throw RuntimeError("combat-run: a loot batch needs a player (the step maya:<level> or player:<level>) first");
    const int count = std::stoi(parts[4]);
    const std::string tag = parts.size() == 6 ? parts[5] : parts[1];
    const auto savedRows = rows_;
    const auto savedHits = hitCounts_;
    const size_t savedKillRows = killRows_.size(), savedResults = results_.size();
    struct Sum { double chance = -1, chanceMax = 0, tried = 0, returned = 0, items = 0, dropped = 0, errors = 0; };
    std::map<std::string, Sum> sums;
    std::map<int, int> stages, quantities;
    int kills = 0, levelMismatches = 0;
    for (int i = 0; i < count; ++i) {
        const size_t errorsBefore = errors_.size();
        apply("spawn:" + parts[1] + ":" + parts[2] + ":" + parts[3]);
        if (errors_.size() != errorsBefore) throw RuntimeError("combat-run: lootbatch: " + errors_.back());
        const std::string index = std::to_string(pawns_.size() - 1);
        const size_t before = combat_.drops().size();
        apply("hit:" + index + ":9999:" + killDamageType + ":Shotgun");
        for (size_t at = before; at < combat_.drops().size(); ++at) {
            const auto& drop = combat_.drops()[at];
            if (drop.pawn != pawns_.back().enemy.pawn) continue;
            ++kills;
            ++stages[drop.stage];
            for (const auto& pool : drop.pools) {
                Sum& sum = sums[pool.pool.substr(pool.pool.rfind('.') + 1)];
                sum.chance = sum.chance < 0 ? pool.chance : std::min(sum.chance, pool.chance);
                sum.chanceMax = std::max(sum.chanceMax, pool.chance);
                sum.tried += pool.tried;
                sum.errors += !pool.error.empty();
                sum.returned += pool.tried && pool.returned;
                sum.items += double(pool.items.size());
                for (const auto& item : pool.items) { sum.dropped += item.dropped; levelMismatches += item.level != drop.stage; }
                if (pool.tried && pool.pool.find("Pool_Money_1or2") != std::string::npos) ++quantities[int(pool.items.size())];
            }
        }
        pawns_.pop_back();
        results_.resize(savedResults);
        killRows_.resize(savedKillRows);
    }
    rows_ = savedRows;
    hitCounts_ = savedHits;
    combat_.drops().clear();
    dropsSeen_ = 0;
    const std::string key = "loot.batch." + tag;
    row(key + ".n", kills);
    for (const auto& [stage, n] : stages) row(key + ".stage." + std::to_string(stage), n);
    if (!stages.empty()) { row(key + ".stage_min", stages.begin()->first); row(key + ".stage_max", stages.rbegin()->first); }
    row(key + ".level_mismatch", levelMismatches);       // items whose level is not the stage they were rolled at (the cap is 50, above the stages used)
    for (const auto& [name, sum] : sums) {
        row(key + "." + name + ".chance", sum.chance);
        row(key + "." + name + ".chance_max", sum.chanceMax);
        row(key + "." + name + ".tried", sum.tried);
        row(key + "." + name + ".true", sum.returned);
        row(key + "." + name + ".items", sum.items);
        row(key + "." + name + ".dropped", sum.dropped);
        if (sum.errors > 0) row(key + "." + name + ".errors", sum.errors);
    }
    for (const auto& [quantity, n] : quantities) row(key + ".Pool_Money_1or2.qty." + std::to_string(quantity), n);
    return "{\"kills\":" + std::to_string(kills) + "}";
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
    for (const Fee& fee : feeRows_) {
        out << (first ? "" : ",") << quote("down.fee." + fee.injury) << ":{\"cash\":" << fee.cash << ",\"fee\":" << fee.charged << "}";
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
