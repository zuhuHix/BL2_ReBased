#include "assets.hpp"

#include <array>
#include <algorithm>
#include <bit>
#include <cmath>
#include <cstdint>
#include <fstream>
#include <iomanip>
#include <limits>
#include <sstream>
#include <stdexcept>

namespace assets {
namespace {

void check(bool condition, const char* message) {
    if (!condition) throw std::runtime_error(message);
}

unsigned char takeByte(Reader& reader) {
    reader.require(1);
    return reader.bytes()[reader.pos++];
}

uint16_t u16(Reader& reader) {
    reader.require(2);
    const auto value = uint16_t(reader.bytes()[reader.pos] | (reader.bytes()[reader.pos + 1] << 8));
    reader.pos += 2;
    return value;
}

float real(Reader& reader) {
    const auto value = std::bit_cast<float>(reader.u32());
    check(std::isfinite(value), "non-finite asset coordinate");
    return value;
}

size_t count(Reader& reader, size_t stride) {
    check(stride != 0, "zero asset array stride");
    const auto number = reader.i32();
    check(number >= 0, "negative asset array count");
    const auto end = std::min(reader.bytes().size(), reader.limit);
    check(reader.pos <= end && size_t(number) <= (end - reader.pos) / stride,
          "asset array exceeds payload");
    return size_t(number);
}

void bulkArraySkip(Reader& reader) {
    const auto stride = reader.u32();
    check(stride > 0 && stride <= 1024, "invalid bulk array stride");
    reader.skip(count(reader, stride) * stride);
}

void write(const std::filesystem::path& path, const Bytes& bytes) {
    std::ofstream output(path, std::ios::binary);
    check(bool(output), "cannot open asset output");
    output.write(reinterpret_cast<const char*>(bytes.data()), std::streamsize(bytes.size()));
    check(bool(output), "asset output write failed");
}

void bigEndian(Bytes& bytes, uint32_t value) {
    for (int i = 24; i >= 0; i -= 8)
        bytes.push_back(static_cast<unsigned char>(value >> i));
}

void pngChunk(Bytes& png, const char* type, const Bytes& data) {
    bigEndian(png, uint32_t(data.size()));
    const auto start = png.size();
    png.insert(png.end(), type, type + 4);
    png.insert(png.end(), data.begin(), data.end());
    uint32_t crc = 0xffffffff;
    for (size_t i = start; i < png.size(); ++i) {
        crc ^= png[i];
        for (int j = 0; j < 8; ++j)
            crc = (crc >> 1) ^ ((crc & 1) ? 0xedb88320u : 0);
    }
    bigEndian(png, ~crc);
}

void png(const std::filesystem::path& path, uint32_t width, uint32_t height, const Bytes& pixels) {
    check(width && height, "invalid PNG dimensions");
    check(pixels.size() == size_t(width) * height * 4, "PNG pixel buffer size mismatch");
    Bytes rows;
    rows.reserve(size_t(height) * (size_t(width) * 4 + 1));
    for (size_t y = 0; y < height; ++y) {
        rows.push_back(0);
        rows.insert(rows.end(), pixels.begin() + y * width * 4,
                    pixels.begin() + (y + 1) * width * 4);
    }

    Bytes zlib{0x78, 0x01};
    for (size_t at = 0; at < rows.size();) {
        const auto size = uint16_t(std::min(size_t(65535), rows.size() - at));
        zlib.push_back(at + size == rows.size() ? 1 : 0);
        zlib.push_back(size & 255);
        zlib.push_back(size >> 8);
        zlib.push_back((~size) & 255);
        zlib.push_back((~size) >> 8);
        zlib.insert(zlib.end(), rows.begin() + at, rows.begin() + at + size);
        at += size;
    }
    uint32_t a = 1;
    uint32_t b = 0;
    for (const auto value : rows) {
        a = (a + value) % 65521;
        b = (b + a) % 65521;
    }
    bigEndian(zlib, (b << 16) | a);

    Bytes result{137, 80, 78, 71, 13, 10, 26, 10};
    Bytes header;
    bigEndian(header, width);
    bigEndian(header, height);
    header.insert(header.end(), {8, 6, 0, 0, 0});
    pngChunk(result, "IHDR", header);
    pngChunk(result, "IDAT", zlib);
    pngChunk(result, "IEND", {});
    write(path, result);
}

void textureDimensions(uint32_t width, uint32_t height) {
    check(width && height && width <= 16384 && height <= 16384 &&
              uint64_t(width) * height <= 64u * 1024 * 1024,
          "invalid texture dimensions or exceeds 256 MiB decoded limit");
}

Bytes argb(Bytes data, uint32_t width, uint32_t height) {
    textureDimensions(width, height);
    check(data.size() == size_t(width) * height * 4, "A8R8G8B8 mip byte count mismatch");
    // Little-endian A8R8G8B8 stores B,G,R,A; PNG expects R,G,B,A.
    for (size_t at = 0; at < data.size(); at += 4)
        std::swap(data[at], data[at + 2]);
    return data;
}

Bytes dxt(const Bytes& data, uint32_t width, uint32_t height, bool dxt5) {
    const size_t blockBytes = dxt5 ? 16 : 8;
    textureDimensions(width, height);
    check(data.size() == size_t((width + 3) / 4) * ((height + 3) / 4) * blockBytes,
          "DXT mip byte count mismatch");
    Bytes pixels(size_t(width) * height * 4);
    Reader reader(data);
    for (uint32_t by = 0; by < height; by += 4) {
        for (uint32_t bx = 0; bx < width; bx += 4) {
            std::array<unsigned, 8> alpha{};
            uint64_t alphaBits = 0;
            if (dxt5) {
                alpha[0] = takeByte(reader);
                alpha[1] = takeByte(reader);
                for (unsigned i = 0; i < 6; ++i)
                    alphaBits |= uint64_t(takeByte(reader)) << (8 * i);
                if (alpha[0] > alpha[1]) {
                    for (unsigned i = 1; i <= 6; ++i)
                        alpha[i + 1] = ((7 - i) * alpha[0] + i * alpha[1]) / 7;
                } else {
                    for (unsigned i = 1; i <= 4; ++i)
                        alpha[i + 1] = ((5 - i) * alpha[0] + i * alpha[1]) / 5;
                    alpha[6] = 0;
                    alpha[7] = 255;
                }
            }
            const auto color0 = u16(reader);
            const auto color1 = u16(reader);
            std::array<std::array<unsigned, 4>, 4> colors{};
            for (unsigned i = 0; i < 2; ++i) {
                const auto color = i ? color1 : color0;
                colors[i] = {unsigned(((color >> 11) * 255 + 15) / 31),
                             unsigned((((color >> 5) & 63) * 255 + 31) / 63),
                             unsigned(((color & 31) * 255 + 15) / 31), 255};
            }
            if (color0 > color1 || dxt5) {
                for (unsigned k = 0; k < 4; ++k) {
                    colors[2][k] = (2 * colors[0][k] + colors[1][k]) / 3;
                    colors[3][k] = (colors[0][k] + 2 * colors[1][k]) / 3;
                }
            } else {
                for (unsigned k = 0; k < 4; ++k)
                    colors[2][k] = (colors[0][k] + colors[1][k]) / 2;
                colors[3] = {0, 0, 0, 0};
            }
            const auto bits = reader.u32();
            for (unsigned i = 0; i < 16; ++i) {
                auto color = colors[(bits >> (i * 2)) & 3];
                if (dxt5) color[3] = alpha[(alphaBits >> (i * 3)) & 7];
                const auto x = bx + i % 4;
                const auto y = by + i / 4;
                if (x < width && y < height)
                    for (unsigned k = 0; k < 4; ++k)
                        pixels[(size_t(y) * width + x) * 4 + k] =
                            static_cast<unsigned char>(color[k]);
            }
        }
    }
    return pixels;
}

struct TextureRecord {
    size_t index = 0;
    uint32_t flags = 0;
    uint32_t decodedBytes = 0;
    uint32_t storedBytes = 0;
    uint32_t offset = 0;
    uint32_t width = 0;
    uint32_t height = 0;
    size_t inlineAt = 0;
};

std::pair<std::string, std::string> textureProperties(const Package& package, Reader& reader,
                                                        int32_t index, size_t propertyOffset,
                                                        size_t objectEnd) {
    const auto& object = package.object(index);
    reader.pos = size_t(object.offset) + propertyOffset;
    reader.limit = objectEnd;
    std::string format;
    std::string cache;
    while (true) {
        const auto propertyName = package.name(reader);
        if (propertyName == "None") break;
        const auto type = package.name(reader);
        const auto size = reader.i32();
        if (size < 0) throw std::runtime_error("negative texture property size");
        reader.i32();
        std::string detail;
        if (type == "StructProperty" || type == "ByteProperty") detail = package.name(reader);
        if (type == "BoolProperty") reader.skip(1);
        reader.require(size_t(size));
        const auto end = reader.pos + size_t(size);
        reader.limit = end;
        if (propertyName == "Format" && type == "ByteProperty")
            format = package.name(reader);
        if (propertyName == "TextureFileCacheName" && type == "NameProperty")
            cache = package.name(reader);
        if (reader.pos > end) throw std::runtime_error("texture property exceeds its payload");
        reader.pos = end;
        reader.limit = objectEnd;
    }
    return {format, cache};
}

uint16_t halfBits(Reader& reader) {
    return u16(reader);
}

float half(uint16_t value) {
    const auto exponent = (value >> 10) & 31;
    const auto mantissa = value & 1023;
    check(exponent != 31, "non-finite half UV");
    return (value & 32768 ? -1.f : 1.f) *
           (exponent ? std::ldexp(float(1024 + mantissa), int(exponent) - 25)
                     : std::ldexp(float(mantissa), -24));
}

MeshLod readLod(const Package& package, Reader& reader) {
    MeshLod lod;
    const auto flags = reader.u32();
    reader.u32();
    const auto stored = reader.u32();
    reader.u32();
    if (!(flags & 1u) && !(flags & 32u)) reader.skip(stored);

    const auto sectionCount = count(reader, 41);
    lod.sections.reserve(sectionCount);
    for (size_t i = 0; i < sectionCount; ++i) {
        MeshSection section;
        section.material = reader.i32();
        if (section.material) package.object(section.material);
        section.materialPath = section.material ? package.path(section.material) : "None";
        reader.skip(12);
        section.firstIndex = reader.u32();
        section.faceCount = reader.u32();
        section.minVertex = reader.u32();
        section.maxVertex = reader.u32();
        reader.u32();
        reader.skip(count(reader, 8) * 8);
        reader.require(1);
        check(reader.bytes()[reader.pos++] == 0, "PS3 mesh section unsupported");
        lod.sections.push_back(std::move(section));
    }

    const auto positionStride = reader.u32();
    const auto vertexCount = reader.u32();
    check(positionStride == 12 && vertexCount > 0, "invalid position stream");
    check(reader.u32() == 12, "invalid position bulk stride");
    check(count(reader, 12) == vertexCount, "position count mismatch");

    lod.vertices.resize(vertexCount);
    for (auto& vertex : lod.vertices) {
        vertex.position = {real(reader), real(reader), real(reader)};
    }

    const auto uvSets = reader.u32();
    const auto uvStride = reader.u32();
    const auto uvCount = reader.u32();
    const auto fullPrecision = reader.u32();
    check(uvSets >= 1 && uvSets <= 8 && fullPrecision <= 1 && uvCount == vertexCount &&
              uvStride == 8 + uvSets * (fullPrecision ? 8 : 4),
          "invalid UV stream");
    check(reader.u32() == uvStride, "UV bulk stride mismatch");
    check(count(reader, uvStride) == vertexCount, "UV bulk count mismatch");
    lod.uvSetCount = uvSets;
    for (auto& vertex : lod.vertices) {
        reader.skip(4);
        reader.require(4);
        vertex.normal = {float(reader.bytes()[reader.pos]) / 127.5f - 1,
                         float(reader.bytes()[reader.pos + 1]) / 127.5f - 1,
                         float(reader.bytes()[reader.pos + 2]) / 127.5f - 1};
        reader.skip(4);
        vertex.uvs.reserve(uvSets);
        for (size_t set = 0; set < uvSets; ++set) {
            const auto u = fullPrecision ? real(reader) : half(halfBits(reader));
            const auto v = fullPrecision ? real(reader) : half(halfBits(reader));
            vertex.uvs.push_back({u, v});
        }
    }

    const auto colorStride = reader.u32();
    const auto colors = reader.u32();
    check(colors == 0 || (colors == vertexCount && colorStride == 4),
          "invalid color stream");
    if (colors) {
        check(reader.u32() == colorStride, "color bulk stride mismatch");
        check(count(reader, colorStride) == vertexCount, "color bulk count mismatch");
        for (auto& vertex : lod.vertices) {
            reader.require(4);
            for (auto& component : vertex.color) component = reader.bytes()[reader.pos++];
            vertex.hasColor = true;
        }
    }

    check(reader.u32() == vertexCount, "LOD vertex count mismatch");
    lod.indexWidth = reader.u32();
    check(lod.indexWidth == 2 || lod.indexWidth == 4, "unsupported mesh index width");
    const auto indexCount = count(reader, lod.indexWidth);
    check(indexCount % 3 == 0, "non-triangle index count");
    lod.indices.reserve(indexCount);
    for (size_t i = 0; i < indexCount; ++i) {
        const auto value = lod.indexWidth == 2 ? uint32_t(u16(reader)) : reader.u32();
        check(value < vertexCount, "mesh index out of range");
        lod.indices.push_back(value);
    }
    for (const auto& section : lod.sections) {
        check(section.firstIndex <= lod.indices.size() &&
                  section.faceCount <= (lod.indices.size() - section.firstIndex) / 3 &&
                  section.minVertex <= section.maxVertex && section.maxVertex < vertexCount,
              "invalid mesh section range");
    }
    return lod;
}

} // namespace

Bytes TfcStore::read(const std::string& cacheName, uint32_t offset, uint32_t storedBytes) const {
    check(!cacheName.empty() && cacheName != "None" &&
              cacheName.find_first_of("/\\:") == std::string::npos,
          "invalid/missing TFC cache name");
    const auto suffix = cacheName.size() >= 4 &&
                        cacheName.substr(cacheName.size() - 4) == ".tfc"
                            ? std::string{}
                            : ".tfc";
    const auto filename = cacheName + suffix;
    const auto path = root_ / filename;
    std::ifstream file(path, std::ios::binary | std::ios::ate);
    check(bool(file), "cannot open texture cache");
    const auto total = file.tellg();
    check(total >= 0 && uint64_t(offset) + storedBytes <= uint64_t(total),
          "TFC mip exceeds cache");
    check(storedBytes <= 512u * 1024 * 1024, "TFC mip too large");
    Bytes payload(storedBytes);
    file.seekg(offset);
    if (storedBytes) file.read(reinterpret_cast<char*>(payload.data()), storedBytes);
    check(bool(file) || storedBytes == 0, "TFC read failed");
    return payload;
}

TextureAsset readTexture(const Package& package, int32_t index, size_t propertyOffset,
                         const std::filesystem::path& tfcRoot) {
    const auto& object = package.object(index);
    check(object.cls && package.object(object.cls).name == "Texture2D",
          "export is not Texture2D");
    Reader reader = package.reader();
    package.properties(reader, index, propertyOffset);
    const auto nativeAt = reader.pos;
    const auto objectEnd = reader.limit;
    const auto [format, cache] = textureProperties(package, reader, index, propertyOffset, objectEnd);
    check(format == "PF_DXT1" || format == "PF_DXT5" || format == "PF_A8R8G8B8",
          "texture importer supports PF_DXT1/PF_DXT5/PF_A8R8G8B8");

    reader.pos = nativeAt;
    reader.limit = objectEnd;
    reader.skip(16);
    const auto serializedMips = count(reader, 24);
    check(serializedMips > 0 && serializedMips <= 64, "invalid texture mip count");
    std::vector<TextureRecord> records;
    records.reserve(serializedMips);
    for (size_t i = 0; i < serializedMips; ++i) {
        TextureRecord record;
        record.index = i;
        record.flags = reader.u32();
        record.decodedBytes = reader.u32();
        record.storedBytes = reader.u32();
        record.offset = reader.u32();
        record.inlineAt = reader.pos;
        check(!(record.flags & ~uint32_t(1 | 8 | 16 | 32)),
              "unsupported texture bulk codec/flags");
        if (!(record.flags & 1u) && !(record.flags & 32u)) reader.skip(record.storedBytes);
        record.width = reader.u32();
        record.height = reader.u32();
        check(record.width && record.height, "invalid texture mip dimensions");
        records.push_back(record);
    }

    TextureAsset asset;
    asset.path = package.path(index);
    asset.format = format;
    asset.cacheName = cache;
    asset.serializedMipCount = serializedMips;
    TfcStore tfc(tfcRoot);
    size_t totalDecoded = 0;
    for (const auto& record : records) {
        if ((record.flags & 32u) || !record.decodedBytes) continue;
        Bytes payload;
        if (record.flags & 1u) {
            payload = tfc.read(cache, record.offset, record.storedBytes);
        } else {
            check(record.inlineAt <= objectEnd && record.storedBytes <= objectEnd - record.inlineAt,
                  "inline texture mip exceeds export payload");
            payload.assign(reader.bytes().begin() + record.inlineAt,
                           reader.bytes().begin() + record.inlineAt + record.storedBytes);
        }
        if (record.flags & 16u) payload = decode_package(std::move(payload));
        check(payload.size() == record.decodedBytes, "texture bulk decoded size mismatch");
        check(payload.size() <= 256u * 1024 * 1024,
              "texture mip compressed data exceeds importer limit");
        TextureMip mip;
        mip.index = record.index;
        mip.flags = record.flags;
        mip.decodedBytes = record.decodedBytes;
        mip.storedBytes = record.storedBytes;
        mip.offset = record.offset;
        mip.width = record.width;
        mip.height = record.height;
        mip.rgba = format == "PF_A8R8G8B8"
                       ? argb(std::move(payload), record.width, record.height)
                       : dxt(payload, record.width, record.height, format == "PF_DXT5");
        check(mip.rgba.size() <= 512u * 1024 * 1024 &&
                  totalDecoded <= 512u * 1024 * 1024 - mip.rgba.size(),
              "texture mip decoded data exceeds importer limit");
        totalDecoded += mip.rgba.size();
        asset.mips.push_back(std::move(mip));
    }
    check(!asset.mips.empty(), "no resident texture mip");
    return asset;
}

void writeTexturePng(const TextureMip& mip, const std::filesystem::path& output) {
    png(output, mip.width, mip.height, mip.rgba);
}

std::string texture(const Package& package, int32_t index, size_t propertyOffset,
                    const std::filesystem::path& output, const std::filesystem::path& tfcRoot,
                    size_t selectedMip, const std::filesystem::path& allMipsDirectory) {
    const auto asset = readTexture(package, index, propertyOffset, tfcRoot);
    check(selectedMip < asset.mips.size(), "selected texture mip is unavailable");
    writeTexturePng(asset.mips[selectedMip], output);
    if (!allMipsDirectory.empty()) {
        std::filesystem::create_directories(allMipsDirectory);
        for (const auto& mip : asset.mips) {
            std::ostringstream name;
            name << "mip_" << std::setw(2) << std::setfill('0') << mip.index << ".png";
            writeTexturePng(mip, allMipsDirectory / name.str());
        }
    }

    const auto& selected = asset.mips[selectedMip];
    std::ostringstream report;
    report << "{\"path\":" << quote(asset.path)
           << ",\"format\":" << quote(asset.format)
           << ",\"width\":" << selected.width
           << ",\"height\":" << selected.height
           << ",\"mip\":" << selected.index
           << ",\"streamed\":" << (selected.streamed() ? "true" : "false")
           << ",\"serialized_mips\":" << asset.serializedMipCount
           << ",\"resident_mips\":" << asset.mips.size() << '}';
    return report.str();
}

MeshAsset readMesh(const Package& package, int32_t index, size_t propertyOffset) {
    const auto& object = package.object(index);
    check(object.cls && package.object(object.cls).name == "StaticMesh",
          "export is not StaticMesh");
    Reader reader = package.reader();
    package.properties(reader, index, propertyOffset);
    reader.skip(28);
    const auto body = reader.i32();
    if (body) package.object(body);
    reader.skip(24);
    bulkArraySkip(reader);
    bulkArraySkip(reader);
    reader.u32();
    check(reader.u32() == 0, "source mesh LOD data unsupported");
    reader.skip(count(reader, 7) * 7);
    reader.u32();
    const auto lodCount = count(reader, 16);
    check(lodCount > 0 && lodCount <= 32, "invalid mesh LOD count");

    MeshAsset asset;
    asset.path = package.path(index);
    asset.lods.reserve(lodCount);
    asset.bodySetup = body;
    for (size_t i = 0; i < lodCount; ++i)
        asset.lods.push_back(readLod(package, reader));
    return asset;
}

void writeMeshObj(const MeshAsset& mesh, const std::filesystem::path& output, size_t lodIndex) {
    check(lodIndex < mesh.lods.size(), "selected mesh LOD is unavailable");
    const auto& lod = mesh.lods[lodIndex];
    std::ostringstream obj;
    obj << std::setprecision(9)
        << "# OpenWillow local extraction; UE coordinates X,Y,Z; centimeters\n";
    for (const auto& vertex : lod.vertices)
        obj << "v " << vertex.position[0] << ' ' << vertex.position[1] << ' '
            << vertex.position[2] << '\n';
    for (const auto& vertex : lod.vertices) {
        check(!vertex.uvs.empty(), "mesh vertex has no UV set");
        obj << "vt " << vertex.uvs[0][0] << ' ' << 1 - vertex.uvs[0][1] << '\n';
    }
    for (const auto& vertex : lod.vertices)
        obj << "vn " << vertex.normal[0] << ' ' << vertex.normal[1] << ' '
            << vertex.normal[2] << '\n';

    for (size_t sectionIndex = 0; sectionIndex < lod.sections.size(); ++sectionIndex) {
        const auto& section = lod.sections[sectionIndex];
        obj << "g section_" << sectionIndex << "\n# material " << section.materialPath << '\n';
        for (size_t at = section.firstIndex;
             at < section.firstIndex + size_t(section.faceCount) * 3; at += 3) {
            obj << 'f';
            for (size_t k = 0; k < 3; ++k) {
                const auto vertex = lod.indices[at + k] + 1;
                obj << ' ' << vertex << '/' << vertex << '/' << vertex;
            }
            obj << '\n';
        }
    }
    const auto text = obj.str();
    write(output, Bytes(text.begin(), text.end()));
}

std::string mesh(const Package& package, int32_t index, size_t propertyOffset,
                 const std::filesystem::path& output, size_t selectedLod) {
    const auto asset = readMesh(package, index, propertyOffset);
    check(selectedLod < asset.lods.size(), "selected mesh LOD is unavailable");
    writeMeshObj(asset, output, selectedLod);
    const auto& selected = asset.lods[selectedLod];
    std::ostringstream report;
    report << "{\"path\":" << quote(asset.path)
           << ",\"lods\":" << asset.lods.size()
           << ",\"body_setup\":" << asset.bodySetup
           << ",\"selected_lod\":" << selectedLod
           << ",\"vertices\":" << selected.vertices.size()
           << ",\"triangles\":" << selected.indices.size() / 3
           << ",\"index_width\":" << selected.indexWidth
           << ",\"uv_sets\":" << selected.uvSetCount
           << ",\"sections\":[";
    for (size_t i = 0; i < selected.sections.size(); ++i) {
        if (i) report << ',';
        const auto& section = selected.sections[i];
        report << "{\"material_index\":" << section.material
               << ",\"material\":" << quote(section.materialPath)
               << ",\"triangles\":" << section.faceCount << '}';
    }
    report << "] ,\"lod_summaries\":[";
    for (size_t i = 0; i < asset.lods.size(); ++i) {
        if (i) report << ',';
        const auto& lod = asset.lods[i];
        report << "{\"lod\":" << i << ",\"vertices\":" << lod.vertices.size()
               << ",\"triangles\":" << lod.indices.size() / 3
               << ",\"index_width\":" << lod.indexWidth
               << ",\"uv_sets\":" << lod.uvSetCount << '}';
    }
    report << "]}";
    return report.str();
}

} // namespace assets
