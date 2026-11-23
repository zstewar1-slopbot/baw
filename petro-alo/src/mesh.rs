use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Context;
use bitflags::bitflags;
use glam::Vec3;
use thiserror::Error;

use crate::alternatives::{Alt, Lod, LodGroup};
use crate::error::FormatError;
use crate::raw;

/// Bounding box for a mesh.
#[derive(Default, Debug, Copy, Clone)]
pub struct BoundingBox {
    pub start: Vec3,
    pub end: Vec3,
}

/// A mesh in an ALO model.
#[derive(Debug)]
pub struct Mesh {
    /// Name of this mesh.
    pub name: String,
    /// LOD level to use this mesh at. If None, always use this mesh.
    pub lod: Option<Lod>,
    /// An arbitrary, opaque identifier which identifies which other meshes within this model are
    /// LOD alternatives to this Mesh.
    pub lod_group: LodGroup,
    /// ALT level to use this mesh at. If None, always use this mesh.
    pub alt: Option<Alt>,
    /// Bounding box of the mesh.
    pub bounds: BoundingBox,
    /// Whether the mesh is (initially) hidden.
    pub hidden: bool,
    /// Whether this mesh is used for collision.
    pub collision_enabled: bool,
    pub submeshes: Vec<Submesh>,
    /// Bone to attach this mesh to.
    pub parent_bone: usize,
}

impl Mesh {
    pub(crate) fn from_raw(raw: raw::Mesh<'_>) -> anyhow::Result<Self> {
        let submesh_count = raw.info.submesh_count;
        if raw.submeshes.len() != submesh_count {
            Err(FormatError::WrongContentCount {
                expected: submesh_count,
                actual: raw.submeshes.len(),
            })?;
        }

        let bounds = BoundingBox {
            start: raw.info.bounds[0],
            end: raw.info.bounds[1],
        };

        #[derive(Error, Debug, Copy, Clone)]
        #[error("parsing submesh {submesh_idx} / {submesh_count}")]
        struct SubmeshCtx {
            submesh_count: usize,
            submesh_idx: usize,
        }

        let mut submeshes = Vec::with_capacity(submesh_count);
        for (submesh_idx, raw_submesh) in raw.submeshes.into_iter().enumerate() {
            let ctx = SubmeshCtx {
                submesh_count,
                submesh_idx,
            };
            let submesh = Submesh::from_raw(raw_submesh).context(ctx)?;
            submeshes.push(submesh);
        }

        let name = raw.name.to_owned();
        Ok(Mesh {
            lod: Lod::from_name(&name),
            // Actual group will be decided later.
            lod_group: LodGroup(0),
            alt: Alt::from_name(&name),
            name,
            bounds,
            hidden: raw.info.hidden,
            collision_enabled: raw.info.collision_enabled,
            submeshes,
            // Parent bones will be attached later, when reading the connections chunk.
            parent_bone: 0,
        })
    }
}

bitflags! {
    /// Identifies which vertex fields should actually be bound.
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub struct VertexFormat : u32 {
        const Positions = 0x01;
        const Normals = 0x02;
        const TexCoords = 0x04;
        const TangentBinormals = 0x08;
        const Colors = 0x10;
        const Bones = 0x20;
    }
}

impl FromStr for VertexFormat {
    type Err = FormatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let vertex_format = match s {
            "alD3dVertN" => VertexFormat::Positions | VertexFormat::Normals,
            "alD3dVertNU2" => {
                VertexFormat::Positions | VertexFormat::Normals | VertexFormat::TexCoords
            }
            "alD3dVertNU2C" => {
                VertexFormat::Positions
                    | VertexFormat::Normals
                    | VertexFormat::TexCoords
                    | VertexFormat::Colors
            }
            "alD3dVertNU2U3U3" => {
                VertexFormat::Positions
                    | VertexFormat::Normals
                    | VertexFormat::TexCoords
                    | VertexFormat::TangentBinormals
            }
            "alD3dVertNU2U3U3C" => {
                VertexFormat::Positions
                    | VertexFormat::Normals
                    | VertexFormat::TexCoords
                    | VertexFormat::TangentBinormals
                    | VertexFormat::Colors
            }
            "alD3dVertRSkinNU2" | "alD3dVertB4I4NU2" => {
                VertexFormat::Positions
                    | VertexFormat::Normals
                    | VertexFormat::TexCoords
                    | VertexFormat::Bones
            }
            "alD3dVertRSkinNU2U3U3" | "alD3dVertB4I4NU2U3U3" => {
                VertexFormat::Positions
                    | VertexFormat::Normals
                    | VertexFormat::TexCoords
                    | VertexFormat::TangentBinormals
                    | VertexFormat::Bones
            }
            "alD3dVertGrass" => {
                // TODO: ????? what should this do
                VertexFormat::Positions
            }
            _ => {
                return Err(FormatError::UnknownVertexFormat {
                    format: s.to_owned(),
                });
            }
        };
        Ok(vertex_format)
    }
}

