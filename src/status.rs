use core::fmt;
use std::str::FromStr;

use crate::MprisError;

/// Playback status of a player.
#[derive(Debug, PartialEq, Clone)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

/// Loop status of a player.
#[derive(Debug, PartialEq)]
pub enum LoopStatus {
    None,
    Track,
    Playlist,
}

impl FromStr for PlaybackStatus {
    type Err = MprisError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "playing" => Ok(PlaybackStatus::Playing),
            "paused" => Ok(PlaybackStatus::Paused),
            "stopped" => Ok(PlaybackStatus::Stopped),
            _ => Err(MprisError::Other(
                "PlaybackStatus is not Playing, Paused or Stopped".into(),
            )),
        }
    }
}

impl AsRef<str> for PlaybackStatus {
    fn as_ref(&self) -> &str {
        match self {
            PlaybackStatus::Playing => "Playing",
            PlaybackStatus::Paused => "Paused",
            PlaybackStatus::Stopped => "Stopped",
        }
    }
}

impl fmt::Display for PlaybackStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl FromStr for LoopStatus {
    type Err = MprisError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(LoopStatus::None),
            "track" => Ok(LoopStatus::Track),
            "playlist" => Ok(LoopStatus::Playlist),
            _ => Err(MprisError::Other(
                "LoopStatus is not None, Track or Playlist.".into(),
            )),
        }
    }
}

impl AsRef<str> for LoopStatus {
    fn as_ref(&self) -> &str {
        match self {
            LoopStatus::None => "None",
            LoopStatus::Track => "Track",
            LoopStatus::Playlist => "Playlist",
        }
    }
}

impl fmt::Display for LoopStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}
