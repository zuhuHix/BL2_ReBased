// Weapon generator (see weapon_gen.hpp). Each function names the Python function it ports;
// the rule and its evidence are documented there and in docs/verification/NATIVE_WEAPON_RULES.md.
//
// Single precision: Python computes a double and rounds it with weapon_recipe.f32(); here the
// same double operation is done and F32() casts to float and back at the same points.

#include "weapon_gen.hpp"

#include <algorithm>
#include <charconv>
#include <cmath>
#include <cstdio>
#include <fstream>
#include <set>
#include <sstream>
#include <stdexcept>

namespace weapongen {

const char* const kSlots[kSlotCount] = {"Body", "Grip", "Barrel", "Sight", "Stock",
                                        "Elemental", "Accessory1", "Accessory2", "Material"};

int CrtRand::Rand() {
    state = state * 214013u + 2531011u;  // unsigned arithmetic wraps mod 2^32
    return static_cast<int>((state >> 16) & 0x7FFF);
}

namespace {

// ===================================================================================
// Manifest parsing (format: tools/weapon_gen_manifest.py docstring)
// ===================================================================================

struct ParseError : std::runtime_error {
    using std::runtime_error::runtime_error;
};

std::vector<std::string> Split(const std::string& line, char separator) {
    std::vector<std::string> fields;
    size_t start = 0;
    while (true) {
        size_t end = line.find(separator, start);
        fields.push_back(line.substr(start, end == std::string::npos ? std::string::npos : end - start));
        if (end == std::string::npos) return fields;
        start = end + 1;
    }
}

// Text field: '-' is none (empty here); backslash escapes \t, \n and \\.
std::string Text(const std::string& field) {
    if (field == "-") return {};
    std::string out;
    for (size_t i = 0; i < field.size(); ++i) {
        if (field[i] == '\\' && i + 1 < field.size()) {
            char next = field[++i];
            out += next == 't' ? '\t' : next == 'n' ? '\n' : next;
        } else {
            out += field[i];
        }
    }
    return out;
}

double Number(const std::string& field) {
    double value = 0;
    const char* begin = field.data();
    const char* end = begin + field.size();
    if (!field.empty() && field[0] == '+') ++begin;
    auto result = std::from_chars(begin, end, value);
    if (result.ec != std::errc() || result.ptr != end) throw ParseError("bad number '" + field + "'");
    return value;
}

std::optional<double> OptionalNumber(const std::string& field) {
    if (field == "-") return std::nullopt;
    return Number(field);
}

int Int(const std::string& field) {
    int value = 0;
    auto result = std::from_chars(field.data(), field.data() + field.size(), value);
    if (result.ec != std::errc() || result.ptr != field.data() + field.size()) throw ParseError("bad id '" + field + "'");
    return value;
}

std::vector<int> IdList(const std::string& field) {
    std::vector<int> ids;
    if (field == "-" || field.empty()) return ids;
    for (const std::string& item : Split(field, ',')) ids.push_back(Int(item));
    return ids;
}

std::vector<std::string> NameList(const std::string& field) {
    std::vector<std::string> names;
    if (field == "-" || field.empty()) return names;
    for (const std::string& item : Split(field, ',')) names.push_back(Text(item));
    return names;
}

// "Key:init,Key:init" -> terms in order.
std::vector<GenTerm> Terms(const std::string& field) {
    std::vector<GenTerm> terms;
    if (field == "-") return terms;
    for (const std::string& item : Split(field, ',')) {
        size_t colon = item.find(':');
        if (colon == std::string::npos) throw ParseError("bad term '" + item + "'");
        terms.push_back({item.substr(0, colon), Int(item.substr(colon + 1))});
    }
    return terms;
}

// Parse bookkeeping. Ids are dense (the builder numbers each table 0, 1, 2, ... in order of first use), and
// every id needs its own defining line, so an id at or past the line count is rejected before any table
// grows; Validate() then requires every slot to have been defined.
struct ParseState {
    size_t idLimit = 0;  // the manifest's line count
    bool header = false;
    std::vector<char> attributes, inits, definitions, objects;  // defined flags, one per table slot
};

template <class T>
T& At(std::vector<T>& table, std::vector<char>& defined, int id, size_t limit, const char* kind, bool define) {
    if (id < 0 || static_cast<size_t>(id) >= limit)
        throw ParseError(std::string("bad ") + kind + " id " + std::to_string(id) + " (ids are dense, below the " +
                         std::to_string(limit) + " lines of the manifest)");
    if (static_cast<size_t>(id) >= table.size()) {
        table.resize(static_cast<size_t>(id) + 1);
        defined.resize(static_cast<size_t>(id) + 1, 0);
    }
    if (define) defined[static_cast<size_t>(id)] = 1;
    return table[static_cast<size_t>(id)];
}

void Need(const std::vector<std::string>& f, size_t count) {
    if (f.size() < count) throw ParseError("record '" + f[0] + "' needs " + std::to_string(count) + " fields");
}

GenPoolBound Bound(const std::string& field) {
    GenPoolBound bound;
    if (field == "-") return bound;
    bound.present = true;
    if (field == "u") return bound;
    bound.resolved = true;
    bound.value = Number(field);
    return bound;
}

// One F record: an object field.
void ParseField(WeaponGenManifest& m, ParseState& state, const std::vector<std::string>& f) {
    Need(f, 3);
    GenObject& o = At(m.objects, state.objects, Int(f[1]), state.idLimit, "object", false);
    const std::string& name = f[2];
    auto arg = [&](size_t i) -> const std::string& {
        if (f.size() <= i + 3) throw ParseError("field " + name + " is missing values");
        return f[i + 3];
    };
    if (name == "Effect") {
        o.effects.push_back({Text(arg(0)), Text(arg(1)), Int(arg(2))});
    } else if (name == "SlotUpgrade") {
        o.slotUpgrades.push_back({Text(arg(0)), Number(arg(1)), arg(2) == "1"});
    } else if (name == "PrefixList") {
        o.prefixList = IdList(arg(0));
    } else if (name == "TitleList") {
        o.titleList = IdList(arg(0));
    } else if (name == "CustomPresentations") {
        o.customPresentations = IdList(arg(0));
    } else if (name == "WeaponCardPresentations") {
        o.cardPresentations = IdList(arg(0));
    } else if (name == "CardZoom") {
        o.cardZoom = true;
    } else if (name == "MonetaryValueMod") {
        o.monetaryMod = Int(arg(0));
    } else if (name == "Rarity") {
        o.rarity = Int(arg(0));
    } else if (name == "Base") {
        GenTypeBase base;
        base.field = Text(arg(0));
        if (arg(1) == "init") base.init = Int(arg(2));
        else base.number = Number(arg(2));
        o.bases.push_back(base);
    } else if (name == "SlotEffect") {
        GenSlotEffect e;
        e.slot = Text(arg(0));
        e.attribute = Text(arg(1));
        e.modifier = Text(arg(2));
        e.baseInit = Int(arg(3));
        e.perGradeInit = Int(arg(4));
        e.external = arg(5) == "1";
        e.enforceMin = arg(6) == "1";
        e.minGrade = Number(arg(7));
        e.enforceMax = arg(8) == "1";
        e.maxGrade = Number(arg(9));
        o.slotEffects.push_back(e);
    } else if (name == "SlotBaseGrade") {
        o.slotBaseGrade = Int(arg(0));
    } else if (name == "BaseRarity") {
        o.baseRarity = Int(arg(0));
    } else if (name == "MonetaryValue") {
        o.monetaryValue = Int(arg(0));
    } else if (name == "MonetaryBase") {
        o.monetaryBase = Number(arg(0));
    } else if (name == "SaleCap") {
        o.saleCap = std::stoll(arg(0));
    } else if (name == "PriceChecked") {
        o.priceChecked = true;
    } else if (name == "Gestalt") {
        o.gestalt = Text(arg(0));
    } else if (name == "SpinMode") {
        o.spinMode = Text(arg(0));
    } else if (name == "ZoomedEndFOV") {
        o.zoomedEndFov = Number(arg(0));
    } else if (name == "UsesLevelRequirement") {
        o.usesLevelRequirement = true;
    } else if (name == "DefaultDamageType") {
        o.defaultDamageType = Int(arg(0));
    } else if (name == "DefaultFiringMode") {
        o.defaultFiringMode = Int(arg(0));
    } else if (name == "Fragments") {
        o.fragments = NameList(arg(0));
    } else if (name == "Material") {
        o.material = Text(arg(0));
    } else if (name == "CustomDamageType") {
        o.customDamageType = Int(arg(0));
    } else if (name == "CustomFiringMode") {
        o.customFiringMode = Int(arg(0));
    } else if (name == "Spinning") {
        o.spinning = true;
    } else if (name == "SpinUpDuration") {
        o.spinUpDuration = Int(arg(0));
    } else if (name == "SpinStartScale") {
        o.spinStartScale = OptionalNumber(arg(0));
    } else if (name == "Name") {
        o.isNamePart = true;
        o.nameText = Text(arg(0));
        o.priority = Number(arg(1));
        o.minLevel = Number(arg(2));
        o.maxLevel = Number(arg(3));
        const std::string& qualify = arg(4);
        o.nameNever = qualify == "never";
        if (qualify.rfind("maker:", 0) == 0) o.nameMakers = NameList(qualify.substr(6));
    } else if (name == "NameExpr") {
        GenNameExpr e;
        e.damage = arg(0) == "damage";
        if (!e.damage && arg(0) != "maker") throw ParseError("unknown NameExpr kind " + arg(0));
        if (e.damage) {
            for (const std::string& item : NameList(arg(1))) {
                size_t colon = item.rfind(':');
                if (colon == std::string::npos) throw ParseError("bad NameExpr entry " + item);
                e.matched.emplace_back(item.substr(0, colon), Number(item.substr(colon + 1)));
            }
            e.notMatched = Number(arg(2));
        } else {
            e.maker = Text(arg(1));
        }
        e.op = Text(arg(3));
        e.constant = Number(arg(4));
        o.nameExprs.push_back(e);
    } else if (name == "Line") {
        o.hasLine = true;
        o.lineText = Text(arg(0));
        o.lineColor = Text(arg(1));
        o.lineRed = arg(2) == "1";
        o.lineMarkup = Text(arg(3));
    } else if (name == "DontDisplayNumber") {
        o.dontDisplayNumber = true;
    } else if (name == "DamageType") {
        o.damageTypeEnum = Text(arg(0));
    } else if (name == "StatusEffect") {
        o.statusEffect = Int(arg(0));
    } else if (name == "StatusEffectType") {
        o.statusEffectType = Text(arg(0));
    } else if (name == "Surface") {
        o.surfaces.push_back({Text(arg(0)), Int(arg(1))});
    } else if (name == "DamageOverTime") {
        o.damageOverTime = true;
    } else if (name == "BaseDuration") {
        o.baseDuration = Int(arg(0));
    } else if (name == "Speed") {
        o.speed = Number(arg(0));
    } else if (name == "Quantity") {
        o.isPool = true;
        o.quantity = OptionalNumber(arg(0));
    } else if (name == "Gate") {
        o.isPool = true;
        o.gateMin = Bound(arg(0));
        o.gateMax = Bound(arg(1));
    } else if (name == "Entry") {
        o.entries.push_back({arg(0) == "pool", Int(arg(1)), OptionalNumber(arg(2))});
    } else if (name == "Listed") {
        o.isSource = true;
        o.listed.push_back({arg(0) == "-" ? -1 : Int(arg(0)), OptionalNumber(arg(1))});
    }
    // Unknown fields are ignored so a newer builder can add fields.
}

int SlotIndex(const std::string& slot) {
    for (int i = 0; i < kSlotCount; ++i)
        if (slot == kSlots[i]) return i;
    throw ParseError("unknown slot " + slot);
}

void ParseLine(WeaponGenManifest& m, ParseState& state, const std::string& line) {
    std::vector<std::string> f = Split(line, '\t');
    const std::string& tag = f[0];
    if (!state.header && tag != "V") throw ParseError("missing 'V weapon_gen_manifest <1..3>' header line");
    if (tag == "V") {
        Need(f, 3);
        // Version 2 added MonetaryBase, SaleCap and status_remap; a version 1 manifest keeps the old results.
        // Version 3 writes name expressions as NameExpr fields (qualify 'expr'); older ones keep maker:<A,B>.
        if (f[1] != "weapon_gen_manifest" || (f[2] != "1" && f[2] != "2" && f[2] != "3"))
            throw ParseError("unsupported manifest header '" + f[1] + " " + f[2] + "' (want weapon_gen_manifest 1..3)");
        if (state.header) throw ParseError("second V header line");
        state.header = true;
    } else if (tag == "A") {
        Need(f, 4);
        int id = Int(f[1]);
        At(m.attributes, state.attributes, id, state.idLimit, "attribute", true) = {Text(f[2]), OptionalNumber(f[3])};
        m.attributeByPath[Text(f[2])] = id;
    } else if (tag == "I") {
        Need(f, 6);
        At(m.inits, state.inits, Int(f[1]), state.idLimit, "init", true) = {Number(f[2]), Int(f[3]), Int(f[4]), OptionalNumber(f[5])};
    } else if (tag == "D") {
        Need(f, 11);
        GenDefinition& d = At(m.definitions, state.definitions, Int(f[1]), state.idLimit, "definition", true);
        d.path = Text(f[2]);
        d.mode = Text(f[3]);
        d.conditional = f[4] == "1";
        d.formulaEnabled = f[5] == "1";
        d.formula = Terms(f[6]);
        d.minEnabled = f[7] == "1";
        d.maxEnabled = f[8] == "1";
        d.clamp = Terms(f[9]);
        d.rounding = Text(f[10]);
    } else if (tag == "O") {
        Need(f, 4);
        int id = Int(f[1]);
        GenObject& o = At(m.objects, state.objects, id, state.idLimit, "object", true);
        o.cls = Text(f[2]);
        o.path = Text(f[3]);
        m.objectByPath[o.path] = id;
    } else if (tag == "F") {
        ParseField(m, state, f);
    } else if (tag == "B") {
        Need(f, 5);
        GenBalance b;
        b.object = Int(f[1]);
        b.type = Int(f[2]);
        b.manufacturer = Text(f[3]);
        b.mission = f[4] == "1";
        m.balanceByObject[std::to_string(b.object)] = static_cast<int>(m.balances.size());
        m.balances.push_back(b);
    } else if (tag == "S") {
        Need(f, 7);
        auto found = m.balanceByObject.find(f[1]);
        if (found == m.balanceByObject.end()) throw ParseError("slot entry before its balance");
        GenSlotEntry e;
        e.part = Int(f[3]);
        if (f[4] == "flat") e.kind = GenSlotEntry::Weight::Flat;
        else if (f[4] == "none") e.kind = GenSlotEntry::Weight::None;
        else e.kind = GenSlotEntry::Weight::Init, e.weightInit = Int(f[4]);
        e.minInit = Int(f[5]);
        e.maxInit = Int(f[6]);
        m.balances[static_cast<size_t>(found->second)].slots[SlotIndex(f[2])].push_back(e);
    } else if (tag == "G") {
        Need(f, 2);
        if (f[1] == "rarity") {
            Need(f, 6);
            m.rarity.push_back({Number(f[2]), Number(f[3]), Text(f[4]), Text(f[5])});
        } else if (f[1] == "round") {
            Need(f, 5);
            m.rounding[Text(f[2])] = {Text(f[3]), Number(f[4])};
        } else if (f[1] == "accuracy") {
            Need(f, 6);
            m.hasAccuracy = true;
            for (int i = 0; i < 4; ++i) m.accuracy[i] = Number(f[static_cast<size_t>(i) + 2]);
        } else if (f[1] == "status_remap") {
            Need(f, 7);
            std::vector<double>& bounds = m.statusRemap[Text(f[2])];
            bounds.clear();
            for (size_t i = 3; i < 7; ++i) bounds.push_back(Number(f[i]));
        } else if (f[1] == "shot_cost_text") {
            Need(f, 4);
            m.shotCostPrefix = Text(f[2]);
            m.shotCostSuffix = Text(f[3]);
        }
    }
}

// Every table is dense and fully defined, and every id a record refers to is in range (-1 only where the
// field allows none), so evaluation needs no checks. An initializer chain that refers back to itself is
// rejected too (evaluation would recurse forever).
void Validate(const WeaponGenManifest& m, const ParseState& state) {
    if (!state.header) throw ParseError("missing 'V weapon_gen_manifest <1..3>' header line");
    auto dense = [](const std::vector<char>& defined, const char* kind, char tag) {
        for (size_t id = 0; id < defined.size(); ++id)
            if (!defined[id])
                throw ParseError(std::string(kind) + " id " + std::to_string(id) + " is used but has no " + tag +
                                 " record (ids must be 0..n-1, all defined)");
    };
    dense(state.attributes, "attribute", 'A');
    dense(state.inits, "init", 'I');
    dense(state.definitions, "definition", 'D');
    dense(state.objects, "object", 'O');
    auto check = [](int id, size_t size, bool optional, const char* kind) {
        if (optional && id == -1) return;
        if (id < 0 || static_cast<size_t>(id) >= size)
            throw ParseError(std::string("dangling ") + kind + " id " + std::to_string(id));
    };
    auto init = [&](int id) { check(id, m.inits.size(), false, "init"); };
    auto optInit = [&](int id) { check(id, m.inits.size(), true, "init"); };
    auto object = [&](int id) { check(id, m.objects.size(), false, "object"); };
    auto optObject = [&](int id) { check(id, m.objects.size(), true, "object"); };
    // Init 0 is the empty initializer a missing formula term evaluates (FormulaTerm).
    if (m.inits.empty()) throw ParseError("no init records");
    for (const GenInit& i : m.inits) {
        check(i.attribute, m.attributes.size(), true, "attribute");
        check(i.definition, m.definitions.size(), true, "definition");
    }
    for (const GenDefinition& d : m.definitions) {
        for (const GenTerm& t : d.formula) init(t.init);
        for (const GenTerm& t : d.clamp) init(t.init);
    }
    for (const GenBalance& b : m.balances) {
        object(b.object);
        object(b.type);
        for (const auto& slot : b.slots)
            for (const GenSlotEntry& e : slot) {
                object(e.part);
                if (e.kind == GenSlotEntry::Weight::Init) init(e.weightInit);
                optInit(e.minInit);
                optInit(e.maxInit);
            }
    }
    for (const GenObject& o : m.objects) {
        for (const GenEffect& e : o.effects) init(e.init);
        for (const GenSlotEffect& e : o.slotEffects) init(e.baseInit), init(e.perGradeInit);
        for (const GenTypeBase& b : o.bases) optInit(b.init);
        for (int id : {o.slotBaseGrade, o.baseRarity, o.monetaryValue, o.rarity, o.spinUpDuration, o.baseDuration})
            optInit(id);
        for (const GenSurface& s : o.surfaces) init(s.init);
        for (const auto* list : {&o.prefixList, &o.titleList, &o.customPresentations, &o.cardPresentations})
            for (int id : *list) object(id);
        for (int id : {o.defaultDamageType, o.defaultFiringMode, o.customDamageType, o.customFiringMode, o.statusEffect})
            optObject(id);
        for (const GenPoolEntry& e : o.entries) object(e.object);
        for (const GenListed& l : o.listed) optObject(l.pool);
        check(o.monetaryMod, m.attributes.size(), true, "attribute");
    }
    // Cycle check over init -> definition -> term inits (0 unvisited, 1 on the path, 2 done).
    std::vector<char> mark(m.inits.size(), 0);
    std::vector<std::pair<int, size_t>> stack;  // init id, next term index
    auto terms = [&](int id) -> std::vector<int> {
        std::vector<int> out;
        int def = m.inits[static_cast<size_t>(id)].definition;
        if (def < 0) return out;
        const GenDefinition& d = m.definitions[static_cast<size_t>(def)];
        for (const GenTerm& t : d.formula) out.push_back(t.init);
        for (const GenTerm& t : d.clamp) out.push_back(t.init);
        return out;
    };
    for (size_t root = 0; root < m.inits.size(); ++root) {
        if (mark[root]) continue;
        mark[root] = 1;
        stack.push_back({static_cast<int>(root), 0});
        while (!stack.empty()) {
            auto& [id, next] = stack.back();
            std::vector<int> children = terms(id);
            if (next >= children.size()) {
                mark[static_cast<size_t>(id)] = 2;
                stack.pop_back();
                continue;
            }
            int child = children[next++];
            if (mark[static_cast<size_t>(child)] == 1)
                throw ParseError("init " + std::to_string(child) + " refers back to itself through its definition");
            if (mark[static_cast<size_t>(child)] == 0) {
                mark[static_cast<size_t>(child)] = 1;
                stack.push_back({child, 0});
            }
        }
    }
}

// ===================================================================================
// Attribute evaluation (weapon_recipe.attribute_value and friends)
// ===================================================================================

using Known = std::unordered_map<std::string, double>;

const char* const kWeaponLevel = "D_Attributes.Weapon.WeaponLevel";
const char* const kPartPriceTotal = "D_Attributes.Inventory.InventoryPartMonetaryValueModifierTotal";
const std::string kAttr = "D_Attributes.Weapon.";

// weapon_recipe.f32: round a double to IEEE single precision.
double F32(double value) { return static_cast<double>(static_cast<float>(value)); }

// weapon_stats.remap_value: a presentation's linear remap of [a[0], a[1]] onto [a[2], a[3]], input clamped;
// an empty input range gives the output low.
double Remap(const double* a, double x) {
    if (a[1] <= a[0]) return F32(a[2]);
    double slope = F32((a[3] - a[2]) / (a[1] - a[0]));
    double clamped = std::min(a[1], std::max(a[0], x));
    return F32((clamped - a[0]) * slope + a[2]);
}

int FormulaTerm(const GenDefinition& d, const char* key) {
    for (const GenTerm& t : d.formula)
        if (t.key == key) return t.init;
    return 0;  // a missing term is the empty initializer (value 0)
}

const GenTerm* ClampTerm(const GenDefinition& d, const char* key) {
    for (const GenTerm& t : d.clamp)
        if (t.key == key) return &t;
    return nullptr;
}

bool IsSetsMode(const std::string& mode) { return mode.empty() || mode == "BASEVALUE_InitializationDefSetsBaseValue"; }

bool IsKnownMode(const std::string& mode) {
    return IsSetsMode(mode) || mode == "BASEVALUE_InitializationDefAddsToBaseValue" ||
           mode == "BASEVALUE_InitializationDefScalesBaseValue" || mode == "BASEVALUE_InitializationDefOffsetByBaseValue";
}

// EBaseValueMode: how a definition's result f combines with the base b.
double CombineBase(const std::string& mode, double b, double f) {
    if (mode == "BASEVALUE_InitializationDefAddsToBaseValue") return b + f;
    if (mode == "BASEVALUE_InitializationDefScalesBaseValue") return b * f;
    if (mode == "BASEVALUE_InitializationDefOffsetByBaseValue") return f - b;
    return f;  // SetsBaseValue
}

// weapon_recipe.formula_value: Multiplier * (Level ^ Power + Offset), each step in single precision;
// the power is skipped when it is 1.
double FormulaValue(double multiplier, double level, double power, double offset) {
    double raised = power == 1 ? level : F32(std::pow(level, power));
    return F32(multiplier * F32(raised + offset));
}

// weapon_recipe.ROUNDING (EAttributeInitializationRounding).
double Round(const std::string& mode, double x) {
    if (mode == "ATTRROUNDING_IntRound") return std::floor(x + 0.5);
    if (mode == "ATTRROUNDING_IntFloor") return std::floor(x);
    if (mode == "ATTRROUNDING_IntCeil") return std::ceil(x);
    return x;
}

// weapon_recipe.attribute_value: an initializer's value with the attributes in `known`, or none when it
// needs an unknown attribute or a conditional. Order: base (attribute or constant), the definition's
// formula combined by BaseValueMode, times BaseValueScaleConstant, the enabled clamps, the rounding.
std::optional<double> AttributeValue(const WeaponGenManifest& m, int id, const Known& known) {
    const GenInit& init = m.inits[static_cast<size_t>(id)];
    std::optional<double> base;
    if (init.attribute >= 0) {
        auto it = known.find(m.attributes[static_cast<size_t>(init.attribute)].path);
        if (it != known.end()) base = F32(it->second);
    } else {
        base = F32(init.constant);
    }
    const GenDefinition* def = init.definition >= 0 ? &m.definitions[static_cast<size_t>(init.definition)] : nullptr;
    if (def) {
        if (def->conditional) return std::nullopt;
        if (def->formulaEnabled) {
            auto multiplier = AttributeValue(m, FormulaTerm(*def, "Multiplier"), known);
            auto level = AttributeValue(m, FormulaTerm(*def, "Level"), known);
            auto power = AttributeValue(m, FormulaTerm(*def, "Power"), known);
            auto offset = AttributeValue(m, FormulaTerm(*def, "Offset"), known);
            if (!multiplier || !level || !power || !offset) return std::nullopt;
            if (!IsKnownMode(def->mode) || (!base && !IsSetsMode(def->mode))) return std::nullopt;
            base = F32(CombineBase(def->mode, base.value_or(0.0), FormulaValue(*multiplier, *level, *power, *offset)));
        }
    }
    if (!base) return std::nullopt;
    double value = F32(*base * (init.scale ? *init.scale : 1.0));
    if (!def) return value;
    // An unresolvable bound is skipped, as in Python.
    const GenTerm* low = ClampTerm(*def, "MinValue");
    if (def->minEnabled && low) {
        auto bound = AttributeValue(m, low->init, known);
        if (bound && *bound > value) value = *bound;
    }
    const GenTerm* high = ClampTerm(*def, "MaxValue");
    if (def->maxEnabled && high) {
        auto bound = AttributeValue(m, high->init, known);
        if (bound && *bound < value) value = *bound;
    }
    return F32(Round(def->rounding, value));
}

// Text after the last occurrence of `marker` (Python's rsplit(marker, 1)[1]).
std::string After(const std::string& text, const std::string& marker) {
    size_t at = text.rfind(marker);
    return at == std::string::npos ? text : text.substr(at + marker.size());
}

// weapon_stats.evaluate's value(): attribute_value with attributes resolved lazily. Weapon_Is_<Maker> is 1
// for the weapon's manufacturer, else 0; other attributes take their constant resolver's value. `known`
// accumulates across one evaluation (a clamp bound naming an attribute nobody resolved yet is skipped,
// exactly as Python does).
class Evaluator {
public:
    Evaluator(const WeaponGenManifest& m, std::string maker, int level) : m_(m), maker_(std::move(maker)) {
        known_[kWeaponLevel] = static_cast<double>(level);
    }

