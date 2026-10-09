#include "combat.hpp"

#include <algorithm>

namespace vm {
namespace {
bool isObject(const Value* value) { return value && value->kind == Value::Kind::Object && value->o; }

// WillowPlayerReplicationInfo.Currency[type].CurrentAmount, nullptr when the info has no such slot (a static array of 13 CurrencyState structs; type 0 is credits).
Value* currencyAmount(Runtime& runtime, Object& info, int64_t type) {
    Value* currency = runtime.property(info, "Currency");
    if (!currency || type < 0 || type > 12) return nullptr;
    Value* entry = currency->kind == Value::Kind::Array ? (size_t(type) < currency->elements().size() ? &currency->elements()[size_t(type)] : nullptr) : (type == 0 ? currency : nullptr);
    return entry ? entry->field("CurrentAmount") : nullptr;
}
}

// The amount of a currency on hand, as GetCurrencyOnHand answers it (NATIVE_CONTROLLER_HELPERS.md): 0 outside the table. Type 3 (golden keys) asks the owner controller in the
// game; the VM has no such count (0).
int64_t CombatSystem::currencyOnHand(Object& info, int64_t type) {
    const Value* amount = type == 3 ? nullptr : currencyAmount(runtime_, info, type);
    return amount ? amount->integer() : 0;
}

void CombatSystem::setCurrencyOnHand(Object& info, int64_t type, int64_t amount) {
    if (Value* slot = currencyAmount(runtime_, info, type)) *slot = Value::makeInt(amount);
    else throw RuntimeError("combat: the replication info has no Currency slot " + std::to_string(type));
}

// The player's pawn from the mission bridge is a bare WillowPlayerPawn with its controller (the slice needs no more). The down state's script reads the world through the pawn
// (WorldInfo.TimeSeconds for the successive-injury window, the NetMode and Game tests), so the combat run gives it the world and authority, as adopt() does an enemy.
void CombatSystem::preparePlayerPawn(const ObjectPtr& pawn) {
    if (Value* world = runtime_.property(*pawn, "WorldInfo")) *world = Value::makeObject(world_);
    if (Value* role = runtime_.property(*pawn, "Role")) *role = Value::makeByte(authority_);
    // The respawn point. The game's choice is a TravelStation of the level (GetBestPlayerPlacementPoint, script over the level's actors, and the station's exit point); the VM world
    // has no level, so the script falls back to the controller's StartSpot, which here is a bare PlayerStart at the origin (UNVERIFIED stand-in, plan step S-f).
    if (const Value* controller = runtime_.property(*pawn, "Controller"); isObject(controller))
        if (Value* spot = runtime_.property(*controller->o, "StartSpot"); spot && !isObject(spot))
            if (Class* start = runtime_.findClass("Engine.PlayerStart")) *spot = Value::makeObject(runtime_.instantiate(start));
    // The pawn's replication info is its controller's (the respawn fee reads and charges the cash through it).
    const Value* controller = runtime_.property(*pawn, "Controller");
    const Value* info = isObject(controller) ? runtime_.property(*controller->o, "PlayerReplicationInfo") : nullptr;
    if (Value* own = runtime_.property(*pawn, "PlayerReplicationInfo"); own && isObject(info)) *own = *info;
}

// The natives the stock `injured` state of a player pawn reaches (docs/verification/NATIVE_DAMAGE_DEATH.md section 10, docs/verification/COMBAT_DOWN_RESPAWN.md).
// Every body below is a role-level stand-in, UNVERIFIED: the natives were not read beyond their role (a field read or written by the script around them, or the live
// row of COMBAT_GOLDEN_TRACE.md section D), and each is on the analyst list of the lane's handoff. The injured state's own timing is script (Tick) and needs none.
void CombatSystem::bindDownNatives() {
    // WillowPlayerController.GetInjuredDefinition: the injured definition of the player's class (CharClass_Siren.CharacterInjuredDefinition for Maya); the bridge knows the class.
    bind("WillowGame.WillowPlayerController.GetInjuredDefinition", [this](NativeCall&) {
        return Value::makeObject(injuredDefinition_ ? injuredDefinition_() : nullptr);
    });

    // WillowPawn.SetSecondWindReason(Reason) / ClearSecondWindReason: the reason byte the injured Tick reads (0 = none). Stand-in: the byte is stored and cleared, nothing else.
    // The live row (COMBAT_GOLDEN_TRACE.md section D step 17) has health already at 25 percent in the frame of the kill, with no script SetHealth: the native might
    // raise it itself. The VM raises it on the next Tick (GoFromInjuredToHealthy), so the health of the kill frame differs (reported, not fitted).
    bind("WillowGame.WillowPawn.SetSecondWindReason", [this](NativeCall& c) {
        if (Value* reason = runtime_.property(*c.self, "SecondWindReason")) *reason = Value::makeByte(c.in(0).integer());
        logEvent("SetSecondWindReason " + std::to_string(c.in(0).integer()));
        return Value();
    });
    bind("WillowGame.WillowPawn.ClearSecondWindReason", [this](NativeCall& c) {
        if (Value* reason = runtime_.property(*c.self, "SecondWindReason")) *reason = Value::makeByte(0);
        return Value();
    });

    // GetRevivePct / SetRevivePct: the percentage the HUD shows, stored on the player's replication info when the pawn has one (the bridge's pawn has none: dropped).
    for (const char* path : {"WillowGame.WillowPawn.SetRevivePct", "WillowGame.WillowPlayerPawn.SetRevivePct"})
        bind(path, [this](NativeCall& c) {
            const Value* info = runtime_.property(*c.self, "PlayerReplicationInfo");
            if (isObject(info))
                if (Value* percent = runtime_.property(*info->o, "RevivePct")) *percent = Value::makeFloat(c.in(0).number());
            return Value();
        });
    for (const char* path : {"WillowGame.WillowPawn.GetRevivePct", "WillowGame.WillowPlayerPawn.GetRevivePct"})
        bind(path, [this](NativeCall& c) {
            const Value* info = runtime_.property(*c.self, "PlayerReplicationInfo");
            const Value* percent = isObject(info) ? runtime_.property(*info->o, "RevivePct") : nullptr;
            return Value::makeFloat(percent ? percent->number() : 0.0);
        });

    // Actor.SetCanTarget(bool) / CanTarget() / GetCanTarget(): the actor's bCanTarget flag (the cheat flag bCanTargetCheat is not read). Stand-in for the targeting layer.
    bind("Engine.Actor.SetCanTarget", [this](NativeCall& c) {
        if (Value* flag = runtime_.property(*c.self, "bCanTarget")) *flag = Value::makeBool(c.in(0).truth());
        logEvent(std::string("SetCanTarget ") + (c.in(0).truth() ? "true" : "false"));
        return Value();
    });
    for (const char* path : {"Engine.Actor.CanTarget", "Engine.Actor.GetCanTarget"})
        bind(path, [this](NativeCall& c) {
            const Value* flag = runtime_.property(*c.self, "bCanTarget");
            return Value::makeBool(flag && flag->truth());
        });

    // WillowPlayerPawn.HasActiveDuel(Other): the VM has no versus duel, so no pawn is in one (the injured Tick asks every frame).
    bind("WillowGame.WillowPlayerPawn.HasActiveDuel", [](NativeCall&) { return Value::makeBool(false); });

    // WillowPlayerReplicationInfo.GetCurrencyOnHand / AddCurrencyOnHand (NATIVE_CONTROLLER_HELPERS.md, read from native code, UNVERIFIED): Add ignores a type above 12 and
    // type 3; a negative delta lowers the amount, never below 0; a positive one raises it to at most the type's cap (credits 99,999,999, Eridium 500, the rest 999); then the
    // script event NotifyCurrencyDelegates fires on the info (even when nothing changed). The replication flag bit has no VM counterpart.
    bind("WillowGame.WillowPlayerReplicationInfo.GetCurrencyOnHand", [this](NativeCall& c) { return Value::makeInt(currencyOnHand(*c.self, c.in(0).integer())); });
    bind("WillowGame.WillowPlayerReplicationInfo.AddCurrencyOnHand", [this](NativeCall& c) {
        const int64_t type = c.in(0).integer(), delta = c.in(1).integer();
        if (type < 0 || type > 12 || type == 3) return Value();
        const int64_t cap = type == 0 ? 99999999 : type == 1 ? 500 : 999;
        const int64_t before = currencyOnHand(*c.self, type);
        setCurrencyOnHand(*c.self, type, delta <= 0 ? std::max<int64_t>(0, before + delta) : std::min(cap, before + delta));
        logEvent("AddCurrencyOnHand type " + std::to_string(type) + " delta " + std::to_string(delta) + ": " + std::to_string(before) + " -> " + std::to_string(currencyOnHand(*c.self, type)));
        runtime_.callByName(c.self, "NotifyCurrencyDelegates");
        return Value();
    });

    // WillowPlayerController.NoFailSetPawnLocation(Pawn, Location): puts the pawn at the location whatever is there (the respawn's last resort when Actor.SetLocation, which the VM
    // leaves a logged stub, did not place it). Stand-in: the Location is written and the call succeeds; no collision test, no anchor.
    bind("WillowGame.WillowPlayerController.NoFailSetPawnLocation", [this](NativeCall& c) {
        if (isObject(&c.in(0)))
            if (Value* location = runtime_.property(*c.in(0).o, "Location")) *location = c.in(1);
        logEvent("NoFailSetPawnLocation");
        return Value::makeBool(true);
    });
    // Pawn.IsInjured(): the script sets bIsInjured in the injured state's BeginState and clears it in EndState; the native is taken to answer that flag. IsDead stays the
    // logged stub (the AI death chain depends on its answer).
    bind("Engine.Pawn.IsInjured", [this](NativeCall& c) {
        const Value* flag = runtime_.property(*c.self, "bIsInjured");
        return Value::makeBool(flag && flag->truth());
    });
}

} // namespace vm
