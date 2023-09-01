use std::{
    error::Error,
    io,
    ops::Deref,
    sync::mpsc::{Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use anni_playback::{
    create_unbound_channel,
    types::{PlayerEvent, ProgressState},
    Controls, Decoder,
};
use ratatui::{prelude::*, widgets::*};

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

enum Event {
    Input(crossterm::event::KeyEvent),
    Tick,
    Resize,
    PlayerEvent(PlayerEvent),
}

struct Playlist {
    current: usize,
    playlist: Vec<String>,

    progress: ProgressState,
}

impl Playlist {
    fn preload_next(&self, player: &Player) {
        if self.current + 1 < self.playlist.len() {
            // preload the next(next) track
            player
                .controls
                .open_file(&self.playlist[self.current + 1], true)
                .unwrap();
        }
    }

    fn next(&mut self, player: &Player) -> bool {
        if self.current == self.playlist.len() - 1 {
            return false;
        }

        player.play_preloaded();
        return true;
    }

    fn previous(&mut self, player: &Player) {
        if self.current > 0 {
            self.current -= 1;
            player
                .open_file(&self.playlist[self.current], false)
                .unwrap();
            self.preload_next(player);
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let files = std::env::args().skip(1).collect::<Vec<_>>();
    if files.is_empty() {
        println!("Please provide at least one filename to play!");
        std::process::exit(1);
    }
    let mut playlist = Playlist {
        current: 0,
        playlist: files,
        progress: ProgressState {
            position: 0,
            duration: 0,
        },
    };

    crossterm::terminal::enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(8),
        },
    )?;

    let (player, receiver) = Player::new();

    let (tx, rx) = std::sync::mpsc::channel();
    input_handling(receiver, tx.clone());

    player.open_file(&playlist.playlist[0], false)?;
    playlist.preload_next(&player);
    player.play();

    run_app(&mut terminal, player, playlist, rx)?;

    crossterm::terminal::disable_raw_mode()?;
    terminal.clear()?;

    Ok(())
}

fn input_handling(player_rx: Receiver<PlayerEvent>, tx: Sender<Event>) {
    thread::spawn({
        let tx = tx.clone();
        move || loop {
            if let Ok(event) = player_rx.recv() {
                tx.send(Event::PlayerEvent(event)).unwrap();
            }
        }
    });

    let tick_rate = Duration::from_millis(200);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            // poll for tick rate duration, if no events, sent tick event.
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if let Ok(true) = crossterm::event::poll(timeout) {
                match crossterm::event::read().unwrap() {
                    crossterm::event::Event::Key(key) => tx.send(Event::Input(key)).unwrap(),
                    crossterm::event::Event::Resize(_, _) => tx.send(Event::Resize).unwrap(),
                    _ => {}
                };
            }

            if last_tick.elapsed() >= tick_rate {
                tx.send(Event::Tick).unwrap();
                last_tick = Instant::now();
            }
        }
    });
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    player: Player,
    mut playlist: Playlist,
    rx: Receiver<Event>,
) -> Result<(), Box<dyn Error>> {
    let mut redraw = true;
    loop {
        if redraw {
            terminal.draw(|f| ui(f, &playlist))?;
        }
        redraw = true;

        match rx.recv()? {
            Event::Input(event) => {
                if event.code == crossterm::event::KeyCode::Char('q') {
                    break;
                } else if event.code == crossterm::event::KeyCode::Char(' ') {
                    // space to pause
                    if player.is_playing() {
                        player.pause();
                    } else {
                        player.play();
                    }
                } else if event.code == crossterm::event::KeyCode::Up {
                    playlist.previous(&player);
                } else if event.code == crossterm::event::KeyCode::Down {
                    if !playlist.next(&player) {
                        break;
                    }
                }
            }
            Event::Resize => {
                terminal.autoresize()?;
            }
            Event::Tick => {}
            Event::PlayerEvent(event) => match event {
                PlayerEvent::Play => {
                    // TODO: show playing in ui
                }
                PlayerEvent::Pause => {
                    // TODO: show pause in ui
                }
                PlayerEvent::PreloadPlayed => {
                    // Move to the next track
                    playlist.current += 1;
                    if playlist.current >= playlist.playlist.len() {
                        break;
                    }

                    playlist.preload_next(&player);
                }
                PlayerEvent::Progress(progress) => {
                    playlist.progress = progress;
                }
            },
        };
    }
    Ok(())
}

fn ui<B: Backend>(f: &mut Frame<B>, playlist: &Playlist) {
    let size = f.size();

    let block = Block::default()
        .title(block::Title::from("Example tui for anni-playback").alignment(Alignment::Center));
    f.render_widget(block, size);

    let chunks = Layout::default()
        .constraints(vec![Constraint::Length(2), Constraint::Length(4)])
        .margin(1)
        .split(size);

    let progress = LineGauge::default()
        .gauge_style(Style::default().fg(Color::Blue))
        .label(format!(
            "{}/{}",
            ms_to_mm_ss(playlist.progress.position),
            ms_to_mm_ss(playlist.progress.duration)
        ))
        .ratio(if playlist.progress.duration == 0 {
            0.0
        } else {
            playlist.progress.position as f64 / playlist.progress.duration as f64
        });
    f.render_widget(progress, chunks[0]);
}

fn ms_to_mm_ss(ms: u64) -> String {
    let seconds = ms / 1000;
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{:02}:{:02}", minutes, seconds)
}