    std::optional<double> Value(int id) {
        while (true) {
            auto result = AttributeValue(m_, id, known_);
            if (result) return result;
            std::string missing = FindMissing(id);
            if (missing.empty()) return std::nullopt;
            if (missing.find(".Weapon_Is_") != std::string::npos) {
                known_[missing] = After(missing, "Weapon_Is_") == maker_ ? 1.0 : 0.0;
                continue;
            }
            auto attribute = m_.attributeByPath.find(missing);
            if (attribute == m_.attributeByPath.end() || !m_.attributes[static_cast<size_t>(attribute->second)].constant)
                return std::nullopt;  // unresolved
            known_[missing] = *m_.attributes[static_cast<size_t>(attribute->second)].constant;
        }
    }

private:
    // The first attribute (depth first, in the package's term order) not yet known.
    std::string FindMissing(int id) const {
        const GenInit& init = m_.inits[static_cast<size_t>(id)];
        if (init.attribute >= 0) {
            const std::string& path = m_.attributes[static_cast<size_t>(init.attribute)].path;
            if (!known_.count(path)) return path;
        }
        if (init.definition >= 0) {
            const GenDefinition& d = m_.definitions[static_cast<size_t>(init.definition)];
            for (const auto* terms : {&d.formula, &d.clamp})
                for (const GenTerm& t : *terms) {
                    std::string found = FindMissing(t.init);
                    if (!found.empty()) return found;
                }
        }
        return {};
    }

