//! Synthetic bodies only (AGENTS.md rule 9): invented hulls in a package built with the testkit.

use super::*;
use ow_package::testkit::PackageBuilder;

fn f32s(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// A unit-ish cube's eight corners (invented values).
const CUBE: [[f32; 3]; 8] = [
    [0.0, 0.0, 0.0],
    [10.0, 0.0, 0.0],
    [0.0, 10.0, 0.0],
    [10.0, 10.0, 0.0],
    [0.0, 0.0, 10.0],
    [10.0, 0.0, 10.0],
    [0.0, 10.0, 10.0],
    [10.0, 10.0, 10.0],
];

struct Body<'a> {
    convex: Vec<&'a [[f32; 3]]>,
    triangles: Vec<i32>,
    elem_box: Option<([f32; 3], [f32; 3], u8)>,
    boxes: Vec<([f32; 3], [[f32; 4]; 4])>,
    spheres: u32,
    per_poly: bool,
}

impl Default for Body<'_> {
    fn default() -> Self {
        Body { convex: Vec::new(), triangles: Vec::new(), elem_box: None, boxes: Vec::new(), spheres: 0, per_poly: false }
    }
}

/// A plane's tagged bytes in the reader's W, X, Y, Z order, from (X, Y, Z, W).
fn plane(p: [f32; 4]) -> Vec<u8> {
    f32s(&[p[3], p[0], p[1], p[2]])
}

fn package(body: &Body<'_>, class: &str) -> Package {
    let mut b = PackageBuilder::new_empty();
    b.names(["None"]);
    let mut aggregate = Vec::new();
    if !body.convex.is_empty() {
        let mut elements = Vec::new();
        for hull in &body.convex {
            let flat: Vec<f32> = hull.iter().flatten().copied().collect();
            elements.extend(b.array_tag("VertexData", hull.len() as i32, &f32s(&flat)));
            if !body.triangles.is_empty() {
                let ints: Vec<u8> = body.triangles.iter().flat_map(|i| i.to_le_bytes()).collect();
                elements.extend(b.array_tag("FaceTriData", body.triangles.len() as i32, &ints));
            }
            if let Some((lo, hi, valid)) = body.elem_box {
                let mut bytes = f32s(&lo);
                bytes.extend(f32s(&hi));
                bytes.push(valid);
                elements.extend(b.struct_tag("ElemBox", "Box", &bytes));
            }
            if body.per_poly {
                elements.extend(b.bool_tag("bPerPolyShape", true));
            }
            elements.extend(b.none());
        }
        aggregate.extend(b.array_tag("ConvexElems", body.convex.len() as i32, &elements));
    }
    if !body.boxes.is_empty() {
        let mut elements = Vec::new();
        for (size, tm) in &body.boxes {
            let mut m = Vec::new();
            for row in tm {
                m.extend(plane(*row));
            }
            elements.extend(b.struct_tag("TM", "Matrix", &m));
            elements.extend(b.float_tag("X", size[0]));
            elements.extend(b.float_tag("Y", size[1]));
            elements.extend(b.float_tag("Z", size[2]));
            elements.extend(b.none());
        }
        aggregate.extend(b.array_tag("BoxElems", body.boxes.len() as i32, &elements));
    }
    if body.spheres > 0 {
        let mut elements = Vec::new();
        for _ in 0..body.spheres {
            elements.extend(b.float_tag("Radius", 5.0));
            elements.extend(b.none());
        }
        aggregate.extend(b.array_tag("SphereElems", body.spheres as i32, &elements));
    }
    aggregate.extend(b.none());
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(b.struct_tag("AggGeom", "KAggregateGeom", &aggregate));
    payload.extend(b.none());
    let cls = b.add_import("Core", "Class", 0, class);
    b.add_export(cls, "Body", payload);
    Package::from_decoded(Default::default(), "probe".into(), b.build()).unwrap()
}

const IDENTITY: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

