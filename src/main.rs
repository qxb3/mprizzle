use mprizzle::Mpris;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut mpris = Mpris::new().await?;
    mpris.watch();

    loop {
        let event = mpris.recv().await?;

        match event {
            Ok(event) => match event {
                mprizzle::MprisEvent::PlayerAttached(id) => println!("attached"),
                mprizzle::MprisEvent::PlayerDetached(id) => println!("detached"),
                mprizzle::MprisEvent::PlayerPropertiesChanged(id) => println!("props changed"),
                mprizzle::MprisEvent::PlayerSeeked(id) => println!("player seeked"),
                mprizzle::MprisEvent::PlayerPosition(id, pos) => println!("pos changed"),
            },
            Err(err) => {
                eprintln!("ERR: {err}");
                break;
            }
        }
    }

    Ok(())
}
