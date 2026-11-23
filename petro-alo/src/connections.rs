use crate::error::FormatError;
use crate::raw;

/// Connections found in the ALO file.
pub(crate) struct Connections {
    pub(crate) objects: Vec<ObjectConnection>,
    pub(crate) proxies: Vec<ProxyConnection>,
}

impl Connections {
    pub(crate) fn from_raw(raw: raw::Connections<'_>) -> anyhow::Result<Self> {
        if raw.objects.len() != raw.count.object_count {
            Err(FormatError::WrongContentCount {
                expected: raw.count.object_count,
                actual: raw.objects.len(),
            })?;
        }
        if raw.proxies.len() != raw.count.proxy_count {
            Err(FormatError::WrongContentCount {
                expected: raw.count.proxy_count,
                actual: raw.proxies.len(),
            })?;
        }
        if let Some(dazzle_count) = raw.count.dazzle_count
            && raw.dazzles.len() != dazzle_count
        {
            Err(FormatError::WrongContentCount {
                expected: dazzle_count,
                actual: raw.dazzles.len(),
            })?;
        }

        let objects = raw
            .objects
            .into_iter()
            .map(|o| ObjectConnection {
                object: o.object,
                bone: o.bone,
            })
            .collect();

        let proxies = raw
            .proxies
            .into_iter()
            .map(|p| ProxyConnection {
                proxy_name: p.proxy_name.to_owned(),
                bone: p.bone,
                hidden: p.hidden,
                alt_decrease_stay_hidden: p.alt_decrease_stay_hidden,
            })
            .collect();

        Ok(Self { objects, proxies })
    }
}

/// An object connection.
pub(crate) struct ObjectConnection {
    pub(crate) object: usize,
    pub(crate) bone: usize,
}

/// A proxy connection.
pub(crate) struct ProxyConnection {
    pub(crate) proxy_name: String,
    pub(crate) bone: usize,
    pub(crate) hidden: bool,
    /// When the Alt level of a model is reduced, don't show this proxy. Only show it when the alt
    /// level is reached by increasing alt level.
    pub(crate) alt_decrease_stay_hidden: bool,
}
