#include "container.hpp"
#include <cstdint>
#include <stdexcept>
#include <utility>
#ifdef OW_RESEARCH_LZO
#include <minilzo.h>
#endif

namespace {
uint32_t word(const std::vector<unsigned char>& data, size_t offset) {
    if (offset > data.size() || data.size() - offset < 4)
        throw std::runtime_error("truncated container header/table");
    uint32_t result = 0;
    for (unsigned i = 0; i < 4; ++i) result |= uint32_t(data[offset + i]) << (8 * i);
    return result;
}
}

std::vector<unsigned char> decode_package(std::vector<unsigned char> input) {
    if (word(input, 0) != 0x9e2a83c1) throw std::runtime_error("invalid package magic");
    const auto block = word(input, 4);
    if (block == (832u | (46u << 16))) return input;
    if (block < 4096 || block > 1024 * 1024 || (block & (block - 1)))
        throw std::runtime_error("unsupported package version or container block size");
#ifndef OW_RESEARCH_LZO
    throw std::runtime_error("compressed input requires OPENWILLOW_RESEARCH_LZO=ON; see THIRD_PARTY.md");
#else
    const auto compressed = word(input, 8), decoded = word(input, 12);
    // Bound allocation before trusting file-controlled counts. Raise only with corpus evidence.
    if (!decoded || decoded > 512u * 1024 * 1024)
        throw std::runtime_error("container decoded size exceeds research limit (512 MiB) or is zero");
    const size_t blocks = (size_t(decoded) + block - 1) / block;
    const size_t start = 16 + blocks * 8;
    if (start > input.size() || compressed != input.size() - start)
        throw std::runtime_error("container compressed size/table mismatch");
    size_t totalCompressed = 0, totalDecoded = 0;
    for (size_t i = 0; i < blocks; ++i) {
        const auto c = word(input, 16 + i * 8), u = word(input, 20 + i * 8);
        const auto expected = (i + 1 == blocks) ? decoded - totalDecoded : block;
        if (!c || c > compressed - totalCompressed || !u || u != expected)
            throw std::runtime_error("invalid container block sizes");
        totalCompressed += c; totalDecoded += u;
    }
    if (totalCompressed != compressed || totalDecoded != decoded)
        throw std::runtime_error("container block totals mismatch");
    if (lzo_init() != LZO_E_OK) throw std::runtime_error("miniLZO initialization failed");
    std::vector<unsigned char> output(decoded);
    size_t inOffset = start, outOffset = 0;
    for (size_t i = 0; i < blocks; ++i) {
        const auto c = word(input, 16 + i * 8), u = word(input, 20 + i * 8);
        lzo_uint actual = u;
        const int status = lzo1x_decompress_safe(input.data() + inOffset, c,
            output.data() + outOffset, &actual, nullptr);
        if (status != LZO_E_OK || actual != u)
            throw std::runtime_error("invalid LZO block or decoded length mismatch");
        inOffset += c; outOffset += u;
    }
    return output;
#endif
}
