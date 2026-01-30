//! Event handling for the TUI.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};

/// Terminal events.
#[derive(Debug, Clone, Copy)]
pub enum Event {
    /// Tick event for periodic updates.
    Tick,
    /// Key press event.
    Key(KeyEvent),
    /// Terminal resize event.
    Resize(u16, u16),
}

/// Event handler that runs in a separate thread.
#[derive(Debug)]
pub struct EventHandler {
    /// Event receiver channel.
    receiver: mpsc::Receiver<Event>,
    /// Event sender channel (kept alive to prevent channel from closing).
    #[allow(dead_code)]
    sender: mpsc::Sender<Event>,
}

impl EventHandler {
    /// Create a new event handler with the given tick rate in milliseconds.
    pub fn new(tick_rate: u64) -> Self {
        let tick_rate = Duration::from_millis(tick_rate);
        let (sender, receiver) = mpsc::channel();
        let sender_clone = sender.clone();

        thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(tick_rate);

                if event::poll(timeout).expect("failed to poll events") {
                    match event::read().expect("failed to read event") {
                        CrosstermEvent::Key(key) => {
                            // Only handle key press events (not release or repeat)
                            if key.kind == crossterm::event::KeyEventKind::Press
                                && sender_clone.send(Event::Key(key)).is_err()
                            {
                                return;
                            }
                        }
                        CrosstermEvent::Resize(width, height) => {
                            if sender_clone.send(Event::Resize(width, height)).is_err() {
                                return;
                            }
                        }
                        _ => {}
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    if sender_clone.send(Event::Tick).is_err() {
                        return;
                    }
                    last_tick = Instant::now();
                }
            }
        });

        Self { receiver, sender }
    }

    /// Receive the next event from the handler.
    pub fn next(&self) -> Result<Event, mpsc::RecvError> {
        self.receiver.recv()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_debug() {
        let event = Event::Tick;
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Tick"));
    }

    #[test]
    fn test_event_resize() {
        let event = Event::Resize(100, 50);
        match event {
            Event::Resize(w, h) => {
                assert_eq!(w, 100);
                assert_eq!(h, 50);
            }
            _ => panic!("Expected Resize event"),
        }
    }
}
