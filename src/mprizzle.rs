use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use crate::proxies::{self, ProxyError};
use crate::MetadataError;
use crate::{identity::PlayerIdentity, player::PlayerError};
use crate::player::MprisPlayer;
use tokio::sync::{Mutex, broadcast, mpsc};
use zbus::Connection;

/// The interface of mpris over D-Bus.
pub const DBUS_MPRIS_INTERFACE_NAME: &str = "org.mpris.MediaPlayer2";

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
    PlayerAttached(PlayerIdentity),

    /// Triggers when an existing player has been detached or removed.
    PlayerDetached(PlayerIdentity),

    /// Triggers when one of the player's properties changed.
    PlayerPropertiesChanged(PlayerIdentity),

    /// Triggers when one of the player's position changed due to the user manually changing it.
    PlayerSeeked(PlayerIdentity),

    /// Triggers when one of the player's position changed.
    PlayerPosition(PlayerIdentity, Duration),
}

/// Configuration options for controlling MPRIS behavior.
#[derive(Debug, Clone)]
pub struct MprisOptions<T: IntoIterator<Item = &'static str> + Clone + Send> {
    /// A list of player names to filter.
    /// Only players whose names match entries in this iterator will be added.
    pub filter_players: T,
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
///     let mpris = Mpris::new_without_options().await?;
///
///     // Start watching for mpris events.
///     mpris.watch();
///
///     loop {
///         let event_result = mpris.recv().await?;
///
///         match event_result {
///             Ok(event) => match event {
///                 MprisEvent::PlayerAttached(identity) => println!("ATTACHED = {:?}", identity),
///                 MprisEvent::PlayerDetached(identity) => println!("DETACHED = {:?}", identity),
///             },
///             Err(err) => panic!("{:?}", err),
///         }
///     }
///
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct Mpris<T: IntoIterator<Item = &'static str> + Clone + Send> {
    /// The underlying connection to D-Bus.
    connection: Arc<Mutex<Connection>>,

    /// The current active players.
    players: Arc<Mutex<Vec<MprisPlayer>>>,

    /// Mpris options passed.
    options: Option<MprisOptions<T>>,

    /// Event sender.
    sender: mpsc::UnboundedSender<MprisResult<MprisEvent>>,

    /// Event receiver.
    receiver: mpsc::UnboundedReceiver<MprisResult<MprisEvent>>,
}

impl<T: IntoIterator<Item = &'static str> + Clone + Send + 'static> Mpris<T> {
    pub async fn new(options: Option<MprisOptions<T>>) -> MprisResult<Self> {
        let session = Connection::session()
            .await
            .map_err(|err| MprisError::FailedToConnectDbus(err.to_string()))?;

        let connection = Arc::new(Mutex::new(session));

        let players = Arc::new(Mutex::new(Vec::new()));
        let (sender, receiver) = mpsc::unbounded_channel();

        Ok(Self {
            connection,
            players,
            options,
            sender,
            receiver,
        })
    }

    /// Start watching for mpris events.
    pub fn watch(&self) {
        let shared_connection = self.connection();
        let shared_players = self.players();
        let options = self.options();

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

            // Filter only mpris buses and if the bus is on the filter_players option.
            let existing_players_identity = buses
                .into_iter()
                .filter_map(|bus| {
                    // Creates identity from bus.
                    let identity = PlayerIdentity::new(bus.to_string()).ok()?;

                    // Checks if this bus is on the filter_players.
                    let is_on_filter = options.as_ref().map_or(false, |f| {
                        f.filter_players
                            .clone()
                            .into_iter()
                            .find(|f| identity.matches_either(&f))
                            .is_some()
                    });

                    if bus.starts_with("org.mpris.MediaPlayer2.") && !is_on_filter {
                        return Some(identity);
                    }

                    None
                })
                .collect::<Vec<PlayerIdentity>>();

            // Loop over the existing players identity to add it on shared players and send out the PlayerAttached event.
            for identity in existing_players_identity {
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

                // Push the player in the shared players.
                let mut players = shared_players.lock().await;
                players.push(player);

                // Send out PlayerAttached event along with the identity.
                event_sender
                    .send(Ok(MprisEvent::PlayerAttached(identity)))
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
                            if !name.starts_with("org.mpris.MediaPlayer2.") {
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

                                // Checks if this bus is on the filter_players.
                                let is_on_filter = options.as_ref().map_or(false, |f| {
                                    f.filter_players
                                        .clone()
                                        .into_iter()
                                        .find(|f| identity.matches_either(&f))
                                        .is_some()
                                });

                                // Only add the player and send out the PlayerAttached event if the identity is not on the filter_players.
                                if !is_on_filter {
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

                                    // Push the player in the shared players.
                                    let mut players = shared_players.lock().await;
                                    players.push(player);

                                    // Send out PlayerAttached event along with the identity.
                                    event_sender.send(Ok(MprisEvent::PlayerAttached(identity))).unwrap();
                                }
                            }

                            // There has been a mpris player detached.
                            if !old_owner.is_empty() && new_owner.is_empty() {
                                let mut players = shared_players.lock().await;

                                // Only send out the PlayerDetached event if its on the shared players only.
                                if let Some(index) = players.iter().position(|p| p.identity().matches_either(&name)) {
                                    let player = match players.get(index) {
                                        Some(player) => player,
                                        None => {
                                            event_sender.send(Err(MprisError::Other("Expected a player at index {index} but got None".into()))).unwrap();
                                            return;
                                        }
                                    };

                                    // Gets the player identity.
                                    let identity = player.identity().clone();

                                    // Remove the player at the shared players.
                                    players.remove(index);

                                    // Send an event to the close channel.
                                    close_sender.send(identity.bus().to_string()).unwrap();

                                    // Send out the PlayerDetached event.
                                    event_sender.send(Ok(MprisEvent::PlayerDetached(identity))).unwrap();
                                }
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

    /// Gets the cloned mpris options.
    pub fn options(&self) -> Option<MprisOptions<T>> {
        self.options.clone()
    }

    /// Gets the shared active players.
    pub fn players(&self) -> Arc<Mutex<Vec<MprisPlayer>>> {
        Arc::clone(&self.players)
    }

    /// Gets the cloned event sender.
    fn sender(&self) -> mpsc::UnboundedSender<MprisResult<MprisEvent>> {
        self.sender.clone()
    }
}

/// My shitty fix of automatically inferring the T generic.
/// If u know a better way to this pls help.
/// Ill give u like the $2 on my bank account.
impl Mpris<Vec<&'static str>> {
    pub async fn new_without_options() -> MprisResult<Self> {
        Self::new(None).await
    }
}