    const WeaponGenManifest& m_;
    std::string maker_;
    Known known_;
};

// weapon_stats.price_init: the price calculator's evaluation. Attributes come from `known` or their
// constant resolver; no clamp, rounding or conditional (none occurs in the gun price chain). The constant
// base is not rounded to single precision before the scale, unlike attribute_value.
std::optional<double> PriceInit(const WeaponGenManifest& m, int id, const Known& known) {
    const GenInit& init = m.inits[static_cast<size_t>(id)];
    double base = init.constant;
    if (init.attribute >= 0) {
        const GenAttribute& attribute = m.attributes[static_cast<size_t>(init.attribute)];
        auto it = known.find(attribute.path);
        if (it != known.end()) base = it->second;
        else if (attribute.constant) base = *attribute.constant;
        else return std::nullopt;
    }
    if (init.definition >= 0) {
        const GenDefinition& d = m.definitions[static_cast<size_t>(init.definition)];
        if (d.formulaEnabled) {
            auto multiplier = PriceInit(m, FormulaTerm(d, "Multiplier"), known);
            auto level = PriceInit(m, FormulaTerm(d, "Level"), known);
            auto power = PriceInit(m, FormulaTerm(d, "Power"), known);
            auto offset = PriceInit(m, FormulaTerm(d, "Offset"), known);
            if (!multiplier || !level || !power || !offset) return std::nullopt;
            double value = FormulaValue(*multiplier, *level, *power, *offset);
            if (!IsKnownMode(d.mode)) return std::nullopt;
            base = F32(CombineBase(d.mode, F32(base), value));
        }
    }
    return F32(base * (init.scale ? *init.scale : 1.0));
}

// ===================================================================================
// Rolling parts and names (weapon_recipe.roll)
// ===================================================================================

const double kFlatWeight = 100.0;  // weapon_recipe.FLAT_WEIGHT
const double kMinWeight = 1e-8;    // weapon_recipe.MIN_WEIGHT

const GenObject& Obj(const WeaponGenManifest& m, int id) { return m.objects[static_cast<size_t>(id)]; }

struct Candidate {
    int part = -1;
    std::optional<double> weight;
};

// weapon_recipe.slot_candidates for one merged slot: drop entries outside [trunc(min), trunc(max)] of their
// game-stage window; the weight follows entry_weight (resolved per balance by the manifest builder).
// Weights and bounds are evaluated with no attribute known, as roll() does.
std::vector<Candidate> SlotCandidates(const WeaponGenManifest& m, const std::vector<GenSlotEntry>& entries, int stage) {
    static const Known none;
    std::vector<Candidate> found;
    for (const GenSlotEntry& e : entries) {
        std::optional<double> low = e.minInit >= 0 ? AttributeValue(m, e.minInit, none) : std::nullopt;
        std::optional<double> high = e.maxInit >= 0 ? AttributeValue(m, e.maxInit, none) : std::nullopt;
        if ((low && stage < std::trunc(*low)) || (high && std::trunc(*high) < stage)) continue;
        Candidate c;
        c.part = e.part;
        if (e.kind == GenSlotEntry::Weight::Flat) c.weight = kFlatWeight;
        else if (e.kind == GenSlotEntry::Weight::Init) c.weight = AttributeValue(m, e.weightInit, none);
        found.push_back(c);
    }
    return found;
}

// weapon_recipe.pick: entries weighing <= 1e-8 are skipped; a part listed twice keeps its later weight at
// its first position; r = total * rand() / 32767 and the first entry whose [sum, sum + weight] holds r
// wins (the last one if rounding leaves r past every interval).
struct Pick {
    int part = -1;
    double weight = 0;
    int candidates = 0;
};

Pick PickPart(const std::vector<Candidate>& entries, CrtRand& rng) {
    std::vector<std::pair<int, double>> candidates;
    for (const Candidate& c : entries) {
        if (!c.weight || !(*c.weight > kMinWeight)) continue;
        auto it = std::find_if(candidates.begin(), candidates.end(), [&](const auto& p) { return p.first == c.part; });
        if (it != candidates.end()) it->second = *c.weight;
        else candidates.push_back({c.part, *c.weight});
    }
    Pick pick;
    pick.candidates = static_cast<int>(candidates.size());
    if (candidates.empty()) return pick;
    double total = 0.0;
    for (const auto& c : candidates) total += c.second;
    double r = total * static_cast<double>(rng.Rand()) / 32767.0;
    double running = 0.0;
    for (const auto& c : candidates) {
        if (running <= r && r <= running + c.second) {
            pick.part = c.first, pick.weight = c.second;
            return pick;
        }
        running += c.second;
    }
    pick.part = candidates.back().first, pick.weight = candidates.back().second;
    return pick;
}

std::string ShortName(const std::string& path) { return After(path, "."); }

// weapon_recipe.weapon_damage_type (the card's rule): the elemental part's CustomDamageTypeDefinition, else
// any part's in slot order, else the type's default (object id, -1 for none). The any-part fallback is UNVERIFIED.
int WeaponDamageType(const WeaponGenManifest& m, int type, const int (&parts)[kSlotCount]) {
    const int elemental = 5;
    if (parts[elemental] >= 0 && Obj(m, parts[elemental]).customDamageType >= 0)
        return Obj(m, parts[elemental]).customDamageType;
    for (int slot = 0; slot < kSlotCount; ++slot)
        if (slot != elemental && parts[slot] >= 0 && Obj(m, parts[slot]).customDamageType >= 0)
            return Obj(m, parts[slot]).customDamageType;
    return Obj(m, type).defaultDamageType;
}

// weapon_recipe.native_damage_type (the name rule): the native resolver reads only the elemental part's
// CustomDamageTypeDefinition, else the type's default.
int NativeDamageType(const WeaponGenManifest& m, int type, const int (&parts)[kSlotCount]) {
    const int elemental = 5;
    if (parts[elemental] >= 0 && Obj(m, parts[elemental]).customDamageType >= 0)
        return Obj(m, parts[elemental]).customDamageType;
    return Obj(m, type).defaultDamageType;
}

// weapon_recipe.COMPARISONS.
bool Compare(const std::string& op, double a, double b) {
    if (op == "OPERATOR_EqualTo") return a == b;
    if (op == "OPERATOR_NotEqualTo") return a != b;
    if (op == "OPERATOR_LessThan") return a < b;
    if (op == "OPERATOR_LessThanOrEqualTo") return a <= b;
    if (op == "OPERATOR_GreaterThan") return a > b;
    if (op == "OPERATOR_GreaterThanOrEqualTo") return a >= b;
    return false;
}

// weapon_recipe.name_operand_value.
double NameOperandValue(const GenNameExpr& e, const std::string& manufacturer, const std::string& damageType) {
    if (!e.damage) return !manufacturer.empty() && e.maker == ShortName(manufacturer) ? 1.0 : 0.0;
    if (!damageType.empty())
        for (const auto& [key, value] : e.matched)
            if (key == ShortName(damageType)) return value;
    return e.notMatched;
}

// weapon_recipe.name_part_qualifies: level window, positive priority, and every expression holds
// (version 3: NameExpr; version 2: every Weapon_Is_<Maker> == 1 names the weapon's manufacturer).
// `damageType` is the weapon's damage type object path (empty for none).
bool NameQualifies(const GenObject& n, int level, const std::string& manufacturer, const std::string& damageType) {
    if (!n.isNamePart || n.nameNever) return false;
    if (!(n.minLevel <= level && level <= n.maxLevel)) return false;
    for (const std::string& maker : n.nameMakers)
        if (manufacturer.empty() || maker != ShortName(manufacturer)) return false;
    for (const GenNameExpr& e : n.nameExprs)
        if (!Compare(e.op, NameOperandValue(e, manufacturer, damageType), e.constant)) return false;
    return true;
}

// weapon_recipe.best_name_part: the qualifying entry with the highest priority, a later one on ties.
int BestNamePart(const WeaponGenManifest& m, const std::vector<int>& names, int level, const std::string& manufacturer,
                 const std::string& damageType) {
    int best = -1;
    for (int id : names)
        if (NameQualifies(Obj(m, id), level, manufacturer, damageType) &&
            (best < 0 || Obj(m, best).priority <= Obj(m, id).priority))
            best = id;
    return best;
}

// weapon_recipe.choose_name_parts for one list: the type's pick first, then each part in slot order
// replaces it when its pick's priority is >= the current one. Runs after the parts are chosen.
int ChooseNamePart(const WeaponGenManifest& m, bool prefix, int type, const int (&parts)[kSlotCount], int level,
                   const std::string& manufacturer) {
    auto list = [&](const GenObject& o) -> const std::vector<int>& { return prefix ? o.prefixList : o.titleList; };
    int damageId = NativeDamageType(m, type, parts);
    const std::string damageType = damageId >= 0 ? Obj(m, damageId).path : std::string();
    int current = BestNamePart(m, list(Obj(m, type)), level, manufacturer, damageType);
    for (int slot = 0; slot < kSlotCount; ++slot) {
        if (parts[slot] < 0) continue;
        int candidate = BestNamePart(m, list(Obj(m, parts[slot])), level, manufacturer, damageType);
        if (current < 0 || (candidate >= 0 && Obj(m, current).priority <= Obj(m, candidate).priority))
            current = candidate;
    }
    return current;
}

// ===================================================================================
// Card stats (weapon_stats.evaluate)
// ===================================================================================

// Summed modifiers of one attribute (single precision): PreAdd, PostAdd, positive and non-positive scales.
struct Modifiers {
    double pre = 0, post = 0, scaleUp = 0, scaleDown = 0;
};

const std::set<std::string>& IntAttributes() {
    static const std::set<std::string> names = {kAttr + "WeaponClipSize", kAttr + "WeaponProjectilesPerShot",
                                                kAttr + "WeaponShotCost", kAttr + "WeaponAutomaticBurstCount"};
    return names;
}

// weapon_stats.combine: (base + PreAdd) * (1 + up) / (1 + down) + PostAdd, stored as a float, or
// truncated toward zero for an integer attribute (whose base is truncated first).
double Combine(double start, const Modifiers& mods, bool integer) {
    double factor = (1.0 + mods.scaleUp) / (1.0 + mods.scaleDown);
    if (integer) return std::trunc(factor * (std::trunc(start) + mods.pre) + mods.post);
    return F32((start + mods.pre) * factor + mods.post);
}

// weapon_stats.half_up: floor(number * 10^digits + 0.5) / 10^digits, each step in single precision.
double HalfUp(double number, double digits) {
    double scale = F32(std::pow(10.0, digits));
    return std::floor(F32(F32(number * scale) + 0.5)) / scale;
}

// weapon_stats.present: a stat as the card prints it.
WeaponGenNumber Present(const std::string& field, double value, const std::string& mode, double precision) {
    WeaponGenNumber shown{field, 0, true};
    value = F32(value);
    if (std::fabs(value) < 1e-8) value = 0.0;
    if (mode == "ATTRROUNDING_IntCeil") shown.value = std::ceil(value);
    else if (mode == "ATTRROUNDING_IntFloor") shown.value = std::floor(value);
    else if (mode == "ATTRROUNDING_IntRound") shown.value = std::floor(value + 0.5);
    else shown.value = HalfUp(value, std::max(0.0, std::min(10.0, precision))), shown.integer = false;
    return shown;
}

// The card fields with a presentation, in weapon_stats.PRESENTATIONS order, and their fallback rounding.
const std::vector<std::pair<std::string, std::string>>& PresentationFields() {
    static const std::vector<std::pair<std::string, std::string>> fields = {
        {"damage", "ATTRROUNDING_IntCeil"},  {"magazine", "ATTRROUNDING_IntFloor"},
        {"fire_rate", "ATTRROUNDING_Float"}, {"reload_time", "ATTRROUNDING_Float"},
        {"accuracy", "ATTRROUNDING_Float"},  {"weapon_zoom", "ATTRROUNDING_Float"}};
    return fields;
}

GenRounding RoundingFor(const WeaponGenManifest& m, const std::string& field) {
    auto it = m.rounding.find(field);
    if (it != m.rounding.end()) return it->second;
    for (const auto& [name, mode] : PresentationFields())
        if (name == field) return {mode, 1};
    return {"ATTRROUNDING_Float", 1};
}

std::string HtmlEscape(const std::string& text) {
    std::string out;
    for (char c : text) {
        if (c == '&') out += "&amp;";
        else if (c == '<') out += "&lt;";
        else if (c == '>') out += "&gt;";
        else out += c;
    }
    return out;
}

struct CardLine {
    std::string text, markup;
};

CardLine WhiteLine(const std::string& text) {
    return {text, "\xE2\x80\xA2 <font color=\"#FFFFFF\">" + HtmlEscape(text) + "</font>"};
}

// weapon_stats.card_text_lines: red flavour lines, the zoom line, the ammo line, other text-only custom
// presentations, then text-only card presentations; a text already shown is not repeated.
std::vector<CardLine> CardTextLines(const WeaponGenManifest& m, const std::vector<int>& sources,
                                    std::optional<double> shotCost, std::optional<double> zoom) {
    std::vector<CardLine> red, custom;
    std::set<std::string> seen;
    auto add = [&](std::vector<CardLine>& target, const GenObject& p) {
        if (!p.hasLine || seen.count(p.lineText)) return;
        seen.insert(p.lineText);
        target.push_back({p.lineText, p.lineMarkup});
    };
    for (int owner : sources)
        for (int id : Obj(m, owner).customPresentations) {
            const GenObject& p = Obj(m, id);
            if (!p.hasLine) continue;
            if (p.lineRed) add(red, p);
            else if (p.dontDisplayNumber) add(custom, p);
        }
    std::vector<CardLine> ordered = red;
    if (zoom) {
        char buffer[64];
        std::snprintf(buffer, sizeof buffer, "%.1fx Weapon Zoom", *zoom);
        ordered.push_back(WhiteLine(buffer));
    }
    // weapon_stats.shot_cost_text: only above 1 ammo per shot.
    if (shotCost && *shotCost > 1) {
        char amount[64];
        if (*shotCost == std::trunc(*shotCost)) std::snprintf(amount, sizeof amount, "%lld", static_cast<long long>(*shotCost));
        else std::snprintf(amount, sizeof amount, "%g", *shotCost);
        ordered.push_back(WhiteLine(m.shotCostPrefix + " " + amount + " " + m.shotCostSuffix));
    }
    ordered.insert(ordered.end(), custom.begin(), custom.end());
    for (int owner : sources)
        for (int id : Obj(m, owner).cardPresentations)
            if (Obj(m, id).dontDisplayNumber) add(ordered, Obj(m, id));
    return ordered;
}

std::string Join(const std::vector<std::string>& items, const std::string& separator) {
    std::string out;
    for (size_t i = 0; i < items.size(); ++i) out += (i ? separator : "") + items[i];
    return out;
}

// The rolled gun that evaluate() reads.
struct Recipe {
    const GenBalance* balance = nullptr;
    int parts[kSlotCount] = {-1, -1, -1, -1, -1, -1, -1, -1, -1};
    int prefix = -1, title = -1;
    std::string name;
};

// weapon_stats.evaluate. The value() calls happen in Python's order (they share `known`).
class CardEvaluator {
public:
    CardEvaluator(const WeaponGenManifest& m, const Recipe& recipe, int level)
        : m_(m), r_(recipe), type_(Obj(m, recipe.balance->type)), level_(level),
          maker_(ShortName(recipe.balance->manufacturer)), value_(m, maker_, level) {}

