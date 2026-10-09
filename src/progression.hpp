#pragma once
#include "vm.hpp"

#include <functional>
#include <map>
#include <optional>

namespace vm {

// What the attribute evaluator may read from the world. The real evaluator resolves attributes through context
// resolvers (the player controller, the pawn, the globals); this subset has the values the experience and level-up rules need.
struct AttributeContext {
    int playThroughCount = 1;     // D_Attributes.Balance.PlayThroughCount: 1 on the first playthrough (UNVERIFIED reading)
    float level = 0;              // the global slot a GlobalAttributeValueResolver reads (the experience curve's level)
    // The player's replication info (NATIVE_ATTRIBUTES.md section 5: a controller source resolves to its PlayerReplicationInfo). Null: an
    // attribute whose context chain asks for it does not resolve (a condition on it is false, a base value keeps its constant).
    ObjectPtr replicationInfo;
    // The player controller itself (a ControllerAttributeContextResolver resolves to it; the respawn fee's cost, GlobalsDefinition.TeleportCost, is evaluated with it, UNVERIFIED reading
    // of the resolver, whose body is unread). Null: an attribute whose context chain asks for it does not resolve.
    ObjectPtr controller;
    // The pool of a resource definition (NATIVE_ATTRIBUTES.md section 5: ResourcePoolAttributeContextResolver, the resource's owner's pool for it).
    // Empty or returning null: the attribute does not resolve.
    std::function<ObjectPtr(const Value& resource)> resourcePool;
    // An AI pawn's chain (NATIVE_COMBAT_GROUNDWORK.md section 1.4: a pool made through the pawn's controller has the mind as its provider; the AI
    // parameters are the mind's class values, UNVERIFIED reading). Null: an attribute whose context chain asks for it does not resolve.
    ObjectPtr pawn;               // PawnAttributeContextResolver: the pawn (its ExpLevel, via an object-property resolver)
    ObjectPtr mind;               // WillowMindAttributeContextResolver: the AI mind; its AIClass holds the starting values
    // Single player, not a champion (UNVERIFIED readings of D_Attributes.GameProperties.NumberOfPlayers and Targetable.Targetable_Is_Champion;
    // the data conditions that read them have their first entry for these values).
    int numberOfPlayers = 1;
    bool champion = false;
};

// A data reference as an instantiated object, for the readers of attribute and pool data (the evaluator, src/combat.*): the export it names. An import the
// VM could not resolve (its root is a group of the shared Startup package, such as D_Resources or D_Attributes, imported by the area packages) is looked
// up by its path in Startup, the package the game always has loaded. Throws RuntimeError when the reference is empty or nothing is found.
ObjectPtr instantiateReference(Runtime& runtime, const Value& reference);

// A SUBSET of the native AttributeInitializationData evaluator (docs/verification/NATIVE_PROGRESSION.md section 1, read from
// native code, UNVERIFIED): constants, scale, attributes with a no-context chain of constant / simple-math / global resolvers
// (and, for an AI pawn, the pawn and mind context resolvers with the object-property and noun value resolvers),
// definitions with a ValueFormula or a ConditionalInitialization on PlayThroughCount or PlayerExperienceLevel, the base-value modes,
// range restriction and rounding. Arithmetic is single precision. Any shape outside the subset (random variance, other
// resolvers, other context resolvers, other condition attributes) throws RuntimeError("unsupported ...") instead of guessing.
class AttributeEvaluator {
public:
    explicit AttributeEvaluator(Runtime& runtime) : runtime_(runtime) {}
    // `data` is an AttributeInitializationData struct value.
    float evaluate(const Value& data, const AttributeContext& context);
    // A definition on its own: f(ValueFormula) or the selected conditional value, range restriction and rounding applied.
    float evaluateDefinition(Object& definition, const AttributeContext& context);

private:
    Runtime& runtime_;
    std::map<std::pair<const Package*, int32_t>, ObjectPtr> loaded_;
    std::map<const Object*, std::string> paths_;      // the object path each loaded object was made from (the noun resolver matches attributes by it)
    ObjectPtr load(const Value& reference);
    float nounValue(Object& attribute, Object& resolver, const ObjectPtr& mind, const AttributeContext& context);
    std::optional<float> attributeValue(Object& attribute, const AttributeContext& context);
    float combined(Object& definition, float base, const AttributeContext& context);
    float restricted(Object& definition, float value, const AttributeContext& context);
    bool expressionsHold(const Value& expressions, const AttributeContext& context);
    std::optional<float> operandValue(const Value& attribute, const AttributeContext& context);
};

// Experience required to reach a level (NATIVE_PROGRESSION.md section 3): R(n) = max(0, trunc(f(n)) - trunc(f(1))), f the
// definition's formula evaluated with the level in the global slot (60 x (n ^ 2.8 + 7.33) in the game's data).
class ExperienceCurve {
public:
    ExperienceCurve(AttributeEvaluator& evaluator, ObjectPtr definition) : evaluator_(evaluator), definition_(std::move(definition)) {}
    int64_t required(int level);
private:
    AttributeEvaluator& evaluator_;
    ObjectPtr definition_;
    int64_t point(int level);
};

} // namespace vm
