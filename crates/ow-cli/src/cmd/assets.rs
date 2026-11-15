//! Owner: lane W2-ASSETS. Commands: `--texture <index> --property-offset <bytes> --output <png> --tfc <directory>
//! [--mip <index>] [--all-mips <directory>]`, `--mesh <index> --property-offset <bytes> --output <obj> [--lod
//! <index>]`, `--scene-records <schema>` and `--terrain-records <schema>`: the port of their branches in
//! `src/cli.cpp` and of `assets::texture` / `assets::mesh` (`src/assets.cpp`), whose reading and writing is
//! `ow_assets`.
//!
//! As in the C++ binary, the package is loaded first, then the options are read in order. `--texture` and `--mesh`
//! share the option parser of `--properties`: `--array-schema` loads a schema for the tag walk, `--lod` and `--mip`
//! are read by both (and used by one), `--tfc` and `--all-mips` are refused on `--mesh`.
//!
//! `--scene-records` and `--terrain-records` print one JSON array over every export, the tagged properties of the
//! classes the C++ selects walked from the C++ per-class prefix (observed object prefixes, UNVERIFIED as a UObject
//! layout, as the C++ says) through `cmd::properties::properties_json`. A failure inside one export's walk is that
//! row's `error`; a failure outside it stops the run with the rows printed so far kept, as the C++ stream does.
//!
//! Lines end in `\n` on every platform (hub decision for the Rust port; the C++ text-mode `std::cout` prints
//! `\r\n` on Windows). PNG and OBJ files are written as the C++ writes them, byte for byte.
//!
//! None of these commands depends on the parity mode.

use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use ow_assets::mesh::read_mesh;
use ow_assets::obj::write_mesh_obj;
use ow_assets::texture::{AssetError, read_texture, write_texture_png};
use ow_core::ParityMode;
use ow_package::package::{Package, PackageError};
use ow_package::schema::{SchemaError, SchemaTyper};

use super::properties::{error_text, properties_json, unsigned_number};
use super::tables::signed_number;
use crate::args::{Command, USAGE};
use crate::json::quote;

/// A failed command: the message printed after `ow-package: `.
struct Failure(String);

impl From<PackageError> for Failure {
    fn from(e: PackageError) -> Self {
        Failure(error_text(&e))
    }
}

impl From<AssetError> for Failure {
    fn from(e: AssetError) -> Self {
        Failure(error_text(&e))
    }
}

impl From<SchemaError> for Failure {
    fn from(e: SchemaError) -> Self {
        Failure(e.to_string())
    }
}

fn usage() -> Failure {
    Failure(USAGE.to_string())
}

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let _ = mode;
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let result = execute(command.flag, args, &mut out);
    // Ignore a failed write (a closed pipe), as the C++ binary does.
    let _ = out.flush();
    drop(out);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure(message)) => {
            eprintln!("ow-package: {message}");
            ExitCode::from(1)
        }
    }
}

/// `args` is the command line without the program name: `args[0]` is the package, `args[1]` the command flag. The
/// C++ `argc` is `args.len() + 1`.
fn execute(flag: &str, args: &[String], out: &mut dyn std::io::Write) -> Result<(), Failure> {
    let Some(source) = args.first() else {
        return Err(usage());
    };
    let package = Package::load(Path::new(source))?;
    match flag {
        "--scene-records" => records(&package, args, false, out),
        "--terrain-records" => records(&package, args, true, out),
        "--mesh" => asset(&package, args, Asset::Mesh, out),
        _ => asset(&package, args, Asset::Texture, out),
    }
}

/// The C++ `nextValue`: the argument after `args[*cursor]`, moving the cursor onto it.
fn next_value<'a>(
    cursor: &mut usize,
    args: &'a [String],
    option: &str,
) -> Result<&'a str, Failure> {
    *cursor += 1;
    args.get(*cursor)
        .map(String::as_str)
        .ok_or_else(|| Failure(format!("missing value for {option}")))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Asset {
    Mesh,
    Texture,
}

