#pragma once
#include "vm.hpp"

#include <map>
#include <optional>

namespace vm {

// What the attribute evaluator may read from the world. The real evaluator resolves attributes through context
// resolvers (the player controller, the pawn, the globals); this subset has the two values the experience rules need.
struct AttributeContext {
    int playThroughCount = 1;     // D_Attributes.Balance.PlayThroughCount: 1 on the first playthrough (UNVERIFIED reading)
    float level = 0;              // the global slot a GlobalAttributeValueResolver reads (the experience curve's level)
};

// A SUBSET of the native AttributeInitializationData evaluator (docs/verification/NATIVE_PROGRESSION.md section 1, read from
// native code, UNVERIFIED): constants, scale, attributes with a no-context chain of constant / simple-math / global resolvers,
// definitions with a ValueFormula or a ConditionalInitialization on PlayThroughCount, the base-value modes, range
// restriction and rounding. Arithmetic is single precision. Any shape outside the subset (random variance, other
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
    ObjectPtr load(const Value& reference);
    std::optional<float> attributeValue(Object& attribute, const AttributeContext& context);
    float combined(Object& definition, float base, const AttributeContext& context);
    float restricted(Object& definition, float value, const AttributeContext& context);
    bool expressionsHold(const Value& expressions, const AttributeContext& context);
    float operandValue(const Value& attribute, const AttributeContext& context);
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
