//! `tiles` splits a large static mesh into square tiles on the ground (x, z), each with its own
//! bounding sphere, so the renderer can cull each tile against the camera and every shadow
//! cascade instead of drawing the whole forest in every pass.
//!
//! Interface: `MeshTile`, `MeshTile::Bounding`, `split_mesh_into_tiles`.
//!
//! Example:
//!     let tiles: Vec<MeshTile> = split_mesh_into_tiles(&mesh, STATIC_TILE_M);

use std::collections::BTreeMap;

use glam::Vec3;

use crate::render::{MeshData, Vertex};

/// `MeshTile` is the part of a mesh whose triangles' centroids fall in one tile, and the sphere
/// around its vertices.
pub struct MeshTile {
    pub mesh: MeshData,
    pub bounds_center_m: Vec3,
    pub bounds_radius_m: f32,
}

/// `NO_TILE` marks a vertex that the tile being built doesn't use yet.
const NO_TILE: usize = usize::MAX;

/// `split_mesh_into_tiles` cuts `mesh` into tiles `tile_m` wide on x and z: each triangle goes to
/// the tile under its centroid, whole, with its vertices copied there (a vertex shared by two
/// tiles is copied into both). Tiles come in z-major order, and each keeps its triangles in the
/// mesh's order, so drawing every tile draws exactly the mesh.
///
/// Example: `let tiles: Vec<MeshTile> = split_mesh_into_tiles(&forest, 8.0);`
pub fn split_mesh_into_tiles(mesh: &MeshData, tile_m: f32) -> Vec<MeshTile> {
    assert!(tile_m > 0.0, "tiles must be wider than 0 m, not {tile_m}");
    assert!(
        mesh.skin_vertices.is_none(),
        "only static meshes are split into tiles"
    );
    let mut triangles_by_tile: BTreeMap<(i32, i32), Vec<[u32; 3]>> = BTreeMap::new();
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let centroid_m: Vec3 = triangle
            .iter()
            .map(|&index| Vec3::from_array(mesh.vertices[index as usize].position))
            .sum::<Vec3>()
            / 3.0;
        let tile: (i32, i32) = (
            (centroid_m.z / tile_m).floor() as i32,
            (centroid_m.x / tile_m).floor() as i32,
        );
        triangles_by_tile.entry(tile).or_default().push(*triangle);
    }
    let mut tile_of_vertex: Vec<usize> = vec![NO_TILE; mesh.vertices.len()];
    let mut index_in_tile: Vec<u32> = vec![0; mesh.vertices.len()];
    triangles_by_tile
        .into_values()
        .enumerate()
        .map(|(tile_index, triangles)| {
            let mut vertices: Vec<Vertex> = Vec::new();
            let mut indices: Vec<u32> = Vec::with_capacity(triangles.len() * 3);
            for &index in triangles.iter().flatten() {
                let vertex: usize = index as usize;
                if tile_of_vertex[vertex] != tile_index {
                    tile_of_vertex[vertex] = tile_index;
                    index_in_tile[vertex] = vertices.len() as u32;
                    vertices.push(mesh.vertices[vertex]);
                }
                indices.push(index_in_tile[vertex]);
            }
            MeshTile::Bounding(MeshData {
                vertices,
                skin_vertices: None,
                indices,
            })
        })
        .collect()
}

impl MeshTile {
    /// `Bounding` makes `mesh` a tile, with the sphere around its vertices' box.
    ///
    /// Example: `let tile: MeshTile = MeshTile::Bounding(mesh);`
    #[allow(non_snake_case)]
    pub fn Bounding(mesh: MeshData) -> MeshTile {
        let (low_m, high_m) = mesh.vertices.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(low_m, high_m), vertex| {
                let position_m: Vec3 = Vec3::from_array(vertex.position);
                (low_m.min(position_m), high_m.max(position_m))
            },
        );
        MeshTile {
            mesh,
            bounds_center_m: (low_m + high_m) * 0.5,
            bounds_radius_m: (high_m - low_m).length() * 0.5,
        }
    }
}

