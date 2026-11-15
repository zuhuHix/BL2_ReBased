//! Owner: lane W2-ASSETS. Texture reads: the port of `assets::readTexture` and its pixel decoders (`src/assets.cpp`):
//! `Texture2D` and `TerrainWeightMapTexture` exports in PF_DXT1, PF_DXT5, PF_A8R8G8B8 or PF_G8, their serialized mip
//! records, inline or TFC-streamed bulk data, LZO-compressed bulk through `ow_container::decode_package`, and the
//! decode to RGBA8.
//!
//! This file also holds what `mesh.rs` shares with it: [`AssetError`] and the bounds-checked readers the C++ file
//! keeps in its anonymous namespace (`count`, `real`, `u16`, `bulkArraySkip`, `write`).
//!
//! The layout comes from the C++ reader, which recovered it from the packages and checked it with structural
//! oracles; nothing here is re-derived (AGENTS.md rule 4). Every C++ check is present, in the C++ order, with the
//! C++ message (the CLI prints `Display`), and none is loosened (rule 5). Which pixels the game's renderer shows for
//! these bytes is UNVERIFIED: the decoders reproduce the C++ importer, not the game.

use std::fmt;
use std::path::Path;

use ow_container::ContainerError;
use ow_package::package::{Package, PackageError};
use ow_package::props::{self, ArrayTyper, PropError};
use ow_package::reader::{ReadError, Reader};
use ow_package::tables::read_name;

use crate::png;
use crate::tfc::TfcStore;

