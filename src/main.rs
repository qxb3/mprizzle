use mprizzle::{Mpris, PlayerIdentity};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut mpris = Mpris::new().await?;

    let shared_players = mpris.players();

    let mut curr_player_id: Option<PlayerIdentity> = None;

    mpris.watch();

    while let Ok(ev) = mpris.recv().await? {
        println!("CURR = {:?}", curr_player_id);

        match ev {
            mprizzle::MprisEvent::PlayerAttached(id) => {
                println!("ran");
                if curr_player_id.is_none() {
                    curr_player_id = Some(id);
                }
            }
            mprizzle::MprisEvent::PlayerDetached(id) => {
                let players = shared_players.try_lock()?;

                if let Some(curr_id) = &curr_player_id {
                    if curr_id == &id {
                        if let Some((new_pl_id, _)) = players.iter().next() {
                            curr_player_id = Some(new_pl_id.clone());
                        } else {
                            curr_player_id = None;
                        }
                    }
                }
            }
            mprizzle::MprisEvent::PlayerPropertiesChanged(id) => {
                let players = shared_players.try_lock()?;

                if let Some(curr_id) = &curr_player_id {
                    if curr_id == &id {
                        if let Some(pl) = players.get(curr_id) {
                            println!(
                                "PROP {} = {:?}",
                                curr_id.short(),
                                pl.metadata().await.unwrap().title().unwrap()
                            );
                        }
                    }
                }
            }
            mprizzle::MprisEvent::PlayerSeeked(id) => {}
            mprizzle::MprisEvent::PlayerPosition(id, pos) => {
                let players = shared_players.try_lock()?;

                if let Some(curr_id) = &curr_player_id {
                    if curr_id == &id {
                        if let Some(pl) = players.get(curr_id) {
                            println!("POS {} = {:?}", curr_id.short(), pl.position().await?);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