/// `--mesh` / `--texture <index> [options]`, read as the C++ shared `--properties` / `--mesh` / `--texture` branch
/// reads them.
fn asset(
    package: &Package,
    args: &[String],
    kind: Asset,
    out: &mut dyn std::io::Write,
) -> Result<(), Failure> {
    if args.len() < 3 {
        return Err(usage());
    }
    let index = signed_number(&args[2]).map_err(Failure)?;
    // The C++ marks "not given" with the largest size_t, so that value given explicitly also counts as not given.
    let mut property_offset = usize::MAX;
    let mut typer = SchemaTyper::new_empty();
    let (mut output, mut tfc, mut all_mips) = ("", "", "");
    let (mut lod, mut mip) = (0usize, 0usize);
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--property-offset" => {
                property_offset = unsigned_number(next_value(&mut i, args, "--property-offset")?)
                    .map_err(Failure)?
            }
            "--output" => output = next_value(&mut i, args, "--output")?,
            "--tfc" => tfc = next_value(&mut i, args, "--tfc")?,
            "--array-schema" => {
                typer.load_file(Path::new(next_value(&mut i, args, "--array-schema")?))?
            }
            "--lod" => {
                lod = unsigned_number(next_value(&mut i, args, "--lod")?).map_err(Failure)?
            }
            "--mip" => {
                mip = unsigned_number(next_value(&mut i, args, "--mip")?).map_err(Failure)?
            }
            "--all-mips" => all_mips = next_value(&mut i, args, "--all-mips")?,
            _ => return Err(usage()),
        }
        i += 1;
    }
    if property_offset == usize::MAX {
        return Err(Failure("--property-offset is required".into()));
    }
    let text = match kind {
        Asset::Mesh => {
            if output.is_empty() || !tfc.is_empty() || !all_mips.is_empty() {
                return Err(usage());
            }
            mesh(
                package,
                index,
                property_offset,
                &typer,
                Path::new(output),
                lod,
            )?
        }
        Asset::Texture => {
            if output.is_empty() || tfc.is_empty() {
                return Err(usage());
            }
            let all_mips = (!all_mips.is_empty()).then(|| Path::new(all_mips));
            texture(
                package,
                index,
                property_offset,
                &typer,
                Path::new(output),
                Path::new(tfc),
                mip,
                all_mips,
            )?
        }
    };
    let _ = writeln!(out, "{text}");
    Ok(())
}

/// C++ `assets::texture`: reads the texture, writes the selected mip, then (with `--all-mips`) every resident mip as
/// `mip_NN.png`, and returns the report.
#[allow(clippy::too_many_arguments)]
fn texture(
    package: &Package,
    index: i32,
    property_offset: usize,
    typer: &SchemaTyper,
    output: &Path,
    tfc_root: &Path,
    selected: usize,
    all_mips: Option<&Path>,
) -> Result<String, AssetError> {
    let asset = read_texture(package, index, property_offset, typer, tfc_root)?;
    let chosen = asset.mips.get(selected).ok_or(AssetError::MipUnavailable)?;
    write_texture_png(chosen, output)?;
    if let Some(directory) = all_mips {
        std::fs::create_dir_all(directory).map_err(|_| AssetError::CreateMipDirectory)?;
        for mip in &asset.mips {
            write_texture_png(mip, &directory.join(format!("mip_{:02}.png", mip.index)))?;
        }
    }
    Ok(format!(
        "{{\"path\":{},\"format\":{},\"width\":{},\"height\":{},\"mip\":{},\"streamed\":{},\"serialized_mips\":{},\"resident_mips\":{}}}",
        quote(&asset.path),
        quote(asset.format.name()),
        chosen.width,
        chosen.height,
        chosen.index,
        chosen.streamed(),
        asset.serialized_mip_count,
        asset.mips.len()
    ))
}

/// C++ `assets::mesh`: reads the mesh, writes the selected LOD as OBJ and returns the report (the space before
/// `,"lod_summaries"` is the C++ text's).
fn mesh(
    package: &Package,
    index: i32,
    property_offset: usize,
    typer: &SchemaTyper,
    output: &Path,
    selected: usize,
) -> Result<String, AssetError> {
    let asset = read_mesh(package, index, property_offset, typer)?;
    let chosen = asset.lods.get(selected).ok_or(AssetError::LodUnavailable)?;
    write_mesh_obj(&asset, output, selected)?;
    let sections: Vec<String> = chosen
        .sections
        .iter()
        .map(|s| {
            format!(
                "{{\"material_index\":{},\"material\":{},\"triangles\":{}}}",
                s.material,
                quote(&s.material_path),
                s.face_count
            )
        })
        .collect();
    let summaries: Vec<String> = asset
        .lods
        .iter()
        .enumerate()
        .map(|(i, lod)| {
            format!(
                "{{\"lod\":{i},\"vertices\":{},\"triangles\":{},\"index_width\":{},\"uv_sets\":{}}}",
                lod.vertices.len(),
                lod.indices.len() / 3,
                lod.index_width,
                lod.uv_set_count
            )
        })
        .collect();
    Ok(format!(
        "{{\"path\":{},\"lods\":{},\"body_setup\":{},\"selected_lod\":{selected},\"vertices\":{},\"triangles\":{},\"index_width\":{},\"uv_sets\":{},\"sections\":[{}] ,\"lod_summaries\":[{}]}}",
        quote(&asset.path),
        asset.lods.len(),
        asset.body_setup,
        chosen.vertices.len(),
        chosen.indices.len() / 3,
        chosen.index_width,
        chosen.uv_set_count,
        sections.join(","),
        summaries.join(",")
    ))
}

