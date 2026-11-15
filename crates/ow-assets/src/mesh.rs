//! Owner: lane W2-ASSETS. Static mesh reads: the port of `assets::readMesh` and `readLod` (`src/assets.cpp`).
//! `StaticMesh` and `WiringMesh` exports (GearboxFramework's generated cables, which serialize with the StaticMesh
//! layout: DECISIONS 2026-10-21): the native header after the tags, then every LOD's sections, positions, normals
//! and UVs, colours and indices.
//!
//! The layout is the C++ reader's, recovered from the packages and checked with structural oracles there (sizes
//! meet, indices stay below the vertex count, section ranges fit); nothing is re-derived (AGENTS.md rule 4). Which
//! of a doubled colour stream's entries the renderer binds is UNVERIFIED (colours are not exported), as in the C++.

use ow_package::package::Package;
use ow_package::props::ArrayTyper;
use ow_package::reader::Reader;

use crate::texture::{
    AssetError, bulk_array_skip, check, class_name, count, read_u16, real, walk_properties,
};

/// One vertex of a LOD. Not kept yet: the TangentX word the reader skips; fields may be added.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshVertex {
    pub position: [f32; 3],
    /// The packed tangent-Z normal, each byte mapped to `b / 127.5 - 1`.
    pub normal: [f32; 3],
    /// One (U, V) per UV set.
    pub uvs: Vec<[f32; 2]>,
    pub color: [u8; 4],
    pub has_color: bool,
}

/// A section: a material and a run of triangles. Not kept yet: the section's skipped words and fragment list; fields
/// may be added.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeshSection {
    /// The material reference (0: none).
    pub material: i32,
    /// The material's object path, or `None`.
    pub material_path: String,
    pub first_index: u32,
    pub face_count: u32,
    pub min_vertex: u32,
    pub max_vertex: u32,
}

/// One level of detail. Not kept yet: the extra entries of a doubled colour stream; fields may be added.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshLod {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub sections: Vec<MeshSection>,
    /// 2 or 4 bytes per stored index.
    pub index_width: u32,
    pub uv_set_count: usize,
}

/// A static mesh export. Not kept yet: the native header and bulk arrays before the LODs; fields may be added.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshAsset {
    pub path: String,
    /// The `RB_BodySetup` reference (0: none).
    pub body_setup: i32,
    pub lods: Vec<MeshLod>,
}

/// Reads static mesh export `index`: its tags from `property_offset` (walked with `typer` for their checks and
/// end, as the C++ does), then the native data and every LOD.
pub fn read_mesh(
    package: &Package,
    index: i32,
    property_offset: usize,
    typer: &dyn ArrayTyper,
) -> Result<MeshAsset, AssetError> {
    let class = class_name(package, index)?;
    check(
        class == "StaticMesh" || class == "WiringMesh",
        AssetError::NotStaticMesh,
    )?;
    let (native_at, object_end) = walk_properties(package, index, property_offset, typer)?;
    let mut r = package.reader();
    r.set_pos(native_at);
    r.set_limit(object_end);
    r.skip(28)?;
    let body = r.i32()?;
    if body != 0 {
        package.object(body)?;
    }
    r.skip(24)?;
    bulk_array_skip(&mut r)?;
    bulk_array_skip(&mut r)?;
    r.u32()?;
    check(r.u32()? == 0, AssetError::SourceLod)?;
    let skipped = count(&mut r, 7)?;
    r.skip(skipped * 7)?;
    r.u32()?;
    let lod_count = count(&mut r, 16)?;
    check(lod_count > 0 && lod_count <= 32, AssetError::LodCount)?;

    let mut asset = MeshAsset {
        path: package.path_of(index)?,
        body_setup: body,
        lods: Vec::with_capacity(lod_count),
    };
    for _ in 0..lod_count {
        asset.lods.push(read_lod(package, &mut r)?);
    }
    Ok(asset)
}

