#include "progression.hpp"
#include "behavior.hpp"

#include <algorithm>
#include <cmath>

namespace vm {
namespace {
std::string pathOf(const Value& reference) {
    if (reference.kind != Value::Kind::Object || !reference.o || !reference.o->resourcePackage) return "";
    return reference.o->resourcePackage->path(reference.o->resourceIndex);
}
bool endsWith(const std::string& text, const std::string& tail) {
    return text.size() >= tail.size() && text.compare(text.size() - tail.size(), tail.size(), tail) == 0;
}
// The part of an enum value's name after the last underscore, e.g. MATHRESOLVEROPERAND_Mul -> Mul.
std::string enumSuffix(const std::vector<std::string>& names, int64_t index) {
    if (index < 0 || size_t(index) >= names.size()) throw RuntimeError("enum value out of range");
    const std::string& name = names[size_t(index)];
    return name.substr(name.rfind('_') + 1);
}
float number(const Value* value) { return value ? float(value->number()) : 0.f; }
const Value& need(const Value* value, const char* what) {
    if (!value) throw RuntimeError(std::string("unsupported attribute data: no ") + what);
    return *value;
}
}

ObjectPtr instantiateReference(Runtime& runtime, const Value& reference) {
    if (reference.kind != Value::Kind::Object || !reference.o || !reference.o->resourcePackage) throw RuntimeError("unresolved object reference");
    auto package = reference.o->resourcePackage;
    int32_t index = reference.o->resourceIndex;
    if (index < 0) {
        const std::string path = package->path(index);
        package = runtime.package("Startup");
        index = runtime.findExport(*package, path);
    }
    if (index <= 0) throw RuntimeError("unresolved object reference (" + reference.o->name + ")");
    return runtime.instantiateExport(package, index, 4);
}

ObjectPtr AttributeEvaluator::load(const Value& reference) {
    if (reference.kind != Value::Kind::Object || !reference.o || !reference.o->resourcePackage)
        throw RuntimeError("attribute evaluator: unresolved object reference");
    const auto key = std::make_pair(reference.o->resourcePackage.get(), reference.o->resourceIndex);
    auto found = loaded_.find(key);
    if (found == loaded_.end()) found = loaded_.emplace(key, instantiateReference(runtime_, reference)).first;
    paths_[found->second.get()] = pathOf(reference);
    return found->second;
}

float AttributeEvaluator::evaluate(const Value& data, const AttributeContext& context) {
    float base = number(data.field("BaseValueConstant"));
    if (const Value* attribute = data.field("BaseValueAttribute"); attribute && attribute->o)
        if (auto value = attributeValue(*load(*attribute), context)) base = *value;
    ObjectPtr definition;
    if (const Value* reference = data.field("InitializationDefinition"); reference && reference->o) {
        definition = load(*reference);
        base = combined(*definition, base, context);
    }
    base *= number(data.field("BaseValueScaleConstant"));
    return definition ? restricted(*definition, base, context) : base;
}

float AttributeEvaluator::evaluateDefinition(Object& definition, const AttributeContext& context) {
    return restricted(definition, combined(definition, 0.f, context), context);
}

std::optional<float> AttributeEvaluator::evaluateAttribute(const Value& reference, const AttributeContext& context) {
    if (reference.kind != Value::Kind::Object || !reference.o) return std::nullopt;
    return attributeValue(*load(reference), context);
}

// The attribute's value: with an empty context chain the attribute yields no value and the constant stands; the context resolvers are
// the no-context one (always succeeds), the player replication info one and the controller one (succeed when the context has one), the resource pool one
// (succeeds when the context knows a pool for the resource) and the AI pawn and mind ones (succeed when the context has one); each value resolver
// gets the previous value (NATIVE_PROGRESSION.md section 1, NATIVE_ATTRIBUTES.md sections 5 and 6, UNVERIFIED).
std::optional<float> AttributeEvaluator::attributeValue(Object& attribute, const AttributeContext& context) {
    const Value* contexts = runtime_.property(attribute, "ContextResolverChain");
    if (!contexts || contexts->elements().empty()) return std::nullopt;
    ObjectPtr owner;          // the object the chain resolved (none for the no-context resolver)
    for (const auto& reference : contexts->elements()) {
        const std::string& type = load(reference)->cls->path;
        if (type == "GearboxFramework.NoContextNeededAttributeContextResolver") continue;
        // The game info and the designer-attribute holder always resolve (no state of theirs is read here; a designer attribute keeps its base value, the loot's artifact weights
        // use one: GD_Artifacts.BalanceAttributes.Attribute_DeputyCount). UNVERIFIED readings, plan step S-g.
        if (type == "Engine.GameInfoAttributeContextResolver" || type == "WillowGame.DesignerAttributeContextResolver") continue;
        if (type == "WillowGame.PlayerReplicationInfoAttributeContextResolver") {
            if (!context.replicationInfo) return std::nullopt;      // the context does not resolve
            owner = context.replicationInfo;
            continue;
        }
        if (type == "Engine.ControllerAttributeContextResolver") {
            if (!context.controller) return std::nullopt;
            owner = context.controller;
            continue;
        }
        if (type == "Engine.ResourcePoolAttributeContextResolver") {
            const Value* resource = runtime_.property(*load(reference), "Resource");
            owner = resource && context.resourcePool ? context.resourcePool(*resource) : nullptr;
            if (!owner) return std::nullopt;                        // no such pool: the context does not resolve
            continue;
        }
        if (type == "Engine.PawnAttributeContextResolver") {
            if (!context.pawn) return std::nullopt;
            owner = context.pawn;
            continue;
        }
        if (type == "WillowGame.WillowMindAttributeContextResolver") {
            if (!context.mind) return std::nullopt;
            owner = context.mind;
            continue;
        }
        throw RuntimeError("unsupported attribute context resolver " + type);
    }
    float value = 0;
    if (attribute.cls->path == "WillowGame.DesignerAttributeDefinition") {          // a designer attribute is set by the game's designers' code at run time; here it keeps its base value
        const Value* base = runtime_.property(attribute, "BaseValue");
        return base ? evaluate(*base, context) : 0.f;
    }
    const Value* chain = runtime_.property(attribute, "ValueResolverChain");
    if (!chain) throw RuntimeError("unsupported attribute: no value resolver chain");
    for (const auto& reference : chain->elements()) {
        ObjectPtr resolver = load(reference);
        const std::string& type = resolver->cls->path;
        if (type == "GearboxFramework.ConstantAttributeValueResolver") {
            value = number(runtime_.property(*resolver, "ConstantValue"));
        } else if (type == "GearboxFramework.SimpleMathValueResolver") {
            const auto operands = enumNames(runtime_, "GearboxFramework", "SimpleMathValueResolver.EMathValueResolverOperand");
            const float argument = evaluate(need(runtime_.property(*resolver, "Argument"), "Argument"), context);
            const std::string operand = enumSuffix(operands, need(runtime_.property(*resolver, "Operand"), "Operand").integer());
            if (operand == "Add") value += argument;
            else if (operand == "Sub") value -= argument;
            else if (operand == "Mul") value *= argument;
            else if (operand == "Div") value = argument != 0 ? value / argument : 0;
            else throw RuntimeError("unsupported math resolver operand " + operand);
        } else if (type == "WillowGame.GlobalAttributeValueResolver") {
            value = context.level;      // global slot 0 (the experience curve sets it to the level it asks for)
        } else if (type == "Engine.ObjectPropertyAttributeValueResolver") {
            // the named property of the resolved context object (the player's ExpLevel for PlayerExperienceLevel)
            const Value* name = runtime_.property(*resolver, "PropertyName");
            const Value* property = owner && name ? runtime_.property(*owner, name->s) : nullptr;
            if (!property) throw RuntimeError("unsupported object property attribute: no such property on the resolved context");
            value = float(property->number());
        } else if (type == "WillowGame.CurrencyAttributeValueResolver") {
            // The amount on hand of the resolver's FormOfCurrency, read from the resolved replication info. UNVERIFIED stand-in: the resolver's body is unread
            // (NATIVE_ATTRIBUTES "Not read yet"); the death penalty cost (0.07 x credits) is the case that needs it, and the live fee rows match it (COMBAT_DOWN_RESPAWN.md).
            ObjectPtr info = owner ? owner : context.replicationInfo;
            if (info && !runtime_.property(*info, "Currency"))        // the resolved context is the controller: its replication info holds the amounts
                if (const Value* held = runtime_.property(*info, "PlayerReplicationInfo"); held && held->kind == Value::Kind::Object && held->o) info = held->o;
            const Value* form = runtime_.property(*resolver, "FormOfCurrency");
            const Value* currency = info ? runtime_.property(*info, "Currency") : nullptr;
            if (!form || !currency) throw RuntimeError("unsupported currency attribute: no replication info or no Currency table in the context");
            const size_t at = size_t(form->integer());
            const Value* entry = currency->kind == Value::Kind::Array ? (at < currency->elements().size() ? &currency->elements()[at] : nullptr) : (at == 0 ? currency : nullptr);
            const Value* amount = entry ? entry->field("CurrentAmount") : nullptr;
            if (!amount) throw RuntimeError("unsupported currency attribute: no such currency slot");
            value = float(amount->number());
        } else if (type == "GearboxFramework.NounAttributeValueResolver") {
            value = nounValue(attribute, *resolver, owner, context);
        } else if (type == "GearboxFramework.RandomAttributeValueResolver") {
            // A random number between the bounds replaces the running value (ValueLifetime and the saved values are not modelled: a fresh draw each time). UNVERIFIED: the body is unread
            // (NATIVE_ATTRIBUTES "Not read yet"); plan step S-g needs it for the ammo pools' quantity.
            const Value* integer = runtime_.property(*resolver, "bUseIntegerRandomization");
            value = randomBetween(need(runtime_.property(*resolver, "LowerBound"), "LowerBound"), need(runtime_.property(*resolver, "UpperBound"), "UpperBound"),
                                  integer && integer->truth(), context);
        } else if (type == "GearboxFramework.ConstraintAttributeValueResolver") {
            // The running value clamped to the enabled bounds (minimum first, as the range restriction of a definition). UNVERIFIED: body unread.
            const Value& range = need(runtime_.property(*resolver, "Constraints"), "Constraints");
            if (const Value* on = range.field("bEnableMinValueRestriction"); on && on->truth()) value = std::max(value, evaluate(need(range.field("MinValue"), "MinValue"), context));
            if (const Value* on = range.field("bEnableMaxValueRestriction"); on && on->truth()) value = std::min(value, evaluate(need(range.field("MaxValue"), "MaxValue"), context));
        } else if (type == "GearboxFramework.ConditionalAttributeValueResolver") {
            // The first entry of the list whose expressions hold, else the default. The struct's bEnabled flag is false in the shipped data that has entries (the drop odds of guns by
            // number of players) and the editor's own display shares use the entries, so the flag is not tested (the reading tools/loot_pools.py checks against those shares, UNVERIFIED).
            const Value& expressions = need(runtime_.property(*resolver, "ValueExpressions"), "ValueExpressions");
            const Value* chosen = &need(expressions.field("DefaultBaseValue"), "DefaultBaseValue");
            if (const Value* list = expressions.field("ConditionalExpressionList"))
                for (const auto& entry : list->elements())
                    if (expressionsHold(need(entry.field("Expressions"), "Expressions"), context)) { chosen = &need(entry.field("BaseValueIfTrue"), "BaseValueIfTrue"); break; }
            value = evaluate(*chosen, context);
        } else if (type == "WillowGame.AmmoDropWeightAttributeValueResolver") {
            value = ammoDropWeight(*resolver, context);
        } else {
            throw RuntimeError("unsupported attribute value resolver " + type);
        }
    }
    return value;
}

// The AI parameter (NounAttributeValueResolver; its native body is unread, NATIVE_ATTRIBUTES "Not read yet", so this is a UNVERIFIED reading checked
// against live health values): the resolved mind's class lists starting values (AttributeStartingValues: Attribute, BaseValue); the one for this
// attribute is evaluated, else the resolver's own DefaultValue. The playthrough tables of the pawn balance (also AttributeStartingValues) are not read.
float AttributeEvaluator::nounValue(Object& attribute, Object& resolver, const ObjectPtr& mind, const AttributeContext& context) {
    const Value* aiClass = mind ? runtime_.property(*mind, "AIClass") : nullptr;
    const Value* starting = aiClass && aiClass->kind == Value::Kind::Object && aiClass->o ? runtime_.property(*aiClass->o, "AttributeStartingValues") : nullptr;
    const auto own = paths_.find(&attribute);
    if (starting && own != paths_.end())
        for (const auto& entry : starting->elements()) {
            const Value* named = entry.field("Attribute");
            if (named && pathOf(*named) == own->second) return evaluate(need(entry.field("BaseValue"), "BaseValue"), context);
        }
    return evaluate(need(runtime_.property(resolver, "DefaultValue"), "DefaultValue"), context);
}

// A random number from lower to upper, both evaluated: an integer between them inclusive, or a real one. The generator is the owner's (setRandom): C rand() / 32767 scaled to the
// span, the top draw kept inside the span. UNVERIFIED stand-in: the native generator's exact mapping is unread; the item game stage offset (-2 to +1) reached both ends in the live
// game, which fits the inclusive reading.
float AttributeEvaluator::randomBetween(const Value& lower, const Value& upper, bool integer, const AttributeContext& context) {
    if (!random_) throw RuntimeError("unsupported attribute definition: random variance (no random source)");
    const float low = evaluate(lower, context), high = evaluate(upper, context);
    const double draw = double(random_()) / 32768.0;                // 0 <= draw < 1
    if (!integer) return low + float(draw) * (high - low);
    const double span = std::floor(double(high)) - std::ceil(double(low)) + 1;
    if (span < 1) return std::ceil(low);
    return float(std::ceil(double(low)) + std::floor(draw * span));
}

// The ammo and health drop weight (AmmoDropWeightAttributeValueResolver, UNVERIFIED reading of its data; body unread): the share of the local player's pool for the resource is
// compared with the resource threshold; at or above it the weight is the above-threshold one, below it the weight rises linearly from the minimum (at the threshold) to the
// maximum (empty); a player with no such pool gets the no-pool weight. The direction of the slope is the stand-in's guess (more need, more drops).
float AttributeEvaluator::ammoDropWeight(Object& resolver, const AttributeContext& context) {
    const auto weight = [&](const char* name) { return evaluate(need(runtime_.property(resolver, name), name), context); };
    const Value* resource = runtime_.property(resolver, "Resource");
    const ObjectPtr pool = resource && context.playerPool ? context.playerPool(*resource) : nullptr;
    if (!pool) return weight("NoPoolWeight");
    const float maximum = number(runtime_.property(*pool, "MaxValue"));
    const float share = maximum > 0 ? number(runtime_.property(*pool, "CurrentValue")) / maximum : 0.f;
    const float threshold = weight("ResourceThreshold");
    if (share >= threshold) return weight("AboveThresholdWeight");
    const float low = weight("MinBelowThresholdWeight"), high = weight("MaxBelowThresholdWeight");
    return threshold > 0 ? high + (share / threshold) * (low - high) : low;
}

// f from the definition's formula or conditional, then combined with the base by BaseValueMode. The numbers 1..3 are taken
// as base + f, base x f and f - base in the note's order (which enum name is which was not checked: UNVERIFIED).
float AttributeEvaluator::combined(Object& definition, float base, const AttributeContext& context) {
    const Value* variance = runtime_.property(definition, "RandomVariance");
    const bool random = variance && variance->field("bEnabled") && variance->field("bEnabled")->truth();
    if (random && !random_) throw RuntimeError("unsupported attribute definition: random variance");
    const Value* formula = runtime_.property(definition, "ValueFormula");
    const Value* conditional = runtime_.property(definition, "ConditionalInitialization");
    float f = 0;
    if (formula && formula->field("bEnabled") && formula->field("bEnabled")->truth()) {
        const float multiplier = evaluate(need(formula->field("Multiplier"), "Multiplier"), context);
        const float level = evaluate(need(formula->field("Level"), "Level"), context);
        const float power = evaluate(need(formula->field("Power"), "Power"), context);
        const float offset = evaluate(need(formula->field("Offset"), "Offset"), context);
        f = multiplier * ((power == 1.f ? level : std::pow(level, power)) + offset);     // the offset is added before the multiplier
    } else if (conditional && conditional->field("bEnabled") && conditional->field("bEnabled")->truth()) {
        const Value* chosen = &need(conditional->field("DefaultBaseValue"), "DefaultBaseValue");
        if (const Value* list = conditional->field("ConditionalExpressionList"))
            for (const auto& entry : list->elements())
                if (expressionsHold(need(entry.field("Expressions"), "Expressions"), context)) { chosen = &need(entry.field("BaseValueIfTrue"), "BaseValueIfTrue"); break; }
        f = evaluate(*chosen, context);
    } else if (!random) {
        throw RuntimeError("unsupported attribute definition: neither a value formula nor a conditional");
    }                                                         // a purely random definition (the pool quantities) has f = 0
    float value;
    switch (int(need(runtime_.property(definition, "BaseValueMode"), "BaseValueMode").integer())) {
    case 0: value = f; break;
    case 1: value = base + f; break;
    case 2: value = base * f; break;
    case 3: value = f - base; break;
    default: throw RuntimeError("unsupported base value mode");
    }
    // The random amount between the bounds is added after the mode (NATIVE_PROGRESSION.md section 1 step 2); the scale and the clamp follow in evaluate().
    if (random) value += randomBetween(need(variance->field("LowerBound"), "LowerBound"), need(variance->field("UpperBound"), "UpperBound"),
                                       variance->field("bUseIntegerRandomization") && variance->field("bUseIntegerRandomization")->truth(), context);
    return value;
}

// An entry's expressions are taken to all have to hold (UNVERIFIED). The attributes they may name are PlayThroughCount, NumberOfPlayers,
// Targetable_Is_Champion and PlayerExperienceLevel; an operand that does not resolve makes the expression false (NATIVE_ATTRIBUTES.md section 11).
bool AttributeEvaluator::expressionsHold(const Value& expressions, const AttributeContext& context) {
    const auto operators = enumNames(runtime_, "Engine", "AttributeExpression.EComparisonOperator");
    for (const auto& expression : expressions.elements()) {
        const auto left = operandValue(need(expression.field("AttributeOperand1"), "AttributeOperand1"), context);
        const Value* attribute2 = expression.field("AttributeOperand2");
        const auto right = attribute2 && attribute2->o ? operandValue(*attribute2, context) : std::optional<float>(number(expression.field("ConstantOperand2")));
        if (!left || !right) return false;
        const std::string op = enumSuffix(operators, need(expression.field("ComparisonOperator"), "ComparisonOperator").integer());
        bool holds;
        if (op == "EqualTo") holds = *left == *right;
        else if (op == "NotEqualTo") holds = *left != *right;
        else if (op == "GreaterThan") holds = *left > *right;
        else if (op == "GreaterThanOrEqual") holds = *left >= *right;
        else if (op == "LessThan") holds = *left < *right;
        else if (op == "LessThanOrEqual") holds = *left <= *right;
        else throw RuntimeError("unsupported comparison operator " + op);
        if (!holds) return false;
    }
    return true;
}

std::optional<float> AttributeEvaluator::operandValue(const Value& attribute, const AttributeContext& context) {
    const std::string path = pathOf(attribute);
    if (endsWith(path, "PlayThroughCount")) return float(context.playThroughCount);
    if (endsWith(path, "NumberOfPlayers")) return float(context.numberOfPlayers);
    if (endsWith(path, "Targetable_Is_Champion")) return context.champion ? 1.f : 0.f;
    if (endsWith(path, "PlayerExperienceLevel")) return attributeValue(*load(attribute), context);     // the player's ExpLevel, when there is a player
    throw RuntimeError("unsupported condition attribute " + path);
}

// Only when a definition is set (the caller checks): minimum, then maximum; then RoundingMode 0 none, 1 nearest, 2 floor, 3 ceiling.
float AttributeEvaluator::restricted(Object& definition, float value, const AttributeContext& context) {
    if (const Value* range = runtime_.property(definition, "RangeRestriction")) {
        if (const Value* on = range->field("bEnableMinValueRestriction"); on && on->truth()) value = std::max(value, evaluate(need(range->field("MinValue"), "MinValue"), context));
        if (const Value* on = range->field("bEnableMaxValueRestriction"); on && on->truth()) value = std::min(value, evaluate(need(range->field("MaxValue"), "MaxValue"), context));
    }
    if (const Value* mode = runtime_.property(definition, "RoundingMode")) {
        if (mode->integer() == 1) value = std::round(value);
        else if (mode->integer() == 2) value = std::floor(value);
        else if (mode->integer() == 3) value = std::ceil(value);
    }
    return value;
}

int64_t ExperienceCurve::point(int level) {
    AttributeContext context;
    context.level = float(level);
    return int64_t(std::trunc(evaluator_.evaluateDefinition(*definition_, context)));
}

int64_t ExperienceCurve::required(int level) {
    return std::max<int64_t>(0, point(level) - point(1));
}

} // namespace vm