#[test]
fn a_convex_element_keeps_its_vertices_and_triangles() {
    let body = Body { convex: vec![&CUBE], triangles: vec![0, 1, 2, 4, 5, 6], ..Body::default() };
    let read = read_body_setup(&package(&body, "RB_BodySetup"), 1).unwrap();
    assert_eq!(read.path, "Body");
    assert_eq!(read.hulls.len(), 1);
    assert_eq!(read.hulls[0].vertices[7], [10.0, 10.0, 10.0]);
    assert_eq!(read.hulls[0].triangles, vec![[0, 1, 2], [4, 5, 6]]);
}

#[test]
fn a_box_element_gives_its_eight_corners_through_its_tm() {
    let mut tm = IDENTITY;
    tm[3] = [100.0, 0.0, 50.0, 1.0];
    let body = Body { boxes: vec![([20.0, 10.0, 4.0], tm)], ..Body::default() };
    let read = read_body_setup(&package(&body, "RB_BodySetup"), 1).unwrap();
    let v = &read.hulls[0].vertices;
    assert_eq!(v.len(), 8);
    // product((-1, 1), repeat=3): first (-,-,-), then (-,-,+), last (+,+,+).
    assert_eq!(v[0], [90.0, -5.0, 48.0]);
    assert_eq!(v[1], [90.0, -5.0, 52.0]);
    assert_eq!(v[7], [110.0, 5.0, 52.0]);
    assert!(read.hulls[0].triangles.is_empty());
}

#[test]
fn an_empty_body_has_no_hulls() {
    let read = read_body_setup(&package(&Body::default(), "RB_BodySetup"), 1).unwrap();
    assert!(read.hulls.is_empty());
}

#[test]
fn refusals() {
    let flat: [[f32; 3]; 4] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]];
    let three: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let cases: Vec<(Body<'_>, CollisionError)> = vec![
        (Body { convex: vec![&flat], ..Body::default() }, CollisionError::Degenerate),
        (Body { convex: vec![&three], ..Body::default() }, CollisionError::VertexCount),
        (Body { convex: vec![&CUBE], triangles: vec![0, 1, 8], ..Body::default() }, CollisionError::TriangleIndices),
        (Body { convex: vec![&CUBE], triangles: vec![0, 1], ..Body::default() }, CollisionError::TriangleIndices),
        (Body { convex: vec![&CUBE], elem_box: Some(([0.0; 3], [10.0; 3], 0)), ..Body::default() }, CollisionError::Bounds),
        (
            Body { convex: vec![&CUBE], elem_box: Some(([0.0; 3], [5.0, 10.0, 10.0], 1)), ..Body::default() },
            CollisionError::OutsideBounds,
        ),
        (Body { convex: vec![&CUBE], per_poly: true, ..Body::default() }, CollisionError::UnsupportedFlags),
        (Body { boxes: vec![([0.0, 1.0, 1.0], IDENTITY)], ..Body::default() }, CollisionError::BoxDimensions),
        (
            Body { boxes: vec![([1.0, 1.0, 1.0], [[1.0, 0.0, 0.0, 0.5], IDENTITY[1], IDENTITY[2], IDENTITY[3]])], ..Body::default() },
            CollisionError::NonAffine,
        ),
        (Body { spheres: 1, ..Body::default() }, CollisionError::UnsupportedShape("SphereElems".into())),
    ];
    for (body, expected) in cases {
        assert_eq!(read_body_setup(&package(&body, "RB_BodySetup"), 1), Err(expected));
    }
}

#[test]
fn bounds_inside_tolerance_pass() {
    let body = Body { convex: vec![&CUBE], elem_box: Some(([0.05; 3], [9.95; 3], 1)), ..Body::default() };
    assert!(read_body_setup(&package(&body, "RB_BodySetup"), 1).is_ok());
}

#[test]
fn another_class_is_refused() {
    let body = Body { convex: vec![&CUBE], ..Body::default() };
    assert_eq!(
        read_body_setup(&package(&body, "StaticMesh"), 1),
        Err(CollisionError::NotBodySetup("StaticMesh".into()))
    );
}
