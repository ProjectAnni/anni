use std::{
    fs::File,
    sync::{atomic::AtomicBool, mpsc::Receiver, Arc},
    thread,
};

use anni_playback::{
    create_unbound_channel,
    types::{MediaSource, PlayerEvent, RealPlayerEvent},
    Controls, Decoder,
};

pub struct Player {
    controls: Controls,
}

impl Player {
    pub fn new() -> (Player, Receiver<RealPlayerEvent>) {
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

    let (player, receiver) = Player::new();

    let thread = thread::spawn({
        move || loop {
            match receiver.recv() {
                Ok(msg) => match msg {
                    RealPlayerEvent::Play => println!("Play"),
                    RealPlayerEvent::Pause => println!("Pause"),
                    RealPlayerEvent::Stop => println!("Stop"),
                    RealPlayerEvent::Done => println!("Done"),
                    RealPlayerEvent::Progress(progress) => {
                        println!("Progress: {}/{}", progress.position, progress.duration);
                    }
                },
                Err(e) => {
                    println!("{}", e);
                }
            }
        }
    });

    let source = Box::new(File::open(filename)?);
    player.open(source)?;
    thread.join().unwrap();

    Ok(())
}