    WeaponGenCard Run();

private:
    void AddModifier(const std::string& attribute, const std::string& kind, std::optional<double> amount);
    void ApplyEffects(const GenObject& o);
    void Bases();
    void SlotEffects();
    std::optional<double> Stat(const char* name) const;
    std::vector<int> OrderedParts() const;
    std::vector<int> PresentationSources() const;
    void SaleValue(WeaponGenCard& card) const;
    void Rarity(WeaponGenCard& card);
    int DamageType() const;
    void Status(WeaponGenCard& card, int damageType);
    std::optional<double> Zoom() const;
    void Display(WeaponGenCard& card) const;

    const WeaponGenManifest& m_;
    const Recipe& r_;
    const GenObject& type_;
    int level_;
    std::string maker_;
    Evaluator value_;
    std::map<std::string, std::optional<double>> base_;
    std::map<std::string, Modifiers> mods_;
    std::map<std::string, double> final_;
};

// evaluate()'s add(): each modifier kind sums in single precision; a scale also counts toward the
// positive or non-positive side by its sign.
void CardEvaluator::AddModifier(const std::string& attribute, const std::string& kind, std::optional<double> amount) {
    if (!amount) return;
    double value = F32(*amount);
    Modifiers& bucket = mods_[attribute];
    if (kind == "MT_PreAdd") bucket.pre = F32(bucket.pre + value);
    else if (kind == "MT_PostAdd") bucket.post = F32(bucket.post + value);
    else if (kind == "MT_Scale") {
        double& side = value > 0 ? bucket.scaleUp : bucket.scaleDown;
        side = F32(side + std::fabs(value));
    }
}

void CardEvaluator::ApplyEffects(const GenObject& o) {
    for (const GenEffect& e : o.effects) AddModifier(e.attribute, e.modifier, value_.Value(e.init));
}

// Bases: the type's fields (class defaults applied by the builder), then the WillowWeapon defaults,
// then a spinning barrel's spin-up duration.
void CardEvaluator::Bases() {
    static const std::vector<std::pair<std::string, std::string>> typeBases = {
        {"InstantHitDamage", "WeaponDamage"}, {"FireRate", "WeaponFireInterval"}, {"ClipSize", "WeaponClipSize"},
        {"ReloadTime", "WeaponReloadSpeed"}, {"Spread", "WeaponSpread"},
        {"ProjectilesPerShot", "WeaponProjectilesPerShot"}, {"StatusEffectDamage", "WeaponStatusEffectDamage"},
        {"BaseStatusEffectChanceModifier", "WeaponBaseStatusEffectChanceModifier"}};
    for (const auto& [field, attribute] : typeBases)
        for (const GenTypeBase& b : type_.bases)
            if (b.field == field) base_[kAttr + attribute] = b.init >= 0 ? value_.Value(b.init) : F32(b.number);
    for (const char* name : {"WeaponShotCost", "WeaponStatusEffectChanceModifier", "WeaponProjectileSpeedMultiplier"}) {
        auto& slot = base_[kAttr + name];
        if (!slot) slot = 1.0;
    }
    int barrel = r_.parts[2];
    if (barrel >= 0 && Obj(m_, barrel).spinning && Obj(m_, barrel).spinUpDuration >= 0)
        base_[kAttr + "WeaponBarrelSpinUpDuration"] = value_.Value(Obj(m_, barrel).spinUpDuration);
}

// Attribute slots (weapon_stats.slot_grade / slot_value): grade = round-half-up(type base grade, default 1)
// + every GradeIncrease of the type and the parts, clamped by the maximum then the minimum; an activated
// internal slot adds f32(f32((grade - 1) * PerGradeUpgrade) + BaseModifierValue).
void CardEvaluator::SlotEffects() {
    std::map<std::string, double> grades;
    std::set<std::string> activated;
    std::vector<int> owners = {r_.balance->type};
    for (int part : OrderedParts()) owners.push_back(part);
    for (size_t i = 0; i < owners.size(); ++i) {
        const GenObject& o = Obj(m_, owners[i]);
        if (i > 0) ApplyEffects(o);
        for (const GenSlotUpgrade& u : o.slotUpgrades) {
            grades[u.slot] += u.grade;
            if (u.activate) activated.insert(u.slot);
        }
    }
    // The builder writes the class default (the constant 1) when the type has no AttributeSlotBaseGrade; a
    // manifest without the field gets the same 1, as weapon_stats.slot_grade does for an unresolved grade.
    std::optional<double> slotBase = type_.slotBaseGrade >= 0 ? value_.Value(type_.slotBaseGrade) : std::nullopt;
    double baseGrade = std::floor(slotBase.value_or(1.0) + 0.5);
    for (const GenSlotEffect& e : type_.slotEffects) {
        if (!activated.count(e.slot) || e.external) continue;
        double grade = baseGrade + (grades.count(e.slot) ? grades[e.slot] : 0.0);
        if (e.enforceMax && grade > e.maxGrade) grade = e.maxGrade;
        if (e.enforceMin && grade < e.minGrade) grade = e.minGrade;
        double perGrade = value_.Value(e.perGradeInit).value_or(0.0);
        double base = value_.Value(e.baseInit).value_or(0.0);
        AddModifier(e.attribute, e.modifier, F32(F32((grade - 1) * perGrade) + base));
    }
}

std::optional<double> CardEvaluator::Stat(const char* name) const {
    auto it = final_.find(kAttr + name);
    if (it == final_.end()) return std::nullopt;
    return it->second;
}

std::vector<int> CardEvaluator::OrderedParts() const {
    std::vector<int> parts;
    for (int id : r_.parts)
        if (id >= 0) parts.push_back(id);
    return parts;
}

// weapon_stats._presentation_sources: balance, type, parts, prefix, title (each once).
std::vector<int> CardEvaluator::PresentationSources() const {
    std::vector<int> sources;
    auto add = [&](int id) {
        if (id >= 0 && std::find(sources.begin(), sources.end(), id) == sources.end()) sources.push_back(id);
    };
    add(r_.balance->object);
    add(r_.balance->type);
    for (int part : OrderedParts()) add(part);
    add(r_.prefix);
    add(r_.title);
    return sources;
}

// weapon_stats.sale_value: the type's price calculator fed the part total (the type's
// BaseMonetaryValueModifier times every part's and name part's MonetaryValueMod, single precision) and
// the item level, truncated and capped by the type's currency.
void CardEvaluator::SaleValue(WeaponGenCard& card) const {
    if (type_.monetaryValue < 0) return;
    std::vector<int> owners = OrderedParts();
    for (int id : {r_.prefix, r_.title})
        if (id >= 0) owners.push_back(id);
    double total = F32(type_.monetaryBase);
    for (int id : owners) {
        int modifier = Obj(m_, id).monetaryMod;
        if (modifier < 0) continue;
        const auto& factor = m_.attributes[static_cast<size_t>(modifier)].constant;
        if (!factor) return;
        total = F32(total * *factor);
    }
    Known known = {{kPartPriceTotal, total}, {kWeaponLevel, static_cast<double>(level_)}};
    auto value = PriceInit(m_, type_.monetaryValue, known);
    if (!value) return;
    card.saleValue = std::min(static_cast<long long>(std::trunc(*value)), type_.saleCap);
    card.saleValueKnown = type_.priceChecked;
}

// weapon_stats.rarity_of: trunc(type BaseRarity) + sum of trunc(part Rarity); the tier is the first
// RarityLevelColors row whose [MinLevel, MaxLevel] holds it.
void CardEvaluator::Rarity(WeaponGenCard& card) {
    long long level = 0;
    double base = type_.baseRarity >= 0 ? value_.Value(type_.baseRarity).value_or(0.0) : 0.0;
    level += static_cast<long long>(std::trunc(base));
    for (int part : OrderedParts()) {
        int rarity = Obj(m_, part).rarity;
        double amount = rarity >= 0 ? value_.Value(rarity).value_or(0.0) : 0.0;
        level += static_cast<long long>(std::trunc(amount));
    }
    card.rarityLevel = static_cast<int>(level);
    for (const GenRarityRow& row : m_.rarity)
        if (row.minLevel <= level && level <= row.maxLevel) {
            card.rarityRating = row.rating;
            card.rarityColor = row.color;
            break;
        }
    static const std::map<std::string, int> numbers = {{"RARITY_Common", 1}, {"RARITY_Uncommon", 2}, {"RARITY_Rare", 3},
                                                       {"RARITY_VeryRare", 4}, {"RARITY_Legendary", 5}};
    if (card.rarityRating && numbers.count(*card.rarityRating)) card.rarity = numbers.at(*card.rarityRating);
    else if (level) card.rarity = static_cast<int>(std::max<long long>(1, std::min<long long>(5, level)));
}

// weapon_stats.damage_type_of: the card's rule (not the name rule's NativeDamageType).
int CardEvaluator::DamageType() const { return WeaponDamageType(m_, r_.balance->type, r_.parts); }

// weapon_stats.status_rows: per-surface chance = BaseChance * the two chance modifiers; the card's chance
// is the remap below; damage per second only for damage-over-time effects.
void CardEvaluator::Status(WeaponGenCard& card, int damageType) {
    if (damageType < 0 || Obj(m_, damageType).statusEffect < 0) return;
    int effectId = Obj(m_, damageType).statusEffect;
    const GenObject& effect = Obj(m_, effectId);
    auto modifier = [&](const char* name) { return final_.count(kAttr + name) ? final_.at(kAttr + name) : 1.0; };
    double factor = modifier("WeaponBaseStatusEffectChanceModifier") * modifier("WeaponStatusEffectChanceModifier");
    for (const GenSurface& row : effect.surfaces) {
        auto chance = value_.Value(row.init);
        if (!chance) continue;
        auto it = std::find_if(card.statusChanceBySurface.begin(), card.statusChanceBySurface.end(),
                               [&](const auto& p) { return p.first == row.surface; });
        if (it != card.statusChanceBySurface.end()) it->second = *chance * factor;
        else card.statusChanceBySurface.push_back({row.surface, *chance * factor});
    }
    card.hasStatus = true;
    card.statusEffect = effect.statusEffectType.empty() ? std::nullopt : std::optional<std::string>(effect.statusEffectType);
    card.statusEffectDefinition = effect.path;
    // The card row: the mapped presentation's remap of the combined modifier; unmapped damage types keep
    // the Generic-surface chance.
    auto remap = m_.statusRemap.find(Obj(m_, damageType).path);
    if (remap != m_.statusRemap.end() && remap->second.size() == 4)
        card.statusChance = Remap(remap->second.data(), F32(factor));
    else
        for (const auto& [surface, chance] : card.statusChanceBySurface)
            if (surface == "generic") card.statusChance = chance;
    if (effect.damageOverTime) card.statusDps = Stat("WeaponStatusEffectDamage");
    card.statusDuration = effect.baseDuration >= 0 ? value_.Value(effect.baseDuration) : std::nullopt;
}

// weapon_stats.zoom_multiplier: 80 / the type's ZoomedEndFOV after its WeaponZoomFOV modifiers, when a
// source asks the card to show the zoom.
std::optional<double> CardEvaluator::Zoom() const {
    bool shown = false;
    for (int id : PresentationSources()) shown = shown || Obj(m_, id).cardZoom;
    if (!shown || !type_.zoomedEndFov || *type_.zoomedEndFov <= 0) return std::nullopt;
    auto mods = mods_.find(kAttr + "WeaponZoomEndFOV");
    double fov = Combine(F32(*type_.zoomedEndFov), mods == mods_.end() ? Modifiers{} : mods->second, false);
    if (fov <= 0) return std::nullopt;
    return F32(80.0 / fov);
}

// weapon_stats.display: the numbers as the card prints them.
void CardEvaluator::Display(WeaponGenCard& card) const {
    const std::map<std::string, std::optional<double>> values = {
        {"damage", card.damage}, {"magazine", card.magazine}, {"fire_rate", card.fireRate},
        {"reload_time", card.reloadTime}, {"accuracy", card.accuracy}, {"weapon_zoom", card.weaponZoom}};
    for (const auto& [field, fallback] : PresentationFields()) {
        const auto& value = values.at(field);
        if (!value) continue;
        GenRounding rounding = RoundingFor(m_, field);
        card.display.push_back(Present(field, *value, rounding.mode, rounding.precision));
    }
    if (card.statusChance) card.display.push_back(Present("status_chance", *card.statusChance, "ATTRROUNDING_Float", 1));
    if (card.statusDps) card.display.push_back(Present("status_dps", *card.statusDps, "ATTRROUNDING_Float", 1));
    if (card.projectiles && *card.projectiles > 1)
        card.display.push_back({"projectiles", static_cast<double>(*card.projectiles), true});
}

WeaponGenCard CardEvaluator::Run() {
    // Effect order of script WillowWeapon.InitializeInternal: the type, the parts in slot order, the
    // attribute slots, then the prefix and title name parts.
    Bases();
    ApplyEffects(type_);
    SlotEffects();
    for (int id : {r_.prefix, r_.title})
        if (id >= 0) ApplyEffects(Obj(m_, id));
    for (const auto& [attribute, start] : base_) {
        if (!start) continue;
        auto mods = mods_.find(attribute);
        final_[attribute] = Combine(*start, mods == mods_.end() ? Modifiers{} : mods->second,
                                    IntAttributes().count(attribute) > 0);
    }

    WeaponGenCard card;
    card.name = r_.name;
    card.level = level_;
    // weapon_stats.level_requirement: 0 for mission balances and types without the flag, else the level.
    card.levelRequirement = (r_.balance->mission || !type_.usesLevelRequirement) ? 0 : std::max(level_, 1);
    card.levelLine = card.levelRequirement > 1;
    card.damage = Stat("WeaponDamage");
    auto interval = Stat("WeaponFireInterval");
    if (interval && *interval != 0) card.fireRate = F32(1.0 / *interval);  // the card shows 1 / FireInterval
    card.reloadTime = Stat("WeaponReloadSpeed");
    card.magazine = Stat("WeaponClipSize");
    card.spread = Stat("WeaponSpread");
    card.shotCost = Stat("WeaponShotCost");
    card.spinUp = Stat("WeaponBarrelSpinUpDuration");
    int barrel = r_.parts[2];
    bool spin = barrel >= 0 && Obj(m_, barrel).spinning;
    if (spin && !type_.spinMode.empty()) card.spinMode = type_.spinMode;
    if (spin) card.spinStartIntervalScale = Obj(m_, barrel).spinStartScale;
    SaleValue(card);
    Rarity(card);
    int damageType = DamageType();
    // weapon_stats.card_element: the damage type's element, else the elemental part's name suffix.
    static const std::map<std::string, std::string> elements = {{"DAMAGE_TYPE_Explosive", "Explosive"},
                                                                {"DAMAGE_TYPE_Incindiary", "Fire"},
                                                                {"DAMAGE_TYPE_Shock", "Shock"},
                                                                {"DAMAGE_TYPE_Corrosive", "Corrosive"}};
    if (damageType >= 0 && elements.count(Obj(m_, damageType).damageTypeEnum))
        card.element = elements.at(Obj(m_, damageType).damageTypeEnum);
    else if (r_.parts[5] >= 0)
        card.element = After(Obj(m_, r_.parts[5]).path, "_");
    Status(card, damageType);
    auto projectiles = Stat("WeaponProjectilesPerShot");
    if (projectiles) card.projectiles = static_cast<int>(*projectiles);
    card.manufacturer = maker_.empty() ? std::nullopt : std::optional<std::string>(maker_);
    // weapon_stats.accuracy_percent: the spread remapped linearly onto the card's percent, input clamped.
    if (card.spread && m_.hasAccuracy) card.accuracy = Remap(m_.accuracy, *card.spread);
    card.weaponZoom = Zoom();
    std::optional<double> zoomShown;
    if (card.weaponZoom) {
        GenRounding rounding = RoundingFor(m_, "weapon_zoom");
        zoomShown = Present("weapon_zoom", *card.weaponZoom, rounding.mode, rounding.precision).value;
    }
    std::vector<int> sources = PresentationSources();
    if (damageType >= 0 && std::find(sources.begin(), sources.end(), damageType) == sources.end())
        sources.push_back(damageType);
    std::vector<std::string> texts, markups;
    for (const CardLine& line : CardTextLines(m_, sources, card.shotCost, zoomShown)) {
        texts.push_back(line.text);
        markups.push_back(line.markup);
    }
    if (!texts.empty()) card.funStats = Join(texts, "; "), card.funStatsMarkup = Join(markups, "\n");
    // A barrel's CustomFiringModeDefinition replaces the type's default.
    int firingMode = barrel >= 0 && Obj(m_, barrel).customFiringMode >= 0 ? Obj(m_, barrel).customFiringMode
                                                                          : type_.defaultFiringMode;
    if (firingMode >= 0) {
        card.firingMode = Obj(m_, firingMode).path;
        auto speedScale = Stat("WeaponProjectileSpeedMultiplier");
        const auto& speed = Obj(m_, firingMode).speed;
        if (speed && *speed != 0 && speedScale) card.projectileSpeed = *speed * *speedScale;
    }
    if (damageType >= 0) card.damageType = Obj(m_, damageType).path;
    Display(card);
    return card;
}

// ===================================================================================
// Pools (loot_pools.expand / roll_pool_crt / roll_crt)
// ===================================================================================

// loot_pools.pool_gate: true / false / unknown for the pool's game-stage requirements.
std::optional<bool> PoolGate(const GenObject& pool, int stage) {
    if (pool.gateMin.present) {
        if (!pool.gateMin.resolved) return std::nullopt;
        if (!(stage >= pool.gateMin.value)) return false;
    }
    if (pool.gateMax.present) {
        if (!pool.gateMax.resolved) return std::nullopt;
        if (!(stage <= pool.gateMax.value)) return false;
    }
    return true;
}

struct PoolWalk {
    const WeaponGenManifest& m;
    int stage;
    CrtRand& rng;
    std::vector<std::string> drops;

