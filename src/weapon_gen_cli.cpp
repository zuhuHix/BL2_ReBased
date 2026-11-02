// ow-weapongen: roll guns from a weapon generator manifest (tools/weapon_gen_manifest.py).
//
//   ow-weapongen <manifest> --balance <path> --stage N --seed S [--count K] [--stage-cap C]
//   ow-weapongen <manifest> --pool <path> --stage N --seed S [--count K] [--stage-cap C]
//   ow-weapongen <manifest> --batch <file> [--stage-cap C]
//                                   lines: balance|pool <path> <stage> <seed> [<stage cap>]
//
// One JSON object per line. --count K rolls seeds S .. S+K-1. A pool roll prints
// {"pool", "stage", "seed", "drops"} and then one gun per weapon balance it dropped; drop k
// (counting every drop) is rolled with seed S + k. A pool path that names an
// ItemPoolListDefinition or AIPawnBalanceDefinition rolls it as a source (loot_pools.roll_crt).
// --stage-cap C (or a batch line's fifth field) builds each gun at min(stage, C) like a pool spawn
// (weapon_recipe.roll stage_cap); pool gates still see the spawn stage. A capped gun also prints
// "spawn_stage" and "stage_cap".

#include "weapon_gen.hpp"

#include <cstdio>
#include <fstream>
#include <iostream>
#include <optional>
#include <sstream>
#include <string>

namespace {

int Usage() {
    std::cerr << "usage: ow-weapongen <manifest> (--balance <path> | --pool <path>) --stage N --seed S [--count K]"
                 " [--stage-cap C]\n"
                 "       ow-weapongen <manifest> --batch <file> [--stage-cap C]"
                 "   (lines: balance|pool <path> <stage> <seed> [<stage cap>])\n";
    return 2;
}

bool IsWeaponBalance(const weapongen::WeaponGenManifest& m, const std::string& path) {
    auto object = m.objectByPath.find(path);
    return object != m.objectByPath.end() && m.balanceByObject.count(std::to_string(object->second));
}

void RollOne(const weapongen::WeaponGenManifest& m, const std::string& kind, const std::string& path, int stage,
             uint32_t seed, std::optional<int> cap) {
    if (kind == "balance") {
        std::cout << weapongen::WeaponResultJson(weapongen::RollWeapon(m, path, stage, seed, cap)) << '\n';
        return;
    }
    weapongen::CrtRand rng(seed);
    std::vector<std::string> drops = weapongen::IsPoolSource(m, path) ? weapongen::RollSource(m, path, stage, rng)
                                                                      : weapongen::RollPool(m, path, stage, rng);
    std::string list = "[";
    for (size_t i = 0; i < drops.size(); ++i) list += (i ? "," : "") + weapongen::JsonString(drops[i]);
    std::cout << "{\"pool\":" << weapongen::JsonString(path) << ",\"stage\":" << stage << ",\"seed\":" << seed
              << ",\"drops\":" << list << "]}\n";
    for (size_t k = 0; k < drops.size(); ++k)
        if (IsWeaponBalance(m, drops[k]))
            std::cout << weapongen::WeaponResultJson(
                             weapongen::RollWeapon(m, drops[k], stage, seed + static_cast<uint32_t>(k), cap))
                      << '\n';
}

}  // namespace

int main(int argc, char** argv) {
    if (argc < 3) return Usage();
    std::string manifestPath = argv[1], kind, path, batch;
    long long stage = -1, seed = -1, count = 1, cap = -1;
    try {
        for (int i = 2; i < argc; ++i) {
            std::string arg = argv[i];
            if (i + 1 >= argc) return Usage();
            std::string value = argv[++i];
            if (arg == "--balance") kind = "balance", path = value;
            else if (arg == "--pool") kind = "pool", path = value;
            else if (arg == "--stage") stage = std::stoll(value);
            else if (arg == "--seed") seed = std::stoll(value);
            else if (arg == "--count") count = std::stoll(value);
            else if (arg == "--batch") batch = value;
            else if (arg == "--stage-cap") cap = std::stoll(value);
            else return Usage();
        }
    } catch (const std::exception&) {
        return Usage();
    }
    if (batch.empty() && (kind.empty() || stage < 0 || seed < 0 || seed > 0xFFFFFFFFLL || count < 1)) return Usage();
    std::optional<int> stageCap;
    if (cap >= 0) stageCap = static_cast<int>(cap);
    std::string error;
    weapongen::WeaponGenManifest manifest = weapongen::LoadWeaponGenManifest(manifestPath, &error);
    if (!error.empty()) {
        std::cerr << "ow-weapongen: " << error << '\n';
        return 1;
    }
    if (!batch.empty()) {
        std::ifstream file(batch);
        if (!file) {
            std::cerr << "ow-weapongen: cannot open " << batch << '\n';
            return 1;
        }
        std::string line;
        while (std::getline(file, line)) {
            std::istringstream fields(line);
            std::string lineKind, linePath;
            long long lineStage = 0, lineSeed = 0;
            if (!(fields >> lineKind >> linePath >> lineStage >> lineSeed)) continue;
            long long lineCap = -1;
            std::optional<int> capped = stageCap;
            if (fields >> lineCap && lineCap >= 0) capped = static_cast<int>(lineCap);
            RollOne(manifest, lineKind, linePath, static_cast<int>(lineStage), static_cast<uint32_t>(lineSeed), capped);
        }
        return 0;
    }
    for (long long k = 0; k < count; ++k)
        RollOne(manifest, kind, path, static_cast<int>(stage), static_cast<uint32_t>(seed + k), stageCap);
    return 0;
}
