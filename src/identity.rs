use crate::{MprisError, MprisResult, proxies::DBUS_MPRIS_INTERFACE_NAME};

/// A struct representing the identity of [`crate::player::MprisPlayer`].
///
/// The main responsibility of this struct is to store
/// a `short` and the full `bus` name (e.g., `org.mpris.MediaPlayer2.spotify`) of a player.
///
/// # Example
///
/// ```no_run
/// let spotify_identity = PlayerIdentity::new("org.mpris.MediaPlayer2.spotify".into())?;
///
/// assert!("spotify", spotify_identity.short());
/// assert!("org.mpris.MediaPlayer2.spotify", spotify_identity.bus());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayerIdentity {
    /// The short name of the player.
    short: String,

    /// The full long bus name of the player.
    bus: String,
}

impl PlayerIdentity {
    /// Creates a new player identity from the provided D-Bus bus name.
    ///
    /// # Errors
    ///
    /// Returns an [`MprisError::InvalidBusName`] if:
    /// - The bus name does not contain a valid short name part.
    /// - The short name does not start with the expected MPRIS D-Bus interface prefix.
    pub fn new(bus: String) -> MprisResult<Self> {
        // Creates the short name based on the bus name passed.
        let short = bus
            .split('.')
            .nth(3)
            .ok_or(MprisError::InvalidBusName)?
            .to_string();

        // Err if the bus name doesnt start with the proper mpris dbus interface name.
        if !bus.starts_with(DBUS_MPRIS_INTERFACE_NAME) {
            return Err(MprisError::InvalidBusName);
        }

        Ok(Self { short, bus })
    }

    /// Returns `true` if the short name matches the given string.
    pub fn matches_short(&self, other: &str) -> bool {
        self.short() == other
    }

    /// Returns `true` if the bus name starts with the given string.
    pub fn matches_bus_prefix(&self, other: &str) -> bool {
        other.starts_with(DBUS_MPRIS_INTERFACE_NAME) && self.bus().starts_with(other)
    }

    /// Returns `true` if either the short name or the bus name matches.
    pub fn matches_either(&self, other: &str) -> bool {
        self.matches_short(other) || self.matches_bus_prefix(other)
    }

    /// Returns `true` only if both the short name and the bus name match.
    pub fn matches_both(&self, other: &str) -> bool {
        self.matches_short(other) && self.matches_bus_prefix(other)
    }

    /// Gets the short name.
    pub fn short(&self) -> &str {
        &self.short
    }

    /// Gets the bus name.
    pub fn bus(&self) -> &str {
        &self.bus
    }
}