/// Holds properties of a SubMesh.
#[derive(Debug)]
pub struct Submesh {
    /// Shader to use for this mesh.
    pub shader: Shader,
    /// The vertex buffer's bone_indicies are actually an indirection, the indicies in bone_indices
    /// point to the bone_mappings vector stored here. The indices here then refer to actual bone
    /// indicies from the model's bones.
    pub bone_mappings: Vec<usize>,
    /// Buffer of vertex indices. Each index points into all of the vertex buffers, sets of 3
    /// vertices make a triangle.
    pub primitives: Vec<[u16; 3]>,
    /// Vertex format of this submesh. Determines which of the vertex buffers should actually be
    /// used.
    ///
    /// TODO: Figure out what each of the values actually means and avoid copying (leave empty) the
    /// vertex buffers which aren't used.
    pub vertex_format_string: String,
    /// Interpreted vertex format.
    pub vertex_format: VertexFormat,
    /// Vertex data for this mesh.
    pub vertex_data: SubmeshVertexData,
}

impl Submesh {
    /// Convert a raw submesh into a validated `Submesh`.
    fn from_raw(raw: raw::Submesh<'_>) -> anyhow::Result<Self> {
        let shader =
            Shader::from_raw(raw.material).context("parsing the submesh material information")?;

        let num_vertices = raw.data.info.num_vertices;
        let num_primitives = raw.data.info.num_primitives;

        let vertex_format_string = raw.data.vertex_format.to_owned();
        let vertex_format = vertex_format_string
            .parse()
            .context("parsing the submesh vertex format (chunk 00010002)")?;

        let primitives = raw.data.primitives;
        if primitives.is_empty() {
            Err(FormatError::MissingPrimitives)?;
        }
        if primitives.len() != num_primitives {
            Err(FormatError::WrongContentCount {
                expected: num_primitives,
                actual: primitives.len(),
            })
            .context("parsing the submesh index buffer (chunk 00010004)")?;
        }

        let mut vertex_data = SubmeshVertexData::from_raw(raw.data.vertices, num_vertices)?;
        if vertex_data.positions.is_empty() {
            Err(FormatError::MissingVertices)?;
        }

        for prim in &primitives {
            for &idx in prim {
                if idx as usize >= vertex_data.positions.len() {
                    Err(FormatError::PrimitiveIndexOutOfRange {
                        idx,
                        len: vertex_data.positions.len(),
                    })?;
                }
            }
        }

        let bone_mappings = raw.data.bone_mappings;
        if !bone_mappings.is_empty() {
            let bone_index_count = bone_mappings.len().min(u16::MAX as usize) as u16;
            for (bidx, &bwt) in vertex_data
                .bone_indices
                .iter_mut()
                .zip(vertex_data.bone_weights.iter())
            {
                *bidx = squash_bone_indices(*bidx, bwt, bone_index_count)?;
            }
        }

        Ok(Submesh {
            shader,
            vertex_format_string,
            vertex_format,
            primitives,
            bone_mappings,
            vertex_data,
        })
    }
}

/// Contains data for each vertex in the submesh.
#[derive(Debug, Default)]
pub struct SubmeshVertexData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tex_coords: [Vec<[f32; 2]>; 4],
    pub tangents: Vec<[f32; 3]>,
    pub binormals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    /// Indexes into the bone_mappings array.
    pub bone_indices: Vec<[u16; 4]>,
    pub bone_weights: Vec<[f32; 4]>,
}

impl SubmeshVertexData {
    /// Create buffers for the submesh vertex data with the expected capacity.
    fn with_capacity(capacity: usize) -> Self {
        Self {
            positions: Vec::with_capacity(capacity),
            normals: Vec::with_capacity(capacity),
            tex_coords: [
                Vec::with_capacity(capacity),
                Vec::with_capacity(capacity),
                Vec::with_capacity(capacity),
                Vec::with_capacity(capacity),
            ],
            tangents: Vec::with_capacity(capacity),
            binormals: Vec::with_capacity(capacity),
            colors: Vec::with_capacity(capacity),
            bone_indices: Vec::with_capacity(capacity),
            bone_weights: Vec::with_capacity(capacity),
        }
    }