    // loot_pools.expand's cycle guard: a pool already on the path, or deeper than 12, expands to nothing.
    static bool Cycle(const std::vector<int>& seen, int pool, int depth) {
        return depth > 12 || std::find(seen.begin(), seen.end(), pool) != seen.end();
    }

    // One firing of a pool: round-half-even(Quantity) draws (missing or 0 counts 1); each takes
    // r = total * rand() / 32767 over the eligible shares and the first interval holding r.
    void Roll(int poolId, int depth, std::vector<int>& seen) {
        if (Cycle(seen, poolId, depth)) return;
        const GenObject& pool = Obj(m, poolId);
        // Shares among eligible entries: a sub-pool whose gate is false is not eligible.
        std::vector<std::pair<const GenPoolEntry*, double>> weights;
        double total = 0.0;
        seen.push_back(poolId);
        for (const GenPoolEntry& e : pool.entries) {
            bool eligible = true;
            if (e.isPool && !Cycle(seen, e.object, depth + 1)) eligible = PoolGate(Obj(m, e.object), stage) != false;
            bool counted = e.weight && *e.weight != 0 && eligible;
            weights.push_back({&e, counted ? *e.weight : 0.0});
            if (counted) total += *e.weight;
        }
        std::vector<std::pair<const GenPoolEntry*, double>> eligible;
        for (const auto& [entry, weight] : weights) {
            double share = total > 0 && weight != 0 ? weight / total : 0.0;
            if (share > 0) eligible.push_back({entry, share});
        }
        double quantity = pool.quantity && *pool.quantity != 0 ? *pool.quantity : 1.0;
        long long draws = std::max(0LL, static_cast<long long>(std::nearbyint(quantity)));
        for (long long i = 0; i < draws && !eligible.empty(); ++i) {
            const GenPoolEntry* chosen = Choose(eligible);
            if (chosen->isPool) Roll(chosen->object, depth + 1, seen);
            else drops.push_back(Obj(m, chosen->object).path);
        }
        seen.pop_back();
    }

