use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};

use futures::StreamExt;
use tokio::sync::{Mutex, broadcast, mpsc};
use zbus::{Connection, Proxy, zvariant};

use crate::{MprisError, MprisResult, status::PlaybackStatus};

use super::{
    MprisEvent,
    identity::PlayerIdentity,
    metadata::PlayerMetadata,
    proxies::{self, create_player_proxy, create_properties_proxy},
};

/// Represents errors that can occur in MPRIS Player operations.
#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("Failed to get player prop: {0}: {1}")]
    FailedToGetProp(String, String),

    #[error("Failed to set player prop: {0}: {1}")]
    FailedToSetProp(String, String),
}

impl PlayerError {
    pub fn failed_to_get_prop<P, E>(prop: P, err: E) -> MprisError
    where
        P: Into<String>,
        E: Into<String>,
    {
        MprisError::PlayerErr(PlayerError::FailedToGetProp(prop.into(), err.into()))
    }

    pub fn failed_to_set_prop<P, E>(prop: P, err: E) -> MprisError
    where
        P: Into<String>,
        E: Into<String>,
    {
        MprisError::PlayerErr(PlayerError::FailedToSetProp(prop.into(), err.into()))
    }
}

/// Represents an MPRIS media player instance.
///
/// This struct provides an interface to control and retrieve information from an MPRIS-compatible media player.
/// It uses a D-Bus proxy to communicate with the player and manage playback.
///
/// # Example
///
/// ```no_run
/// use mprizzle::{Mpris, MprisPlayer, PlayerIdentity};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mpris = Mpris::new().await?;
///
///     // Ideally you should never create your own player and just use the one from `mpris`
///     // but you can just create a player yourself.
///     let spotify = Player::new(mpris.connection(), PlayerIdentity::new("org.mpris.MediaPlayer2.spotify".into())).await?;
///
///     let metadata = spotify.metadata().await?;
///
///     let title = metadata.title()?.unwrap_or("No Title".into());
///     println("Current song: {title}");
///
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct MprisPlayer {
    /// A shared D-Bus connection.
    connection: Arc<Mutex<Connection>>,

    /// Player proxy.
    player_proxy: Proxy<'static>,

    /// The identity of this player.
    identity: PlayerIdentity,
}

impl MprisPlayer {
    pub async fn new(
        shared_connection: Arc<Mutex<Connection>>,
        identity: PlayerIdentity,
    ) -> MprisResult<Self> {
        let shared_conn = Arc::clone(&shared_connection);
        let player_proxy = proxies::create_player_proxy(shared_conn, identity.bus()).await?;

        Ok(Self {
            connection: shared_connection,
            player_proxy,
            identity,
        })
    }