/// Why an asset could not be read or written. `Display` gives the C++ exception text, which the CLI prints after
/// `ow-package: `. The wrapped reader, package, property and container errors print their own text and pass their
/// causes on through `source()`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetError {
    Read(ReadError),
    Package(PackageError),
    Props(PropError),
    /// A compressed bulk payload that does not decode. The C++ prints its container decoder's own message here; the
    /// Rust container's texts differ from those (they are W1-LZO's), so this line's stderr differs from the C++.
    Container(ContainerError),
    /// "zero asset array stride"
    ZeroStride,
    /// "negative asset array count"
    NegativeCount,
    /// "asset array exceeds payload"
    ArrayPastPayload,
    /// "invalid bulk array stride"
    BulkArrayStride,
    /// "non-finite asset coordinate"
    NonFiniteCoordinate,
    /// "non-finite half UV"
    NonFiniteHalf,
    /// "cannot open asset output"
    OpenOutput,
    /// "asset output write failed"
    WriteOutput,
    /// The `--all-mips` directory could not be created. The C++ prints the `std::filesystem_error` text of its
    /// runtime here, which this text does not reproduce (UNVERIFIED wording, no test asserts it).
    CreateMipDirectory,
    /// "invalid PNG dimensions"
    PngDimensions,
    /// "PNG pixel buffer size mismatch"
    PngBufferSize,
    /// "invalid texture dimensions or exceeds 256 MiB decoded limit"
    TextureDimensions,
    /// "A8R8G8B8 mip byte count mismatch"
    ArgbSize,
    /// "G8 mip byte count mismatch"
    G8Size,
    /// "DXT mip byte count mismatch"
    DxtSize,
    /// "negative texture property size"
    NegativePropertySize,
    /// "texture property exceeds its payload"
    PropertyPastPayload,
    /// "export is not Texture2D or TerrainWeightMapTexture"
    NotTexture,
    /// "texture importer supports PF_DXT1/PF_DXT5/PF_A8R8G8B8/PF_G8"
    UnsupportedFormat,
    /// "TerrainWeightMapTexture importer supports PF_G8 only"
    TerrainFormat,
    /// "invalid texture mip count"
    MipCount,
    /// "unsupported texture bulk codec/flags"
    BulkFlags,
    /// "invalid texture mip dimensions"
    MipDimensions,
    /// "inline texture mip exceeds export payload"
    InlineMipPastPayload,
    /// "texture bulk decoded size mismatch"
    DecodedSize,
    /// "texture mip compressed data exceeds importer limit"
    CompressedLimit,
    /// "texture mip decoded data exceeds importer limit"
    DecodedLimit,
    /// "no resident texture mip"
    NoResidentMip,
    /// "selected texture mip is unavailable"
    MipUnavailable,
    /// "invalid/missing TFC cache name"
    TfcName,
    /// "cannot open texture cache"
    TfcOpen,
    /// "TFC mip exceeds cache"
    TfcRange,
    /// "TFC mip too large"
    TfcTooLarge,
    /// "TFC read failed"
    TfcRead,
    /// "export is not StaticMesh"
    NotStaticMesh,
    /// "source mesh LOD data unsupported"
    SourceLod,
    /// "invalid mesh LOD count"
    LodCount,
    /// "PS3 mesh section unsupported"
    Ps3Section,
    /// "invalid position stream"
    PositionStream,
    /// "invalid position bulk stride"
    PositionBulkStride,
    /// "position count mismatch"
    PositionCount,
    /// "invalid UV stream"
    UvStream,
    /// "UV bulk stride mismatch"
    UvBulkStride,
    /// "UV bulk count mismatch"
    UvBulkCount,
    /// "invalid color stream"
    ColorStream,
    /// "color bulk stride mismatch"
    ColorBulkStride,
    /// "color bulk count mismatch"
    ColorBulkCount,
    /// "LOD vertex count mismatch"
    LodVertexCount,
    /// "unsupported mesh index width"
    IndexWidth,
    /// "non-triangle index count"
    NonTriangle,
    /// "mesh index out of range"
    IndexRange,
    /// "invalid mesh section range"
    SectionRange,
    /// "selected mesh LOD is unavailable"
    LodUnavailable,
    /// "mesh vertex has no UV set"
    NoUvSet,
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use AssetError as E;
        let text = match self {
            E::Read(e) => return e.fmt(f),
            E::Package(e) => return e.fmt(f),
            E::Props(e) => return e.fmt(f),
            E::Container(e) => return e.fmt(f),
            E::ZeroStride => "zero asset array stride",
            E::NegativeCount => "negative asset array count",
            E::ArrayPastPayload => "asset array exceeds payload",
            E::BulkArrayStride => "invalid bulk array stride",
            E::NonFiniteCoordinate => "non-finite asset coordinate",
            E::NonFiniteHalf => "non-finite half UV",
            E::OpenOutput => "cannot open asset output",
            E::WriteOutput => "asset output write failed",
            E::CreateMipDirectory => "cannot create the all-mips directory",
            E::PngDimensions => "invalid PNG dimensions",
            E::PngBufferSize => "PNG pixel buffer size mismatch",
            E::TextureDimensions => "invalid texture dimensions or exceeds 256 MiB decoded limit",
            E::ArgbSize => "A8R8G8B8 mip byte count mismatch",
            E::G8Size => "G8 mip byte count mismatch",
            E::DxtSize => "DXT mip byte count mismatch",
            E::NegativePropertySize => "negative texture property size",
            E::PropertyPastPayload => "texture property exceeds its payload",
            E::NotTexture => "export is not Texture2D or TerrainWeightMapTexture",
            E::UnsupportedFormat => "texture importer supports PF_DXT1/PF_DXT5/PF_A8R8G8B8/PF_G8",
            E::TerrainFormat => "TerrainWeightMapTexture importer supports PF_G8 only",
            E::MipCount => "invalid texture mip count",
            E::BulkFlags => "unsupported texture bulk codec/flags",
            E::MipDimensions => "invalid texture mip dimensions",
            E::InlineMipPastPayload => "inline texture mip exceeds export payload",
            E::DecodedSize => "texture bulk decoded size mismatch",
            E::CompressedLimit => "texture mip compressed data exceeds importer limit",
            E::DecodedLimit => "texture mip decoded data exceeds importer limit",
            E::NoResidentMip => "no resident texture mip",
            E::MipUnavailable => "selected texture mip is unavailable",
            E::TfcName => "invalid/missing TFC cache name",
            E::TfcOpen => "cannot open texture cache",
            E::TfcRange => "TFC mip exceeds cache",
            E::TfcTooLarge => "TFC mip too large",
            E::TfcRead => "TFC read failed",
            E::NotStaticMesh => "export is not StaticMesh",
            E::SourceLod => "source mesh LOD data unsupported",
            E::LodCount => "invalid mesh LOD count",
            E::Ps3Section => "PS3 mesh section unsupported",
            E::PositionStream => "invalid position stream",
            E::PositionBulkStride => "invalid position bulk stride",
            E::PositionCount => "position count mismatch",
            E::UvStream => "invalid UV stream",
            E::UvBulkStride => "UV bulk stride mismatch",
            E::UvBulkCount => "UV bulk count mismatch",
            E::ColorStream => "invalid color stream",
            E::ColorBulkStride => "color bulk stride mismatch",
            E::ColorBulkCount => "color bulk count mismatch",
            E::LodVertexCount => "LOD vertex count mismatch",
            E::IndexWidth => "unsupported mesh index width",
            E::NonTriangle => "non-triangle index count",
            E::IndexRange => "mesh index out of range",
            E::SectionRange => "invalid mesh section range",
            E::LodUnavailable => "selected mesh LOD is unavailable",
            E::NoUvSet => "mesh vertex has no UV set",
        };
        f.write_str(text)
    }
}

