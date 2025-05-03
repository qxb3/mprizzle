# mprizzle

An async library for interacting with mpris over D-BUS built.

# Installing

```toml
[dependecies]
mprizzle = "0.0.1"
```

# Usage

```rust
use mprizzle::{Mpris, MprisEvent};
                                                                                                             ```
#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut mpris = Mpris::new_without_options().await?;
    let shared_players = mpris.players();

    mpris.watch();

    while let Ok(event) = mpris.recv().await? {
        match event {
            MprisEvent::PlayerAttached(identity) => println!("NEW PLAYER = {}", identity.short()),
            MprisEvent::PlayerDetached(identity) => println!("REMOVED PLAYER = {}", identity.short()),

            MprisEvent::PlayerPropertiesChanged(identity) => {
                let players = shared_players.lock().await;
                if let Some(player) = players.iter().find(|p| *p.identity() == identity) {
                    println!("PLAYER PROP CHANGED: {} = {:#?}", identity.short(), player.metadata().await?);
                }
            },

            MprisEvent::PlayerSeeked(identity) => {
                let players = shared_players.lock().await;
                if let Some(_) = players.iter().find(|p| *p.identity() == identity) {
                    println!("PLAYER SEEKED: {}", identity.short());
                }
            },

            MprisEvent::PlayerPosition(identity, position) => {
                println!("PLAYER POSITION: {} = {}", identity.short(), position.as_secs());
            }
        }
    }

    Ok(())
}

# Documentation

Documentation is available at [docs.rs](https://docs.rs/mprizzle/latest/mprizzle/).

# Contributing

Feel free! :D
