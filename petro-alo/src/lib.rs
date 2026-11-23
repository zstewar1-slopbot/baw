//! Provides support for reading (but not writing) Petroglyph's ALO model/particle files and ALA
//! animation files. Based on Mike Lankamps's excellent documentation of these formats at
//! [https://glyphxtools.github.io].

use std::collections::{HashMap, HashSet};

use anyhow::Context;
use thiserror::Error;

use crate::alternatives::{Alt, Lod, LodGroup};
use crate::animations::BoneAnimation;
use crate::connections::Connections;
pub use crate::error::AloError;
use crate::error::FormatError;
use crate::mesh::Mesh;
use crate::skeleton::{Proxy, Skeleton};

pub mod alternatives;
pub mod animations;
mod connections;
mod error;
mod ids;
pub mod mesh;
pub mod raw;
pub mod skeleton;

/// An ALO model or particle system.
#[derive(Debug)]
pub struct AloFile {
    pub skeleton: Skeleton,
    pub meshes: Vec<Mesh>,
}

impl AloFile {
    pub fn parse(bytes: &[u8]) -> Result<Self, AloError> {
        let raw = raw::AloFile::parse(bytes).context("parsing raw ALO chunk structure")?;
        Ok(Self::from_raw(raw)?)
    }

    pub fn from_raw(raw: raw::AloFile<'_>) -> anyhow::Result<Self> {
        #[derive(Copy, Clone)]
        enum ConnectionIdx {
            Mesh(usize),
            Light(#[allow(unused)] usize),
        }

        let mut skeleton = match raw.skeleton {
            Some(raw_skel) => Skeleton::from_raw(raw_skel).context("parsing skeleton")?,
            None => Skeleton::default(),
        };

        let mut meshes = Vec::new();
        let mut lights = 0;
        let mut connection_indices = Vec::with_capacity(raw.objects.len());

        for obj in raw.objects {
            match obj {
                raw::Object::Mesh(raw_mesh) => {
                    #[derive(Error, Debug)]
                    #[error("parsing mesh {idx}")]
                    struct MeshCtx {
                        idx: usize,
                    }
                    let idx = meshes.len();
                    let mesh = Mesh::from_raw(raw_mesh).context(MeshCtx { idx })?;
                    connection_indices.push(ConnectionIdx::Mesh(idx));
                    meshes.push(mesh);
                }
                raw::Object::Light => {
                    connection_indices.push(ConnectionIdx::Light(lights));
                    lights += 1;
                }
            }
        }

        let connections = match raw.connections {
            Some(raw_conn) => Some(Connections::from_raw(raw_conn).context("parsing connections")?),
            None => None,
        };

        #[derive(Error, Debug, Copy, Clone)]
        #[error("validating submesh {smidx} of mesh {midx}")]
        struct SubmeshValidationCtx {
            midx: usize,
            smidx: usize,
        }

        let mut lod_groups = HashMap::<&str, LodGroup>::new();
        for (midx, mesh) in meshes.iter_mut().enumerate() {
            if mesh.lod.is_some() {
                let lod_group_name = Lod::lod_group_name(&mesh.name);
                let next_lod_group_id = lod_groups.len();
                mesh.lod_group = *lod_groups
                    .entry(lod_group_name)
                    .or_insert(LodGroup(next_lod_group_id));
            }
            for (smidx, submesh) in mesh.submeshes.iter().enumerate() {
                let ctx = SubmeshValidationCtx { midx, smidx };
                for &bone_index in &submesh.bone_mappings {
                    if bone_index as u64 >= skeleton.bones.len() as u64 {
                        Err(FormatError::MappingIndexOutOfRange {
                            idx: bone_index,
                            count: skeleton.bones.len(),
                        })
                        .context(ctx)?;
                    }
                }
            }
        }

        if let Some(connections) = connections {
            #[derive(Error, Debug, Copy, Clone)]
            #[error("wiring object connection {obj_idx}")]
            struct ObjConnCtx {
                obj_idx: usize,
            }
            let mut parented_meshes = HashSet::<usize>::new();
            for (obj_idx, obj_conn) in connections.objects.into_iter().enumerate() {
                let ctx = ObjConnCtx { obj_idx };
                if obj_conn.bone >= skeleton.bones.len() {
                    Err(FormatError::ConnBoneOutOfRange {
                        idx: obj_conn.bone,
                        count: skeleton.bones.len(),
                    })
                    .context(ctx)?;
                }

                let obj = connection_indices
                    .get(obj_conn.object)
                    .ok_or(FormatError::ConnObjectOutOfRange {
                        idx: obj_conn.object,
                        count: connection_indices.len(),
                    })
                    .context(ctx)?;
                match *obj {
                    ConnectionIdx::Mesh(mesh_idx) => {
                        if !parented_meshes.insert(mesh_idx) {
                            Err(FormatError::MultiParentMesh { mesh_idx }).context(ctx)?;
                        }
                        meshes[mesh_idx].parent_bone = obj_conn.bone;
                    }
                    ConnectionIdx::Light(_) => {}
                }
            }
            if parented_meshes.len() < meshes.len() {
                Err(FormatError::DetachedMeshes {
                    count: meshes.len() - parented_meshes.len(),
                })?;
            }
            #[derive(Error, Debug, Copy, Clone)]
            #[error("wiring proxy connection {proxy_idx}")]
            struct ProxyConnCtx {
                proxy_idx: usize,
            }
            for (proxy_idx, proxy_conn) in connections.proxies.into_iter().enumerate() {
                let ctx = ProxyConnCtx { proxy_idx };
                let num_bones = skeleton.bones.len();
                let bone = skeleton
                    .bones
                    .get_mut(proxy_conn.bone)
                    .ok_or(FormatError::ConnObjectOutOfRange {
                        idx: proxy_conn.bone,
                        count: num_bones,
                    })
                    .context(ctx)?;
                bone.proxies.push(Proxy {
                    alt: Alt::from_name(&proxy_conn.proxy_name),
                    name: proxy_conn.proxy_name,
                    hidden: proxy_conn.hidden,
                    alt_decrease_stay_hidden: proxy_conn.alt_decrease_stay_hidden,
                });
            }
        } else if !meshes.is_empty() || lights > 0 {
            Err(FormatError::MissingChunk {
                id: ids::CONNECTION,
            })?;
        }

        Ok(AloFile { skeleton, meshes })
    }
}

/// An ALA animation file.
#[derive(Debug)]
pub struct AlaFile {
    /// Number of animation frames per second.
    pub fps: f32,
    /// Animations to apply to various bones.
    pub bone_animations: Vec<BoneAnimation>,
}

impl AlaFile {
    pub fn parse(bytes: &[u8]) -> Result<Self, AloError> {
        let raw_root = raw::AlaRoot::parse(bytes).context("parsing raw ALA chunk structure")?;
        Ok(Self::from_raw(raw_root.animation)?)
    }
}
