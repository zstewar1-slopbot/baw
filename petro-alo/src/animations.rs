// Temporary because this module is incomplete.
#![allow(unused)]

use anyhow::Context;
use thiserror::Error;

use crate::AlaFile;
use crate::error::FormatError;
use crate::raw;

impl AlaFile {
    pub(crate) fn from_raw(raw: raw::AlaFile<'_>) -> anyhow::Result<Self> {
        match (
            raw.header.rot_frame_size,
            raw.header.tra_frame_size,
            raw.header.sca_frame_size,
        ) {
            (None, None, None) => Self::from_raw_v1(raw),
            (Some(rot_frame_size), Some(tra_frame_size), Some(sca_frame_size)) => {
                Self::from_raw_v2(raw, rot_frame_size, tra_frame_size, sca_frame_size)
            }
            (_, _, _) => Err(FormatError::MixedAnimationFormats)?,
        }
    }

    pub(crate) fn from_raw_v1(raw: raw::AlaFile<'_>) -> anyhow::Result<Self> {
        let num_bones = raw.header.num_bones;
        if raw.bones.len() != num_bones {
            Err(FormatError::WrongContentCount {
                expected: num_bones,
                actual: raw.bones.len(),
            })?;
        }

        #[derive(Error, Debug, Copy, Clone)]
        #[error("parsing the bone data (chunk 0x00001002) for bone index {bidx}")]
        struct BoneAnimCtx {
            bidx: usize,
        }

        let mut bone_animations = Vec::with_capacity(num_bones);
        for (bidx, raw_bone) in raw.bones.into_iter().enumerate() {
            let ctx = BoneAnimCtx { bidx };
            let animation =
                BoneAnimation::from_raw_v1(raw_bone, raw.header.num_frames).context(ctx)?;
            bone_animations.push(animation);
        }

        Ok(AlaFile {
            fps: raw.header.fps,
            bone_animations,
        })
    }

    pub(crate) fn from_raw_v2(
        raw: raw::AlaFile<'_>,
        rot_frame_size: usize,
        tra_frame_size: usize,
        sca_frame_size: usize,
    ) -> anyhow::Result<Self> {
        todo!()
    }
}

#[derive(Debug)]
pub struct BoneAnimation {
    /// Name of the bone this animation applies to.
    pub name: String,
    /// Index of the bone in the model that this animation applies to. This should be used for
    /// animation linking rather than the name.
    pub bone_index: usize,
    /// Unknown property from the animation header.
    pub unknown_prop: Option<u32>,
}

impl BoneAnimation {
    fn from_raw_v1(raw: raw::BoneAnimation<'_>, num_frames: usize) -> anyhow::Result<Self> {
        Ok(Self {
            name: raw.header.name.to_owned(),
            bone_index: raw.header.bone_index,
            unknown_prop: raw.header.unknown_prop,
        })
    }
}
