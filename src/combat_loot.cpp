#include "combat.hpp"

#include <algorithm>
#include <cmath>

namespace vm {
namespace {
constexpr int LevelCap = 50;            // the global level cap on the first playthrough (NATIVE_LOOT.md, "Level cap"; the third-playthrough branch is not modelled)
constexpr float WeightFloor = 1e-8f;    // a candidate weighing this or less is not added
constexpr int MaxDepth = 16;            // nested pools and included lists (the game has no guard; the data has none this deep)
constexpr double AmmoDropsPerPlayerMultiplier = 1.0;    // BalanceModifierDefinition.GetAmmoDropsPerPlayerMultiplier below the third playthrough (NATIVE_LOOT.md; the later table is unread)

Value& required(Runtime& runtime, Object& object, const char* name) {
    Value* value = runtime.property(object, name);
    if (!value) throw RuntimeError(std::string("combat loot: ") + object.cls->name + " has no property " + name);
    return *value;
}
bool isObject(const Value* value) { return value && value->kind == Value::Kind::Object && value->o; }
std::string pathOf(const ObjectPtr& object) {
    return object && object->resourcePackage ? object->resourcePackage->path(object->resourceIndex) : "";
}
bool sameObject(const ObjectPtr& a, const ObjectPtr& b) {
    if (a == b) return true;
    const std::string path = pathOf(a);
    return !path.empty() && path == pathOf(b);
}
std::string leaf(const std::string& path) { return path.substr(path.rfind('.') + 1); }
}

// One weighted choice of a pool's roll: a nested pool, or one manufacturer of a balance (NATIVE_LOOT.md, "Candidate list").
struct CombatSystem::LootCandidate {
    ObjectPtr pool;                     // a nested pool, or null for a balance candidate
    ObjectPtr balance, manufacturer;    // the entry's balance as listed (the one the definition is asked of) and the candidate's manufacturer
    int word = 0, level = 0;            // the grade-or-level word and the stored level (the stage capped)
    float weight = 0;
    bool dropFlag = true;               // the entry's bDropOnDeath
};

// ---- the engine's random (a stand-in) ----

// The Microsoft C runtime's rand(): a linear congruential generator, 15 bits of the state after the step. The note says the game uses C rand() / 32767; its seed is unknown, so the
// state is seeded by the owner (seedRandom). UNVERIFIED as the game's generator.
int CombatSystem::rand15() {
    randomState_ = randomState_ * 214013u + 2531011u;
    return int((randomState_ >> 16) & 0x7fff);
}
float CombatSystem::frand() { return float(rand15()) / 32767.f; }

// ---- the local player (what the ammo and health drop weights look at) ----

// The first player controller the game's replication info lists (as the experience award walks it): its pawn.
ObjectPtr CombatSystem::localPlayerPawn() {
    const Value* replication = runtime_.property(*world_, "GRI");
    const Value* players = isObject(replication) ? runtime_.property(*replication->o, "PRIArray") : nullptr;
    if (!players) return nullptr;
    for (const Value& info : players->elements()) {
        if (!isObject(&info)) continue;
        const Value* owner = runtime_.property(*info.o, "Owner");
        if (!isObject(owner) || !isPlayerController(owner->o)) continue;
        const Value* pawn = runtime_.property(*owner->o, "Pawn");
        return isObject(pawn) ? pawn->o : nullptr;
    }
    return nullptr;
}

// The local player's pool for `resource`: only the health pool is modelled (the VM's player has no ammo pools, so an ammo weight is the resolver's no-pool weight).
ObjectPtr CombatSystem::localPlayerPool(const Value& resource) {
    const ObjectPtr pawn = localPlayerPawn();
    const Value* reference = pawn ? runtime_.property(*pawn, "HealthPool") : nullptr;
    if (!reference) return nullptr;
    Value copy = *reference;
    const ObjectPtr pool = resolve(copy);
    const Value* definition = pool ? runtime_.property(*pool, "Definition") : nullptr;
    const Value* held = isObject(definition) ? runtime_.property(*definition->o, "Resource") : nullptr;
    return isObject(held) && isObject(&resource) && sameObject(held->o, resource.o) ? pool : nullptr;
}

// ---- pool lists on the pawn (AIPawnBalanceDefinition.SetupPawnItemPoolList) ----

// ItemPoolListDefinition.AddToItemPoolList: the included lists first, depth first, then the list's own pools (NATIVE_LOOT.md; the game has no cycle guard, this has a depth limit).
void CombatSystem::appendPools(const ObjectPtr& list, std::vector<Value>& out, int depth) {
    if (depth > MaxDepth) throw RuntimeError("combat loot: item pool lists nest too deep");
    if (const Value* included = runtime_.property(*list, "ItemPoolIncludedLists"))
        for (const Value& entry : included->elements())
            if (isObject(&entry)) appendPools(load(entry), out, depth + 1);
    if (const Value* own = runtime_.property(*list, "ItemPools"))
        for (const Value& entry : own->elements()) out.push_back(entry);
}

// GetPlayThroughIndex: the first playthrough is entry 0, no further than the last entry; -1 when the balance has none (NATIVE_LOOT.md).
int CombatSystem::playThroughIndex(const ObjectPtr& balance) {
    const Value* entries = runtime_.property(*balance, "PlayThroughs");
    return entries && !entries->elements().empty() ? 0 : -1;
}

// SetupPawnItemPoolList (NATIVE_LOOT.md): the playthrough entry's custom lists replace the balance's default lists when they are not empty (each of the included lists and the pool list
// on its own); the pawn's ItemPoolList is the included lists' pools and then the chosen own pools; true when it is not empty. An index below 0 returns false and sets nothing.
bool CombatSystem::setupItemPoolList(const ObjectPtr& balance, const ObjectPtr& pawn) {
    const int index = playThroughIndex(balance);
    const Value* entries = runtime_.property(*balance, "PlayThroughs");
    if (index < 0 || !entries || size_t(index) >= entries->elements().size()) return false;
    const Value& playThrough = entries->elements()[size_t(index)];
    const auto chosen = [&](const char* custom, const char* standard) {
        const Value* custom_ = playThrough.field(custom);
        return custom_ && !custom_->elements().empty() ? custom_ : runtime_.property(*balance, standard);
    };
    std::vector<Value> pools;
    if (const Value* included = chosen("CustomItemPoolIncludedLists", "DefaultItemPoolIncludedLists"))
        for (const Value& entry : included->elements())
            if (isObject(&entry)) appendPools(load(entry), pools, 0);
    if (const Value* own = chosen("CustomItemPoolList", "DefaultItemPoolList"))
        for (const Value& entry : own->elements()) pools.push_back(entry);
    Value list = Value::makeArray();
    list.elements() = pools;
    required(runtime_, *pawn, "ItemPoolList") = list;
    return !pools.empty();
}

// The item game stage and the pool list a spawned pawn gets (PopulationFactoryBalancedAIPawn.SetupBalancedPopulationActor, read as script: the pawn's experience level when the
// balance's BaseItemGameStage is 0 (IGSS_ExpLevelOfBalancedActor), else its game stage, plus the truncated ItemGameStageOffset evaluated with the pawn as context).
void CombatSystem::setupLoot(const ObjectPtr& pawn, const ObjectPtr& balance) {
    if (!balance) return;
    const Value* base = runtime_.property(*balance, "BaseItemGameStage");
    int stage = int(required(runtime_, *pawn, base && base->integer() == 0 ? "ExpLevel" : "GameStage").integer());
    if (const Value* offset = runtime_.property(*balance, "ItemGameStageOffset")) {
        stage += int(std::trunc(evaluator_.evaluate(*offset, contextFor(Value::makeObject(pawn)))));
    }
    required(runtime_, *pawn, "GameStageForSpawnedInventory") = Value::makeInt(stage);
    Function* setup = runtime_.findMethod(balance->cls, "SetupPawnItemPoolList");           // the call the stock script makes, so the binding below is what runs
    if (!setup) throw RuntimeError("combat loot: the balance class has no SetupPawnItemPoolList");
    const bool listed = runtime_.call(*setup, balance, {Value::makeObject(pawn)}).truth();
    logEvent("SetupPawnItemPoolList " + leaf(pathOf(balance)) + ": " + std::to_string(required(runtime_, *pawn, "ItemPoolList").elements().size()) + " pools, item game stage "
             + std::to_string(stage) + (listed ? "" : " (no list)"));
}

// ---- inventory balances (NATIVE_LOOT.md, "Balance candidate expansion" and the two getters) ----

// GetInventoryDefinitionForManufacturerGrade: this balance's own entry for the manufacturer decides (an interpolating balance answers with its own definition, another by grade index, the
// grade's custom definition first); a definition without an inventory class is no answer; no answer asks the base. A balance already in the middle of the call yields nothing.
ObjectPtr CombatSystem::inventoryDefinitionFor(const ObjectPtr& balance, const ObjectPtr& manufacturer, int word, std::vector<const Object*>& active) {
    if (!balance || std::find(active.begin(), active.end(), balance.get()) != active.end()) return nullptr;
    active.push_back(balance.get());
    const Value* interpolate = runtime_.property(*balance, "bInterpolateExpLevel");
    const auto definitionOf = [&](const Value* reference) -> ObjectPtr {
        if (!isObject(reference)) return nullptr;
        const ObjectPtr definition = load(*reference);
        const Value* cls = runtime_.property(*definition, "InventoryClass");
        const bool has = cls && ((cls->kind == Value::Kind::Class && cls->cls) || isObject(cls));
        return has ? definition : nullptr;
    };
    ObjectPtr answer;
    if (const Value* manufacturers = runtime_.property(*balance, "Manufacturers"))
        for (const Value& entry : manufacturers->elements()) {
            const Value* listed = entry.field("Manufacturer");
            if (!isObject(listed) || !sameObject(load(*listed), manufacturer)) continue;
            const Value* grades = entry.field("Grades");
            if (interpolate && interpolate->truth()) {
                answer = definitionOf(runtime_.property(*balance, "InventoryDefinition"));
            } else if (grades && size_t(word & 0xffff) < grades->elements().size()) {
                const Value* modifiers = grades->elements()[size_t(word & 0xffff)].field("GradeModifiers");
                const Value* custom = modifiers ? modifiers->field("CustomInventoryDefinition") : nullptr;
                answer = definitionOf(isObject(custom) ? custom : runtime_.property(*balance, "InventoryDefinition"));
            }
            break;
        }
    if (!answer)
        if (const Value* base = runtime_.property(*balance, "BaseDefinition"); isObject(base)) answer = inventoryDefinitionFor(load(*base), manufacturer, word, active);
    active.pop_back();
    return answer;
}

// GetExpLevelFromManufacturerData: 1 by default; an interpolating balance answers with the word it was given (the stage); otherwise the grade's ExpLevel (or the high half of the word, at most
// 63) from this balance's own entry, else the base's answer.
int CombatSystem::expLevelFor(const ObjectPtr& balance, const ObjectPtr& manufacturer, int word, std::vector<const Object*>& active) {
    if (!balance || std::find(active.begin(), active.end(), balance.get()) != active.end()) return 1;
    const Value* interpolate = runtime_.property(*balance, "bInterpolateExpLevel");
    if (interpolate && interpolate->truth()) return word;
    active.push_back(balance.get());
    int level = 1;
    bool resolved = false;
    if (const Value* manufacturers = runtime_.property(*balance, "Manufacturers"))
        for (const Value& entry : manufacturers->elements()) {
            const Value* listed = entry.field("Manufacturer");
            if (!isObject(listed) || !sameObject(load(*listed), manufacturer)) continue;
            const Value* grades = entry.field("Grades");
            if (grades && size_t(word & 0xffff) < grades->elements().size()) {
                const Value* modifiers = grades->elements()[size_t(word & 0xffff)].field("GradeModifiers");
                const int high = (word >> 16) & 0xffff;
                level = high == 0 ? (modifiers && modifiers->field("ExpLevel") ? int(modifiers->field("ExpLevel")->integer()) : 1) : std::min(high, 63);
                resolved = true;
            }
            break;
        }
    if (!resolved)
        if (const Value* base = runtime_.property(*balance, "BaseDefinition"); isObject(base)) level = expLevelFor(load(*base), manufacturer, word, active);
    active.pop_back();
    return level;
}

// The candidates of a balance at a stage: the balance, then its bases, the walk stopping at the first that gives any; one per manufacturer entry whose first grade window holds
// min(stage, cap) and whose definition the original balance can name; the weight is the grade's modifier interpolated across the window (single precision).
std::vector<CombatSystem::LootCandidate> CombatSystem::expandBalance(const ObjectPtr& original, int stage, int cap, const AttributeContext& context) {
    std::vector<LootCandidate> result;
    const int s = std::min(stage, cap > 0 ? cap : LevelCap);
    const Value* interpolate = runtime_.property(*original, "bInterpolateExpLevel");
    std::vector<const Object*> visited;
    for (ObjectPtr current = original; current && std::find(visited.begin(), visited.end(), current.get()) == visited.end();) {
        visited.push_back(current.get());
        if (const Value* manufacturers = runtime_.property(*current, "Manufacturers"))
            for (const Value& entry : manufacturers->elements()) {
                const Value* listed = entry.field("Manufacturer");
                const Value* grades = entry.field("Grades");
                if (!isObject(listed) || !grades) continue;
                for (size_t g = 0; g < grades->elements().size(); ++g) {
                    const Value& grade = grades->elements()[g];
                    const Value* window = grade.field("GameStageRequirement");
                    const int low = window ? int(window->field("MinGameStage")->integer()) : 0, high = window ? int(window->field("MaxGameStage")->integer()) : 0;
                    if (s < low || s > high) continue;          // the first grade that holds the stage is the one
                    const ObjectPtr manufacturer = load(*listed);
                    const int word = interpolate && interpolate->truth() ? s : int(g);
                    std::vector<const Object*> active;
                    if (!inventoryDefinitionFor(original, manufacturer, word, active)) break;
                    const float minimum = evaluator_.evaluate(*grade.field("MinSpawnProbabilityModifier"), context);
                    const float maximum = evaluator_.evaluate(*grade.field("MaxSpawnProbabilityModifier"), context);
                    const int span = high - low;
                    LootCandidate candidate;
                    candidate.balance = original;
                    candidate.manufacturer = manufacturer;
                    candidate.word = word;
                    candidate.level = s;
                    candidate.weight = span > 0 ? minimum + float(s - low) / float(span) * (maximum - minimum) : minimum;
                    result.push_back(candidate);
                    break;
                }
            }
        if (!result.empty()) break;
        const Value* base = runtime_.property(*current, "BaseDefinition");
        current = isObject(base) ? load(*base) : nullptr;
    }
    return result;
}

// ---- the pool roll (ItemPool.SpawnBalancedInventoryFromPool) ----

// The pool gate: the stage window admits the stage (minimum 0, or at most the stage and the cap; maximum 0, or at least the stage), and some entry can be reached: a real nested
// pool that passes the same gate, or an entry with a balance.
bool CombatSystem::poolPasses(const ObjectPtr& pool, int stage, int cap, const AttributeContext& context, int depth) {
    if (depth > MaxDepth) return false;
    const auto bound = [&](const char* name) {
        const Value* attribute = runtime_.property(*pool, name);
        const auto value = attribute ? evaluator_.evaluateAttribute(*attribute, context) : std::nullopt;
        return value ? int(std::trunc(*value)) : 0;               // an unset attribute is 0
    };
    const int minimum = bound("MinGameStageRequirement"), maximum = bound("MaxGameStageRequirement");
    if (!((minimum == 0 || (minimum <= stage && minimum <= cap)) && (maximum == 0 || stage <= maximum))) return false;
    const Value* entries = runtime_.property(*pool, "BalancedItems");
    if (!entries) return false;
    for (const Value& entry : entries->elements()) {
        const Value* nested = entry.field("ItmPoolDefinition");
        if (isObject(nested)) {
            const ObjectPtr inner = load(*nested);
            const Value* innerEntries = runtime_.property(*inner, "BalancedItems");
            if (innerEntries && !innerEntries->elements().empty() && poolPasses(inner, stage, cap, context, depth + 1)) return true;
        }
        if (isObject(entry.field("InvBalanceDefinition"))) return true;
    }
    return false;
}

// SpawnBalancedInventoryFromPool, from NATIVE_LOOT.md (every rule UNVERIFIED unless COMBAT_LOOT.md names the live row): Quantity is evaluated with the context source and truncated; the
// pool must pass the gate (else false, no random drawn); the entries become one flat weighted list (nested pools with weight 0 unless they pass the gate; a balance entry one
// candidate per manufacturer with weight modifier x entry weight / count; a weight of 1e-8 or less is dropped); Quantity times, r = total x rand() / 32767 and the first candidate whose
// interval [sum, sum + weight] holds r is picked, with replacement. A picked nested pool is rolled again with the entry's flag ANDed into bInventoryMayDropOnDeath and its result ignored;
// a picked balance candidate becomes one item whose level is the capped stage and whose bDropOnDeath is the entry's flag AND the caller's. A pick that finds no candidate fails; the
// result is false when the gate fails or any pick failed. No variance formula is passed on a death roll, so the stage is the caller's.
bool CombatSystem::spawnFromPool(const ObjectPtr& pool, int stage, int awesome, const ObjectPtr& source, bool mayDrop, std::vector<LootItem>& out, int depth) {
    if (depth > MaxDepth) throw RuntimeError("combat loot: item pools nest too deep");
    const AttributeContext context = contextFor(Value::makeObject(source));
    const Value* entries = runtime_.property(*pool, "BalancedItems");
    const int quantity = int(std::trunc(evaluator_.evaluate(required(runtime_, *pool, "Quantity"), context)));
    if (!poolPasses(pool, stage, LevelCap, context, 0) || !entries) return false;

    std::vector<LootCandidate> candidates;
    for (const Value& entry : entries->elements()) {
        const Value* nested = entry.field("ItmPoolDefinition");
        const Value* balance = entry.field("InvBalanceDefinition");
        const Value* dropFlag = entry.field("bDropOnDeath");
        const float weight = evaluator_.evaluate(*entry.field("Probability"), context);
        ObjectPtr inner;
        if (isObject(nested)) {
            inner = load(*nested);
            const Value* innerEntries = runtime_.property(*inner, "BalancedItems");
            if (!innerEntries || innerEntries->elements().empty()) inner = nullptr;       // an empty pool counts as absent
        }
        if (inner) {
            LootCandidate candidate;
            candidate.pool = inner;
            candidate.weight = poolPasses(inner, stage, LevelCap, context, 0) ? weight : 0.f;
            candidate.dropFlag = dropFlag && dropFlag->truth();
            if (candidate.weight > WeightFloor) candidates.push_back(candidate);
        } else if (isObject(balance)) {
            const ObjectPtr loaded = load(*balance);
            std::vector<LootCandidate> expanded = expandBalance(loaded, stage, LevelCap, context);
            for (LootCandidate& candidate : expanded) {
                candidate.weight = candidate.weight * weight / float(expanded.size());
                candidate.dropFlag = dropFlag && dropFlag->truth();
                if (candidate.weight > WeightFloor) candidates.push_back(candidate);
            }
        }
    }

    float total = 0.f;
    for (const LootCandidate& candidate : candidates) total += candidate.weight;
    bool ok = true;
    for (int k = 0; k < quantity; ++k) {
        if (candidates.empty()) { ok = false; continue; }              // no candidate: the pick fails
        const float r = total * (float(rand15()) / 32767.f);
        const LootCandidate* picked = nullptr;
        float sum = 0.f;
        for (const LootCandidate& candidate : candidates) {
            if (r >= sum && r <= sum + candidate.weight) { picked = &candidate; break; }
            sum += candidate.weight;
        }
        if (!picked) { ok = false; continue; }
        if (picked->pool) {
            spawnFromPool(picked->pool, stage, awesome, source, picked->dropFlag && mayDrop, out, depth + 1);
            continue;
        }
        std::vector<const Object*> active;
        const ObjectPtr definition = inventoryDefinitionFor(picked->balance, picked->manufacturer, picked->word, active);
        const Value* inventoryClass = definition ? runtime_.property(*definition, "InventoryClass") : nullptr;
        if (!inventoryClass) { ok = false; continue; }
        LootItem item;
        item.pool = pathOf(pool);
        item.balance = pathOf(picked->balance);
        item.manufacturer = pathOf(picked->manufacturer);
        active.clear();
        item.level = std::min(expLevelFor(picked->balance, picked->manufacturer, picked->word, active), 63);
        item.dropOnDeath = picked->dropFlag && mayDrop;
        if (inventoryClass->kind == Value::Kind::Class) item.cls = inventoryClass->cls;
        else if (isObject(inventoryClass) && inventoryClass->o->resourcePackage) item.cls = runtime_.classAt(inventoryClass->o->resourcePackage, inventoryClass->o->resourceIndex);
        item.inventoryClass = item.cls ? item.cls->name : leaf(pathOf(inventoryClass->o));
        out.push_back(item);
    }
    return ok;
}

// ---- the death roll (WillowPawn.DropLootOnDeath, read from the installed script) ----

// For each pool of the pawn's list whose bAutoReadyItems is false (the ready ones are the carried gear, rolled at spawn): FRand is drawn first, then the pool's probability is evaluated with
// the pawn's controller as context (a probability definition named AmmoDrops_PerPlayer is multiplied once per death by GetAmmoDropsPerPlayerMultiplier, 1.0 on the first
// playthrough); the pool is tried when the chance is above 0 and the draw is at most the chance, with the pawn's item game stage and awesome level and the pawn as context. Of the items
// it returns, one with bDropOnDeath set is dropped unless the pawn is below the level's KillZ and the item is a weapon; the others are destroyed. Here "dropped" is a flag of the record: the
// VM has no pickups.
void CombatSystem::dropLoot(const ObjectPtr& pawn) {
    const Value* itemStage = runtime_.property(*pawn, "GameStageForSpawnedInventory");        // only an AI pawn has the stage and the list; any other pawn drops nothing
    const Value* awesome = runtime_.property(*pawn, "AwesomeLevel");
    if (!itemStage || !awesome) return;
    LootDrop drop;
    drop.pawn = pawn;
    drop.stage = int(itemStage->integer());
    drop.awesome = int(awesome->integer());
    const Value* location = runtime_.property(*pawn, "Location");
    const Value* z = location ? location->field("Z") : nullptr;
    const Value* killZ = runtime_.property(*world_, "KillZ");
    drop.belowKillZ = z && killZ && z->number() < killZ->number();
    Class* weapon = nullptr;
    try { weapon = runtime_.findClass("WillowGame.WillowWeapon"); } catch (const RuntimeError&) {}
    const Value* controller = runtime_.property(*pawn, "Controller");
    const Value* list = runtime_.property(*pawn, "ItemPoolList");
    bool scaled = false;
    size_t skipped = 0;
    if (list)
        for (const Value& entry : list->elements()) {
            const Value* reference = entry.field("ItemPool");
            if (!isObject(reference)) continue;
            const ObjectPtr pool = load(*reference);
            const Value* ready = runtime_.property(*pool, "bAutoReadyItems");
            if (ready && ready->truth()) { ++skipped; continue; }
            LootPool rolled;
            rolled.pool = pathOf(pool);
            rolled.roll = frand();
            try {
                const Value* probability = entry.field("PoolProbability");
                if (!probability || !controller) throw RuntimeError("the pool entry has no probability or the pawn no controller");
                rolled.chance = evaluator_.evaluate(*probability, contextFor(*controller));
                const Value* definition = probability->field("InitializationDefinition");
                if (isObject(definition) && leaf(pathOf(definition->o)) == "AmmoDrops_PerPlayer" && !scaled) {
                    rolled.chance *= AmmoDropsPerPlayerMultiplier;
                    scaled = true;
                }
                rolled.tried = rolled.chance > 0 && rolled.roll <= rolled.chance;
                if (rolled.tried) {
                    rolled.returned = spawnFromPool(pool, drop.stage, drop.awesome, pawn, true, rolled.items);
                    for (LootItem& item : rolled.items)
                        item.dropped = item.dropOnDeath && !(drop.belowKillZ && item.cls && weapon && item.cls->isChildOf(weapon));
                }
            } catch (const RuntimeError& error) {
                rolled.error = error.what();                 // a probability or pool the evaluator does not cover: reported, nothing dropped for this pool
                rolled.tried = false;
            }
            drop.pools.push_back(std::move(rolled));
        }
    size_t tried = 0, items = 0;
    for (const LootPool& rolled : drop.pools) {
        if (!rolled.error.empty()) { logEvent("loot " + leaf(rolled.pool) + ": not evaluated (" + rolled.error + ")"); continue; }
        if (!rolled.tried) continue;
        ++tried;
        std::string text = "loot " + leaf(rolled.pool) + " chance=" + std::to_string(rolled.chance) + " roll=" + std::to_string(rolled.roll) + " returned " + (rolled.returned ? "true" : "false");
        for (const LootItem& item : rolled.items) { text += " " + leaf(item.balance) + "@" + std::to_string(item.level) + (item.dropped ? "" : " (destroyed)"); items += item.dropped; }
        logEvent(text);
    }
    logEvent("DropLootOnDeath: item stage " + std::to_string(drop.stage) + ", " + std::to_string(drop.pools.size()) + " pools rolled (" + std::to_string(skipped) + " carried-gear pools skipped), "
             + std::to_string(tried) + " tried, " + std::to_string(items) + " items dropped (records only: the VM has no pickups)");
    drops_.push_back(std::move(drop));
}

// ---- natives ----

void CombatSystem::bindLootNatives() {
    evaluator_.setRandom([this] { return rand15(); });          // the random variance of a quantity or of the item game stage offset
    // ItemPool.SpawnBalancedInventoryFromPool(Definition, GameStage, AwesomeLevel, ContextSource, out SpawnedInventory, GameStageVarianceFormula, OuterPoolChance, bInventoryMayDropOnDeath)
    // (static). The inventories are stand-ins: an instance of the item's inventory class with bDropOnDeath set, not initialised (no parts, no stats: InitializeInventory is not run).
    // A variance formula is the path the death roll never takes and is reported as not implemented.
    bind("WillowGame.ItemPool.SpawnBalancedInventoryFromPool", [this](NativeCall& c) {
        if (!isObject(&c.in(0))) return Value::makeBool(false);
        if (c.has(5) && isObject(&c.in(5))) notImplemented("SpawnBalancedInventoryFromPool: a game stage variance formula");
        std::vector<LootItem> items;
        const bool mayDrop = !c.has(7) || c.in(7).kind == Value::Kind::None || c.in(7).truth();       // omitted: true
        const bool ok = spawnFromPool(load(c.in(0)), int(c.in(1).integer()), int(c.in(2).integer()), isObject(&c.in(3)) ? c.in(3).o : nullptr, mayDrop, items);
        Value& spawned = c.out(4);
        if (spawned.kind != Value::Kind::Array) spawned = Value::makeArray();
        for (const LootItem& item : items) {
            try {
                const ObjectPtr made = runtime_.instantiate(item.cls ? item.cls : runtime_.findClass("Engine.Inventory"));
                if (Value* flag = runtime_.property(*made, "bDropOnDeath")) *flag = Value::makeBool(item.dropOnDeath);
                spawned.elements().push_back(Value::makeObject(made));
            } catch (const RuntimeError&) {}
        }
        return Value::makeBool(ok);
    });
    bind("WillowGame.AIPawnBalanceDefinition.SetupPawnItemPoolList", [this](NativeCall& c) {
        return Value::makeBool(isObject(&c.in(0)) && setupItemPoolList(c.self, c.in(0).o));
    });
    // DropLootOnDeath: the installed script is replaced because the VM has no inventory actors or pickups; the loop is the script's (dropLoot above).
    try {
        runtime_.overrideScript("WillowGame.WillowPawn.DropLootOnDeath", [this](NativeCall& c) { dropLoot(c.self); return Value(); });
    } catch (const RuntimeError&) {}
}

} // namespace vm
