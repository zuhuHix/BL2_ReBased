#pragma once

#include <functional>
#include <map>
#include <string>

// Phase 2 (UnrealScript VM) seed: the native dispatch table.
//
// Every one of the 7,141 native functions will eventually be registered here.
// Until then, invoking an unregistered name returns an "UNIMPLEMENTED
// name(args)" log string instead of crashing: unmet natives announce
// themselves. This is scaffolding only; no game behavior is implemented and
// no reference implementation was consulted.
struct NativeRegistry {
    using Handler = std::function<std::string(const std::string& args)>;

    void add(const std::string& name, Handler handler);
    bool has(const std::string& name) const;
    // Registered names dispatch to their handler; anything else yields the
    // UNIMPLEMENTED log string. Never throws for unknown names.
    std::string invoke(const std::string& name, const std::string& args) const;

private:
    std::map<std::string, Handler> handlers_;
};

// In-process synthetic check of both dispatch paths (registered echo stub
// plus an unknown name). Returns a JSON summary; throws on failure. The 286
// Core builtins (Phase 2 step 5) will be registered here in later slices;
// the table intentionally ships empty.
std::string nativeSelfTest();