/// A half-precision UV value (C++ `half`): Inf and NaN are refused. The result is exact: a power-of-two scale of
/// an integer of at most 11 bits, as the C++ `ldexp` gives it.
fn half(value: u16) -> Result<f32, AssetError> {
    let exponent = i32::from((value >> 10) & 31);
    let mantissa = f32::from(value & 1023);
    check(exponent != 31, AssetError::NonFiniteHalf)?;
    let sign = if value & 32768 != 0 { -1.0f32 } else { 1.0 };
    let magnitude = if exponent != 0 {
        (1024.0 + mantissa) * power_of_two(exponent - 25)
    } else {
        mantissa * power_of_two(-24)
    };
    Ok(sign * magnitude)
}

/// 2^`e` for `e` in -24..=5, built from its bits (exact; no float library call).
fn power_of_two(e: i32) -> f32 {
    f32::from_bits(((e + 127) as u32) << 23)
}

/// One LOD (C++ `readLod`).
fn read_lod(package: &Package, r: &mut Reader<'_>) -> Result<MeshLod, AssetError> {
    let mut lod = MeshLod::default();
    let flags = r.u32()?;
    r.u32()?;
    let stored = r.u32()?;
    r.u32()?;
    if flags & 1 == 0 && flags & 32 == 0 {
        r.skip(stored as usize)?;
    }

    let section_count = count(r, 41)?;
    lod.sections.reserve(section_count);
    for _ in 0..section_count {
        let material = r.i32()?;
        if material != 0 {
            package.object(material)?;
        }
        let material_path = if material != 0 {
            package.path_of(material)?
        } else {
            String::from("None")
        };
        r.skip(12)?;
        let first_index = r.u32()?;
        let face_count = r.u32()?;
        let min_vertex = r.u32()?;
        let max_vertex = r.u32()?;
        r.u32()?;
        let skipped = count(r, 8)?;
        r.skip(skipped * 8)?;
        check(r.take(1)?[0] == 0, AssetError::Ps3Section)?;
        lod.sections.push(MeshSection {
            material,
            material_path,
            first_index,
            face_count,
            min_vertex,
            max_vertex,
        });
    }

    let position_stride = r.u32()?;
    let vertex_count = r.u32()?;
    check(
        position_stride == 12 && vertex_count > 0,
        AssetError::PositionStream,
    )?;
    check(r.u32()? == 12, AssetError::PositionBulkStride)?;
    check(
        count(r, 12)? == vertex_count as usize,
        AssetError::PositionCount,
    )?;
    lod.vertices = vec![MeshVertex::default(); vertex_count as usize];
    for vertex in &mut lod.vertices {
        vertex.position = [real(r)?, real(r)?, real(r)?];
    }

    let uv_sets = r.u32()?;
    let uv_stride = r.u32()?;
    let uv_count = r.u32()?;
    let full_precision = r.u32()?;
    check(
        (1..=8).contains(&uv_sets)
            && full_precision <= 1
            && uv_count == vertex_count
            && u64::from(uv_stride)
                == 8 + u64::from(uv_sets) * if full_precision != 0 { 8 } else { 4 },
        AssetError::UvStream,
    )?;
    check(r.u32()? == uv_stride, AssetError::UvBulkStride)?;
    check(
        count(r, uv_stride as usize)? == vertex_count as usize,
        AssetError::UvBulkCount,
    )?;
    lod.uv_set_count = uv_sets as usize;
    for vertex in &mut lod.vertices {
        r.skip(4)?;
        let packed = r.take(4)?;
        vertex.normal = [
            f32::from(packed[0]) / 127.5 - 1.0,
            f32::from(packed[1]) / 127.5 - 1.0,
            f32::from(packed[2]) / 127.5 - 1.0,
        ];
        vertex.uvs.reserve(uv_sets as usize);
        for _ in 0..uv_sets {
            let u = if full_precision != 0 {
                real(r)?
            } else {
                half(read_u16(r)?)?
            };
            let v = if full_precision != 0 {
                real(r)?
            } else {
                half(read_u16(r)?)?
            };
            vertex.uvs.push([u, v]);
        }
    }

    let color_stride = r.u32()?;
    let colors = r.u32()?;
    // A few cooked meshes store exactly twice as many colour entries as vertices, with a matching bulk count; only
    // the first vertex-count entries are read and the rest skipped (C++ note, DECISIONS 2026-10-21; which entries
    // the renderer binds is UNVERIFIED).
    check(
        colors == 0
            || (color_stride == 4
                && (colors == vertex_count || colors == vertex_count.wrapping_mul(2))),
        AssetError::ColorStream,
    )?;
    if colors != 0 {
        check(r.u32()? == color_stride, AssetError::ColorBulkStride)?;
        check(
            count(r, color_stride as usize)? == colors as usize,
            AssetError::ColorBulkCount,
        )?;
        for vertex in &mut lod.vertices {
            let bytes = r.take(4)?;
            vertex.color.copy_from_slice(bytes);
            vertex.has_color = true;
        }
        r.skip((colors - vertex_count) as usize * color_stride as usize)?;
    }

    check(r.u32()? == vertex_count, AssetError::LodVertexCount)?;
    lod.index_width = r.u32()?;
    check(
        lod.index_width == 2 || lod.index_width == 4,
        AssetError::IndexWidth,
    )?;
    let index_count = count(r, lod.index_width as usize)?;
    check(index_count % 3 == 0, AssetError::NonTriangle)?;
    lod.indices.reserve(index_count);
    for _ in 0..index_count {
        let value = if lod.index_width == 2 {
            u32::from(read_u16(r)?)
        } else {
            r.u32()?
        };
        check(value < vertex_count, AssetError::IndexRange)?;
        lod.indices.push(value);
    }
    let total = lod.indices.len();
    for section in &lod.sections {
        let first = section.first_index as usize;
        check(
            first <= total
                && section.face_count as usize <= (total - first) / 3
                && section.min_vertex <= section.max_vertex
                && section.max_vertex < vertex_count,
            AssetError::SectionRange,
        )?;
    }
    Ok(lod)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ow_package::schema::SchemaTyper;
    use ow_package::testkit::{PackageBuilder, words};

    fn f32s(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    /// `assets_test.py`'s `mesh()` (invented values): one LOD, one section over 3 vertices, half or full UVs, an
    /// optional colour stream, three 16-bit indices.
    pub(crate) fn mesh_payload(
        bad_index: bool,
        full: bool,
        colors: u32,
        bulk: Option<u32>,
    ) -> Vec<u8> {
        let mut b = PackageBuilder::new_empty();
        b.names(["None"]);
        let mut native = vec![0u8; 28 + 4 + 24];
        native.extend(words(&[6, 0, 8, 0, 18, 0, 0, 0, 1]));
        native.extend(words(&[0, 0, 0, 0, 1]));
        native.extend(words(&[0, 0, 0, 1, 0, 1, 0, 2, 0, 0]));
        native.push(0);
        native.extend(words(&[12, 3, 12, 3]));
        native.extend(f32s(&[0.0, 0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 100.0, 0.0]));
        let stride = if full { 16 } else { 12 };
        native.extend(words(&[1, stride, 3, u32::from(full), stride, 3]));
        for (u, v) in [(0.0f32, 0.0f32), (1.0, 0.0), (0.0, 1.0)] {
            native.extend([255, 128, 128, 255, 128, 128, 255, 255]);
            if full {
                native.extend(f32s(&[u, v]));
            } else {
                // 0.0 and 1.0 as IEEE halves.
                for x in [u, v] {
                    native.extend(if x == 0.0 { [0u8, 0] } else { [0, 0x3c] });
                }
            }
        }
        if colors != 0 {
            native.extend(words(&[4, colors, 4, bulk.unwrap_or(colors)]));
            native.extend((0..4 * colors).map(|i| i as u8));
        } else {
            native.extend(words(&[0, 0]));
        }
        native.extend(words(&[3, 2, 3]));
        for i in [0u16, 1, if bad_index { 9 } else { 2 }] {
            native.extend(i.to_le_bytes());
        }
        let mut payload = words(&[0]);
        payload.extend(b.none());
        payload.extend(native);
        payload
    }

    pub(crate) fn mesh_package(class: &str, payload: Vec<u8>) -> Package {
        let mut b = PackageBuilder::new_empty();
        b.names(["None"]);
        let cls = b.add_import("Core", "Class", 0, class);
        b.add_export(cls, "Probe", payload);
        Package::from_decoded(Default::default(), "probe".into(), b.build()).unwrap()
    }

    fn read(package: &Package) -> Result<MeshAsset, AssetError> {
        read_mesh(package, 1, 4, &SchemaTyper::new_empty())
    }

    #[test]
    fn halves_are_exact() {
        assert_eq!(half(0x3c00), Ok(1.0));
        assert_eq!(half(0xc000), Ok(-2.0));
        assert_eq!(half(0x0001), Ok(5.960_464_5e-8));
        assert_eq!(half(0x7bff), Ok(65504.0));
        assert!(half(0x8000).unwrap().is_sign_negative());
        assert_eq!(half(0x7c00), Err(AssetError::NonFiniteHalf));
    }

    #[test]
    fn a_triangle_reads_in_both_uv_precisions() {
        for full in [false, true] {
            let mesh = read(&mesh_package(
                "StaticMesh",
                mesh_payload(false, full, 0, None),
            ))
            .unwrap();
            assert_eq!(
                (mesh.path.as_str(), mesh.lods.len(), mesh.body_setup),
                ("Probe", 1, 0)
            );
            let lod = &mesh.lods[0];
            assert_eq!(
                (lod.vertices.len(), lod.indices.as_slice(), lod.index_width),
                (3, &[0, 1, 2][..], 2)
            );
            assert_eq!(lod.vertices[1].position, [100.0, 0.0, 0.0]);
            assert_eq!(lod.vertices[1].uvs, vec![[1.0, 0.0]]);
            assert_eq!(lod.sections[0].material_path, "None");
            assert_eq!(lod.vertices[0].normal[1], 128.0 / 127.5 - 1.0);
        }
        assert!(
            read(&mesh_package(
                "WiringMesh",
                mesh_payload(false, false, 0, None)
            ))
            .is_ok()
        );
    }

    #[test]
    fn colour_streams_hold_one_or_two_entries_per_vertex() {
        for colors in [3, 6] {
            let mesh = read(&mesh_package(
                "StaticMesh",
                mesh_payload(false, false, colors, None),
            ))
            .unwrap();
            assert_eq!(mesh.lods[0].vertices[2].color, [8, 9, 10, 11]);
        }
        assert_eq!(
            read(&mesh_package(
                "StaticMesh",
                mesh_payload(false, false, 5, None)
            )),
            Err(AssetError::ColorStream)
        );
        assert_eq!(
            read(&mesh_package(
                "StaticMesh",
                mesh_payload(false, false, 6, Some(3))
            )),
            Err(AssetError::ColorBulkCount)
        );
    }

    #[test]
    fn corrupt_meshes_are_refused() {
        assert_eq!(
            read(&mesh_package(
                "StaticMesh",
                mesh_payload(true, false, 0, None)
            )),
            Err(AssetError::IndexRange)
        );
        assert_eq!(
            read(&mesh_package(
                "Texture2D",
                mesh_payload(false, false, 0, None)
            )),
            Err(AssetError::NotStaticMesh)
        );
        let full = mesh_payload(false, false, 0, None);
        for end in (0..full.len()).step_by(7) {
            assert!(
                read(&mesh_package("StaticMesh", full[..end].to_vec())).is_err(),
                "{end}"
            );
        }
    }
}
