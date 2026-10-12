use super::{internal, ApiError};
use axum::http::StatusCode;
use glam::{Mat4, Vec3};

#[derive(Debug)]
pub struct Metadata {
    pub width: f32,
    pub depth: f32,
    pub height: f32,
    pub triangles: u32,
}

pub fn inspect(bytes: &[u8], max_bytes: usize, max_triangles: u32) -> Result<Metadata, ApiError> {
    let bad = || {
        ApiError(
            StatusCode::BAD_REQUEST,
            "GLB inválido ou fora dos limites permitidos",
        )
    };
    if bytes.len() > max_bytes || bytes.len() < 20 || &bytes[..4] != b"glTF" {
        return Err(bad());
    }
    let gltf = gltf::Gltf::from_slice(bytes).map_err(|_| bad())?;
    let blob = gltf.blob.as_deref().ok_or_else(bad)?;
    if gltf.buffers().any(|buffer| {
        !matches!(buffer.source(), gltf::buffer::Source::Bin) || buffer.length() > blob.len()
    }) || gltf
        .images()
        .any(|image| matches!(image.source(), gltf::image::Source::Uri { .. }))
    {
        return Err(bad());
    }
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut triangles = 0u32;
    let scene = gltf
        .default_scene()
        .or_else(|| gltf.scenes().next())
        .ok_or_else(bad)?;
    fn walk(
        node: gltf::Node<'_>,
        parent: Mat4,
        blob: &[u8],
        min: &mut Vec3,
        max: &mut Vec3,
        triangles: &mut u32,
        depth: u32,
    ) -> Result<(), ApiError> {
        if depth > 64 {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "Hierarquia GLB profunda demais",
            ));
        }
        let matrix = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    return Err(ApiError(
                        StatusCode::BAD_REQUEST,
                        "Use malhas trianguladas no GLB",
                    ));
                }
                let reader = primitive.reader(|buffer| {
                    if matches!(buffer.source(), gltf::buffer::Source::Bin) {
                        Some(blob)
                    } else {
                        None
                    }
                });
                let positions = reader.read_positions().ok_or(ApiError(
                    StatusCode::BAD_REQUEST,
                    "GLB sem posições de vértices",
                ))?;
                let vertex_count = positions.clone().count();
                for position in positions {
                    let point = matrix.transform_point3(Vec3::from_array(position));
                    if !point.is_finite() {
                        return Err(ApiError(
                            StatusCode::BAD_REQUEST,
                            "Coordenadas GLB inválidas",
                        ));
                    }
                    *min = min.min(point);
                    *max = max.max(point);
                }
                let count = match reader.read_indices() {
                    Some(indices) => {
                        let mut count = 0;
                        for index in indices.into_u32() {
                            if usize::try_from(index).map_err(internal)? >= vertex_count {
                                return Err(ApiError(
                                    StatusCode::BAD_REQUEST,
                                    "Índice GLB fora dos vértices disponíveis",
                                ));
                            }
                            count += 1;
                        }
                        count
                    }
                    None => vertex_count,
                };
                if !count.is_multiple_of(3) {
                    return Err(ApiError(
                        StatusCode::BAD_REQUEST,
                        "Quantidade de índices inválida",
                    ));
                }
                let primitive_triangles = u32::try_from(count / 3).map_err(internal)?;
                *triangles = triangles
                    .checked_add(primitive_triangles)
                    .ok_or(ApiError(StatusCode::BAD_REQUEST, "GLB grande demais"))?;
            }
        }
        for child in node.children() {
            walk(child, matrix, blob, min, max, triangles, depth + 1)?;
        }
        Ok(())
    }
    for node in scene.nodes() {
        walk(
            node,
            Mat4::IDENTITY,
            blob,
            &mut min,
            &mut max,
            &mut triangles,
            0,
        )?;
    }
    let dimensions = max - min;
    if triangles == 0
        || triangles > max_triangles
        || !dimensions.is_finite()
        || dimensions.max_element() <= 0.0
    {
        return Err(bad());
    }
    Ok(Metadata {
        width: dimensions.x,
        depth: dimensions.z,
        height: dimensions.y,
        triangles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn triangle(index: Option<u32>) -> Vec<u8> {
        let mut document = serde_json::json!({"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0,"translation":[5,3,8],"scale":[2,3,4]}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"buffers":[{"byteLength":36}],"bufferViews":[{"buffer":0,"byteLength":36}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,1]}]});
        if index.is_some() {
            document["buffers"][0]["byteLength"] = serde_json::json!(48);
            document["bufferViews"]
                .as_array_mut()
                .expect("buffer views")
                .push(serde_json::json!({"buffer":0,"byteOffset":36,"byteLength":12}));
            document["accessors"].as_array_mut().expect("accessors").push(serde_json::json!({"bufferView":1,"componentType":5125,"count":3,"type":"SCALAR"}));
            document["meshes"][0]["primitives"][0]["indices"] = serde_json::json!(1);
        }
        let mut json = serde_json::to_vec(&document).expect("json");
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let mut bin = Vec::new();
        for value in [0f32, 0., 0., 1., 0., 0., 0., 1., 1.] {
            bin.extend(value.to_le_bytes());
        }
        if let Some(index) = index {
            for value in [0u32, 1, index] {
                bin.extend(value.to_le_bytes());
            }
        }
        let length = 12 + 8 + json.len() + 8 + bin.len();
        let mut glb = b"glTF".to_vec();
        glb.extend(2u32.to_le_bytes());
        glb.extend((length as u32).to_le_bytes());
        glb.extend((json.len() as u32).to_le_bytes());
        glb.extend(0x4e4f534au32.to_le_bytes());
        glb.extend(json);
        glb.extend((bin.len() as u32).to_le_bytes());
        glb.extend(0x004e4942u32.to_le_bytes());
        glb.extend(bin);
        glb
    }
    #[test]
    fn measures_transformed_mesh_and_enforces_limits() {
        let bytes = triangle(None);
        let metadata = inspect(&bytes, 20000, 100).expect("valid glb");
        assert_eq!(metadata.triangles, 1);
        assert_eq!(
            (metadata.width, metadata.height, metadata.depth),
            (2., 3., 4.)
        );
        assert!(inspect(&bytes, 10, 100).is_err());
        assert!(inspect(&bytes, 20000, 0).is_err());
        assert!(inspect(b"invalid", 20000, 100).is_err());
        assert!(inspect(&triangle(Some(2)), 20000, 100).is_ok());
        assert!(inspect(&triangle(Some(99)), 20000, 100).is_err());
    }
}
