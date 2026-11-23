use anyhow::Context;
use glam::{Mat4, Vec4};
use petro_chunked::byteblock::FromByteBlock;
use thiserror::Error;

use crate::alternatives::Alt;
use crate::error::FormatError;
use crate::raw;

/// How a bone should apply billboarding.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum BillboardMode {
    /// No billboarding.
    #[default]
    Disable = 0,
    /// Rotate so the negative Y axis faces the camera.
    Parallel = 1,
    /// Effectively identical to Parallel.
    Face = 2,
    /// Rotate around only the Z-Axis to face the camera.
    ZAxisView = 3,
    /// Rotate only around the Z-Axis but face the light instead of the camera.
    ZAxisLight = 4,
    /// Rotate only around the Z-Axis, but parallel to the wind instead of the camera.
    ZAxisWind = 5,
    /// Used to orient the texture used for the "glow" of "atmosphere" on planets in e.g. the
    /// galactic view. Rotates around the root rather than its own center.
    SunlightGlow = 6,
    /// Rotates a plane positioned at <x, 0, 0> so that its local -Y faces the camera, then rotates
    /// around the parent Z and X to position it in the correct spot in the sky for the light
    /// direction.
    Sun = 7,
}

impl<'b> FromByteBlock<'b> for BillboardMode {
    fn take_from(bytes: &'b [u8]) -> Result<(Self, &'b [u8]), petro_chunked::ChunkError> {
        let (mode, rest) = u32::take_from(bytes)?;
        let mode = match mode {
            0 => Self::Disable,
            1 => Self::Parallel,
            2 => Self::Face,
            3 => Self::ZAxisView,
            4 => Self::ZAxisLight,
            5 => Self::ZAxisWind,
            6 => Self::SunlightGlow,
            7 => Self::Sun,
            _ => Err(FormatError::UnrecognizedBillboardMode { mode })?,
        };
        Ok((mode, rest))
    }
}

/// A proxy object that gets loaded and automatically attached to a bone.
#[derive(Debug)]
pub struct Proxy {
    /// Name of the proxy object to attach.
    pub name: String,
    /// Alt level that this proxy should be shown at. If None, this proxy does not respond to alt
    /// leveling.
    pub alt: Option<Alt>,
    /// Whether this proxy is initially hidden.
    pub hidden: bool,
    /// For an ALT proxy, don't make the proxy visible when the alt-level changes to the target alt
    /// by decreasing.
    pub alt_decrease_stay_hidden: bool,
}

/// A bone in an ALO file.
#[derive(Debug)]
pub struct Bone {
    /// Name of the bone.
    pub name: String,
    /// Index of the parent bone, if any.
    ///
    /// Every bone other than the first (root) will have a parent.
    pub parent: Option<usize>,
    /// Whether things attached to the bone are visible.
    pub visible: bool,
    /// TODO: figure out how to interpet the billboard flags.
    pub billboard: BillboardMode,
    /// Matrix which transforms from this bone to the parent bone.
    pub local_to_parent: Mat4,
    /// Matrix which transforms from this bone to global space.
    pub local_to_global: Mat4,
    /// List of proxies that should attach to this bone.
    pub proxies: Vec<Proxy>,
}

impl Bone {
    fn from_raw(raw: raw::Bone<'_>, existing_bones: &[Bone]) -> anyhow::Result<Self> {
        let (parent, visible, billboard, matrix) = match raw.data {
            raw::BoneData::V1(v1) => (v1.parent, v1.visible, BillboardMode::Disable, v1.matrix),
            raw::BoneData::V2(v2) => (v2.parent, v2.visible, v2.billboard, v2.matrix),
        };
        let (parent, parent_to_global) = match parent {
            u32::MAX => {
                // Only the first bone is parentless.
                if !existing_bones.is_empty() {
                    Err(FormatError::MisorderedBone {
                        idx: existing_bones.len(),
                        // Even if this overflows (which it never will on any supported platform) it
                        // will still appear as all ones.
                        parent: u32::MAX as usize,
                    })?;
                }
                (None, Mat4::IDENTITY)
            }
            parent => {
                let parent: usize = parent
                    .try_into()
                    .context("converting parent index to usize")?;
                if parent >= existing_bones.len() {
                    Err(FormatError::MisorderedBone {
                        idx: existing_bones.len(),
                        parent,
                    })?;
                }
                (Some(parent), existing_bones[parent].local_to_global)
            }
        };
        let local_to_parent = Mat4::from_cols(
            Vec4::from_array(matrix[0]),
            Vec4::from_array(matrix[1]),
            Vec4::from_array(matrix[2]),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        )
        .transpose();
        let local_to_global = parent_to_global * local_to_parent;

        Ok(Bone {
            name: raw.name.to_owned(),
            parent,
            visible,
            billboard,
            local_to_parent,
            local_to_global,
            proxies: Vec::new(),
        })
    }
}

/// The Skeleton from an ALO file.
#[derive(Default, Debug)]
pub struct Skeleton {
    /// The bones that make up the skeleton.
    pub bones: Vec<Bone>,
}

impl Skeleton {
    pub(crate) fn from_raw(raw: raw::Skeleton<'_>) -> anyhow::Result<Self> {
        let bone_count = raw.header.bone_count;
        if raw.bones.len() != bone_count {
            Err(FormatError::WrongContentCount {
                expected: bone_count,
                actual: raw.bones.len(),
            })?;
        }

        #[derive(Error, Debug, Copy, Clone)]
        #[error("parsing bone {bone_idx} / {bone_count}")]
        struct BoneCtx {
            bone_count: usize,
            bone_idx: usize,
        }

        let mut bones = Vec::with_capacity(bone_count);
        for (bone_idx, raw_bone) in raw.bones.into_iter().enumerate() {
            let ctx = BoneCtx {
                bone_count,
                bone_idx,
            };
            let bone = Bone::from_raw(raw_bone, &bones).context(ctx)?;
            bones.push(bone);
        }

        Ok(Self { bones })
    }
}
