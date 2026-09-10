#include "container.hpp"
#include <cstdint>
#include <stdexcept>
#include <utility>
#include <algorithm>
#ifdef OW_LZO
#include <lzokay.hpp>
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
    if (block == (832u | (46u << 16))) {
        const int64_t length = static_cast<int32_t>(word(input, 12));
        const uint64_t stringBytes = length < 0 ? uint64_t(-length) * 2 : uint64_t(length);
        if (stringBytes > input.size() || 16 + stringBytes > input.size())
            throw std::runtime_error("invalid summary folder length");
        const size_t flagsAt = 16 + size_t(stringBytes);
        if (!(word(input, flagsAt) & 0x02000000u)) return input;
        // BL2 832/46 summary: tables, dependency/GUID-table fields, GUID,
        // generations (three words each), engine/cooker versions, compression.
        size_t cursor = flagsAt + 4 + 24 + 20 + 16;
        const auto generations = word(input, cursor); cursor += 4;
        if (generations > input.size() / 12) throw std::runtime_error("invalid generation count");
        cursor += size_t(generations) * 12 + 8;
        const auto codec = word(input, cursor), count = word(input, cursor + 4); cursor += 8;
        if (codec != 2 || !count || count > (input.size() - std::min(cursor, input.size())) / 16)
            throw std::runtime_error("unsupported or invalid partial compression table");
        size_t previousEnd = 0, previousDiskEnd = cursor + size_t(count) * 16;
        std::vector<unsigned char> output;
        for (size_t i = 0; i < count; ++i) {
            const auto at = cursor + i * 16;
            const size_t dest = word(input, at), size = word(input, at + 4);
            const size_t source = word(input, at + 8), stored = word(input, at + 12);
            if (!size || dest > 512u * 1024 * 1024 || size > 512u * 1024 * 1024 - dest ||
                source < previousDiskEnd || source > input.size() || stored > input.size() - source ||
                (i && dest != previousEnd) || (!i && (dest < 48 || dest > source)))
                throw std::runtime_error("invalid partial compression chunk bounds");
            if (!i) output.assign(input.begin(), input.begin() + dest);
            std::vector<unsigned char> chunk(input.begin() + source, input.begin() + source + stored);
            if (word(chunk, 4) == (832u | (46u << 16))) throw std::runtime_error("expected chunk container");
            auto decodedChunk = decode_package(std::move(chunk));
            if (decodedChunk.size() != size) throw std::runtime_error("partial chunk decoded size mismatch");
            output.insert(output.end(), decodedChunk.begin(), decodedChunk.end());
            previousEnd = dest + size; previousDiskEnd = source + stored;
        }
        if (previousDiskEnd != input.size()) throw std::runtime_error("trailing partial package bytes");
        return output;
    }
    if (block < 4096 || block > 1024 * 1024 || (block & (block - 1)))
        throw std::runtime_error("unsupported package version or container block size");
#ifndef OW_LZO
    throw std::runtime_error("compressed input requires OPENWILLOW_LZO=ON; see THIRD_PARTY.md");
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
    std::vector<unsigned char> output(decoded);
    size_t inOffset = start, outOffset = 0;
    for (size_t i = 0; i < blocks; ++i) {
        const auto c = word(input, 16 + i * 8), u = word(input, 20 + i * 8);
        size_t actual = 0;
        const auto status = lzokay::decompress(input.data() + inOffset, c,
            output.data() + outOffset, u, actual);
        if (status != lzokay::EResult::Success || actual != u)
            throw std::runtime_error("invalid LZO block or decoded length mismatch");
        inOffset += c; outOffset += u;
    }
    return output;
#endif
}
