use std::{collections::HashMap, time::Duration};

use crate::{MprisError, MprisResult};

/// Represents errors that can occur in MPRIS Metadata operations.
#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("The field `{field}` expected: `{expected}` but got: {got}.")]
    MetadataInvalidFieldType {
        field: String,
        expected: String,
        got: String,
    },
}

/// A custom wrapper type for representing a track identifier.
#[derive(Debug, Clone)]
pub struct TrackId(String);

impl AsRef<str> for TrackId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Represents the metadata of an MPRIS media player.
///
/// This struct stores key-value pairs of metadata properties retrieved from an MPRIS-compatible player.
/// Metadata includes information such as track title, artist, album, playback details, etc.
#[derive(Debug)]
pub struct PlayerMetadata<'a> {
    metadata: HashMap<String, zvariant::Value<'a>>,
}

impl<'a> PlayerMetadata<'a> {
    /// Creates a new `PlayerMetadata` instance from a given metadata `HashMap`.
    /// You'd typically will not be creating new metadata yourselve and interact
    /// On the one from the [`crate::player::MprisPlayer`]
    ///
    /// # Arguments
    ///
    /// * `metadata` - A `HashMap<String, zvariant::Value<'a>>` containing metadata key-value pairs.
    pub fn new(metadata: HashMap<String, zvariant::Value<'a>>) -> Self {
        Self { metadata }
    }

    /// Metadata mpris:trackid.
    ///
    /// Returns Err when mpris:trackid is somehow a different type.
    /// Returns None when mpris:trackid doesn't exists.
    pub fn track_id(&self) -> MprisResult<Option<TrackId>> {
        self.metadata
            .get("mpris:trackid")
            .map(|track_id| match track_id {
                zvariant::Value::Str(track_id) => Ok(Some(TrackId(track_id.to_string()))),
                zvariant::Value::ObjectPath(track_id) => Ok(Some(TrackId(track_id.to_string()))),
                _ => Err(MprisError::MetadataErr(
                    MetadataError::MetadataInvalidFieldType {
                        field: "mpris:trackid".into(),
                        expected: "s or o".into(),
                        got: track_id.value_signature().to_string(),
                    },
                )),
            })
            .unwrap_or(Ok(None))
    }

    /// Metadata xesam:title.
    ///
    /// Returns Err when xesam:title is somehow a different type.
    /// Returns None when xesam:title doesn't exists.
    pub fn title(&self) -> MprisResult<Option<String>> {
        self.metadata
            .get("xesam:title")
            .map(|title| match title {
                zvariant::Value::Str(title) => Ok(Some(title.to_string())),
                _ => Err(MprisError::MetadataErr(
                    MetadataError::MetadataInvalidFieldType {
                        field: "xesam:title".into(),
                        expected: "s".into(),
                        got: title.value_signature().to_string(),
                    },
                )),
            })
            .unwrap_or(Ok(None))
    }

    /// Metadata xesam:album.
    ///
    /// Returns Err when xesam:album is somehow a different type.
    /// Returns None when xesam:album doesn't exists.
    pub fn album(&self) -> MprisResult<Option<String>> {
        self.metadata
            .get("xesam:album")
            .map(|album| match album {
                zvariant::Value::Str(album) => Ok(Some(album.to_string())),
                _ => Err(MprisError::MetadataErr(
                    MetadataError::MetadataInvalidFieldType {
                        field: "xesam:album".into(),
                        expected: "s".into(),
                        got: album.value_signature().to_string(),
                    },
                )),
            })
            .unwrap_or(Ok(None))
    }

    /// Metadata xesam:artist.
    ///
    /// Returns Err when xesam:artist is somehow a different type.
    /// Returns None when xesam:artist doesn't exists.
    pub fn artists(&self) -> MprisResult<Option<Vec<String>>> {
        self.metadata
            .get("xesam:artist")
            .map(|artists| match artists {
                zvariant::Value::Array(artists) => {
                    let artists: Vec<String> = artists
                        .iter()
                        .filter_map(|a| a.downcast_ref::<&str>().map(|s| s.to_string()).ok())
                        .collect();

                    Ok(Some(artists))
                }
                _ => Err(MprisError::MetadataErr(
                    MetadataError::MetadataInvalidFieldType {
                        field: "xesam:artist".into(),
                        expected: "as".into(),
                        got: artists.value_signature().to_string(),
                    },
                )),
            })
            .unwrap_or(Ok(None))
    }

    /// Metadata mpris:length.
    ///
    /// Returns Err when mpris:length is somehow a different type.
    /// Returns None when mpris:length doesn't exists.
    pub fn length(&self) -> MprisResult<Option<Duration>> {
        self.metadata
            .get("mpris:length")
            .map(|length| match length {
                zvariant::Value::I64(length) => Ok(Some(Duration::from_micros(*length as u64))),
                zvariant::Value::U64(length) => Ok(Some(Duration::from_micros(*length))),
                _ => Err(MprisError::MetadataErr(
                    MetadataError::MetadataInvalidFieldType {
                        field: "mpris:length".into(),
                        expected: "x or u64".into(),
                        got: length.value_signature().to_string(),
                    },
                )),
            })
            .unwrap_or(Ok(None))
    }

    /// Metadata mpris:artUrl.
    ///
    /// Returns Err when mpris:artUrl is somehow a different type.
    /// Returns None when mpris:artUrl doesn't exists.
    pub fn art_url(&self) -> MprisResult<Option<String>> {
        self.metadata
            .get("mpris:artUrl")
            .map(|art_url| match art_url {
                zvariant::Value::Str(art_url) => Ok(Some(art_url.to_string())),
                _ => Err(MprisError::MetadataErr(
                    MetadataError::MetadataInvalidFieldType {
                        field: "mpris:artUrl".into(),
                        expected: "s".into(),
                        got: art_url.value_signature().to_string(),
                    },
                )),
            })
            .unwrap_or(Ok(None))
    }
}
