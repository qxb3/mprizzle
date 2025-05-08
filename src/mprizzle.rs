use std::collections::HashMap;
use std::{sync::Arc, time::Duration};

use crate::player::MprisPlayer;
use crate::proxies::{self, DBUS_MPRIS_INTERFACE_NAME, ProxyError};
use crate::{MetadataError, identity};
use crate::{identity::PlayerIdentity, player::PlayerError};
use futures::StreamExt;
use tokio::sync::{Mutex, broadcast, mpsc};
use zbus::Connection;

/// Represents errors that can occur in MPRIS operations.
#[derive(Debug, thiserror::Error)]
pub enum MprisError {
    #[error("Failed to connect to D-BUS: {0}")]
    FailedToConnectDbus(String),

    #[error("Failed to lock the mpris shared connection: {0}.")]
    FailedToLockSharedConnection(String),

    #[error("Failed to receive mpris event.")]
    FailedToRecvEvent,

    #[error("Failed to call D-Bus function: {0}.")]
    FailedToCallFn(String, String),

    #[error("Invalid formatted bus name.")]
    InvalidBusName,

    #[error("{0}")]
    PlayerErr(#[from] PlayerError),

    #[error("{0}")]
    MetadataErr(#[from] MetadataError),

    #[error("{0}")]
    ProxyErr(#[from] ProxyError),

    #[error("{0}")]
    Other(String),
}

/// A shorthand for `Result<T, MprisError>`.
pub type MprisResult<T> = Result<T, MprisError>;

/// Represents events triggered by changes in an MPRIS media player.
pub enum MprisEvent {
    /// Triggers when a new player has been attached or added.
    /// This is the only event that has the MprisPlayer on it.
    /// You should add it on state to be managed.
    PlayerAttached(MprisPlayer),

    /// Triggers when an existing player has been detached or removed.
    PlayerDetached(PlayerIdentity),

    /// Triggers when one of the player's properties changed.
    PlayerPropertiesChanged(PlayerIdentity),

    /// Triggers when one of the player's position changed due to the user manually changing it.
    PlayerSeeked(PlayerIdentity),

    /// Triggers when one of the player's position changed.
    PlayerPosition(PlayerIdentity, Duration),
}

/// Represents an MPRIS connection.
///
/// This struct provides access to an MPRIS-compatible media player using D-Bus.
/// It allows sending commands and retrieving properties via the D-Bus connection.
///
/// # Example
///
/// ```no_run
/// use mpris::Mpris;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mpris = Mpris::new(None).await?;
///
///     // Start watching for mpris events.
///     mpris.watch();
///
///     loop {
///         let event_result = mpris.recv().await?;
///
///         match event_result {
///             Ok(event) => match event {
///                 MprisEvent::PlayerAttached(player) => println!("ATTACHED = {:?}", player.identity().short()),
///                 MprisEvent::PlayerDetached(identity) => println!("DETACHED = {:?}", identity.short()),
///             },
///             Err(err) => panic!("{:?}", err),
///         }
///     }
///
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct Mpris {
    /// The underlying connection to D-Bus.
    connection: Arc<Mutex<Connection>>,

    /// Event sender.
    sender: mpsc::UnboundedSender<MprisResult<MprisEvent>>,

    /// Event receiver.
    receiver: mpsc::UnboundedReceiver<MprisResult<MprisEvent>>,
}

impl Mpris {
    pub async fn new() -> MprisResult<Self> {
        let session = Connection::session()
            .await
            .map_err(|err| MprisError::FailedToConnectDbus(err.to_string()))?;

        let connection = Arc::new(Mutex::new(session));

        let (sender, receiver) = mpsc::unbounded_channel();

        Ok(Self {
            connection,
            sender,
            receiver,
        })
    }

    /// Start watching for mpris events.
    pub fn watch(&self) {
        let shared_connection = self.connection();
        let event_sender = self.sender();

        // Creates a broadcast channel for indicating to a player,
        // that they have been removed.
        // This channel will be sending out full bus names.
        let (close_sender, _) = broadcast::channel::<String>(69); // 69 for good measure.

        tokio::spawn(async move {
            // Creates a new dbus proxy.
            let shared_conn = Arc::clone(&shared_connection);
            let dbus_proxy = match proxies::create_dbus_proxy(shared_conn).await {
                Ok(dbus_proxy) => dbus_proxy,
                Err(err) => {
                    event_sender.send(Err(err)).unwrap();
                    return;
                }
            };

            // Creates a NameOwnerChanged signal stream.
            let mut noc_stream = match dbus_proxy.receive_signal("NameOwnerChanged").await {
                Ok(noc_stream) => noc_stream,
                Err(err) => {
                    event_sender
                        .send(Err(MprisError::Other(format!(
                            "Failed to create a stream for NameOwnerChanged: {err}"
                        ))))
                        .unwrap();

                    return;
                }
            };

            // Gets existing mpris player buses.
            let buses: Vec<String> = match dbus_proxy.call("ListNames", &()).await {
                Ok(buses) => buses,
                Err(err) => {
                    event_sender
                        .send(Err(MprisError::FailedToCallFn(
                            "ListNames".into(),
                            err.to_string(),
                        )))
                        .unwrap();

                    return;
                }
            };

            // Filter out mpris buses.
            let existing_identities = buses
                .into_iter()
                .filter_map(|bus| {
                    // Creates identity from bus.
                    let identity = PlayerIdentity::new(bus.to_string()).ok()?;
                    Some(identity)
                })
                .collect::<Vec<PlayerIdentity>>();

            // Loop over the existing players identity to add it on shared players and send out the PlayerAttached event.
            for identity in existing_identities {
                // Creates the player.
                let shared_conn = Arc::clone(&shared_connection);
                let player = match MprisPlayer::new(shared_conn, identity.clone()).await {
                    Ok(player) => player,
                    Err(err) => {
                        event_sender.send(Err(err.into())).unwrap();
                        return;
                    }
                };

                // Watch this existing player for events.
                player.watch(event_sender.clone(), close_sender.subscribe());

                // Send out PlayerAttached event along with the player.
                event_sender
                    .send(Ok(MprisEvent::PlayerAttached(player)))
                    .unwrap();
            }

            loop {
                tokio::select! {
                    // Tells tokio::select to check for the result chronologically.
                    // So it checks if event channel has been closed first, then the rest.
                    biased;

                    // Break out of the loop if the event channel has been closed.
                    _ = event_sender.closed() => break,

                    // Receive NameOwnerChanged signal.
                    Some(signal) = noc_stream.next() => {
                        if let Ok((name, old_owner, new_owner)) = signal.body().deserialize::<(String, String, String)>() {
                            // Only accepts mpris signals.
                            if !name.starts_with(DBUS_MPRIS_INTERFACE_NAME) {
                                continue;
                            }

                            // There has been a new mpris player.
                            if old_owner.is_empty() && !new_owner.is_empty() {
                                // Creates the player identity.
                                let identity = match PlayerIdentity::new(name.to_string()) {
                                    Ok(identity) => identity,
                                    Err(err) => {
                                        event_sender.send(Err(err.into())).unwrap();
                                        return;
                                    }
                                };

                                // Creates the player itself with the shared connection.
                                let shared_conn = Arc::clone(&shared_connection);
                                let player = match MprisPlayer::new(shared_conn, identity.clone()).await {
                                    Ok(player) => player,
                                    Err(err) => {
                                        event_sender.send(Err(err.into())).unwrap();
                                        return;
                                    }
                                };

                                // Watch this newly created player for events.
                                player.watch(event_sender.clone(), close_sender.subscribe());

                                // Send out PlayerAttached event along with the player.
                                event_sender.send(Ok(MprisEvent::PlayerAttached(player))).unwrap();
                            }

                            // There has been a mpris player detached.
                            if !old_owner.is_empty() && new_owner.is_empty() {
                                let identity = match PlayerIdentity::new(name.to_string()) {
                                    Ok(identity) => identity,
                                    Err(err) => {
                                        event_sender
                                            .send(Err(MprisError::Other(format!("Failed to create a player identity on detached player: {err}"))))
                                            .unwrap();

                                        return;
                                    }
                                };

                                // Sends out the event to close the async task of player.
                                close_sender.send(name).unwrap();

                                // Send out the PlayerDetached event.
                                event_sender.send(Ok(MprisEvent::PlayerDetached(identity))).unwrap();
                            }
                        }
                    }
                }
            }
        });
    }

    /// Recieve mpris events.
    pub async fn recv(&mut self) -> MprisResult<MprisResult<MprisEvent>> {
        self.receiver
            .recv()
            .await
            .ok_or(MprisError::FailedToRecvEvent)
    }

    /// Gets the shared mpris connection.
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.connection)
    }

    /// Gets the cloned event sender.
    fn sender(&self) -> mpsc::UnboundedSender<MprisResult<MprisEvent>> {
        self.sender.clone()
    }
}