    fn from_raw(raw: raw::VertexBuffer, expected: usize) -> anyhow::Result<Self> {
        let mut out = Self::with_capacity(expected);
        match raw {
            raw::VertexBuffer::V1(verts) => {
                if verts.len() != expected {
                    Err(FormatError::WrongContentCount {
                        expected,
                        actual: verts.len(),
                    })
                    .context("parsing the submesh v1 vertex buffer (chunk 00010005)")?;
                }
                for v in verts {
                    let bidx = squash_bone_indices(v.bone_indices, v.bone_weights, u16::MAX)
                        .context("parsing the submesh v1 vertex buffer (chunk 00010005)")?;
                    let [t0, t1, t2, t3] = v.tex_coords;
                    out.positions.push(v.position);
                    out.normals.push(v.normal);
                    out.tex_coords[0].push(t0);
                    out.tex_coords[1].push(t1);
                    out.tex_coords[2].push(t2);
                    out.tex_coords[3].push(t3);
                    out.tangents.push(v.tangent);
                    out.binormals.push(v.binormal);
                    out.colors.push(v.color);
                    out.bone_indices.push(bidx);
                    out.bone_weights.push(v.bone_weights);
                }
            }
            raw::VertexBuffer::V2(verts) => {
                if verts.len() != expected {
                    Err(FormatError::WrongContentCount {
                        expected,
                        actual: verts.len(),
                    })
                    .context("parsing the submesh v2 vertex buffer (chunk 00010007)")?;
                }
                for v in verts {
                    let bidx = squash_bone_indices(v.bone_indices, v.bone_weights, u16::MAX)
                        .context("parsing the submesh v2 vertex buffer (chunk 00010007)")?;
                    let [t0, t1, t2, t3] = v.tex_coords;
                    out.positions.push(v.position);
                    out.normals.push(v.normal);
                    out.tex_coords[0].push(t0);
                    out.tex_coords[1].push(t1);
                    out.tex_coords[2].push(t2);
                    out.tex_coords[3].push(t3);
                    out.tangents.push(v.tangent);
                    out.binormals.push(v.binormal);
                    out.colors.push(v.color);
                    out.bone_indices.push(bidx);
                    out.bone_weights.push(v.bone_weights);
                }
            }
        }
        Ok(out)
    }
}

/// Squash an array of u32 values down to u16. Uses bone weights to decide whether to treat
/// out-of-bounds indexes as an error, as some models have invalid indices in zero-weight bones.
fn squash_bone_indices(
    [i1, i2, i3, i4]: [impl Into<u32>; 4],
    [w1, w2, w3, w4]: [f32; 4],
    index_count: u16,
) -> anyhow::Result<[u16; 4]> {
    Ok([
        squash_bone_idx(i1, w1, index_count)?,
        squash_bone_idx(i2, w2, index_count)?,
        squash_bone_idx(i3, w3, index_count)?,
        squash_bone_idx(i4, w4, index_count)?,
    ])
}

fn squash_bone_idx(idx: impl Into<u32>, weight: f32, index_count: u16) -> anyhow::Result<u16> {
    let idx = idx.into();
    if idx >= index_count as u32 {
        // Some models have index 3 set to a large number for unknown reason.
        if weight == 0.0 {
            Ok(0)
        } else {
            Err(FormatError::BoneNotMapped { idx }.into())
        }
    } else {
        Ok(idx as u16)
    }
}

/// Submesh shader configuration.
#[derive(Debug, Clone)]
pub struct Shader {
    /// Name of the shader file.
    pub filename: String,
    /// Shader parameters for this submesh's shader.
    pub params: HashMap<String, ShaderParam>,
}

impl Shader {
    fn from_raw(raw: raw::Shader<'_>) -> anyhow::Result<Self> {
        let mut params = HashMap::with_capacity(raw.params.len());
        for param in raw.params {
            let (name, value) = match param {
                raw::ShaderParam::Int(entry) => (entry.name, ShaderParam::Int(entry.value)),
                raw::ShaderParam::Float(entry) => (entry.name, ShaderParam::Float(entry.value)),
                raw::ShaderParam::Float3(entry) => (entry.name, ShaderParam::Float3(entry.value)),
                raw::ShaderParam::Texture(entry) => {
                    (entry.name, ShaderParam::Texture(entry.value.to_owned()))
                }
                raw::ShaderParam::Float4(entry) => (entry.name, ShaderParam::Float4(entry.value)),
            };
            if params.contains_key(name) {
                return Err(FormatError::DuplicateShaderParam {
                    name: name.to_owned(),
                }
                .into());
            }
            params.insert(name.to_owned(), value);
        }

        Ok(Self {
            filename: raw.filename.to_owned(),
            params,
        })
    }
}

/// A value passed to an ALO shader.
#[derive(Debug, Clone)]
pub enum ShaderParam {
    Int(i32),
    Float(f32),
    Float3([f32; 3]),
    Texture(PathBuf),
    Float4([f32; 4]),
}