    /// Start watching for player events.
    pub fn watch(
        &self,
        event_sender: mpsc::UnboundedSender<MprisResult<MprisEvent>>,
        mut close_rx: broadcast::Receiver<String>,
    ) {
        let shared_connection = self.connection();
        let identity = self.identity().clone();

        tokio::spawn(async move {
            // Creates a properties proxy.
            let shared_conn = Arc::clone(&shared_connection);
            let properties_proxy = match create_properties_proxy(shared_conn, identity.bus()).await
            {
                Ok(properties_proxy) => properties_proxy,
                Err(err) => {
                    event_sender.send(Err(err.into())).unwrap();
                    return;
                }
            };

            // Creates a player proxy.
            let shared_conn = Arc::clone(&shared_connection);
            let player_proxy = match create_player_proxy(shared_conn, identity.bus()).await {
                Ok(player_proxy) => player_proxy,
                Err(err) => {
                    event_sender.send(Err(err.into())).unwrap();
                    return;
                }
            };

            // Creates a PropertiesChanged signal stream.
            let mut prop_changed_stream =
                match properties_proxy.receive_signal("PropertiesChanged").await {
                    Ok(properties_changed) => properties_changed,
                    Err(err) => {
                        event_sender
                            .send(Err(MprisError::Other(format!(
                                "Failed to create a signal stream for PropertiesChanged: {err}"
                            ))))
                            .unwrap();

                        return;
                    }
                };

            // Creates a Seeked signal stream.
            let mut seeked_stream = match player_proxy.receive_signal("Seeked").await {
                Ok(seeked_stream) => seeked_stream,
                Err(err) => {
                    event_sender
                        .send(Err(MprisError::Other(format!(
                            "Failed to create a signal stream for Seeked: {err}"
                        ))))
                        .unwrap();

                    return;
                }
            };

            // Create a ticker that tick each seconds to tick me.
            let mut tickler = tokio::time::interval(Duration::from_secs(1));

            loop {
                tokio::select! {
                    // Tells tokio::select to check for the result chronologically.
                    // So it checks if event channel has been closed or
                    // if this player should stop receiving events first, then the rest.
                    biased;

                    // Break out of the loop if the event channel has been closed.
                    _ = event_sender.closed() => break,

                    // Break out of the loop if the close channel event bus matches the identity.
                    close_res = close_rx.recv() => {
                        let bus = match close_res {
                            Ok(bus) => bus,
                            Err(err) => {
                                event_sender.send(Err(MprisError::Other(format!("Failed to receive close event: {err}")))).unwrap();
                                break;
                            }
                        };

                        // Break if it checks out.
                        if identity.matches_bus_prefix(&bus) {
                            break
                        }
                    },

                    // Receive PropertiesChanged signal.
                    Some(_) = prop_changed_stream.next() => {
                        // Send out PlayerPropertiesChanged event.
                        event_sender.send(Ok(MprisEvent::PlayerPropertiesChanged(identity.clone()))).unwrap();
                    },

                    // Receive Seeked signal.
                    Some(_) = seeked_stream.next() => {
                        // Send out PlayerSeeked event.
                        event_sender.send(Ok(MprisEvent::PlayerSeeked(identity.clone()))).unwrap();
                    },

                    // Tick that tickler!
                    _ = tickler.tick() => {
                        // Gets the player playback status from D-Bus.
                        let playback_status: String = match player_proxy.get_property("PlaybackStatus").await {
                            Ok(playback_status) => playback_status,
                            Err(err) => {
                                event_sender.send(Err(PlayerError::failed_to_get_prop("PlaybackStatus", err.to_string()))).unwrap();
                                return;
                            }
                        };

                        // Converts the playback status into PlaybackStatus type.
                        let playback_status = match PlaybackStatus::from_str(&playback_status) {
                            Ok(playback_status) => playback_status,
                            Err(err) => {
                                event_sender.send(Err(MprisError::Other(format!("Failed to parse playback status: {err}")))).unwrap();
                                return;
                            }
                        };

                        // Only send out the PlayerPosition event if the playback is Playing.
                        if playback_status == PlaybackStatus::Playing {
                            // Gets the player position from the D-Bus.
                            let position: i64 = match player_proxy.get_property("Position").await {
                                Ok(position) => position,
                                Err(err) => {
                                event_sender.send(Err(PlayerError::failed_to_get_prop("Position", err.to_string()))).unwrap();
                                    return;
                                }
                            };

                            // Converts the player position into Duration type.
                            let position = Duration::from_micros(position as u64);

                            // Send out PlayerPosition event.
                            event_sender.send(Ok(MprisEvent::PlayerPosition(identity.clone(), position))).unwrap();
                        }
                    },
                }
            }
        });
    }

    /// Metadata of player.
    pub async fn metadata(&self) -> MprisResult<PlayerMetadata> {
        let metadata: HashMap<String, zvariant::Value> = self
            .player_proxy
            .get_property("Metadata")
            .await
            .map_err(|err| PlayerError::failed_to_get_prop("Metadata", err.to_string()))?;

        Ok(PlayerMetadata::new(metadata))
    }