impl std::error::Error for AssetError {
    /// The wrapped error prints its own text as this error's text, so its cause (if any) is this error's cause.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AssetError::Package(e) => e.source(),
            AssetError::Props(e) => e.source(),
            AssetError::Container(e) => e.source(),
            _ => None,
        }
    }
}

impl From<ReadError> for AssetError {
    fn from(e: ReadError) -> Self {
        AssetError::Read(e)
    }
}

impl From<PackageError> for AssetError {
    fn from(e: PackageError) -> Self {
        AssetError::Package(e)
    }
}

impl From<PropError> for AssetError {
    fn from(e: PropError) -> Self {
        AssetError::Props(e)
    }
}

impl From<ContainerError> for AssetError {
    fn from(e: ContainerError) -> Self {
        AssetError::Container(e)
    }
}

/// Fails with `error` unless `condition` holds (the C++ `check`).
pub(crate) fn check(condition: bool, error: AssetError) -> Result<(), AssetError> {
    if condition { Ok(()) } else { Err(error) }
}

/// One byte at the cursor (C++ `takeByte`).
pub(crate) fn take_byte(r: &mut Reader<'_>) -> Result<u8, AssetError> {
    Ok(r.take(1)?[0])
}

/// A little-endian `u16` at the cursor (C++ `u16`).
pub(crate) fn read_u16(r: &mut Reader<'_>) -> Result<u16, AssetError> {
    let b = r.take(2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

/// A finite `f32` at the cursor (C++ `real`).
pub(crate) fn real(r: &mut Reader<'_>) -> Result<f32, AssetError> {
    let value = f32::from_bits(r.u32()?);
    check(value.is_finite(), AssetError::NonFiniteCoordinate)?;
    Ok(value)
}

/// An `i32` element count whose `stride`-byte elements fit between the cursor and the reader's end (C++ `count`).
pub(crate) fn count(r: &mut Reader<'_>, stride: usize) -> Result<usize, AssetError> {
    check(stride != 0, AssetError::ZeroStride)?;
    let number = r.i32()?;
    check(number >= 0, AssetError::NegativeCount)?;
    let number = number as usize;
    let end = r.bytes().len().min(r.limit());
    check(
        r.pos() <= end && number <= (end - r.pos()) / stride,
        AssetError::ArrayPastPayload,
    )?;
    Ok(number)
}

/// Skips a bulk array: an element size of 1 to 1024 bytes, then a [`count`] of elements (C++ `bulkArraySkip`).
pub(crate) fn bulk_array_skip(r: &mut Reader<'_>) -> Result<(), AssetError> {
    let stride = r.u32()? as usize;
    check(stride > 0 && stride <= 1024, AssetError::BulkArrayStride)?;
    let number = count(r, stride)?;
    r.skip(number * stride)?;
    Ok(())
}

/// Writes `bytes` to `path`, created or truncated (C++ `write`, a binary-mode `ofstream`).
pub(crate) fn write_output(path: &Path, bytes: &[u8]) -> Result<(), AssetError> {
    use std::io::Write as _;
    let mut file = std::fs::File::create(path).map_err(|_| AssetError::OpenOutput)?;
    file.write_all(bytes).map_err(|_| AssetError::WriteOutput)?;
    file.flush().map_err(|_| AssetError::WriteOutput)
}

/// The tag walk of `Package::properties` that the C++ readers run before the native data, for its checks and its
/// end: the range checks, the export's path, then the walk with `typer`. Returns the absolute position after the
/// `None` tag and the payload's end.
pub(crate) fn walk_properties(
    package: &Package,
    index: i32,
    property_offset: usize,
    typer: &dyn ArrayTyper,
) -> Result<(usize, usize), AssetError> {
    let (_, object_end) = props::property_range(package, index, property_offset)?;
    package.path_of(index)?;
    let walked = props::read_tagged(package, index, property_offset, typer)?;
    let base = package.object(index)?.offset as usize;
    Ok((base + walked.end, object_end))
}

/// The class name of export or import `index` (the class object's name; empty for a class itself), as the C++
/// readers compare it.
pub(crate) fn class_name(package: &Package, index: i32) -> Result<String, AssetError> {
    let object = package.object(index)?;
    if object.class == 0 {
        return Ok(String::new());
    }
    Ok(package.object(object.class)?.name.clone())
}

/// The pixel formats the importer decodes.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Dxt1,
    Dxt5,
    A8R8G8B8,
    G8,
}

impl PixelFormat {
    fn parse(name: &str) -> Option<PixelFormat> {
        Some(match name {
            "PF_DXT1" => PixelFormat::Dxt1,
            "PF_DXT5" => PixelFormat::Dxt5,
            "PF_A8R8G8B8" => PixelFormat::A8R8G8B8,
            "PF_G8" => PixelFormat::G8,
            _ => return None,
        })
    }

    /// The `EPixelFormat` name the package stores.
    pub fn name(self) -> &'static str {
        match self {
            PixelFormat::Dxt1 => "PF_DXT1",
            PixelFormat::Dxt5 => "PF_DXT5",
            PixelFormat::A8R8G8B8 => "PF_A8R8G8B8",
            PixelFormat::G8 => "PF_G8",
        }
    }
}