/// The classes `--terrain-records` walks.
const TERRAIN_CLASSES: &[&str] = &[
    "Engine.Terrain",
    "Engine.TerrainComponent",
    "Engine.TerrainLayerSetup",
    "Engine.TerrainMaterial",
    "Engine.TerrainWeightMapTexture",
    "Engine.Model",
    "Engine.ModelComponent",
    "Engine.Polys",
    "Engine.BrushComponent",
];

/// Whether `--scene-records` walks class `cls`. TerrainLayerSetup.Materials is a struct array, unlike static-mesh
/// Materials object references, so the two selections never share a schema (the C++ note).
fn scene_class(cls: &str) -> bool {
    cls.contains("StaticMesh")
        || cls == "Engine.RB_BodySetup"
        || cls.contains("InterpActor")
        || cls.contains("StaticMeshComponent")
        || cls.contains("LevelStreaming")
        || cls.contains("Material")
        || cls.contains("PlayerStart")
        || cls == "Engine.World"
}

/// Where the tags of an object of class `cls` start: the object prefixes observed on Ash's packages, not a
/// universal UObject layout (UNVERIFIED native prefix semantics; no retry or offset scan), as in the C++.
fn record_prefix(cls: &str) -> usize {
    if cls.contains("CollectionActor") {
        4
    } else if cls.contains("Component") {
        8
    } else if cls.contains("Actor") || cls.contains("PlayerStart") || cls == "Engine.Terrain" {
        26
    } else {
        4
    }
}

/// `--scene-records <schema>` / `--terrain-records <schema>`: a row per export with its index, path, class path
/// (`Class` for a class) and outer, and for the selected classes the walked properties or the walk's error.
fn records(
    package: &Package,
    args: &[String],
    terrain: bool,
    out: &mut dyn std::io::Write,
) -> Result<(), Failure> {
    if args.len() != 3 {
        return Err(usage());
    }
    let mut typer = SchemaTyper::new_empty();
    typer.load_file(Path::new(&args[2]))?;
    let _ = out.write_all(b"[");
    for i in 1..=package.exports.len() as i32 {
        if i > 1 {
            let _ = out.write_all(b",");
        }
        let object = package.object(i)?;
        let cls = if object.class != 0 {
            package.path_of(object.class)?
        } else {
            String::from("Class")
        };
        // C++17 sequences the `<<` chain left to right: the index reaches the stream before the path can fail.
        let _ = write!(out, "{{\"index\":{i},\"path\":");
        let mut row = format!(
            "{},\"class\":{},\"outer\":{}",
            quote(&package.path_of(i)?),
            quote(&cls),
            object.outer
        );
        let selected = if terrain {
            TERRAIN_CLASSES.contains(&cls.as_str())
        } else {
            scene_class(&cls)
        };
        if selected {
            match properties_json(package, i, record_prefix(&cls), &typer) {
                Ok(props) => {
                    row.push_str(",\"data\":");
                    row.push_str(&props);
                }
                Err(e) => {
                    row.push_str(",\"error\":");
                    row.push_str(&quote(&error_text(&e)));
                }
            }
        }
        row.push('}');
        let _ = out.write_all(row.as_bytes());
    }
    let _ = out.write_all(b"]\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_prefixes_follow_the_cxx_order() {
        assert_eq!(record_prefix("Engine.StaticLightCollectionActor"), 4);
        assert_eq!(record_prefix("Engine.StaticMeshComponent"), 8);
        assert_eq!(record_prefix("Engine.StaticMeshActor"), 26);
        assert_eq!(record_prefix("Engine.PlayerStart"), 26);
        assert_eq!(record_prefix("Engine.Terrain"), 26);
        assert_eq!(record_prefix("Engine.TerrainLayerSetup"), 4);
        assert_eq!(record_prefix("Engine.World"), 4);
    }

    #[test]
    fn scene_and_terrain_selections_do_not_overlap_on_materials() {
        assert!(scene_class("Engine.MaterialInstanceConstant"));
        assert!(scene_class("Engine.TerrainMaterial"));
        assert!(!scene_class("Engine.TerrainLayerSetup"));
        assert!(TERRAIN_CLASSES.contains(&"Engine.TerrainLayerSetup"));
        assert!(!TERRAIN_CLASSES.contains(&"Engine.StaticMeshComponent"));
    }
}
