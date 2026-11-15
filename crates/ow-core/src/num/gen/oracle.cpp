// The C++ half of the ow_core::num / ow_core::fmt oracle tables (lane W1-NUM; architecture section 5.2).
// AI-assisted (Claude Code). It reads the invented inputs oracle.py writes and prints what the C++ core's own forms
// give for each: std::pow on doubles (weapon_gen.cpp FormulaValue), std::pow on floats (progression.cpp), the Core
// natives' double transcendentals (natives_core.cpp), int64_t(double) then int32_t (natives_core.cpp, vm.cpp makeInt),
// weapon_gen.cpp HalfUp, and every float printer the outputs use. Build it with the toolset of the C++ core's build
// and its Release flags (see README.md beside this file); it is not part of the CMake build.
#include <charconv>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <string>
#include <vector>

namespace {

double from64(const std::string& h) { uint64_t b = std::stoull(h, nullptr, 16); double x; std::memcpy(&x, &b, 8); return x; }
float from32(const std::string& h) { uint32_t b = uint32_t(std::stoul(h, nullptr, 16)); float x; std::memcpy(&x, &b, 4); return x; }
std::string h64(double x) { uint64_t b; std::memcpy(&b, &x, 8); char s[32]; std::snprintf(s, sizeof s, "%016llx", (unsigned long long)b); return s; }
std::string h32(float x) { uint32_t b; std::memcpy(&b, &x, 4); char s[16]; std::snprintf(s, sizeof s, "%08x", b); return s; }

std::string print(const char* format, double x) {
    std::vector<char> buffer(4096);
    std::snprintf(buffer.data(), buffer.size(), format, x);
    return buffer.data();
}

double F32(double value) { return static_cast<double>(static_cast<float>(value)); }

// F32 and HalfUp: copies of src/weapon_gen.cpp at commit 3c1c47f (F32 line 521, HalfUp lines 878-881).
double HalfUp(double number, double digits) {
    double scale = F32(std::pow(10.0, digits));
    return std::floor(F32(F32(number * scale) + 0.5)) / scale;
}

}  // namespace

int main(int argc, char** argv) {
    if (argc != 2) { std::cerr << "usage: oracle <inputs.txt>\n"; return 2; }
    std::ifstream in(argv[1]);
    std::string line;
    while (std::getline(in, line)) {
        std::istringstream words(line);
        std::string kind, a, b;
        words >> kind >> a;
        std::vector<std::string> out;
        if (kind == "pow64") { words >> b; out.push_back(h64(std::pow(from64(a), from64(b)))); }
        else if (kind == "pow32") { words >> b; out.push_back(h32(std::pow(from32(a), from32(b)))); }
        else if (kind == "exp") out.push_back(h64(std::exp(from64(a))));
        else if (kind == "ln") out.push_back(h64(std::log(from64(a))));
        else if (kind == "sqrt") out.push_back(h64(std::sqrt(from64(a))));
        else if (kind == "sin") out.push_back(h64(std::sin(from64(a))));
        else if (kind == "cos") out.push_back(h64(std::cos(from64(a))));
        else if (kind == "tan") out.push_back(h64(std::tan(from64(a))));
        else if (kind == "asin") out.push_back(h64(std::asin(from64(a))));
        else if (kind == "acos") out.push_back(h64(std::acos(from64(a))));
        else if (kind == "atan") out.push_back(h64(std::atan(from64(a))));
        else if (kind == "atan2") { words >> b; out.push_back(h64(std::atan2(from64(a), from64(b)))); }
        else if (kind == "round32") out.push_back(h32(std::round(from32(a))));   // progression.cpp restricted(), mode 1
        else if (kind == "trunc") {
            const double x = from64(a);
            out.push_back(std::to_string(int32_t(int64_t(x))));   // natives_core.cpp int64_t(...) then makeInt
            out.push_back(std::to_string(int32_t(x)));            // the plain hardware conversion, for comparison
        } else if (kind == "card") { words >> b; out.push_back(h64(HalfUp(double(from32(a)), std::stod(b)))); }
        else if (kind == "fmt") {
            const double x = from64(a);
            for (const char* f : {"%.1g", "%.5g", "%g", "%.9g", "%.17g", "%.0f", "%.1f", "%.3f", "%f"}) out.push_back(print(f, x));
            out.push_back(std::to_string(float(x)));
            std::ostringstream os6; os6 << x; out.push_back(os6.str());
            std::ostringstream os9; os9 << std::setprecision(9) << x; out.push_back(os9.str());
            char buffer[128];
            auto result = std::to_chars(buffer, buffer + sizeof buffer, x);
            out.push_back(std::string(buffer, result.ptr));
            out.push_back("-");   // json.dumps form: Python only
        } else { std::cerr << "unknown input kind " << kind << "\n"; return 2; }
        std::cout << line;
        for (const std::string& cell : out) std::cout << '\t' << cell;
        std::cout << '\n';
    }
    return 0;
}
