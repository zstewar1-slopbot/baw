//! Raw file-format AST structures for Petroglyph `.alo` and `.ala` files, parsed declaratively via
//! `petro-chunked-derive`.

use std::path::Path;

use glam::Vec3;
use petro_chunked::byteblock::FromByteBlock;
use petro_chunked::wire_types::{U32Bool, U32Usize, U32UsizeVec};
use petro_chunked::{
    ChunkErrorV2, ChunkStreamItem, ChunkStructure, MiniChunkStructure, iter_chunks,
};

use crate::ids;
use crate::skeleton::BillboardMode;

/// Raw top-level `.alo` file structure.
#[derive(Debug, ChunkStructure)]
#[chunk(map)]
pub struct AloFile<'b> {
    #[chunk(ids::SKELETON, children, optional)]
    pub skeleton: Option<Skeleton<'b>>,
    #[chunk(stream, repeated)]
    pub objects: Vec<Object<'b>>,
    #[chunk(ids::CONNECTION, children, optional)]
    pub connections: Option<Connections<'b>>,
    #[chunk(ids::PARTICLES, ignore, repeated)]
    pub particles: Vec<()>,
}

impl<'b> AloFile<'b> {
    pub fn parse(bytes: &'b [u8]) -> Result<Self, ChunkErrorV2> {
        iter_chunks(bytes).expect_struct()
    }
}

/// A connectible top-level object chunk (`Mesh` or `Light`) in an `.alo` file.
#[derive(Debug, ChunkStreamItem)]
pub enum Object<'b> {
    #[chunk(ids::MESH, children)]
    Mesh(Mesh<'b>),
    #[chunk(ids::LIGHT)]
    Light,
}

// =================================================================================================
// Skeleton (0x0000_0200)
// =================================================================================================

/// Raw skeleton header chunk (`0x0000_0201`).
#[derive(Debug, FromByteBlock)]
#[byte_block(pad_after = 124)]
pub struct SkeletonHeader {
    #[byte_block(wire_format = U32Usize)]
    pub bone_count: usize,
}

/// Raw skeleton chunk (`0x0000_0200`).
#[derive(Debug, ChunkStructure)]
#[chunk(sequence)]
pub struct Skeleton<'b> {
    #[chunk(ids::BONE_COUNT)]
    pub header: SkeletonHeader,
    #[chunk(ids::BONE, children, repeated)]
    pub bones: Vec<Bone<'b>>,
}

/// Raw v1 bone data chunk (`0x0000_0205`, without billboard mode).
#[derive(Debug, FromByteBlock)]
pub struct BoneDataV1 {
    pub parent: u32,
    #[byte_block(wire_format = U32Bool)]
    pub visible: bool,
    pub matrix: [[f32; 4]; 3],
}

/// Raw v2 bone data chunk (`0x0000_0206`, with billboard mode).
#[derive(Debug, FromByteBlock)]
pub struct BoneDataV2 {
    pub parent: u32,
    #[byte_block(wire_format = U32Bool)]
    pub visible: bool,
    pub billboard: BillboardMode,
    pub matrix: [[f32; 4]; 3],
}

/// Either a v1 (`0x0000_0205`) or v2 (`0x0000_0206`) bone data chunk.
#[derive(Debug, ChunkStreamItem)]
pub enum BoneData {
    #[chunk(ids::BONE_V1)]
    V1(BoneDataV1),
    #[chunk(ids::BONE_V2)]
    V2(BoneDataV2),
}

/// Raw bone chunk (`0x0000_0202`).
#[derive(Debug, ChunkStructure)]
#[chunk(sequence)]
pub struct Bone<'b> {
    #[chunk(ids::BONE_NAME)]
    pub name: &'b str,
    #[chunk(stream)]
    pub data: BoneData,
}

// =================================================================================================
// Connections (0x0000_0600)
// =================================================================================================

/// Raw connection count chunk (`0x0000_0601`).
#[derive(Debug, MiniChunkStructure)]
#[mini_chunk(map)]
pub struct ConnectionCount {
    #[mini_chunk(ids::CONNECTION_COUNT_OBJECT, wire_format = U32Usize)]
    pub object_count: usize,
    #[mini_chunk(ids::CONNECTION_COUNT_PROXY, wire_format = U32Usize)]
    pub proxy_count: usize,
    #[mini_chunk(ids::CONNECTION_COUNT_DAZZLE, optional, wire_format = U32Usize)]
    pub dazzle_count: Option<usize>,
}