/// Bulk data flags of a mip record: streamed from a TFC file.
pub const BULK_STREAMED: u32 = 1;
/// Bulk data flag: LZO-compressed (decoded as a package container).
pub const BULK_COMPRESSED: u32 = 16;
/// Bulk data flag: no data stored (the mip is skipped).
pub const BULK_UNUSED: u32 = 32;
/// Every flag the importer accepts (8 is accepted and has no effect, as in the C++).
const BULK_KNOWN: u32 = 1 | 8 | 16 | 32;

/// One decoded mip. Not kept yet: the raw DXT block bytes (for a BC1/BC3 upload); fields may be added.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureMip {
    /// The mip's position among the serialized records.
    pub index: usize,
    pub flags: u32,
    pub decoded_bytes: u32,
    pub stored_bytes: u32,
    pub offset: u32,
    pub width: u32,
    pub height: u32,
    /// `width * height` pixels, 4 bytes each, R, G, B, A, rows top first.
    pub rgba: Vec<u8>,
}

impl TextureMip {
    /// Whether the mip's bytes came from a TFC file.
    pub fn streamed(&self) -> bool {
        self.flags & BULK_STREAMED != 0
    }
}

/// A texture export with its resident mips. Fields may be added (non-exhaustive) as the host needs them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureAsset {
    pub path: String,
    pub format: PixelFormat,
    /// The `TextureFileCacheName` property (empty when absent).
    pub cache_name: String,
    pub serialized_mip_count: usize,
    /// The mips with data, in serialized order (records flagged unused or with no decoded bytes are left out).
    pub mips: Vec<TextureMip>,
}

/// A serialized mip record before its data is read.
struct MipRecord {
    index: usize,
    flags: u32,
    decoded_bytes: u32,
    stored_bytes: u32,
    offset: u32,
    width: u32,
    height: u32,
    inline_at: usize,
}

const MIB: usize = 1024 * 1024;

