use std::{
    fs::File,
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use anni_playback::{types::PlayerEvent, Controls, Decoder};
use crossbeam::channel::unbounded;
use symphonia::core::io::MediaSource;

pub struct Player {
    controls: Controls,
}

impl Player {
    pub fn new() -> Player {
        let controls = Controls::default();
        let thread_killer = unbounded();

        thread::spawn({
            let controls = controls.clone();
            move || {
                let decoder = Decoder::new(controls, thread_killer.1.clone());
                decoder.start();
            }
        });

        Player { controls }
    }

    pub fn open(&self, source: Box<dyn MediaSource>) -> anyhow::Result<()> {
        let buffer_signal = Arc::new(AtomicBool::new(false));

        self.controls
            .event_handler()
            .0
            .send(PlayerEvent::Open(source, buffer_signal))?;

        self.controls.play();

        Ok(())
    }

    pub fn is_playing(&self) -> bool {
        self.controls.is_playing()
    }
}

fn main() -> anyhow::Result<()> {
    let filename = std::env::args()
        .nth(1)
        .expect("Please provide the filename to play");

    let player = Player::new();
    let source = Box::new(File::open(filename)?);
    player.open(source)?;

    while player.is_playing() {
        thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
}
