//! Owner: lane W2-ASSETS. The OBJ writer: the port of `assets::writeMeshObj` (`src/assets.cpp`). One LOD as text:
//! a comment line, every vertex's `v`, `vt` (V flipped to `1 - v`, first UV set) and `vn`, then one group per
//! section with its material path as a comment and its faces (`f a/a/a b/b/b c/c/c`, 1-based). Numbers print as the
//! C++ `std::setprecision(9)` stream prints a float (through `ow_core::fmt`, rule N6). Lines end in `\n` on every
//! platform: the C++ writes the text through a binary-mode file, so its bytes have no `\r` either.

use std::fmt::Write as _;
use std::path::Path;

use ow_core::fmt::fmt_g;

use crate::mesh::MeshAsset;
use crate::texture::{AssetError, check, write_output};

/// A float as `std::ostream` with `setprecision(9)` prints it.
fn number(x: f32) -> String {
    fmt_g(f64::from(x), 9)
}

/// The OBJ text of LOD `lod` of `mesh`.
pub fn obj_text(mesh: &MeshAsset, lod: usize) -> Result<String, AssetError> {
    check(lod < mesh.lods.len(), AssetError::LodUnavailable)?;
    let lod = &mesh.lods[lod];
    let mut obj =
        String::from("# OpenWillow local extraction; UE coordinates X,Y,Z; centimeters\n");
    for vertex in &lod.vertices {
        let [x, y, z] = vertex.position;
        let _ = writeln!(obj, "v {} {} {}", number(x), number(y), number(z));
    }
    for vertex in &lod.vertices {
        let uv = vertex.uvs.first().ok_or(AssetError::NoUvSet)?;
        let _ = writeln!(obj, "vt {} {}", number(uv[0]), number(1.0 - uv[1]));
    }
    for vertex in &lod.vertices {
        let [x, y, z] = vertex.normal;
        let _ = writeln!(obj, "vn {} {} {}", number(x), number(y), number(z));
    }
    for (i, section) in lod.sections.iter().enumerate() {
        let _ = write!(obj, "g section_{i}\n# material {}\n", section.material_path);
        let first = section.first_index as usize;
        // read_mesh checked every section's range against the indices.
        let faces = lod
            .indices
            .get(first..first + section.face_count as usize * 3)
            .ok_or(AssetError::SectionRange)?;
        for face in faces.chunks_exact(3) {
            obj.push('f');
            for &index in face {
                let vertex = u64::from(index) + 1;
                let _ = write!(obj, " {vertex}/{vertex}/{vertex}");
            }
            obj.push('\n');
        }
    }
    Ok(obj)
}

/// Writes LOD `lod` of `mesh` to `output` as OBJ (C++ `writeMeshObj`).
pub fn write_mesh_obj(mesh: &MeshAsset, output: &Path, lod: usize) -> Result<(), AssetError> {
    let text = obj_text(mesh, lod)?;
    write_output(output, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::read_mesh;
    use crate::mesh::tests::{mesh_package, mesh_payload};
    use ow_package::schema::SchemaTyper;

    #[test]
    fn a_triangle_prints_the_cxx_text() {
        let mesh = read_mesh(
            &mesh_package("StaticMesh", mesh_payload(false, false, 0, None)),
            1,
            4,
            &SchemaTyper::new_empty(),
        )
        .unwrap();
        // The text the C++ binary writes for this fixture (`assets_test.py`'s mesh(); the OBJ bytes of both binaries
        // were compared on the local differential run): normal bytes 128 -> 0.003921628 (128 / 127.5 - 1 in float),
        // 255 -> 1; V flipped.
        let expected = concat!(
            "# OpenWillow local extraction; UE coordinates X,Y,Z; centimeters\n",
            "v 0 0 0\nv 100 0 0\nv 0 100 0\n",
            "vt 0 1\nvt 1 1\nvt 0 0\n",
            "vn 0.003921628 0.003921628 1\nvn 0.003921628 0.003921628 1\nvn 0.003921628 0.003921628 1\n",
            "g section_0\n# material None\n",
            "f 1/1/1 2/2/2 3/3/3\n",
        );
        assert_eq!(obj_text(&mesh, 0).unwrap(), expected);
        assert_eq!(obj_text(&mesh, 1), Err(AssetError::LodUnavailable));
    }
}
