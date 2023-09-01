use std::{ops::Deref, sync::mpsc::Receiver, thread};

use anni_playback::{create_unbound_channel, types::PlayerEvent, Controls, Decoder};

pub struct Player {
    controls: Controls,
}

impl Player {
    pub fn new() -> (Player, Receiver<PlayerEvent>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let controls = Controls::new(sender);
        let thread_killer = create_unbound_channel();

        thread::spawn({
            let controls = controls.clone();
            move || {
                let decoder = Decoder::new(controls, thread_killer.1.clone());
                decoder.start();
            }
        });

        (Player { controls }, receiver)
    }
}

impl Deref for Player {
    type Target = Controls;

    fn deref(&self) -> &Self::Target {
        &self.controls
    }
}

fn main() -> anyhow::Result<()> {
    let (Some(first), Some(second), Some(third)) = (
        std::env::args().nth(1),
        std::env::args().nth(2),
        std::env::args().nth(3),
    ) else {
        println!("Please provide three filenames to play!");
        std::process::exit(1);
    };

    let (player, receiver) = Player::new();

    let thread = thread::spawn({
        let controls = player.controls.clone();

        move || loop {
            match receiver.recv() {
                Ok(msg) => match msg {
                    PlayerEvent::Play => {
                        println!("Play");

                        // Preload the second track after first track has started playing
                        let _ = controls.open_file(second.clone(), true);
                    }
                    PlayerEvent::Pause => println!("Pause"),
                    PlayerEvent::PreloadPlayed => {
                        println!("PreloadPlayed");

                        // The second track is played, load the third track
                        let _ = controls.open_file(third.clone(), true);
                    }
                    PlayerEvent::Progress(progress) => {
                        println!("Progress: {}/{}", progress.position, progress.duration);
                    }
                },
                Err(e) => {
                    eprintln!("{}", e);
                }
            }
        }
    });

    player.open_file(first, false)?;
    player.play();
    thread.join().unwrap();

    Ok(())
}
