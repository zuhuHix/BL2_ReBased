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

ObjectPtr AttributeEvaluator::load(const Value& reference) {
    if (reference.kind != Value::Kind::Object || !reference.o || !reference.o->resourcePackage)
        throw RuntimeError("attribute evaluator: unresolved object reference");
    const auto key = std::make_pair(reference.o->resourcePackage.get(), reference.o->resourceIndex);
    auto found = loaded_.find(key);
    if (found == loaded_.end())
        found = loaded_.emplace(key, runtime_.instantiateExport(reference.o->resourcePackage, reference.o->resourceIndex, 4)).first;
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

// The attribute's value: with an empty context chain the attribute yields no value and the constant stands; every context
// resolver must be the no-context one; each value resolver gets the previous value (NATIVE_PROGRESSION.md section 1).
std::optional<float> AttributeEvaluator::attributeValue(Object& attribute, const AttributeContext& context) {
    const Value* contexts = runtime_.property(attribute, "ContextResolverChain");
    if (!contexts || contexts->elements().empty()) return std::nullopt;
    for (const auto& resolver : contexts->elements())
        if (load(resolver)->cls->path != "GearboxFramework.NoContextNeededAttributeContextResolver")
            throw RuntimeError("unsupported attribute context resolver " + load(resolver)->cls->path);
    float value = 0;
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
        } else {
            throw RuntimeError("unsupported attribute value resolver " + type);
        }
    }
    return value;
}

// f from the definition's formula or conditional, then combined with the base by BaseValueMode. The numbers 1..3 are taken
// as base + f, base x f and f - base in the note's order (which enum name is which was not checked: UNVERIFIED).
float AttributeEvaluator::combined(Object& definition, float base, const AttributeContext& context) {
    if (const Value* variance = runtime_.property(definition, "RandomVariance"))
        if (const Value* enabled = variance->field("bEnabled"); enabled && enabled->truth())
            throw RuntimeError("unsupported attribute definition: random variance");
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
    } else {
        throw RuntimeError("unsupported attribute definition: neither a value formula nor a conditional");
    }
    switch (int(need(runtime_.property(definition, "BaseValueMode"), "BaseValueMode").integer())) {
    case 0: return f;
    case 1: return base + f;
    case 2: return base * f;
    case 3: return f - base;
    default: throw RuntimeError("unsupported base value mode");
    }
}

// An entry's expressions are taken to all have to hold (UNVERIFIED). The only attribute they may name is PlayThroughCount.
bool AttributeEvaluator::expressionsHold(const Value& expressions, const AttributeContext& context) {
    const auto operators = enumNames(runtime_, "Engine", "AttributeExpression.EComparisonOperator");
    for (const auto& expression : expressions.elements()) {
        const float left = operandValue(need(expression.field("AttributeOperand1"), "AttributeOperand1"), context);
        const Value* attribute2 = expression.field("AttributeOperand2");
        const float right = attribute2 && attribute2->o ? operandValue(*attribute2, context) : number(expression.field("ConstantOperand2"));
        const std::string op = enumSuffix(operators, need(expression.field("ComparisonOperator"), "ComparisonOperator").integer());
        bool holds;
        if (op == "EqualTo") holds = left == right;
        else if (op == "NotEqualTo") holds = left != right;
        else if (op == "GreaterThan") holds = left > right;
        else if (op == "GreaterThanOrEqual") holds = left >= right;
        else if (op == "LessThan") holds = left < right;
        else if (op == "LessThanOrEqual") holds = left <= right;
        else throw RuntimeError("unsupported comparison operator " + op);
        if (!holds) return false;
    }
    return true;
}

float AttributeEvaluator::operandValue(const Value& attribute, const AttributeContext& context) {
    const std::string path = pathOf(attribute);
    if (!endsWith(path, "PlayThroughCount")) throw RuntimeError("unsupported condition attribute " + path);
    return float(context.playThroughCount);
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