/// Raw object connection chunk (`0x0000_0602`).
#[derive(Debug, MiniChunkStructure)]
#[mini_chunk(map)]
pub struct ObjectConnection {
    #[mini_chunk(ids::CONNECTION_OBJECT_OBJ, wire_format = U32Usize)]
    pub object: usize,
    #[mini_chunk(ids::CONNECTION_OBJECT_BONE, wire_format = U32Usize)]
    pub bone: usize,
}

/// Raw proxy connection chunk (`0x0000_0603`).
#[derive(Debug, MiniChunkStructure)]
#[mini_chunk(map)]
pub struct ProxyConnection<'b> {
    #[mini_chunk(ids::CONNECTION_PROXY_NAME)]
    pub proxy_name: &'b str,
    #[mini_chunk(ids::CONNECTION_PROXY_BONE, wire_format = U32Usize)]
    pub bone: usize,
    #[mini_chunk(ids::CONNECTION_PROXY_HIDDEN, default, wire_format = U32Bool)]
    pub hidden: bool,
    #[mini_chunk(ids::CONNECTION_PROXY_ADSH, default, wire_format = U32Bool)]
    pub alt_decrease_stay_hidden: bool,
}

/// Raw connections chunk (`0x0000_0600`).
#[derive(Debug, ChunkStructure)]
#[chunk(sequence)]
pub struct Connections<'b> {
    #[chunk(ids::CONNECTION_COUNT, mini_chunks)]
    pub count: ConnectionCount,
    #[chunk(ids::CONNECTION_OBJECT, mini_chunks, repeated)]
    pub objects: Vec<ObjectConnection>,
    #[chunk(ids::CONNECTION_PROXY, mini_chunks, repeated)]
    pub proxies: Vec<ProxyConnection<'b>>,
    #[chunk(ids::CONNECTION_DAZZLE, ignore, repeated)]
    pub dazzles: Vec<()>,
}

// =================================================================================================
// Mesh (0x0000_0400) & Submesh (0x0001_0100, 0x0001_0000)
// =================================================================================================

/// Raw mesh info chunk (`0x0000_0402`).
#[derive(Debug, FromByteBlock)]
#[byte_block(pad_after = 88)]
pub struct MeshInfo {
    #[byte_block(wire_format = U32Usize)]
    pub submesh_count: usize,
    pub bounds: [Vec3; 2],
    pub unknown: u32,
    #[byte_block(wire_format = U32Bool)]
    pub hidden: bool,
    #[byte_block(wire_format = U32Bool)]
    pub collision_enabled: bool,
}

/// Raw mesh chunk (`0x0000_0400`).
#[derive(Debug, ChunkStructure)]
#[chunk(sequence)]
pub struct Mesh<'b> {
    #[chunk(ids::MESH_NAME)]
    pub name: &'b str,
    #[chunk(ids::MESH_INFO)]
    pub info: MeshInfo,
    #[chunk(stream, repeated)]
    pub submeshes: Vec<Submesh<'b>>,
}

/// Raw submesh pair of sibling chunks (`0x0001_0100` material and `0x0001_0000` data).
#[derive(Debug, ChunkStreamItem)]
pub struct Submesh<'b> {
    #[chunk(ids::SUBMESH_MATERIAL, children)]
    pub material: Shader<'b>,
    #[chunk(ids::SUBMESH_DATA, children)]
    pub data: SubmeshData<'b>,
}

/// Raw `(name, value)` mini-chunk pair inside a shader parameter chunk.
#[derive(Debug, MiniChunkStructure)]
#[mini_chunk(sequence)]
pub struct ShaderParamEntry<'b, T: FromByteBlock<'b>> {
    #[mini_chunk(ids::SUBMESH_MATERIAL_PARAM_NAME)]
    pub name: &'b str,
    #[mini_chunk(ids::SUBMESH_MATERIAL_PARAM_VALUE)]
    pub value: T,
}

