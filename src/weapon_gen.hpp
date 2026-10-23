#pragma once

// Weapon generator: the C++ port of tools/weapon_recipe.roll() (with CrtRand) and
// tools/weapon_stats.evaluate(). It reads the line-based manifest written by
// tools/weapon_gen_manifest.py (format in that tool's docstring) and reproduces the
// Python results exactly, including single-precision rounding (tools/weapon_gen_parity.py
// checks this). Standard C++ only, so the UE module can compile it as is.
//
// The rules are the Python ones and keep their status: read from the game's code or
// fitted on item cards, UNVERIFIED in game unless docs/verification says otherwise.
// The random sequence is ours (MSVC rand() seeded with the caller's seed), not the game's.

#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

namespace weapongen {

constexpr int kSlotCount = 9;
// Part slots in the game's order (weapon_recipe.SLOTS).
extern const char* const kSlots[kSlotCount];

// The MSVC C runtime rand(): state = state * 214013 + 2531011 (mod 2^32), value = bits 16..30.
// Seeding sets the state (srand). Same sequence as weapon_recipe.CrtRand.
struct CrtRand {
    uint32_t state = 0;
    explicit CrtRand(uint32_t seed) : state(seed) {}
    int Rand();  // 0 .. 32767
};

// ---- manifest records (one struct per record kind; ids index the vectors) ----

struct GenAttribute {
    std::string path;
    std::optional<double> constant;  // its ConstantAttributeValueResolver's value, if any
};

// One AttributeInitializationData.
struct GenInit {
    double constant = 0;   // BaseValueConstant
    int attribute = -1;    // BaseValueAttribute (attribute id) or -1
    int definition = -1;   // InitializationDefinition (definition id) or -1
    std::optional<double> scale;  // BaseValueScaleConstant; absent means 1
};

struct GenTerm {
    std::string key;  // Multiplier, Level, Power, Offset, MinValue, MaxValue
    int init = 0;
};

// One AttributeInitializationDefinition.
struct GenDefinition {
    std::string path, mode, rounding;  // empty mode = SetsBaseValue, empty rounding = Float
    bool conditional = false, formulaEnabled = false, minEnabled = false, maxEnabled = false;
    std::vector<GenTerm> formula, clamp;  // in the order the package lists them
};

struct GenEffect {
    std::string attribute, modifier;  // AttributeToModify, ModifierType (MT_PreAdd / MT_Scale / MT_PostAdd)
    int init = 0;                     // BaseModifierValue
};

struct GenSlotUpgrade {
    std::string slot;
    double grade = 0;
    bool activate = false;
};

struct GenSlotEffect {
    std::string slot, attribute, modifier;
    int baseInit = 0, perGradeInit = 0;
    bool external = false, enforceMin = false, enforceMax = false;
    double minGrade = 0, maxGrade = 0;
};

// A weapon type base field: an initializer (init >= 0) or a plain float.
struct GenTypeBase {
    std::string field;
    int init = -1;
    double number = 0;
};

struct GenPoolEntry {
    bool isPool = false;
    int object = -1;
    std::optional<double> weight;
};

struct GenPoolBound {
    bool present = false, resolved = false;
    double value = 0;
};

struct GenSurface {
    std::string surface;  // lower case, without DMGSURFACE_
    int init = 0;         // BaseChance
};

struct GenListed {
    int pool = -1;
    std::optional<double> probability;
};

// One name-part expression (manifest version 3, NameExpr): the operand's value compared with the
// constant. maker: 1 when the weapon's manufacturer short name equals `maker`, else 0. damage: the
// value of the first entry naming the weapon's damage type object, else `notMatched`.
struct GenNameExpr {
    bool damage = false;
    std::string maker;
    std::vector<std::pair<std::string, double>> matched;
    double notMatched = 0;
    std::string op;  // EComparisonOperator name, e.g. OPERATOR_EqualTo
    double constant = 1;
};

// Every non-numeric object: balances, weapon types, parts, name parts, presentations,
// damage types, status effects, firing modes, pools and pool sources. Only the fields of
// its kind are set.
struct GenObject {
    std::string cls, path;
    // Gear: effects, slot upgrades, name lists, presentations, price and rarity.
    std::vector<GenEffect> effects;
    std::vector<GenSlotUpgrade> slotUpgrades;
    std::vector<int> prefixList, titleList, customPresentations, cardPresentations;
    bool cardZoom = false;   // WeaponCardAttributes lists WeaponZoomEndFOV
    int monetaryMod = -1;    // MonetaryValueMod attribute id
    int rarity = -1;         // Rarity init
    // Weapon type.
    std::vector<GenTypeBase> bases;
    std::vector<GenSlotEffect> slotEffects;
    int slotBaseGrade = -1, baseRarity = -1, monetaryValue = -1;
    // BaseMonetaryValueModifier (the part total's start) and the FormOfCurrency sale cap. The defaults
    // reproduce a version 1 manifest, which predates both (every player gun type has 1 and Credits).
    double monetaryBase = 1.0;
    long long saleCap = 99999999;
    bool priceChecked = false, usesLevelRequirement = false;
    std::string gestalt, spinMode;
    std::optional<double> zoomedEndFov;
    int defaultDamageType = -1, defaultFiringMode = -1;
    // Part.
    std::vector<std::string> fragments;
    std::string material;
    int customDamageType = -1, customFiringMode = -1;
    bool spinning = false;
    int spinUpDuration = -1;
    std::optional<double> spinStartScale;
    // Name part.
    bool isNamePart = false, nameNever = false;
    std::string nameText;
    double priority = 1, minLevel = 1, maxLevel = 100;
    std::vector<std::string> nameMakers;  // every Weapon_Is_<Maker> == 1 expression (manifest version 2)
    std::vector<GenNameExpr> nameExprs;   // every expression, all must hold (manifest version 3)
    // Text presentation.
    bool hasLine = false, lineRed = false, dontDisplayNumber = false;
    std::string lineText, lineColor, lineMarkup;
    // Damage type, status effect, firing mode.
    std::string damageTypeEnum, statusEffectType;
    int statusEffect = -1, baseDuration = -1;
    std::vector<GenSurface> surfaces;
    bool damageOverTime = false;
    std::optional<double> speed;
    // Pools and sources.
    bool isPool = false, isSource = false;
    std::optional<double> quantity;
    GenPoolBound gateMin, gateMax;
    std::vector<GenPoolEntry> entries;
    std::vector<GenListed> listed;
};

// One merged slot candidate of a balance.
struct GenSlotEntry {
    enum class Weight { Flat, None, Init };
    int part = -1;
    Weight kind = Weight::Flat;
    int weightInit = -1;
    int minInit = -1, maxInit = -1;
};

struct GenBalance {
    int object = -1, type = -1;
    std::string manufacturer;
    bool mission = false;
    std::vector<GenSlotEntry> slots[kSlotCount];
};

struct GenRarityRow {
    double minLevel = 0, maxLevel = 0;
    std::string rating, color;
};

struct GenRounding {
    std::string mode;
    double precision = 1;
};

struct WeaponGenManifest {
    std::vector<GenAttribute> attributes;
    std::vector<GenInit> inits;
    std::vector<GenDefinition> definitions;
    std::vector<GenObject> objects;
    std::vector<GenBalance> balances;
    std::unordered_map<std::string, int> attributeByPath, objectByPath, balanceByObject;
    std::vector<GenRarityRow> rarity;
    std::map<std::string, GenRounding> rounding;  // card field -> presentation rounding
    bool hasAccuracy = false;
    double accuracy[4] = {0, 0, 0, 0};  // input low, input high, output low, output high
    // The card's status chance remap per damage type path (same bound order as accuracy); a damage type
    // without one keeps the Generic-surface BaseChance rule (weapon_stats.status_rows).
    std::map<std::string, std::vector<double>> statusRemap;
    std::string shotCostPrefix = "Consumes", shotCostSuffix = "ammo per shot.";
};

WeaponGenManifest LoadWeaponGenManifest(const std::string& path, std::string* error);
// Same, from the manifest text itself (tests, UE packaging).
WeaponGenManifest ParseWeaponGenManifest(const std::string& text, std::string* error);

// ---- results (plain fields, so a host wrapper can copy them into its item struct) ----

struct WeaponGenPart {
    std::string slot, part;
    double weight = 0;
    int candidates = 0;
};

struct WeaponGenNamePart {
    std::string part, text;
    double priority = 0;
};

// A card number as the item card prints it; integer when the presentation rounds to an integer.
struct WeaponGenNumber {
    std::string field;
    double value = 0;
    bool integer = false;
};

struct WeaponGenCard {
    std::string name;
    int level = 0, levelRequirement = 0;
    bool levelLine = false;
    std::optional<double> damage, fireRate, reloadTime, magazine, spread, shotCost, spinUp, spinStartIntervalScale;
    std::optional<std::string> spinMode, element, rarityRating, rarityColor, manufacturer, firingMode, damageType;
    std::optional<int> rarity;  // host scale 1 Common .. 5 Legendary
    int rarityLevel = 0;
    std::optional<double> accuracy;
    bool accuracyKnown = true;
    std::optional<long long> saleValue;
    bool saleValueKnown = false;
    std::optional<double> weaponZoom;
    std::optional<std::string> funStats, funStatsMarkup;
    std::optional<int> projectiles;
    std::optional<double> projectileSpeed;
    // Status rows: present only when the damage type has a status effect.
    bool hasStatus = false;
    std::optional<std::string> statusEffect, statusEffectDefinition;
    std::optional<double> statusChance, statusDps, statusDuration;
    std::vector<std::pair<std::string, double>> statusChanceBySurface;
    std::vector<WeaponGenNumber> display;
};

struct WeaponGenResult {
    bool ok = false;
    std::string error;
    std::string balance, manufacturer, weaponType, gestalt, material, name;
    int stage = 0;        // the stage the gun was built at (item level): the spawn stage after any cap
    int spawnStage = 0;   // the requested stage
    std::optional<int> stageCap;
    uint32_t seed = 0;
    std::optional<WeaponGenNamePart> prefix, title;
    std::vector<WeaponGenPart> parts;      // slot order; empty slots are left out
    std::vector<std::string> fragments;    // sorted, unique
    std::vector<std::string> notes;
    WeaponGenCard card;
};

// Roll one gun from a balance at a game stage (item level = stage) with CrtRand(seed).
WeaponGenResult RollWeapon(const WeaponGenManifest& manifest, const std::string& balance, int stage, uint32_t seed);
// Same with a caller-owned generator (draws continue from its state).
WeaponGenResult RollWeapon(const WeaponGenManifest& manifest, const std::string& balance, int stage, CrtRand& rng);
// Same with a pool spawn's stage cap (weapon_recipe.roll stage_cap): the gun is built at min(stage, cap),
// item level and part pick alike; no cap when stageCap is empty. weapon_recipe.default_stage_cap gives
// the game's cap (50; UVHM: the level cap, at most 80); Overpower levels are not modelled.
WeaponGenResult RollWeapon(const WeaponGenManifest& manifest, const std::string& balance, int stage, uint32_t seed,
                           std::optional<int> stageCap);
WeaponGenResult RollWeapon(const WeaponGenManifest& manifest, const std::string& balance, int stage, CrtRand& rng,
                           std::optional<int> stageCap);

// One firing of an item pool (loot_pools.roll_pool_crt): item balance paths in drop order.
std::vector<std::string> RollPool(const WeaponGenManifest& manifest, const std::string& pool, int stage, CrtRand& rng);
// A source (ItemPoolListDefinition / AIPawnBalanceDefinition) roll (loot_pools.roll_crt).
std::vector<std::string> RollSource(const WeaponGenManifest& manifest, const std::string& source, int stage, CrtRand& rng);
bool IsPoolSource(const WeaponGenManifest& manifest, const std::string& path);

// One result as a JSON object on one line (the ow-weapongen output format).
std::string WeaponResultJson(const WeaponGenResult& result);
std::string JsonNumber(double value);
std::string JsonString(const std::string& value);

}  // namespace weapongen
