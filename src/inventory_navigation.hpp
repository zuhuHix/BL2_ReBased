#pragma once

#include <filesystem>
#include <memory>
#include <string>

namespace vm {
// First inventory/host slice: execute the installed game's MoveDelta on an
// item-only list. Sorting, headers, empty entries and equipment remain outside
// this adapter. No navigation algorithm or game bytecode is embedded here.
class InventoryNavigation {
public:
    explicit InventoryNavigation(const std::filesystem::path& cooked);
    ~InventoryNavigation();
    InventoryNavigation(const InventoryNavigation&) = delete;
    InventoryNavigation& operator=(const InventoryNavigation&) = delete;
    struct Result {
        int index = -1;
        size_t steps = 0;
        std::string error;
    };
    Result move(int delta, int start, int count);
private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};
}