    const GenPoolEntry* Choose(const std::vector<std::pair<const GenPoolEntry*, double>>& eligible) {
        double total = 0.0;
        for (const auto& e : eligible) total += e.second;
        double r = total * static_cast<double>(rng.Rand()) / 32767.0;
        double running = 0.0;
        for (const auto& e : eligible) {
            if (running <= r && r <= running + e.second) return e.first;
            running += e.second;
        }
        return eligible.back().first;
    }
};

// ===================================================================================
// JSON output
// ===================================================================================

class JsonObject {
public:
    void Raw(const std::string& key, const std::string& value) {
        out_ += (out_.empty() ? "{" : ",") + JsonString(key) + ":" + value;
    }
    void Str(const std::string& key, const std::optional<std::string>& value) {
        Raw(key, value ? JsonString(*value) : "null");
    }
    void Num(const std::string& key, const std::optional<double>& value) { Raw(key, value ? JsonNumber(*value) : "null"); }
    void Int(const std::string& key, const std::optional<long long>& value) {
        Raw(key, value ? std::to_string(*value) : "null");
    }
    void Bool(const std::string& key, bool value) { Raw(key, value ? "true" : "false"); }
    std::string Done() const { return out_.empty() ? "{}" : out_ + "}"; }

private:
    std::string out_;
};

std::optional<std::string> NonEmpty(const std::string& text) {
    return text.empty() ? std::nullopt : std::optional<std::string>(text);
}

std::optional<long long> OptionalInt(const std::optional<int>& value) {
    return value ? std::optional<long long>(*value) : std::nullopt;
}

std::string CardJson(const WeaponGenCard& c) {
    JsonObject o;
    o.Str("name", c.name);
    o.Int("level", c.level);
    o.Int("level_requirement", c.levelRequirement);
    o.Bool("level_line", c.levelLine);
    o.Num("damage", c.damage);
    o.Num("fire_rate", c.fireRate);
    o.Num("reload_time", c.reloadTime);
    o.Num("magazine", c.magazine);
    o.Num("spread", c.spread);
    o.Num("shot_cost", c.shotCost);
    o.Num("spin_up", c.spinUp);
    o.Str("spin_mode", c.spinMode);
    o.Num("spin_start_interval_scale", c.spinStartIntervalScale);
    o.Str("element", c.element);
    o.Int("rarity", OptionalInt(c.rarity));
    o.Int("rarity_level", c.rarityLevel);
    o.Str("rarity_rating", c.rarityRating);
    o.Str("rarity_color", c.rarityColor);
    o.Str("manufacturer", c.manufacturer);
    o.Num("accuracy", c.accuracy);
    o.Bool("accuracy_known", c.accuracyKnown);
    o.Int("sale_value", c.saleValue);
    o.Bool("sale_value_known", c.saleValueKnown);
    o.Num("weapon_zoom", c.weaponZoom);
    o.Str("fun_stats", c.funStats);
    o.Str("fun_stats_markup", c.funStatsMarkup);
    o.Int("projectiles", OptionalInt(c.projectiles));
    o.Str("firing_mode", c.firingMode);
    o.Num("projectile_speed", c.projectileSpeed);
    o.Str("damage_type", c.damageType);
    if (c.hasStatus) {
        o.Str("status_effect", c.statusEffect);
        o.Str("status_effect_definition", c.statusEffectDefinition);
        o.Num("status_chance", c.statusChance);
        JsonObject surfaces;
        for (const auto& [surface, chance] : c.statusChanceBySurface) surfaces.Num(surface, chance);
        o.Raw("status_chance_by_surface", surfaces.Done());
        o.Num("status_dps", c.statusDps);
        o.Num("status_duration", c.statusDuration);
    }
    JsonObject display;
    for (const WeaponGenNumber& n : c.display) {
        if (n.integer) display.Int(n.field, static_cast<long long>(n.value));
        else display.Num(n.field, n.value);
    }
    o.Raw("display", display.Done());
    return o.Done();
}

std::string NamePartJson(const std::optional<WeaponGenNamePart>& n) {
    if (!n) return "null";
    JsonObject o;
    o.Str("part", n->part);
    o.Str("text", NonEmpty(n->text));
    o.Num("priority", n->priority);
    return o.Done();
}

}  // namespace

// ===================================================================================
// Public API
// ===================================================================================

WeaponGenManifest ParseWeaponGenManifest(const std::string& text, std::string* error) {
    WeaponGenManifest m;
    ParseState state;
    state.idLimit = static_cast<size_t>(std::count(text.begin(), text.end(), '\n')) + 1;
    std::istringstream stream(text);
    std::string line;
    size_t number = 0;
    try {
        while (std::getline(stream, line)) {
            ++number;
            if (!line.empty() && line.back() == '\r') line.pop_back();
            if (line.empty() || line[0] == '#') continue;
            ParseLine(m, state, line);
        }
    } catch (const std::exception& e) {
        if (error) *error = "manifest line " + std::to_string(number) + ": " + e.what();
        return WeaponGenManifest{};
    }
    try {
        Validate(m, state);
    } catch (const std::exception& e) {
        if (error) *error = std::string("manifest: ") + e.what();
        return WeaponGenManifest{};
    }
    if (error) error->clear();
    return m;
}

WeaponGenManifest LoadWeaponGenManifest(const std::string& path, std::string* error) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        if (error) *error = "cannot open " + path;
        return WeaponGenManifest{};
    }
    std::ostringstream text;
    text << file.rdbuf();
    return ParseWeaponGenManifest(text.str(), error);
}