/// Reads texture export `index`: its tags from `property_offset` (walked with `typer` first, as the C++ does), its
/// mip records, and the data of every resident mip, from the package or from the TFC files under `tfc_root`.
pub fn read_texture(
    package: &Package,
    index: i32,
    property_offset: usize,
    typer: &dyn ArrayTyper,
    tfc_root: &Path,
) -> Result<TextureAsset, AssetError> {
    let class = class_name(package, index)?;
    check(
        class == "Texture2D" || class == "TerrainWeightMapTexture",
        AssetError::NotTexture,
    )?;
    let (native_at, object_end) = walk_properties(package, index, property_offset, typer)?;
    let mut r = package.reader();
    let (format, cache) = texture_properties(package, &mut r, index, property_offset, object_end)?;
    let format = PixelFormat::parse(&format).ok_or(AssetError::UnsupportedFormat)?;
    check(
        class != "TerrainWeightMapTexture" || format == PixelFormat::G8,
        AssetError::TerrainFormat,
    )?;

    r.set_pos(native_at);
    r.set_limit(object_end);
    r.skip(16)?;
    let serialized = count(&mut r, 24)?;
    check(serialized > 0 && serialized <= 64, AssetError::MipCount)?;
    let mut records = Vec::with_capacity(serialized);
    for i in 0..serialized {
        let flags = r.u32()?;
        let decoded_bytes = r.u32()?;
        let stored_bytes = r.u32()?;
        let offset = r.u32()?;
        let inline_at = r.pos();
        check(flags & !BULK_KNOWN == 0, AssetError::BulkFlags)?;
        if flags & BULK_STREAMED == 0 && flags & BULK_UNUSED == 0 {
            r.skip(stored_bytes as usize)?;
        }
        let width = r.u32()?;
        let height = r.u32()?;
        check(width != 0 && height != 0, AssetError::MipDimensions)?;
        records.push(MipRecord {
            index: i,
            flags,
            decoded_bytes,
            stored_bytes,
            offset,
            width,
            height,
            inline_at,
        });
    }

    let mut asset = TextureAsset {
        path: package.path_of(index)?,
        format,
        cache_name: cache,
        serialized_mip_count: serialized,
        mips: Vec::new(),
    };
    let tfc = TfcStore::new(tfc_root);
    let mut total_decoded = 0usize;
    for record in &records {
        if record.flags & BULK_UNUSED != 0 || record.decoded_bytes == 0 {
            continue;
        }
        let mut payload = if record.flags & BULK_STREAMED != 0 {
            tfc.read(&asset.cache_name, record.offset, record.stored_bytes)?
        } else {
            let stored = record.stored_bytes as usize;
            check(
                record.inline_at <= object_end && stored <= object_end - record.inline_at,
                AssetError::InlineMipPastPayload,
            )?;
            package.data[record.inline_at..record.inline_at + stored].to_vec()
        };
        if record.flags & BULK_COMPRESSED != 0 {
            payload = ow_container::decode_package(&payload)?;
        }
        check(
            payload.len() == record.decoded_bytes as usize,
            AssetError::DecodedSize,
        )?;
        check(payload.len() <= 256 * MIB, AssetError::CompressedLimit)?;
        let rgba = match format {
            PixelFormat::G8 => grayscale(&payload, record.width, record.height)?,
            PixelFormat::A8R8G8B8 => argb(payload, record.width, record.height)?,
            PixelFormat::Dxt1 => dxt(&payload, record.width, record.height, false)?,
            PixelFormat::Dxt5 => dxt(&payload, record.width, record.height, true)?,
        };
        check(
            rgba.len() <= 512 * MIB && total_decoded <= 512 * MIB - rgba.len(),
            AssetError::DecodedLimit,
        )?;
        total_decoded += rgba.len();
        asset.mips.push(TextureMip {
            index: record.index,
            flags: record.flags,
            decoded_bytes: record.decoded_bytes,
            stored_bytes: record.stored_bytes,
            offset: record.offset,
            width: record.width,
            height: record.height,
            rgba,
        });
    }
    check(!asset.mips.is_empty(), AssetError::NoResidentMip)?;
    Ok(asset)
}

/// Writes `mip` as an RGBA PNG (C++ `writeTexturePng`).
pub fn write_texture_png(mip: &TextureMip, output: &Path) -> Result<(), AssetError> {
    let bytes = png::encode(mip.width, mip.height, &mip.rgba)?;
    write_output(output, &bytes)
}