    /// Playback Rate of player.
    pub async fn playback_rate(&self) -> MprisResult<f64> {
        let rate: f64 = self
            .player_proxy
            .get_property("Rate")
            .await
            .map_err(|err| PlayerError::failed_to_get_prop("Rate", err.to_string()))?;

        Ok(rate)
    }

    /// Set Playback Rate of player.
    pub async fn set_playback_rate(&self, rate: f64) -> MprisResult<()> {
        if !self.can_control().await? {
            return Err(PlayerError::failed_to_set_prop(
                "Rate",
                "Cannot set the Rate when CanControl is false",
            ));
        }

        let min_rate = self.min_playback_rate().await?;
        let max_rate = self.max_playback_rate().await?;

        if rate < min_rate || rate > max_rate {
            return Err(PlayerError::failed_to_set_prop(
                "Rate",
                "Cannot set the Rate when its passed the MinimumRate or MaximumRate bounds",
            ));
        }

        self.player_proxy
            .set_property("Rate", rate)
            .await
            .map_err(|err| PlayerError::failed_to_get_prop("Rate", err.to_string()))?;

        Ok(())
    }

    /// Minimum Playback Rate of player.
    pub async fn min_playback_rate(&self) -> MprisResult<f64> {
        let min_rate: f64 = self
            .player_proxy
            .get_property("MinimumRate")
            .await
            .map_err(|err| PlayerError::failed_to_get_prop("MinimumRate", err.to_string()))?;

        Ok(min_rate)
    }

    /// Maximum Playback Rate of player.
    pub async fn max_playback_rate(&self) -> MprisResult<f64> {
        let max_rate: f64 = self
            .player_proxy
            .get_property("MaximumRate")
            .await
            .map_err(|err| PlayerError::failed_to_get_prop("MaximumRate", err.to_string()))?;

        Ok(max_rate)
    }

    /// Can the player go next.
    pub async fn can_next(&self) -> MprisResult<bool> {
        let can_go_next: bool = self
            .player_proxy
            .get_property("CanGoNext")
            .await
            .map_err(|err| PlayerError::failed_to_get_prop("CanGoNext", err.to_string()))?;

        Ok(can_go_next)
    }

    /// Can the player go previous.
    pub async fn can_previous(&self) -> MprisResult<bool> {
        let can_go_previous: bool = self
            .player_proxy
            .get_property("CanGoPrevious")
            .await
            .map_err(|err| PlayerError::failed_to_get_prop("CanGoPrevious", err.to_string()))?;

        Ok(can_go_previous)
    }

    /// Can the player play.
    pub async fn can_play(&self) -> MprisResult<bool> {
        let can_play: bool = self
            .player_proxy
            .get_property("CanPlay")
            .await
            .map_err(|err| PlayerError::failed_to_get_prop("CanPlay", err.to_string()))?;

        Ok(can_play)
    }

    /// Can the player pause.
    pub async fn can_pause(&self) -> MprisResult<bool> {
        let can_pause: bool = self
            .player_proxy
            .get_property("CanPause")
            .await
            .map_err(|err| PlayerError::failed_to_get_prop("CanPause", err.to_string()))?;

        Ok(can_pause)
    }

    /// Can the player seek.
    pub async fn can_seek(&self) -> MprisResult<bool> {
        let can_seek: bool = self
            .player_proxy
            .get_property("CanSeek")
            .await
            .map_err(|err| PlayerError::failed_to_get_prop("CanSeek", err.to_string()))?;

        Ok(can_seek)
    }

    /// Can the player be controlled.
    pub async fn can_control(&self) -> MprisResult<bool> {
        let can_control: bool = self
            .player_proxy
            .get_property("CanControl")
            .await
            .map_err(|err| PlayerError::failed_to_get_prop("CanControl", err.to_string()))?;

        Ok(can_control)
    }

    /// Gets the shared mpris connection.
    fn connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.connection)
    }

    /// Gets the identity of the player.
    pub fn identity(&self) -> &PlayerIdentity {
        &self.identity
    }
}