/// `tests` checks that tiles together hold exactly the mesh's triangles, that a triangle goes
/// whole to the tile under its centroid, and that each tile's sphere holds its vertices.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | triangles per tile | one, several | `test_triangles_go_to_the_tile_under_their_centroid` |
/// | triangle vs tile edges | inside a tile, straddling an edge, at negative coordinates | `test_triangles_go_to_the_tile_under_their_centroid` |
/// | shared vertices | shared within a tile, shared across tiles | `test_tiles_hold_exactly_the_mesh` |
/// | tile bounds | holds every vertex | `test_tiles_hold_exactly_the_mesh` |
/// | mesh | empty | `test_empty_mesh_has_no_tiles` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `TILE_M` is the tile width the tests split with.
    const TILE_M: f32 = 4.0;

    /// `vertex_at` is a vertex at `position_m` with no other attributes.
    fn vertex_at(position_m: [f32; 3]) -> Vertex {
        Vertex {
            position: position_m,
            ..Vertex::default()
        }
    }

    /// `get_triangles_m` lists a mesh's triangles by their corners' positions.
    fn get_triangles_m(mesh: &MeshData) -> Vec<[[f32; 3]; 3]> {
        mesh.indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|triangle| triangle.map(|index| mesh.vertices[index as usize].position))
            .collect()
    }

    /// `fan_mesh` is four triangles sharing the vertex at the origin, one in each quadrant of the
    /// x-z plane, plus two triangles sharing an edge inside the tile at x, z in [4, 8).
    fn fan_mesh() -> MeshData {
        let positions_m: [[f32; 3]; 9] = [
            [0.0, 0.0, 0.0],
            [3.0, 1.0, 0.5],
            [0.5, 2.0, 3.0],
            [-3.0, 0.0, 0.5],
            [-0.5, 0.0, -3.0],
            [5.0, 0.0, 5.0],
            [6.0, 0.0, 5.0],
            [5.0, 9.0, 6.0],
            [6.0, 0.0, 6.0],
        ];
        MeshData {
            vertices: positions_m.into_iter().map(vertex_at).collect(),
            skin_vertices: None,
            indices: vec![0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 1, 5, 6, 7, 6, 8, 7],
        }
    }

    /// `test_tiles_hold_exactly_the_mesh` checks that the tiles' triangles, in order, are the
    /// mesh's triangles, and that every tile's sphere holds its vertices.
    #[test]
    fn test_tiles_hold_exactly_the_mesh() {
        let mesh: MeshData = fan_mesh();
        let tiles: Vec<MeshTile> = split_mesh_into_tiles(&mesh, TILE_M);
        let mut tiled_m: Vec<[[f32; 3]; 3]> = tiles
            .iter()
            .flat_map(|tile| get_triangles_m(&tile.mesh))
            .collect();
        let mut expected_m: Vec<[[f32; 3]; 3]> = get_triangles_m(&mesh);
        assert_eq!(tiled_m.len(), expected_m.len());
        let key = |triangle: &[[f32; 3]; 3]| format!("{triangle:?}");
        tiled_m.sort_by_key(key);
        expected_m.sort_by_key(key);
        assert_eq!(tiled_m, expected_m);
        for tile in &tiles {
            for vertex in &tile.mesh.vertices {
                let distance_m: f32 =
                    Vec3::from_array(vertex.position).distance(tile.bounds_center_m);
                assert!(distance_m <= tile.bounds_radius_m + 1e-5);
            }
        }
    }

    /// `test_triangles_go_to_the_tile_under_their_centroid` checks the fan's tiles: each quadrant
    /// triangle has its own tile (the shared vertex copied into each), and the far pair shares
    /// one tile and its two shared vertices.
    #[test]
    fn test_triangles_go_to_the_tile_under_their_centroid() {
        let tiles: Vec<MeshTile> = split_mesh_into_tiles(&fan_mesh(), TILE_M);
        let triangle_counts: Vec<usize> = tiles
            .iter()
            .map(|tile| tile.mesh.indices.len() / 3)
            .collect();
        assert_eq!(triangle_counts, vec![1, 1, 1, 1, 2]);
        assert!(tiles[..4].iter().all(|tile| tile.mesh.vertices.len() == 3));
        let far: &MeshTile = tiles.last().expect("five tiles");
        assert_eq!(far.mesh.vertices.len(), 4);
        assert_eq!(
            get_triangles_m(&far.mesh),
            vec![
                [[5.0, 0.0, 5.0], [6.0, 0.0, 5.0], [5.0, 9.0, 6.0]],
                [[6.0, 0.0, 5.0], [6.0, 0.0, 6.0], [5.0, 9.0, 6.0]]
            ]
        );
    }

    /// `test_empty_mesh_has_no_tiles` checks that a mesh without triangles gives no tiles.
    #[test]
    fn test_empty_mesh_has_no_tiles() {
        let mesh: MeshData = MeshData {
            vertices: Vec::new(),
            skin_vertices: None,
            indices: Vec::new(),
        };
        assert!(split_mesh_into_tiles(&mesh, TILE_M).is_empty());
    }
}
