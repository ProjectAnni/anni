use std::thread;

use anni_playback::{types::PlayerEvent, Player};

fn main() -> anyhow::Result<()> {
    let (Some(first), Some(second), Some(third)) = (
        std::env::args().nth(1),
        std::env::args().nth(2),
        std::env::args().nth(3),
    ) else {
        println!("Please provide three filenames to play!");
        std::process::exit(1);
    };

    let (player, receiver) = Player::builder()
        .preferred_sample_rate(Some(48_000))
        .build()?;

    let thread = thread::spawn({
        let controls = player.controls().clone();
        let mut third = Some(third);

        move || loop {
            match receiver.recv() {
                Ok(msg) => match msg {
                    PlayerEvent::Ready(progress) => {
                        println!("Ready: {} ms", progress.duration)
                    }
                    PlayerEvent::Play => println!("Play"),
                    PlayerEvent::Pause => println!("Pause"),
                    PlayerEvent::PreloadPlayed => {
                        println!("PreloadPlayed");

                        if let Some(third) = third.take() {
                            let _ = controls.open_file(third, true);
                        }
                    }
                    PlayerEvent::PreloadReady => println!("PreloadReady"),
                    PlayerEvent::EndOfTrack => println!("EndOfTrack"),
                    PlayerEvent::Buffering(buffering) => println!("Buffering: {buffering}"),
                    PlayerEvent::Error(error) => eprintln!("Playback error: {error:?}"),
                    PlayerEvent::Progress(progress) => {
                        println!("Progress: {}/{}", progress.position, progress.duration);
                    }
                    PlayerEvent::Stop => break,
                    _ => {}
                },
                Err(e) => {
                    eprintln!("{}", e);
                }
            }
        }
    });

    player.open_file(first, false)?;
    player.open_file(second, true)?;
    player.play();
    thread
        .join()
        .map_err(|_| anyhow::anyhow!("player event thread panicked"))?;

    Ok(())
}