/// The `Format` (ByteProperty) and `TextureFileCacheName` (NameProperty) tags, read by the C++ importer's own
/// second walk of the tag stream (C++ `textureProperties`): each tag is skipped by its size, with the struct or enum
/// name and the bool byte read as the tag header holds them. A later tag of the same name replaces an earlier one.
fn texture_properties(
    package: &Package,
    r: &mut Reader<'_>,
    index: i32,
    property_offset: usize,
    object_end: usize,
) -> Result<(String, String), AssetError> {
    let object = package.object(index)?;
    r.set_pos(object.offset as usize + property_offset);
    r.set_limit(object_end);
    let mut format = String::new();
    let mut cache = String::new();
    loop {
        let property = read_name(r, &package.names)?;
        if property == "None" {
            break;
        }
        let ty = read_name(r, &package.names)?;
        let size = r.i32()?;
        check(size >= 0, AssetError::NegativePropertySize)?;
        r.i32()?;
        if ty == "StructProperty" || ty == "ByteProperty" {
            read_name(r, &package.names)?;
        }
        if ty == "BoolProperty" {
            r.skip(1)?;
        }
        let size = size as usize;
        r.require(size)?;
        let end = r.pos() + size;
        r.set_limit(end);
        if property == "Format" && ty == "ByteProperty" {
            format = read_name(r, &package.names)?;
        }
        if property == "TextureFileCacheName" && ty == "NameProperty" {
            cache = read_name(r, &package.names)?;
        }
        check(r.pos() <= end, AssetError::PropertyPastPayload)?;
        r.set_pos(end);
        r.set_limit(object_end);
    }
    Ok((format, cache))
}

/// The importer's size bound: 1 to 16384 pixels each way and at most 64 Mi pixels (C++ `textureDimensions`).
fn texture_dimensions(width: u32, height: u32) -> Result<(), AssetError> {
    check(
        width != 0
            && height != 0
            && width <= 16384
            && height <= 16384
            && u64::from(width) * u64::from(height) <= 64 * 1024 * 1024,
        AssetError::TextureDimensions,
    )
}

