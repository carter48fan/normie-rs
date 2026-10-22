use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub enum Event {
    Input(KeyEvent),
    Resize,
    Tick,
}

pub struct Events {
    rx: mpsc::Receiver<Event>,
}

impl Events {
    pub fn new(tick_rate: Duration) -> Events {
        let (tx, rx) = mpsc::sync_channel(100);
        let _ = thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));

                if event::poll(timeout).expect("poll failed") {
                    match event::read().expect("read event failed") {
                        CrosstermEvent::Key(key) if tx.send(Event::Input(key)).is_err() => break,
                        CrosstermEvent::Resize(_, _) if tx.send(Event::Resize).is_err() => break,
                        _ => {}
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    if tx.send(Event::Tick).is_err() {
                        break;
                    }
                    last_tick = Instant::now();
                }
            }
        });
        Events { rx }
    }

    pub fn next(&self) -> Result<Event, mpsc::RecvError> {
        self.rx.recv()
    }
}
