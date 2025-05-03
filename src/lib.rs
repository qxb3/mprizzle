#![allow(unused_imports)]

//! # mprizzle
//! An async library for interacting with mpris over D-Bus.
//!
//! # Usage
//!
//! ```no_run
//! use mprizzle::{Mpris, MprisEvent};
//!
//! #[tokio::main]
//! pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut mpris = Mpris::new_without_options().await?;
//!     let shared_players = mpris.players();
//!
//!     // Start watching for mpris events.
//!     mpris.watch();
//!
//!     while let Ok(event) = mpris.recv().await? {
//!         match event {
//!             // Player Attached / Detached events.
//!             MprisEvent::PlayerAttached(identity) => println!("NEW PLAYER = {}", identity.short()),
//!             MprisEvent::PlayerDetached(identity) => println!("REMOVED PLAYER = {}", identity.short()),
//!
//!             // Player properties changed event.
//!             MprisEvent::PlayerPropertiesChanged(identity) => {
//!                 let players = shared_players.lock().await;
//!                 if let Some(player) = players.iter().find(|p| *p.identity() == identity) {
//!                     println!("PLAYER PROP CHANGED: {} = {:#?}", identity.short(), player.metadata().await?);
//!                 }
//!             },
//!
//!             // Player seeked event.
//!             MprisEvent::PlayerSeeked(identity) => {
//!                 let players = shared_players.lock().await;
//!                 if let Some(_) = players.iter().find(|p| *p.identity() == identity) {
//!                     println!("PLAYER SEEKED: {}", identity.short());
//!                 }
//!             },
//!
//!             // Player position event.
//!             MprisEvent::PlayerPosition(identity, position) => {
//!                 println!("PLAYER POSITION: {} = {}", identity.short(), position.as_secs());
//!             }
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

mod mprizzle;
pub use mprizzle::*;

mod identity;
pub use identity::*;

mod metadata;
pub use metadata::*;

mod player;
pub use player::*;

mod status;
pub use status::*;

mod proxies;
