#include "inventory_navigation.hpp"
#include "vm.hpp"

namespace vm {
namespace {
int sourceEntryKind(Runtime& runtime, Function& getter) {
    if (!getter.result || getter.result->type != "ByteProperty")
        throw RuntimeError("inventory entry getter has no enum return");
    const auto resolved = runtime.resolveRef(getter.result->package, getter.result->typeRef);
    if (!resolved || runtime.indexOf(*resolved.package).classNames.at(size_t(resolved.index - 1)) != "Enum")
        throw RuntimeError("inventory entry enum is unresolved");
    const auto& object = resolved.package->object(resolved.index);
    auto reader = resolved.package->reader();
    reader.pos = size_t(object.offset);
    reader.require(size_t(object.size));
    reader.limit = reader.pos + size_t(object.size);
    // Existing enum layout used by tagged defaults in vm.cpp. Checked against
    // this getter's reflected enum, not a guessed numeric entry kind.
    reader.i32();
    if (resolved.package->name(reader) != "None") throw RuntimeError("unexpected enum property prefix");
    reader.reference(int32_t(resolved.package->imports.size()), int32_t(resolved.package->exports.size()));
    const int32_t count = reader.i32();
    if (count <= 0 || count > 256) throw RuntimeError("invalid inventory entry enum count");
    reader.require(size_t(count) * 8);
    int source = -1;
    for (int i = 0; i < count; ++i) {
        if (resolved.package->name(reader) == "EAK_Source") {
            if (source != -1) throw RuntimeError("duplicate source entry enum");
            source = i;
        }
    }
    if (reader.pos != reader.limit || source == -1) throw RuntimeError("unsupported inventory entry enum");
    return source;
}
}

struct InventoryNavigation::Impl {
    PackageStore store;
    Runtime runtime;
    ObjectPtr panel, provider;
    Function* movement = nullptr;
    int count = 0;
    explicit Impl(const std::filesystem::path& cooked) : store(cooked), runtime(store) {
        runtime.registerCoreNatives();
        runtime.stepLimit = 20000;
        movement = runtime.findFunction("WillowGame.InventoryListPanelGFxObject.MoveDelta");
        if (movement->isNative() || movement->params.size() != 3 || !movement->result || movement->result->type != "IntProperty")
            throw RuntimeError("unsupported inventory MoveDelta signature");
        const char* names[] = {"Delta", "StartIndex", "OriginalIndex"};
        for (size_t i = 0; i < 3; ++i)
            if (movement->params[i].name != names[i] || movement->params[i].type != "IntProperty")
                throw RuntimeError("unsupported inventory MoveDelta parameter");
        // Decode now so opening the menu fails visibly, rather than on its first key.
        movement->code = std::make_shared<script::Code>(script::decode(*movement->package, movement->info));
        auto* getter = runtime.findFunction("WillowGame.InventoryDataProviderGFxObject.GetEntryKindAtIndex");
        if (!getter->isNative() || getter->params.size() != 1 || getter->params[0].type != "IntProperty")
            throw RuntimeError("unsupported inventory entry getter signature");
        const int source = sourceEntryKind(runtime, *getter);
        panel = runtime.instantiate(runtime.findClass("WillowGame.InventoryListPanelGFxObject"));
        provider = runtime.instantiate(runtime.findClass("WillowGame.InventoryDataProviderGFxObject"));
        auto* data = runtime.property(*panel, "DataProvider");
        auto* cached = runtime.property(*provider, "CachedObjects");
        if (!data || data->kind != Value::Kind::Object || !cached || cached->kind != Value::Kind::Array)
            throw RuntimeError("unsupported inventory provider properties");
        *data = Value::makeObject(provider);
        runtime.registerNative(getter->nativeKey, [this, source](NativeCall& call) {
            if (call.self != provider || call.count() != 1 || call.in(0).integer() < 0 || call.in(0).integer() >= count)
                throw RuntimeError("inventory entry getter outside supplied item list");
            return Value::makeByte(source);
        });
    }
};

InventoryNavigation::InventoryNavigation(const std::filesystem::path& cooked) : impl_(std::make_unique<Impl>(cooked)) {}
InventoryNavigation::~InventoryNavigation() = default;

InventoryNavigation::Result InventoryNavigation::move(int delta, int start, int count) {
    Result result;
    auto& in = *impl_;
    in.runtime.log.clear();
    in.runtime.steps = 0;
    try {
        if ((delta != -1 && delta != 1) || count <= 0 || count > 2048 || start < 0 || start >= count)
            throw RuntimeError("invalid inventory movement request");
        in.count = count;
        auto& values = in.runtime.property(*in.provider, "CachedObjects")->elements();
        values.assign(size_t(count), Value::makeObject(nullptr));
        const auto value = in.runtime.call(*in.movement, in.panel,
            {Value::makeInt(delta), Value::makeInt(start), Value::makeInt(start)});
        if (!in.runtime.log.empty()) throw RuntimeError("inventory movement emitted a diagnostic: " + in.runtime.log.front());
        if (value.kind != Value::Kind::Int || value.i < 0 || value.i >= count)
            throw RuntimeError("inventory movement returned an invalid index");
        result.index = int(value.i);
    } catch (const std::exception& error) { result.error = error.what(); }
    result.steps = in.runtime.steps;
    return result;
}
}