WeaponGenResult RollWeapon(const WeaponGenManifest& m, const std::string& balance, int stage, uint32_t seed) {
    return RollWeapon(m, balance, stage, seed, std::nullopt);
}

WeaponGenResult RollWeapon(const WeaponGenManifest& m, const std::string& balance, int stage, CrtRand& rng) {
    return RollWeapon(m, balance, stage, rng, std::nullopt);
}

WeaponGenResult RollWeapon(const WeaponGenManifest& m, const std::string& balance, int stage, uint32_t seed,
                           std::optional<int> stageCap) {
    CrtRand rng(seed);
    WeaponGenResult result = RollWeapon(m, balance, stage, rng, stageCap);
    result.seed = seed;
    return result;
}

// weapon_recipe.roll: cap the stage, pick a part per merged slot, choose the name parts, then evaluate
// the card.
WeaponGenResult RollWeapon(const WeaponGenManifest& m, const std::string& balance, int stage, CrtRand& rng,
                           std::optional<int> stageCap) {
    WeaponGenResult result;
    result.balance = balance;
    result.spawnStage = stage;
    result.stageCap = stageCap;
    if (stageCap) stage = std::min(stage, *stageCap);
    result.stage = stage;
    result.seed = rng.state;
    auto object = m.objectByPath.find(balance);
    auto found = object == m.objectByPath.end() ? m.balanceByObject.end()
                                                : m.balanceByObject.find(std::to_string(object->second));
    if (found == m.balanceByObject.end()) {
        result.error = "unknown weapon balance " + balance;
        return result;
    }
    const GenBalance& b = m.balances[static_cast<size_t>(found->second)];
    Recipe recipe;
    recipe.balance = &b;
    for (int slot = 0; slot < kSlotCount; ++slot) {
        std::vector<Candidate> entries = SlotCandidates(m, b.slots[slot], stage);
        if (entries.empty()) continue;
        bool unresolved = std::any_of(entries.begin(), entries.end(), [](const Candidate& c) { return !c.weight; });
        if (unresolved) {
            result.notes.push_back(std::string(kSlots[slot]) + ": some weights need runtime attributes; treated as 1");
            for (Candidate& c : entries)
                if (!c.weight) c.weight = 1.0;
        }
        Pick pick = PickPart(entries, rng);
        if (pick.part < 0) {
            result.notes.push_back(std::string(kSlots[slot]) + ": every candidate weighs 0; the game leaves the slot empty");
            continue;
        }
        recipe.parts[slot] = pick.part;
        result.parts.push_back({kSlots[slot], Obj(m, pick.part).path, pick.weight, pick.candidates});
    }
    // Item level = the game stage it was spawned at.
    recipe.prefix = ChooseNamePart(m, true, b.type, recipe.parts, stage, b.manufacturer);
    recipe.title = ChooseNamePart(m, false, b.type, recipe.parts, stage, b.manufacturer);
    std::vector<std::string> words;
    for (int id : {recipe.prefix, recipe.title}) {
        if (id < 0) continue;
        WeaponGenNamePart part{Obj(m, id).path, Obj(m, id).nameText, Obj(m, id).priority};
        (id == recipe.prefix ? result.prefix : result.title) = part;
        if (!part.text.empty()) words.push_back(part.text);
    }
    recipe.name = Join(words, " ");
    result.name = recipe.name;
    std::set<std::string> fragments;
    for (int id : recipe.parts)
        if (id >= 0) fragments.insert(Obj(m, id).fragments.begin(), Obj(m, id).fragments.end());
    result.fragments.assign(fragments.begin(), fragments.end());
    if (recipe.parts[8] >= 0) result.material = Obj(m, recipe.parts[8]).material;
    result.manufacturer = b.manufacturer;
    result.weaponType = Obj(m, b.type).path;
    result.gestalt = Obj(m, b.type).gestalt;
    result.card = CardEvaluator(m, recipe, stage).Run();
    result.ok = true;
    return result;
}

