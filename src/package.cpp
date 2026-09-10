// Independent table reader based on the layout recorded in research/native_count.py.
// Local research tool, not a runtime loader.
#include "container.hpp"
#include <cstdint>
#include <fstream>
#include <iostream>
#include <iterator>
#include <stdexcept>
#include <string>
#include <vector>

struct Reader {
    std::vector<unsigned char> data;
    size_t pos = 0;
    void require(size_t n) const {
        if (pos > data.size() || n > data.size() - pos)
            throw std::runtime_error("truncated package");
    }
    void skip(size_t n) { require(n); pos += n; }
    uint32_t u32() {
        require(4);
        uint32_t v = 0;
        for (unsigned i = 0; i < 4; ++i) v |= uint32_t(data[pos++]) << (8 * i);
        return v;
    }
    int32_t i32() { return static_cast<int32_t>(u32()); }
    void string() {
        const int64_t n = i32();
        const size_t bytes = static_cast<size_t>(n < 0 ? -n * 2 : n);
        require(bytes);
        if (bytes && (data[pos + bytes - 1] != 0 || (n < 0 && data[pos + bytes - 2] != 0)))
            throw std::runtime_error("unterminated FString");
        skip(bytes);
    }
    void table(int32_t count, int32_t offset, size_t minimum) {
        if (count < 0 || offset < 0 || size_t(offset) > data.size() ||
            size_t(count) > (data.size() - size_t(offset)) / minimum)
            throw std::runtime_error("invalid table bounds");
        pos = size_t(offset);
    }
    void name(int32_t count) {
        auto index = i32(); auto number = i32();
        if (index < 0 || index >= count || number < 0) throw std::runtime_error("invalid FName");
    }
    void reference(int32_t imports, int32_t exports) {
        const int64_t value = i32();
        if (value < -int64_t(imports) || value > exports) throw std::runtime_error("invalid object reference");
    }
};

int main(int argc, char** argv) {
    try {
        if (argc != 2 && !(argc == 4 && std::string(argv[2]) == "--verify-decoded"))
            throw std::runtime_error("usage: ow-package <package> [--verify-decoded <reference-file>]");
        std::ifstream file(argv[1], std::ios::binary);
        if (!file) throw std::runtime_error("cannot open input");
        Reader r{decode_package({std::istreambuf_iterator<char>(file), std::istreambuf_iterator<char>()})};
        if (argc == 4) {
            std::ifstream reference(argv[3], std::ios::binary);
            if (!reference) throw std::runtime_error("cannot open decoded reference");
            const std::vector<unsigned char> expected{std::istreambuf_iterator<char>(reference), std::istreambuf_iterator<char>()};
            if (r.data != expected) throw std::runtime_error("decoded bytes differ from reference");
        }
        if (r.u32() != 0x9e2a83c1) throw std::runtime_error("invalid package magic");
        if (r.u32() != (832u | (46u << 16))) throw std::runtime_error("requires decompressed BL2 version 832/46");
        const auto header = r.i32();
        if (header < 0 || size_t(header) > r.data.size()) throw std::runtime_error("invalid header size");
        r.string(); r.skip(4);
        const auto names = r.i32(), nameOffset = r.i32();
        const auto exports = r.i32(), exportOffset = r.i32();
        const auto imports = r.i32(), importOffset = r.i32();
        r.table(names, nameOffset, 12);
        for (int32_t i = 0; i < names; ++i) { r.string(); r.skip(8); }
        r.table(imports, importOffset, 28);
        for (int32_t i = 0; i < imports; ++i) {
            r.name(names); r.name(names); r.reference(imports, exports); r.name(names);
        }
        r.table(exports, exportOffset, 68);
        for (int32_t i = 0; i < exports; ++i) {
            r.reference(imports, exports); r.reference(imports, exports); r.reference(imports, exports);
            r.name(names); r.reference(imports, exports); r.skip(8);
            const auto size = r.i32(), offset = r.i32();
            if (size < 0 || offset < 0 || size_t(offset) > r.data.size() || size_t(size) > r.data.size() - size_t(offset))
                throw std::runtime_error("invalid export payload bounds");
            r.skip(4);
            const auto net = r.i32();
            if (net < 0 || size_t(net) > r.data.size() / 4) throw std::runtime_error("invalid net object count");
            r.skip(size_t(net) * 4); r.skip(20);
        }
        std::cout << "{\"version\":832,\"licensee\":46,\"names\":" << names
                  << ",\"imports\":" << imports << ",\"exports\":" << exports << "}\n";
    } catch (const std::exception& e) {
        std::cerr << "ow-package: " << e.what() << '\n'; return 1;
    }
}
