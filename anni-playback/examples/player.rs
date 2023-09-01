use std::{
    fs::File,
    sync::{atomic::AtomicBool, mpsc::Receiver, Arc},
    thread,
};

use anni_playback::{
    create_unbound_channel,
    types::{MediaSource, PlayerEvent},
    Controls, Decoder,
};

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

    pub fn open(&self, source: Box<dyn MediaSource>) -> anyhow::Result<()> {
        let buffer_signal = Arc::new(AtomicBool::new(false));
        self.controls.open(source, buffer_signal);
        self.controls.play();

        Ok(())
    }

    pub fn is_playing(&self) -> bool {
        self.controls.is_playing()
    }
}

fn main() -> anyhow::Result<()> {
    let Some(filename) = std::env::args().nth(1) else {
        println!("Please provide the filename to play!");
        std::process::exit(1);
    };

    let (player, receiver) = Player::new();

    let thread = thread::spawn({
        // let controls = player.controls.clone();

        move || loop {
            match receiver.recv() {
                Ok(msg) => match msg {
                    PlayerEvent::Play => println!("Play"),
                    PlayerEvent::Pause => println!("Pause"),
                    PlayerEvent::PreloadPlayed => {
                        println!("PreloadPlayed");
                        // TODO: Load the next track
                        // controls.open(source, buffer_signal)
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

    let source = Box::new(File::open(filename)?);
    player.open(source)?;
    thread.join().unwrap();

    Ok(())
}
