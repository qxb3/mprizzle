use mprizzle::Mpris;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut mpris = Mpris::new().await?;
    mpris.watch();

    while let Ok(ev) = mpris.recv().await? {
        match ev {
            mprizzle::MprisEvent::PlayerAttached(id) => println!("attached"),
            mprizzle::MprisEvent::PlayerDetached(id) => println!("detached"),
            mprizzle::MprisEvent::PlayerPropertiesChanged(id) => println!("props changed"),
            mprizzle::MprisEvent::PlayerSeeked(id) => println!("player seeked"),
            mprizzle::MprisEvent::PlayerPosition(id, pos) => println!("pos changed"),
        }
    }

    Ok(())
}
