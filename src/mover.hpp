#pragma once
#include <filesystem>
#include <memory>
#include <string>
#include <vector>

namespace vm {
// Bounded InterpActor lifecycle adapter. Motion comes from installed Matinee
// data in the host; this runs its installed script notifications and timers.
class Mover {
public:
    struct Result { size_t steps = 0; bool checkpoint = false; std::string error; };
    Mover(const std::filesystem::path& cooked, const std::string& package,
          const std::string& actor, const std::string& action);
    ~Mover();
    Result notify(bool finished, bool reverse);
    Result advance(double seconds);
    const std::vector<std::string>& loadingDiagnostics() const;
private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};
}
