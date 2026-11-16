// FROZEN (lane FIX-VMGAPS, 2026-11-15; docs/rust-port/ARCHITECTURE.md section 6; decisions 2026-11-15-23 and
// 2026-11-15-28). The C++ script VM stops being developed: no new feature, no new consumer, edits only to keep it
// building. It stays the differential reference of the Rust port (crates/ow-vm; --vm-sweep, --native-census, --run)
// until W7-RETIRE deletes it. It was frozen on the sweep clause of the amended ow-vm gate, which decision 23 set as the
// freeze condition: --vm-sweep on the nine code packages under OW_PARITY=reference gives the Rust result apart from
// decision 2026-11-15-07's string-unit differences and decision 2026-11-15-23's causes A and B. The gate's test-file
// clauses are not all met yet (tests/vm_test.py on the Rust binary; counts in decision 2026-11-15-28).
#include "natives.hpp"

#include <sstream>
#include <stdexcept>

void NativeRegistry::add(const std::string& name, Handler handler) {
    if (name.empty() || !handler) throw std::runtime_error("native registration requires a name and handler");
    if (!handlers_.emplace(name, std::move(handler)).second)
        throw std::runtime_error("duplicate native registration: " + name);
}

bool NativeRegistry::has(const std::string& name) const {
    return handlers_.find(name) != handlers_.end();
}

std::string NativeRegistry::invoke(const std::string& name, const std::string& args) const {
    const auto found = handlers_.find(name);
    if (found == handlers_.end()) return "UNIMPLEMENTED " + name + "(" + args + ")";
    return found->second(args);
}

std::string nativeSelfTest() {
    NativeRegistry registry;
    registry.add("Test.Echo", [](const std::string& args) { return args; });
    if (!registry.has("Test.Echo") || registry.has("Test.Missing")) throw std::runtime_error("registry lookup failed");
    if (registry.invoke("Test.Echo", "a,b") != "a,b") throw std::runtime_error("registered dispatch failed");
    const std::string stub = registry.invoke("WillowGame.WillowWeapon.Fire", "a,b");
    if (stub != "UNIMPLEMENTED WillowGame.WillowWeapon.Fire(a,b)") throw std::runtime_error("stub log mismatch");
    bool duplicateRejected = false;
    try {
        registry.add("Test.Echo", [](const std::string& args) { return args; });
    } catch (const std::runtime_error&) {
        duplicateRejected = true;
    }
    if (!duplicateRejected) throw std::runtime_error("duplicate registration accepted");
    std::ostringstream output;
    output << "{\"registered\":1,\"unimplemented\":1,\"duplicate_rejected\":true,\"passed\":true}";
    return output.str();
}
