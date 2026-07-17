//! Navigation mesh resource API.

use crate::ResPathSource;
use perro_ids::NavMeshID;
use perro_structs::{BitMask, Vector3};
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct NavMeshTriangle3D {
    pub vertices: [u32; 3],
    pub layers: BitMask,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NavMesh3D {
    pub vertices: Vec<Vector3>,
    pub triangles: Vec<NavMeshTriangle3D>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavMeshLink3D {
    pub start: Vector3,
    pub end: Vector3,
    pub bidirectional: bool,
    pub layers: BitMask,
    pub cost: f32,
    pub snap_distance: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NavMeshResource3D {
    pub mesh: NavMesh3D,
    pub triangle_areas: Vec<u8>,
    pub links: Vec<NavMeshLink3D>,
}

impl NavMeshResource3D {
    pub fn from_mesh(mesh: NavMesh3D) -> Self {
        let triangle_areas = vec![1; mesh.triangles.len()];
        Self {
            mesh,
            triangle_areas,
            links: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.mesh.validate().map_err(|err| err.to_string())?;
        if self.triangle_areas.len() != self.mesh.triangles.len() {
            return Err("navmesh triangle area count does not match triangle count".to_string());
        }
        if let Some((triangle, _)) = self
            .triangle_areas
            .iter()
            .enumerate()
            .find(|&(_, area)| !(1u8..=32).contains(area))
        {
            return Err(format!("navmesh triangle {triangle} area must be 1..=32"));
        }
        for (link, data) in self.links.iter().enumerate() {
            if !vector_is_finite(data.start) || !vector_is_finite(data.end) {
                return Err(format!("navmesh link {link} endpoint is not finite"));
            }
            if data.layers.is_empty() {
                return Err(format!("navmesh link {link} has empty layers"));
            }
            if !data.cost.is_finite() || data.cost <= 0.0 {
                return Err(format!("navmesh link {link} cost must be finite and > 0"));
            }
            if !data.snap_distance.is_finite() || data.snap_distance < 0.0 {
                return Err(format!(
                    "navmesh link {link} snap distance must be finite and >= 0"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavMeshValidationError {
    EmptyVertices,
    EmptyTriangles,
    NonFiniteVertex {
        vertex: usize,
    },
    EmptyLayers {
        triangle: usize,
    },
    VertexIndexOutOfBounds {
        triangle: usize,
        vertex: u32,
        vertex_count: usize,
    },
    DuplicateVertexIndex {
        triangle: usize,
        vertex: u32,
    },
    DegenerateTriangleXZ {
        triangle: usize,
    },
}

impl fmt::Display for NavMeshValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyVertices => write!(formatter, "navmesh has no vertices"),
            Self::EmptyTriangles => write!(formatter, "navmesh has no triangles"),
            Self::NonFiniteVertex { vertex } => {
                write!(formatter, "navmesh vertex {vertex} is not finite")
            }
            Self::EmptyLayers { triangle } => {
                write!(formatter, "navmesh triangle {triangle} has empty layers")
            }
            Self::VertexIndexOutOfBounds {
                triangle,
                vertex,
                vertex_count,
            } => write!(
                formatter,
                "navmesh triangle {triangle} vertex {vertex} is out of range for {vertex_count} vertices"
            ),
            Self::DuplicateVertexIndex { triangle, vertex } => write!(
                formatter,
                "navmesh triangle {triangle} repeats vertex {vertex}"
            ),
            Self::DegenerateTriangleXZ { triangle } => {
                write!(formatter, "navmesh triangle {triangle} is degenerate in XZ")
            }
        }
    }
}

impl std::error::Error for NavMeshValidationError {}

impl NavMesh3D {
    pub fn try_new(
        vertices: Vec<Vector3>,
        triangles: Vec<NavMeshTriangle3D>,
    ) -> Result<Self, NavMeshValidationError> {
        let navmesh = Self {
            vertices,
            triangles,
        };
        navmesh.validate()?;
        Ok(navmesh)
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.triangles.is_empty()
    }

    pub fn validate(&self) -> Result<(), NavMeshValidationError> {
        if self.vertices.is_empty() {
            return Err(NavMeshValidationError::EmptyVertices);
        }
        if self.triangles.is_empty() {
            return Err(NavMeshValidationError::EmptyTriangles);
        }
        for (vertex, point) in self.vertices.iter().enumerate() {
            if !point.x.is_finite() || !point.y.is_finite() || !point.z.is_finite() {
                return Err(NavMeshValidationError::NonFiniteVertex { vertex });
            }
        }

        for (triangle, data) in self.triangles.iter().enumerate() {
            if data.layers.is_empty() {
                return Err(NavMeshValidationError::EmptyLayers { triangle });
            }
            for vertex in data.vertices {
                if vertex as usize >= self.vertices.len() {
                    return Err(NavMeshValidationError::VertexIndexOutOfBounds {
                        triangle,
                        vertex,
                        vertex_count: self.vertices.len(),
                    });
                }
            }
            for index in 0..3 {
                if data.vertices[(index + 1) % 3] == data.vertices[index]
                    || data.vertices[(index + 2) % 3] == data.vertices[index]
                {
                    return Err(NavMeshValidationError::DuplicateVertexIndex {
                        triangle,
                        vertex: data.vertices[index],
                    });
                }
            }

            let [a, b, c] = data.vertices.map(|index| self.vertices[index as usize]);
            let ab_x = f64::from(b.x) - f64::from(a.x);
            let ab_z = f64::from(b.z) - f64::from(a.z);
            let ac_x = f64::from(c.x) - f64::from(a.x);
            let ac_z = f64::from(c.z) - f64::from(a.z);
            if ab_x * ac_z - ab_z * ac_x == 0.0 {
                return Err(NavMeshValidationError::DegenerateTriangleXZ { triangle });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/pnav_fuzz_tests.rs"]
mod pnav_fuzz_tests;

pub fn parse_pnav_text(text: &str) -> Result<NavMesh3D, String> {
    parse_pnav_resource_text(text).map(|resource| resource.mesh)
}

pub fn parse_pnav_resource_text(text: &str) -> Result<NavMeshResource3D, String> {
    let mut navmesh = NavMesh3D::default();
    let mut triangle_areas = Vec::new();
    let mut links = Vec::new();
    let mut saw_header = false;

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_no = line_index + 1;
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.first().copied() {
            Some("pnav") => {
                if saw_header {
                    return Err(format!("line {line_no}: duplicate pnav header"));
                }
                saw_header = true;
                if parts.get(1).copied().unwrap_or("1") != "1" {
                    return Err(format!("line {line_no}: unsupported pnav version"));
                }
            }
            Some("v") => {
                if parts.len() != 4 {
                    return Err(format!("line {line_no}: v needs x y z"));
                }
                navmesh.vertices.push(Vector3::new(
                    parse_f32(parts[1], line_no)?,
                    parse_f32(parts[2], line_no)?,
                    parse_f32(parts[3], line_no)?,
                ));
            }
            Some("tri") => {
                if parts.len() < 4 {
                    return Err(format!("line {line_no}: tri needs a b c"));
                }
                let tri = [
                    parse_u32(parts[1], line_no)?,
                    parse_u32(parts[2], line_no)?,
                    parse_u32(parts[3], line_no)?,
                ];
                for index in tri {
                    if index as usize >= navmesh.vertices.len() {
                        return Err(format!("line {line_no}: tri vertex out of range"));
                    }
                }
                navmesh.triangles.push(NavMeshTriangle3D {
                    vertices: tri,
                    layers: parse_layers(&parts[4..], line_no)?,
                });
                triangle_areas.push(parse_area(&parts[4..], line_no)?);
            }
            Some("link") => {
                if parts.len() < 7 {
                    return Err(format!(
                        "line {line_no}: link needs start x y z and end x y z"
                    ));
                }
                links.push(NavMeshLink3D {
                    start: Vector3::new(
                        parse_f32(parts[1], line_no)?,
                        parse_f32(parts[2], line_no)?,
                        parse_f32(parts[3], line_no)?,
                    ),
                    end: Vector3::new(
                        parse_f32(parts[4], line_no)?,
                        parse_f32(parts[5], line_no)?,
                        parse_f32(parts[6], line_no)?,
                    ),
                    bidirectional: parse_bool_option(&parts[7..], "bidirectional", true, line_no)?,
                    layers: parse_layers(&parts[7..], line_no)?,
                    cost: parse_f32_option(&parts[7..], "cost", 1.0, line_no)?,
                    snap_distance: parse_f32_option(&parts[7..], "snap", 1.0, line_no)?,
                });
            }
            Some(kind) => return Err(format!("line {line_no}: unknown pnav record {kind}")),
            None => {}
        }
    }

    let resource = NavMeshResource3D {
        mesh: navmesh,
        triangle_areas,
        links,
    };
    resource.validate()?;
    Ok(resource)
}

pub fn parse_pnav_bytes(bytes: &[u8]) -> Result<NavMesh3D, String> {
    let text = std::str::from_utf8(bytes).map_err(|err| err.to_string())?;
    parse_pnav_text(text)
}

pub fn parse_pnav_resource_bytes(bytes: &[u8]) -> Result<NavMeshResource3D, String> {
    let text = std::str::from_utf8(bytes).map_err(|err| err.to_string())?;
    parse_pnav_resource_text(text)
}

fn vector_is_finite(value: Vector3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}

fn parse_f32(raw: &str, line_no: usize) -> Result<f32, String> {
    let value = raw
        .parse::<f32>()
        .map_err(|_| format!("line {line_no}: invalid float {raw}"))?;
    if !value.is_finite() {
        return Err(format!("line {line_no}: non-finite float {raw}"));
    }
    Ok(value)
}

fn parse_u32(raw: &str, line_no: usize) -> Result<u32, String> {
    raw.parse::<u32>()
        .map_err(|_| format!("line {line_no}: invalid index {raw}"))
}

fn parse_layers(parts: &[&str], line_no: usize) -> Result<BitMask, String> {
    let Some(raw) = parts.iter().find_map(|part| {
        part.strip_prefix("layers=")
            .or_else(|| part.strip_prefix("mask="))
    }) else {
        return Ok(BitMask::ALL);
    };
    if raw.is_empty() {
        return Err(format!("line {line_no}: empty layers"));
    }
    if let Some(bits) = raw.strip_prefix("0x") {
        let bits = u32::from_str_radix(bits, 16)
            .map_err(|_| format!("line {line_no}: invalid layer mask {raw}"))?;
        return Ok(BitMask::from_bits(bits));
    }
    if raw.contains(',') {
        let mut layers = Vec::new();
        for part in raw.split(',') {
            layers.push(
                part.parse::<u8>()
                    .map_err(|_| format!("line {line_no}: invalid layer {part}"))?,
            );
        }
        return BitMask::try_from_layers(&layers)
            .ok_or_else(|| format!("line {line_no}: layer must be 1..=32"));
    }
    let layer = raw
        .parse::<u8>()
        .map_err(|_| format!("line {line_no}: invalid layer {raw}"))?;
    BitMask::try_layer(layer).ok_or_else(|| format!("line {line_no}: layer must be 1..=32"))
}

fn parse_area(parts: &[&str], line_no: usize) -> Result<u8, String> {
    let Some(raw) = parts.iter().find_map(|part| part.strip_prefix("area=")) else {
        return Ok(1);
    };
    let area = raw
        .parse::<u8>()
        .map_err(|_| format!("line {line_no}: invalid area {raw}"))?;
    if !(1..=32).contains(&area) {
        return Err(format!("line {line_no}: area must be 1..=32"));
    }
    Ok(area)
}

fn parse_f32_option(
    parts: &[&str],
    name: &str,
    default: f32,
    line_no: usize,
) -> Result<f32, String> {
    let Some(raw) = parts.iter().find_map(|part| {
        part.strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('='))
    }) else {
        return Ok(default);
    };
    parse_f32(raw, line_no)
}

fn parse_bool_option(
    parts: &[&str],
    name: &str,
    default: bool,
    line_no: usize,
) -> Result<bool, String> {
    let Some(raw) = parts.iter().find_map(|part| {
        part.strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('='))
    }) else {
        return Ok(default);
    };
    match raw {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(format!("line {line_no}: invalid boolean {raw}")),
    }
}

pub trait NavMeshAPI {
    fn load_navmesh_hashed(&self, source_hash: u64, source: Option<&str>) -> NavMeshID;
    fn reserve_navmesh_hashed(&self, source_hash: u64, source: Option<&str>) -> NavMeshID;
    fn create_navmesh_data(&self, data: NavMesh3D) -> NavMeshID;
    fn create_navmesh_from_bytes(&self, bytes: &[u8]) -> NavMeshID;
    fn get_navmesh_data(&self, id: NavMeshID) -> Option<NavMesh3D>;
    fn write_navmesh_data(&self, id: NavMeshID, data: NavMesh3D) -> bool;
    fn is_navmesh_loaded(&self, id: NavMeshID) -> bool;
    fn drop_navmesh(&self, id: NavMeshID) -> bool;

    fn create_navmesh_resource_data(&self, data: NavMeshResource3D) -> NavMeshID {
        self.create_navmesh_data(data.mesh)
    }

    fn get_navmesh_resource_data(&self, id: NavMeshID) -> Option<NavMeshResource3D> {
        self.get_navmesh_data(id).map(NavMeshResource3D::from_mesh)
    }

    fn write_navmesh_resource_data(&self, id: NavMeshID, data: NavMeshResource3D) -> bool {
        self.write_navmesh_data(id, data.mesh)
    }

    fn load_navmesh(&self, source: &str) -> NavMeshID {
        self.load_navmesh_hashed(perro_ids::string_to_u64(source), Some(source))
    }

    fn reserve_navmesh(&self, source: &str) -> NavMeshID {
        self.reserve_navmesh_hashed(perro_ids::string_to_u64(source), Some(source))
    }
}

pub struct NavMeshModule<'res, R: NavMeshAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: NavMeshAPI + ?Sized> NavMeshModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn load<S: ResPathSource>(&self, source: S) -> NavMeshID {
        self.api.load_navmesh(source.as_res_path_str())
    }

    #[inline]
    pub fn load_hashed_with_source<S: ResPathSource>(
        &self,
        source_hash: u64,
        source: S,
    ) -> NavMeshID {
        self.api
            .load_navmesh_hashed(source_hash, Some(source.as_res_path_str()))
    }

    #[inline]
    pub fn reserve<S: ResPathSource>(&self, source: S) -> NavMeshID {
        self.api.reserve_navmesh(source.as_res_path_str())
    }

    #[inline]
    pub fn create(&self, data: NavMesh3D) -> NavMeshID {
        self.api.create_navmesh_data(data)
    }

    #[inline]
    pub fn create_resource(&self, data: NavMeshResource3D) -> NavMeshID {
        self.api.create_navmesh_resource_data(data)
    }

    #[inline]
    pub fn create_from_bytes(&self, bytes: &[u8]) -> NavMeshID {
        self.api.create_navmesh_from_bytes(bytes)
    }

    #[inline]
    pub fn get_data(&self, id: NavMeshID) -> Option<NavMesh3D> {
        self.api.get_navmesh_data(id)
    }

    #[inline]
    pub fn get_resource(&self, id: NavMeshID) -> Option<NavMeshResource3D> {
        self.api.get_navmesh_resource_data(id)
    }

    #[inline]
    pub fn write(&self, id: NavMeshID, data: NavMesh3D) -> bool {
        self.api.write_navmesh_data(id, data)
    }

    #[inline]
    pub fn write_resource(&self, id: NavMeshID, data: NavMeshResource3D) -> bool {
        self.api.write_navmesh_resource_data(id, data)
    }

    #[inline]
    pub fn is_loaded(&self, id: NavMeshID) -> bool {
        self.api.is_navmesh_loaded(id)
    }

    #[inline]
    pub fn drop(&self, id: NavMeshID) -> bool {
        self.api.drop_navmesh(id)
    }
}

#[macro_export]
macro_rules! navmesh_load {
    ($res:expr, $source:literal) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.NavMeshes().load_hashed_with_source(__HASH, $source)
    }};
    ($res:expr, $source:expr) => {
        $res.NavMeshes().load($source)
    };
}

#[macro_export]
macro_rules! navmesh_create {
    ($res:expr, $data:expr) => {
        $res.NavMeshes().create($data)
    };
}

#[macro_export]
macro_rules! navmesh_create_from_bytes {
    ($res:expr, $bytes:expr) => {
        $res.NavMeshes().create_from_bytes($bytes)
    };
}

#[cfg(test)]
mod tests {
    use super::{
        NavMesh3D, NavMeshTriangle3D, NavMeshValidationError, parse_pnav_resource_text,
        parse_pnav_text,
    };
    use perro_structs::{BitMask, Vector3};

    #[test]
    fn parse_pnav_accepts_layers() {
        let nav = parse_pnav_text(
            "pnav 1
v 0 0 0
v 1 0 0
v 0 0 1
tri 0 1 2 layers=1,3
",
        )
        .unwrap();

        assert_eq!(nav.vertices.len(), 3);
        assert_eq!(nav.triangles.len(), 1);
        assert_eq!(nav.triangles[0].layers.bits(), 0b101);
    }

    #[test]
    fn parse_pnav_accepts_areas_and_links() {
        let nav = parse_pnav_resource_text(
            "pnav 1
v 0 0 0
v 1 0 0
v 0 0 1
v 4 0 0
v 5 0 0
v 4 0 1
tri 0 1 2 layers=1 area=3
tri 3 4 5 layers=1 area=7
link 0.2 0 0.2 4.2 0 0.2 layers=1 cost=1.5 snap=0.5 bidirectional=false
",
        )
        .unwrap();

        assert_eq!(nav.triangle_areas, vec![3, 7]);
        assert_eq!(nav.links.len(), 1);
        assert!(!nav.links[0].bidirectional);
        assert_eq!(nav.links[0].cost, 1.5);
        assert_eq!(nav.links[0].snap_distance, 0.5);
    }

    #[test]
    fn legacy_parser_keeps_geometry_from_extended_pnav() {
        let nav = parse_pnav_text(
            "pnav 1
v 0 0 0
v 1 0 0
v 0 0 1
tri 0 1 2 area=2
link 0.1 0 0.1 0.2 0 0.2
",
        )
        .unwrap();

        assert_eq!(nav.triangles.len(), 1);
    }

    #[test]
    fn parse_pnav_rejects_bad_tri_index() {
        let err = parse_pnav_text(
            "pnav 1
v 0 0 0
v 1 0 0
tri 0 1 2
",
        )
        .unwrap_err();

        assert!(err.contains("out of range"));
    }

    #[test]
    fn parse_pnav_rejects_non_finite_vertices() {
        for value in ["NaN", "inf", "-inf"] {
            let text = format!("pnav 1\nv {value} 0 0\nv 1 0 0\nv 0 0 1\ntri 0 1 2\n");
            assert!(parse_pnav_text(&text).unwrap_err().contains("non-finite"));
        }
    }

    #[test]
    fn parse_pnav_rejects_out_of_range_layers_without_panic() {
        for layers in ["0", "33", "1,0", "1,33"] {
            let text = format!("pnav 1\nv 0 0 0\nv 1 0 0\nv 0 0 1\ntri 0 1 2 layers={layers}\n");
            let result = std::panic::catch_unwind(|| parse_pnav_text(&text));
            assert!(result.unwrap().unwrap_err().contains("1..=32"));
        }
    }

    #[test]
    fn parse_pnav_rejects_bad_area_and_link_options() {
        let base = "pnav 1\nv 0 0 0\nv 1 0 0\nv 0 0 1\n";
        assert!(
            parse_pnav_resource_text(&format!("{base}tri 0 1 2 area=0\n"))
                .unwrap_err()
                .contains("area must be 1..=32")
        );
        assert!(
            parse_pnav_resource_text(&format!("{base}tri 0 1 2\nlink 0 0 0 1 0 1 cost=0\n"))
                .unwrap_err()
                .contains("cost must be finite and > 0")
        );
    }

    #[test]
    fn validate_rejects_each_invalid_triangle_shape() {
        let vertices = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
        ];
        let triangle = |vertices, layers| NavMeshTriangle3D { vertices, layers };

        let out_of_bounds = NavMesh3D {
            vertices: vertices.clone(),
            triangles: vec![triangle([0, 1, 3], BitMask::ALL)],
        };
        assert!(matches!(
            out_of_bounds.validate(),
            Err(NavMeshValidationError::VertexIndexOutOfBounds { .. })
        ));

        let repeated = NavMesh3D {
            vertices: vertices.clone(),
            triangles: vec![triangle([0, 1, 1], BitMask::ALL)],
        };
        assert!(matches!(
            repeated.validate(),
            Err(NavMeshValidationError::DuplicateVertexIndex { .. })
        ));

        let degenerate = NavMesh3D {
            vertices: vertices.clone(),
            triangles: vec![triangle([0, 1, 2], BitMask::ALL)],
        };
        assert!(matches!(
            degenerate.validate(),
            Err(NavMeshValidationError::DegenerateTriangleXZ { .. })
        ));

        let no_layers = NavMesh3D {
            vertices,
            triangles: vec![triangle([0, 1, 2], BitMask::NONE)],
        };
        assert!(matches!(
            no_layers.validate(),
            Err(NavMeshValidationError::EmptyLayers { .. })
        ));
    }
}