/// PF_A8R8G8B8: little-endian words store B, G, R, A; the PNG wants R, G, B, A (C++ `argb`).
pub fn argb(mut data: Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, AssetError> {
    texture_dimensions(width, height)?;
    check(
        data.len() == width as usize * height as usize * 4,
        AssetError::ArgbSize,
    )?;
    for pixel in data.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok(data)
}

/// PF_G8: each byte is R, G and B, alpha 255 (C++ `grayscale`).
pub fn grayscale(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, AssetError> {
    texture_dimensions(width, height)?;
    check(
        data.len() == width as usize * height as usize,
        AssetError::G8Size,
    )?;
    let mut pixels = Vec::with_capacity(data.len() * 4);
    for &value in data {
        pixels.extend_from_slice(&[value, value, value, 255]);
    }
    Ok(pixels)
}

/// PF_DXT1 and PF_DXT5 blocks to RGBA (C++ `dxt`): 4x4 blocks in rows, the partial blocks of an edge cut off. The
/// integer rounding of the colour expansion and the palette is the C++ importer's.
pub fn dxt(data: &[u8], width: u32, height: u32, dxt5: bool) -> Result<Vec<u8>, AssetError> {
    let block_bytes = if dxt5 { 16 } else { 8 };
    texture_dimensions(width, height)?;
    check(
        data.len() == width.div_ceil(4) as usize * height.div_ceil(4) as usize * block_bytes,
        AssetError::DxtSize,
    )?;
    let (w, h) = (width as usize, height as usize);
    let mut pixels = vec![0u8; w * h * 4];
    let mut r = Reader::new(data);
    for by in (0..h).step_by(4) {
        for bx in (0..w).step_by(4) {
            let mut alpha = [0u32; 8];
            let mut alpha_bits = 0u64;
            if dxt5 {
                alpha[0] = u32::from(take_byte(&mut r)?);
                alpha[1] = u32::from(take_byte(&mut r)?);
                for i in 0..6 {
                    alpha_bits |= u64::from(take_byte(&mut r)?) << (8 * i);
                }
                if alpha[0] > alpha[1] {
                    for i in 1..=6u32 {
                        alpha[i as usize + 1] = ((7 - i) * alpha[0] + i * alpha[1]) / 7;
                    }
                } else {
                    for i in 1..=4u32 {
                        alpha[i as usize + 1] = ((5 - i) * alpha[0] + i * alpha[1]) / 5;
                    }
                    alpha[6] = 0;
                    alpha[7] = 255;
                }
            }
            let color0 = read_u16(&mut r)?;
            let color1 = read_u16(&mut r)?;
            let mut colors = [[0u32; 4]; 4];
            for (i, color) in [color0, color1].into_iter().enumerate() {
                let c = u32::from(color);
                colors[i] = [
                    ((c >> 11) * 255 + 15) / 31,
                    (((c >> 5) & 63) * 255 + 31) / 63,
                    ((c & 31) * 255 + 15) / 31,
                    255,
                ];
            }
            let (c0, c1) = (colors[0], colors[1]);
            if color0 > color1 || dxt5 {
                colors[2] = std::array::from_fn(|k| (2 * c0[k] + c1[k]) / 3);
                colors[3] = std::array::from_fn(|k| (c0[k] + 2 * c1[k]) / 3);
            } else {
                colors[2] = std::array::from_fn(|k| (c0[k] + c1[k]) / 2);
                colors[3] = [0; 4];
            }
            let bits = r.u32()?;
            for i in 0..16usize {
                let mut color = colors[((bits >> (i * 2)) & 3) as usize];
                if dxt5 {
                    color[3] = alpha[((alpha_bits >> (i * 3)) & 7) as usize];
                }
                let (x, y) = (bx + i % 4, by + i / 4);
                if x < w && y < h {
                    let at = (y * w + x) * 4;
                    for k in 0..4 {
                        // Every palette entry is 0..=255 by construction.
                        pixels[at + k] = color[k] as u8;
                    }
                }
            }
        }
    }
    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ow_package::schema::SchemaTyper;
    use ow_package::testkit::{PackageBuilder, words};

    /// A Texture2D-like export (invented values): a 4-byte prefix, a `Format` and a `TextureFileCacheName` tag,
    /// `None`, 16 bytes, then the mip records (`assets_test.py`'s `texture_mips`).
    fn texture_package(class: &str, format: &str, mips: &[(&[u8], u32, u32, u32)]) -> Package {
        let mut b = PackageBuilder::new_empty();
        b.names(["None"]);
        let cls = b.add_import("Core", "Class", 0, class);
        let mut payload = words(&[0]);
        let detail = b.fname("EPixelFormat", 0);
        let value = b.fname(format, 0);
        payload.extend(b.tag_detail("Format", "ByteProperty", &detail, &value));
        let cache = b.fname("TestCache", 0);
        payload.extend(b.tag("TextureFileCacheName", "NameProperty", &cache));
        payload.extend(b.none());
        payload.extend(vec![0u8; 16]);
        payload.extend(words(&[mips.len() as u32]));
        for (block, flags, width, height) in mips {
            payload.extend(words(&[*flags, block.len() as u32, block.len() as u32, 0]));
            if flags & (BULK_STREAMED | BULK_UNUSED) == 0 {
                payload.extend_from_slice(block);
            }
            payload.extend(words(&[*width, *height]));
        }
        b.add_export(cls, "Probe", payload);
        Package::from_decoded(Default::default(), "probe".into(), b.build()).unwrap()
    }

    fn read(package: &Package) -> Result<TextureAsset, AssetError> {
        read_texture(package, 1, 4, &SchemaTyper::new_empty(), Path::new("."))
    }

    #[test]
    fn dxt1_red_and_transparent_blocks() {
        let red = [0x00, 0xf8, 0, 0, 0, 0, 0, 0];
        assert_eq!(dxt(&red, 4, 4, false).unwrap(), [255, 0, 0, 255].repeat(16));
        // color0 <= color1 with index 3 everywhere: transparent black.
        let clear = [0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        assert_eq!(dxt(&clear, 4, 4, false).unwrap(), vec![0u8; 64]);
        // A 2x3 image cuts the block's edge off.
        assert_eq!(dxt(&red, 2, 3, false).unwrap(), [255, 0, 0, 255].repeat(6));
        assert_eq!(dxt(&red[..7], 4, 4, false), Err(AssetError::DxtSize));
    }

    #[test]
    fn dxt5_alpha_palette() {
        let mut block = vec![64, 0, 0, 0, 0, 0, 0, 0];
        block.extend_from_slice(&[0x00, 0xf8, 0, 0, 0, 0, 0, 0]);
        assert_eq!(dxt(&block, 4, 4, true).unwrap(), [255, 0, 0, 64].repeat(16));
        // Index 2 of an 8-entry ramp from 70 down to 0: (6*70 + 0) / 7 = 60.
        let mut ramp = vec![70, 0, 0b0000_0010, 0, 0, 0, 0, 0];
        ramp.extend_from_slice(&[0x00, 0xf8, 0, 0, 0, 0, 0, 0]);
        let pixels = dxt(&ramp, 4, 4, true).unwrap();
        assert_eq!(pixels[3], 60);
        assert_eq!(pixels[7], 70);
    }

    #[test]
    fn argb_and_g8_swizzle() {
        assert_eq!(argb(vec![3, 17, 241, 0], 1, 1).unwrap(), [241, 17, 3, 0]);
        assert_eq!(argb(vec![3, 17, 241], 1, 1), Err(AssetError::ArgbSize));
        assert_eq!(
            grayscale(&[0, 37], 2, 1).unwrap(),
            [0, 0, 0, 255, 37, 37, 37, 255]
        );
        assert_eq!(
            grayscale(&[0], 16385, 1),
            Err(AssetError::TextureDimensions)
        );
        assert_eq!(
            grayscale(&[0], 16384, 16384),
            Err(AssetError::TextureDimensions)
        );
    }

    #[test]
    fn textures_read_their_resident_mips() {
        let red: &[u8] = &[0x00, 0xf8, 0, 0, 0, 0, 0, 0];
        let green: &[u8] = &[0xe0, 0x07, 0, 0, 0, 0, 0, 0];
        let package = texture_package(
            "Texture2D",
            "PF_DXT1",
            &[(red, 0, 4, 4), (green, 0, 4, 4), (red, 32, 4, 4)],
        );
        let asset = read(&package).unwrap();
        assert_eq!(
            (asset.path.as_str(), asset.format, asset.cache_name.as_str()),
            ("Probe", PixelFormat::Dxt1, "TestCache")
        );
        assert_eq!((asset.serialized_mip_count, asset.mips.len()), (3, 2));
        assert_eq!(asset.mips[1].rgba, [0, 255, 0, 255].repeat(16));
        assert!(!asset.mips[0].streamed());
    }

    #[test]
    fn texture_refusals_carry_the_cxx_texts() {
        let gray: &[u8] = &[0, 37, 128, 255];
        let cases: Vec<(Package, AssetError)> = vec![
            (
                texture_package("TerrainWeightMapTexture", "PF_A8R8G8B8", &[(gray, 0, 1, 1)]),
                AssetError::TerrainFormat,
            ),
            (
                texture_package("Texture2D", "PF_Unknown", &[(gray, 0, 2, 2)]),
                AssetError::UnsupportedFormat,
            ),
            (
                texture_package("StaticMesh", "PF_G8", &[(gray, 0, 2, 2)]),
                AssetError::NotTexture,
            ),
            (
                texture_package("Texture2D", "PF_G8", &[(gray, 2, 2, 2)]),
                AssetError::BulkFlags,
            ),
            (
                texture_package("Texture2D", "PF_G8", &[(gray, 0, 0, 2)]),
                AssetError::MipDimensions,
            ),
            (
                texture_package("Texture2D", "PF_G8", &[(gray, 0, 3, 2)]),
                AssetError::G8Size,
            ),
            (
                texture_package("Texture2D", "PF_G8", &[]),
                AssetError::MipCount,
            ),
            (
                texture_package("Texture2D", "PF_G8", &[(gray, 32, 2, 2)]),
                AssetError::NoResidentMip,
            ),
        ];
        for (package, expected) in cases {
            assert_eq!(read(&package), Err(expected.clone()), "{expected}");
        }
        assert_eq!(
            AssetError::TerrainFormat.to_string(),
            "TerrainWeightMapTexture importer supports PF_G8 only"
        );
    }

    #[test]
    fn counts_stay_inside_the_reader() {
        let data = words(&[2, 0, 0]);
        assert_eq!(count(&mut Reader::new(&data), 4), Ok(2));
        assert_eq!(
            count(&mut Reader::new(&data), 5),
            Err(AssetError::ArrayPastPayload)
        );
        assert_eq!(
            count(&mut Reader::new(&data).with_limit(8), 4),
            Err(AssetError::ArrayPastPayload)
        );
        assert_eq!(
            count(&mut Reader::new(&words(&[u32::MAX])), 1),
            Err(AssetError::NegativeCount)
        );
        assert_eq!(
            count(&mut Reader::new(&data), 0),
            Err(AssetError::ZeroStride)
        );
    }
}