/// Raw shader parameter chunk inside `0x0001_0100`.
#[derive(Debug, ChunkStreamItem)]
pub enum ShaderParam<'b> {
    #[chunk(ids::SUBMESH_MATERIAL_INT_PARAM, mini_chunks)]
    Int(ShaderParamEntry<'b, i32>),
    #[chunk(ids::SUBMESH_MATERIAL_FLOAT_PARAM, mini_chunks)]
    Float(ShaderParamEntry<'b, f32>),
    #[chunk(ids::SUBMESH_MATERIAL_FLOAT3_PARAM, mini_chunks)]
    Float3(ShaderParamEntry<'b, [f32; 3]>),
    #[chunk(ids::SUBMESH_MATERIAL_TEX_PARAM, mini_chunks)]
    Texture(ShaderParamEntry<'b, &'b Path>),
    #[chunk(ids::SUBMESH_MATERIAL_FLOAT4_PARAM, mini_chunks)]
    Float4(ShaderParamEntry<'b, [f32; 4]>),
}

/// Raw submesh material / shader chunk (`0x0001_0100`).
#[derive(Debug, ChunkStructure)]
#[chunk(sequence)]
pub struct Shader<'b> {
    #[chunk(ids::SUBMESH_MATERIAL_SHADER_NAME)]
    pub filename: &'b str,
    #[chunk(stream, repeated)]
    pub params: Vec<ShaderParam<'b>>,
}

/// Raw submesh info header chunk (`0x0001_0001`).
#[derive(Debug, FromByteBlock)]
#[byte_block(pad_after = 120)]
pub struct SubmeshInfo {
    #[byte_block(wire_format = U32Usize)]
    pub num_vertices: usize,
    #[byte_block(wire_format = U32Usize)]
    pub num_primitives: usize,
}

/// Raw v1 vertex record (`0x0001_0005`).
#[derive(Debug, FromByteBlock)]
pub struct VertexV1 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [[f32; 2]; 4],
    pub tangent: [f32; 3],
    pub binormal: [f32; 3],
    pub color: [f32; 4],
    pub bone_indices: [u32; 4],
    pub bone_weights: [f32; 4],
}

/// Raw v2 vertex record (`0x0001_0007`).
#[derive(Debug, FromByteBlock)]
pub struct VertexV2 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [[f32; 2]; 4],
    pub tangent: [f32; 3],
    pub binormal: [f32; 3],
    pub color: [f32; 4],
    pub unknown: [f32; 4],
    pub bone_indices: [u32; 4],
    pub bone_weights: [f32; 4],
}

/// Raw submesh vertex buffer chunk (`0x0001_0005` or `0x0001_0007`).
#[derive(Debug, ChunkStreamItem)]
pub enum VertexBuffer {
    #[chunk(ids::SUBMESH_VERTEX_BUF_V1)]
    V1(Vec<VertexV1>),
    #[chunk(ids::SUBMESH_VERTEX_BUF_V2)]
    V2(Vec<VertexV2>),
}

/// Raw submesh data chunk (`0x0001_0000`).
#[derive(Debug, ChunkStructure)]
#[chunk(map)]
pub struct SubmeshData<'b> {
    #[chunk(ids::SUBMESH_INFO)]
    pub info: SubmeshInfo,
    #[chunk(ids::SUBMESH_VERTEX_FORMAT)]
    pub vertex_format: &'b str,
    #[chunk(ids::SUBMESH_INDEX_BUF)]
    pub primitives: Vec<[u16; 3]>,
    #[chunk(ids::SUBMESH_ANIM_MAP, default, wire_format = U32UsizeVec)]
    pub bone_mappings: Vec<usize>,
    #[chunk(stream)]
    pub vertices: VertexBuffer,
    #[chunk(ids::SUBMESH_COLLISION, ignore, optional)]
    pub collision: Option<()>,
}

// =================================================================================================
// ALA Animation (0x0000_1000)
// =================================================================================================

/// Raw animation file header chunk (`0x0000_1001`).
#[derive(Debug, MiniChunkStructure)]
#[mini_chunk(sequence)]
pub struct AnimationHeader {
    #[mini_chunk(ids::ANIMATION_HEADER_NUM_FRAMES, wire_format = U32Usize)]
    pub num_frames: usize,
    #[mini_chunk(ids::ANIMATION_HEADER_FPS)]
    pub fps: f32,
    #[mini_chunk(ids::ANIMATION_HEADER_NUM_BONES, wire_format = U32Usize)]
    pub num_bones: usize,
    #[mini_chunk(ids::ANIMATION_HEADER_ROT_FRAME_SIZE, optional, wire_format = U32Usize)]
    pub rot_frame_size: Option<usize>,
    #[mini_chunk(ids::ANIMATION_HEADER_TRA_FRAME_SIZE, optional, wire_format = U32Usize)]
    pub tra_frame_size: Option<usize>,
    #[mini_chunk(ids::ANIMATION_HEADER_SCA_FRAME_SIZE, optional, wire_format = U32Usize)]
    pub sca_frame_size: Option<usize>,
}

