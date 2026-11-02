#include "combat.hpp"

#include <algorithm>
#include <cmath>

namespace vm {
namespace {
Value& required(Runtime& runtime, Object& object, const char* name) {
    Value* value = runtime.property(object, name);
    if (!value) throw RuntimeError(std::string("combat xp: ") + object.cls->name + " has no property " + name);
    return *value;
}
bool isObject(const Value* value) { return value && value->kind == Value::Kind::Object && value->o; }
float number(const Value* value) { return value ? float(value->number()) : 0.f; }
}

// The pawn's ExperiencePointMultiplier, the attribute the base kill experience scales (NATIVE_COMBAT_GROUNDWORK.md section 5.1): the stock spawn writes the AI class's
// starting values into the pawn's attributes (PopulationFactoryBalancedAIPawn.ApplyAttributeStartingValues, unread), and the class lists Attribute_ExperienceMultiplier with
// one of the GD_AI_Balance.XP.XPMultiplier_* definitions (Chump 0.5 ... Boss 40). The entry is evaluated like the health multiplier and written to the pawn's property and its
// base value (UNVERIFIED reading of the native; the live pawns read 0.5, 0.75, 1.25, ... in COMBAT_GOLDEN_TRACE.md "Open-rule checks").
void CombatSystem::applyExperienceMultiplier(const ObjectPtr& pawnObject, const ObjectPtr& mind) {
    Object& pawn = *pawnObject;
    Value* property = runtime_.property(pawn, "ExperiencePointMultiplier");
    const Value* aiClass = runtime_.property(pawn, "AIClass");
    const Value* starting = isObject(aiClass) ? runtime_.property(*aiClass->o, "AttributeStartingValues") : nullptr;
    if (!property || !starting) return;
    for (const Value& entry : starting->elements()) {
        const Value* attribute = entry.field("Attribute");
        if (!isObject(attribute) || !attribute->o->resourcePackage) continue;
        const std::string path = attribute->o->resourcePackage->path(attribute->o->resourceIndex);
        const std::string wanted = "Attribute_ExperienceMultiplier";
        if (path.size() < wanted.size() || path.compare(path.size() - wanted.size(), wanted.size(), wanted) != 0) continue;
        AttributeContext context;
        context.pawn = pawnObject;
        context.mind = mind;
        const float value = evaluator_.evaluate(*entry.field("BaseValue"), context);
        *property = Value::makeFloat(value);
        if (Value* base = runtime_.property(pawn, "ExperiencePointMultiplierBaseValue")) *base = Value::makeFloat(value);
        return;
    }
}

void CombatSystem::bindExperienceNatives() {
    // WillowExperiencePipeline.CalculateExperiencePointsForKill(Killer, KilledPawn, bKilledByCritical, DamageSourceClass, DamageTypeDef) -> float, static
    // (NATIVE_COMBAT_GROUNDWORK.md section 5.1, UNVERIFIED; live rows: COMBAT_GOLDEN_TRACE.md "Open-rule checks", 5 of 5 awards plus lane L3's two). The amount for the whole
    // party, before the split: the base formula of the game's globals (BaseEnemyExperienceFormula, evaluated for the killed pawn from the data: its ExperiencePointMultiplier
    // x Init_BaseEnemyExperience, which is the playthrough's multiplier x (level + 4)), then the critical and damage-type factors. No rounding here.
    bind("WillowGame.WillowExperiencePipeline.CalculateExperiencePointsForKill", [this](NativeCall& c) {
        const ObjectPtr killer = isObject(&c.in(0)) ? c.in(0).o : nullptr;
        const ObjectPtr killed = isObject(&c.in(1)) ? c.in(1).o : nullptr;
        const bool critical = c.in(2).truth();
        Kill record;
        record.killed = killed;
        float total = 0.f;
        if (killed) {
            AttributeContext context;
            if (!describe(killed, context)) notImplemented("CalculateExperiencePointsForKill: a killed actor that is not an AI pawn");
            const Value* formula = runtime_.property(*globals(), "BaseEnemyExperienceFormula");
            if (!formula) throw RuntimeError("combat xp: the globals have no BaseEnemyExperienceFormula");
            total = evaluator_.evaluate(*formula, context);
            record.killedLevel = levelOf(killed);
        }
        // The vehicle adjustment (a vehicle victim is worth 0 without an occupant; a killer driving a vehicle scales by VehicleVSCharacterExpModifier, not set in the data) is not
        // modelled: reported when it would apply.
        if (killed && killed->cls->name.find("Vehicle") != std::string::npos) notImplemented("CalculateExperiencePointsForKill: a vehicle victim");
        if (killer)
            if (const Value* pawn = runtime_.property(*killer, "Pawn"); isObject(pawn))
                if (const Value* driven = runtime_.property(*pawn->o, "DrivenVehicle"); isObject(driven)) notImplemented("CalculateExperiencePointsForKill: a killer in a vehicle");
        // The critical factor is the killer's ExpScaleForCriticalHitKills attribute; the lists of damage-source and damage-type factors are empty in the data (an entry would
        // be a rule nobody has read here).
        if (critical && killer) total *= attribute(*killer, "ExpScaleForCriticalHitKills");
        if (const Value* list = runtime_.property(*globals(), "DamageTypeExpModifiers"); list && !list->elements().empty())
            notImplemented("CalculateExperiencePointsForKill: DamageTypeExpModifiers entries");
        record.award = total;
        kills_.push_back(record);
        logEvent("CalculateExperiencePointsForKill award=" + std::to_string(total));
        return Value::makeFloat(total);
    });

    // WillowExperiencePipeline.AwardCombatExperienceToParty(ExperiencePoints, Killer, KilledActor), static (section 5.2, UNVERIFIED): with the game's replication info, for each
    // player replication info whose owner is a player controller, in list order: the killer's share is (KillerExpBonus + ExpWeight) x points and every other player's ExpWeight
    // x points (the table row at position N - 1, N = the game's EffectiveNumPlayers; outside the table: bonus 0, weight 1), scaled by the level-difference table
    // (ExpScaleByLevelDifference at position min(|d|, length) - 1: HigherLevelEnemyExpScale when the victim is above the player, LowerLevelEnemyExpScale below, 1 when equal),
    // times the combat multiplier (1.0 on the first playthrough below the level cap), truncated toward zero, and given to the controller's ExpEarn(amount, 1, 1).
    bind("WillowGame.WillowExperiencePipeline.AwardCombatExperienceToParty", [this](NativeCall& c) {
        const float points = float(c.in(0).number());
        const ObjectPtr killer = isObject(&c.in(1)) ? c.in(1).o : nullptr;
        const ObjectPtr killed = isObject(&c.in(2)) ? c.in(2).o : nullptr;
        const Value* replication = runtime_.property(*world_, "GRI");
        const Value* players = isObject(replication) ? runtime_.property(*replication->o, "PRIArray") : nullptr;
        if (!players) { logEvent("AwardCombatExperienceToParty: the world has no replication info (nothing awarded)"); return Value(); }
        int64_t count = 1;
        if (const Value* game = runtime_.property(*world_, "Game"); isObject(game))
            if (const Value* effective = runtime_.property(*game->o, "EffectiveNumPlayers")) count = effective->integer();
        float bonus = 0.f, weight = 1.f;
        const Value* weights = runtime_.property(*globals(), "ExpAwardWeights");
        if (weights && count >= 1 && size_t(count) <= weights->elements().size()) {
            const Value& row = weights->elements()[size_t(count - 1)];
            bonus = number(row.field("KillerExpBonus"));
            weight = number(row.field("ExpWeight"));
        }
        const Value* scales = runtime_.property(*globals(), "ExpScaleByLevelDifference");
        const int victimLevel = killed ? levelOf(killed) : 0;
        for (const Value& info : players->elements()) {
            if (!isObject(&info)) continue;
            const Value* owner = runtime_.property(*info.o, "Owner");
            if (!isObject(owner) || !isPlayerController(owner->o)) continue;
            const ObjectPtr controller = owner->o;
            float amount = (controller == killer ? bonus + weight : weight) * points;
            const int playerLevel = levelOf(controller);
            if (killed && playerLevel != victimLevel && scales && !scales->elements().empty()) {
                const size_t position = std::min<size_t>(size_t(std::abs(playerLevel - victimLevel)), scales->elements().size()) - 1;
                const Value& row = scales->elements()[position];
                amount *= number(row.field(victimLevel > playerLevel ? "HigherLevelEnemyExpScale" : "LowerLevelEnemyExpScale"));
            }
            const int credited = int(std::trunc(amount));        // the combat multiplier is 1.0 (first playthrough, below the cap): not applied
            if (!kills_.empty() && kills_.back().killed == killed) kills_.back().credits.push_back({controller, playerLevel, credited});
            logEvent("AwardCombatExperienceToParty player_level=" + std::to_string(playerLevel) + " victim_level=" + std::to_string(victimLevel) + " ExpEarn(" + std::to_string(credited) + ", 1, 1)");
            Function* expEarn = runtime_.findMethod(controller->cls, "ExpEarn");
            if (!expEarn) throw RuntimeError("combat xp: the player controller has no ExpEarn");
            std::vector<Value> arguments{Value::makeInt(credited), Value::makeByte(1), Value::makeByte(1)};
            arguments.resize(expEarn->params.size());
            runtime_.callFromNative(*expEarn, controller, std::move(arguments));
        }
        return Value();
    });
}

} // namespace vm