std::vector<std::string> RollPool(const WeaponGenManifest& m, const std::string& pool, int stage, CrtRand& rng) {
    PoolWalk walk{m, stage, rng, {}};
    auto found = m.objectByPath.find(pool);
    if (found == m.objectByPath.end()) return {};
    std::vector<int> seen;
    walk.Roll(found->second, 0, seen);
    return walk.drops;
}

// loot_pools.roll_crt: each listed pool draws one rand() (none when its probability is unresolved or its
// gate is false) and fires when the probability is >= 1 or rand() / 32767 is below it.
std::vector<std::string> RollSource(const WeaponGenManifest& m, const std::string& source, int stage, CrtRand& rng) {
    auto found = m.objectByPath.find(source);
    if (found == m.objectByPath.end()) return {};
    std::vector<std::string> drops;
    for (const GenListed& listed : Obj(m, found->second).listed) {
        if (listed.pool < 0 || !listed.probability || PoolGate(Obj(m, listed.pool), stage) == false) continue;
        double draw = static_cast<double>(rng.Rand()) / 32767.0;
        if (*listed.probability >= 1.0 || draw < *listed.probability) {
            std::vector<std::string> more = RollPool(m, Obj(m, listed.pool).path, stage, rng);
            drops.insert(drops.end(), more.begin(), more.end());
        }
    }
    return drops;
}

bool IsPoolSource(const WeaponGenManifest& m, const std::string& path) {
    auto found = m.objectByPath.find(path);
    return found != m.objectByPath.end() && Obj(m, found->second).isSource;
}

std::string JsonNumber(double value) {
    if (std::isnan(value)) return "NaN";
    if (std::isinf(value)) return value > 0 ? "Infinity" : "-Infinity";
    char buffer[64];
    auto result = std::to_chars(buffer, buffer + sizeof buffer, value);  // shortest round-trip form
    std::string text(buffer, result.ptr);
    if (text.find_first_of(".eE") == std::string::npos) text += ".0";
    return text;
}

std::string JsonString(const std::string& value) {
    std::string out = "\"";
    for (unsigned char c : value) {
        if (c == '"') out += "\\\"";
        else if (c == '\\') out += "\\\\";
        else if (c == '\n') out += "\\n";
        else if (c == '\t') out += "\\t";
        else if (c < 0x20) {
            char buffer[8];
            std::snprintf(buffer, sizeof buffer, "\\u%04x", c);
            out += buffer;
        } else {
            out += static_cast<char>(c);
        }
    }
    return out + "\"";
}

std::string WeaponResultJson(const WeaponGenResult& r) {
    JsonObject o;
    o.Str("balance", r.balance);
    o.Int("stage", r.stage);
    if (r.stageCap) {
        o.Int("spawn_stage", r.spawnStage);
        o.Int("stage_cap", *r.stageCap);
    }
    o.Int("seed", static_cast<long long>(r.seed));
    if (!r.ok) {
        o.Str("error", r.error);
        return o.Done();
    }
    o.Str("manufacturer", NonEmpty(r.manufacturer));
    o.Str("weapon_type", r.weaponType);
    o.Str("gestalt", NonEmpty(r.gestalt));
    o.Str("name", r.name);
    o.Raw("prefix", NamePartJson(r.prefix));
    o.Raw("title", NamePartJson(r.title));
    JsonObject parts;
    for (const WeaponGenPart& p : r.parts) {
        JsonObject part;
        part.Str("part", p.part);
        part.Num("weight", p.weight);
        part.Int("candidates", p.candidates);
        parts.Raw(p.slot, part.Done());
    }
    o.Raw("parts", parts.Done());
    std::string fragments = "[";
    for (size_t i = 0; i < r.fragments.size(); ++i) fragments += (i ? "," : "") + JsonString(r.fragments[i]);
    o.Raw("gestalt_fragments", fragments + "]");
    o.Str("material", NonEmpty(r.material));
    std::string notes = "[";
    for (size_t i = 0; i < r.notes.size(); ++i) notes += (i ? "," : "") + JsonString(r.notes[i]);
    o.Raw("notes", notes + "]");
    o.Raw("card", CardJson(r.card));
    return o.Done();
}

}  // namespace weapongen
