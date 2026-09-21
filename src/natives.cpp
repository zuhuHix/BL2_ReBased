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