/// Raw bone animation header chunk (`0x0000_1003`).
#[derive(Debug, MiniChunkStructure)]
#[mini_chunk(map)]
pub struct BoneAnimationHeader<'b> {
    #[mini_chunk(ids::ANIMATION_BONE_DATA_HEADER_NAME)]
    pub name: &'b str,
    #[mini_chunk(ids::ANIMATION_BONE_DATA_HEADER_IDX, wire_format = U32Usize)]
    pub bone_index: usize,
    #[mini_chunk(ids::ANIMATION_BONE_DATA_HEADER_TRANS_OFFSET)]
    pub trans_offset: Vec3,
    #[mini_chunk(ids::ANIMATION_BONE_DATA_HEADER_TRANS_SCALE)]
    pub trans_scale: Vec3,
    #[mini_chunk(ids::ANIMATION_BONE_DATA_HEADER_SCALE_OFFSET)]
    pub scale_offset: Vec3,
    #[mini_chunk(ids::ANIMATION_BONE_DATA_HEADER_SCALE_SCALE)]
    pub scale_scale: Vec3,
    #[mini_chunk(ids::ANIMATION_BONE_DATA_HEADER_UNKNOWN, optional)]
    pub unknown_prop: Option<u32>,
    #[mini_chunk(ids::ANIMATION_BONE_DATA_HEADER_TRANS_IDX, optional)]
    pub translation_index: Option<u16>,
    #[mini_chunk(ids::ANIMATION_BONE_DATA_HEADER_SCALE_IDX, optional)]
    pub scale_index: Option<u16>,
    #[mini_chunk(ids::ANIMATION_BONE_DATA_HEADER_ROT_IDX, optional)]
    pub rotation_index: Option<u16>,
    #[mini_chunk(ids::ANIMATION_BONE_DATA_HEADER_DEFAULT_ROT, optional)]
    pub default_rotation: Option<[i16; 4]>,
}

/// Raw bone animation chunk (`0x0000_1002`).
#[derive(Debug, ChunkStructure)]
#[chunk(map)]
pub struct BoneAnimation<'b> {
    #[chunk(ids::ANIMATION_BONE_DATA_HEADER, mini_chunks)]
    pub header: BoneAnimationHeader<'b>,
    #[chunk(ids::ANIMATION_BONE_TRANSLATIONS, optional)]
    pub translations: Option<Vec<[u16; 3]>>,
    #[chunk(ids::ANIMATION_BONE_SCALES, optional)]
    pub scales: Option<Vec<[u16; 3]>>,
    #[chunk(ids::ANIMATION_BONE_ROTATIONS, optional)]
    pub rotations: Option<Vec<[i16; 4]>>,
    #[chunk(ids::ANIMATION_BONE_VISIBILITY, optional)]
    pub visibility: Option<Vec<u8>>,
    #[chunk(ids::ANIMATION_BONE_STEP, optional)]
    pub step: Option<Vec<u8>>,
}

/// Raw animation contents inside chunk `0x0000_1000`.
#[derive(Debug, ChunkStructure)]
#[chunk(map)]
pub struct AlaFile<'b> {
    #[chunk(ids::ANIMATION_HEADER, mini_chunks)]
    pub header: AnimationHeader,
    #[chunk(ids::ANIMATION_BONE_DATA, children, repeated)]
    pub bones: Vec<BoneAnimation<'b>>,
    #[chunk(ids::ANIMATION_ROTATIONS, optional)]
    pub rotations: Option<Vec<[i16; 4]>>,
    #[chunk(ids::ANIMATION_TRANSLATIONS, optional)]
    pub translations: Option<Vec<[i16; 3]>>,
    #[chunk(ids::ANIMATION_SCALES, optional)]
    pub scales: Option<Vec<[i16; 3]>>,
}

/// Raw top-level `.ala` file wrapper (`0x0000_1000`).
#[derive(Debug, ChunkStructure)]
#[chunk(sequence)]
pub struct AlaRoot<'b> {
    #[chunk(ids::ANIMATION_ROOT, children)]
    pub animation: AlaFile<'b>,
}

impl<'b> AlaRoot<'b> {
    pub fn parse(bytes: &'b [u8]) -> Result<Self, ChunkErrorV2> {
        iter_chunks(bytes).expect_struct()
    }
}
