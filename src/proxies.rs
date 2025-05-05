use std::sync::Arc;

use tokio::sync::Mutex;
use zbus::{Connection, Proxy};

use crate::{MprisError, MprisResult};

pub const DBUS_MPRIS_INTERFACE_NAME: &str = "org.mpris.MediaPlayer2";
pub const DBUS_MPRIS_INTERFACE_PATH: &str = "/org/mpris/MediaPlayer2";

/// Represents errors that can occur in MPRIX Proxy operations.
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("Failed to create: {0} proxy.")]
    FailedToCreate(String),

    #[error("{0}")]
    Other(#[from] zbus::Error),
}

impl ProxyError {
    pub fn failed_to_create(proxy: impl Into<String>) -> MprisError {
        MprisError::ProxyErr(ProxyError::FailedToCreate(proxy.into()))
    }

    pub fn other(other: zbus::Error) -> MprisError {
        MprisError::ProxyErr(ProxyError::Other(other))
    }
}

/// Proxy for "org.freedesktop.DBUS" interface.
pub async fn create_dbus_proxy(
    shared_connection: Arc<Mutex<Connection>>,
) -> MprisResult<Proxy<'static>> {
    let connection = shared_connection
        .try_lock()
        .map_err(|err| MprisError::FailedToLockSharedConnection(err.to_string()))?;

    let proxy = Proxy::new(
        &*connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .await
    .map_err(|_| ProxyError::failed_to_create("org.freedesktop.DBus"))?;

    Ok(proxy)
}

/// Creates a proxy for "org.freedesktop.DBus.Properties".
pub async fn create_properties_proxy(
    shared_connection: Arc<Mutex<Connection>>,
    bus: &str,
) -> MprisResult<Proxy<'static>> {
    let connection = shared_connection
        .try_lock()
        .map_err(|err| MprisError::FailedToLockSharedConnection(err.to_string()))?;

    let properties_proxy = Proxy::new(
        &*connection,
        bus.to_string(),
        DBUS_MPRIS_INTERFACE_PATH,
        "org.freedesktop.DBus.Properties",
    )
    .await
    .map_err(|_| ProxyError::failed_to_create("org.freedesktop.DBus.Properties"))?;

    Ok(properties_proxy)
}

/// Proxy for "org.mpris.MediaPlayer2.Player" interface.
pub async fn create_player_proxy(
    shared_connection: Arc<Mutex<Connection>>,
    bus: &str,
) -> MprisResult<Proxy<'static>> {
    let connection = shared_connection
        .try_lock()
        .map_err(|err| MprisError::FailedToLockSharedConnection(err.to_string()))?;

    let proxy: Proxy = zbus::proxy::Builder::new(&*connection)
        .destination(bus.to_string())
        .map_err(|err| ProxyError::other(err))?
        .path(DBUS_MPRIS_INTERFACE_PATH)
        .map_err(|err| ProxyError::other(err))?
        .interface(format!("{DBUS_MPRIS_INTERFACE_NAME}.Player"))
        .map_err(|err| ProxyError::other(err))?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .map_err(|_| ProxyError::failed_to_create(format!("{DBUS_MPRIS_INTERFACE_NAME}.Player")))?;

    Ok(proxy)
}
