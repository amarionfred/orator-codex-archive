//! Dictation panel backed by a local speech worker over JSON pipes.

use super::BottomPaneView;
use super::CancellationEvent;
use super::selection_popup_common::menu_surface_inset;
use super::selection_popup_common::menu_surface_padding_height;
use super::selection_popup_common::render_menu_surface;
use crate::render::renderable::Renderable;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;
use serde::Deserialize;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum Message {
    Recording,
    Transcribing,
    Result { text: String },
    Error { message: String },
}

#[derive(Debug, PartialEq)]
enum Phase {
    Starting,
    Recording,
    Transcribing,
    Empty,
    Failed(String),
    Complete,
}

pub(crate) struct OratorView {
    phase: Phase,
    started: Instant,
    child: Option<Child>,
    messages: mpsc::Receiver<Message>,
    insertion: Option<String>,
}

impl OratorView {
    pub(crate) fn start() -> Self {
        let (tx, messages) = mpsc::sync_channel(/*bound*/ 8);
        let mut view = Self {
            phase: Phase::Starting,
            started: Instant::now(),
            child: None,
            messages,
            insertion: None,
        };
        let Some(data_dir) = dirs::data_local_dir() else {
            view.phase = Phase::Failed("Cannot find Orator's installation directory.".into());
            return view;
        };
        match Command::new(data_dir.join("orator/bin/orator-backend"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                let Some(stdout) = child.stdout.take() else {
                    drop(child.stdin.take());
                    view.child = Some(child);
                    view.phase = Phase::Failed("Cannot read Orator's output.".into());
                    return view;
                };
                std::thread::spawn(move || {
                    for line in BufReader::new(stdout).lines() {
                        let message = match line {
                            Ok(line) => serde_json::from_str(&line).unwrap_or(Message::Error {
                                message: "Orator returned an invalid response.".into(),
                            }),
                            Err(error) => Message::Error {
                                message: error.to_string(),
                            },
                        };
                        let failed = matches!(message, Message::Error { .. });
                        if tx.send(message).is_err() || failed {
                            break;
                        }
                    }
                });
                view.child = Some(child);
            }
            Err(error) => {
                view.phase = Phase::Failed(format!(
                    "Cannot start Orator: {error}. Run the Orator installer."
                ));
            }
        }
        view
    }
}

impl BottomPaneView for OratorView {
    fn handle_key_event(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        if key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            self.on_ctrl_c();
        } else if key.code == KeyCode::Enter {
            match self.phase {
                Phase::Starting | Phase::Recording => {
                    let sent = self
                        .child
                        .as_mut()
                        .and_then(|child| child.stdin.as_mut())
                        .is_some_and(|stdin| stdin.write_all(b"stop\n").is_ok());
                    self.phase = if sent {
                        Phase::Transcribing
                    } else {
                        Phase::Failed("Orator stopped unexpectedly. Try /record again.".into())
                    };
                }
                Phase::Empty | Phase::Failed(_) => self.phase = Phase::Complete,
                Phase::Transcribing | Phase::Complete => {}
            }
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.insertion = None;
        self.phase = Phase::Complete;
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.phase == Phase::Complete
    }

    fn take_composer_insertion(&mut self) -> Option<String> {
        self.insertion.take()
    }

    fn pre_draw_tick(&mut self, now: Instant) -> bool {
        if matches!(
            self.phase,
            Phase::Empty | Phase::Failed(_) | Phase::Complete
        ) {
            return false;
        }
        loop {
            match self.messages.try_recv() {
                Ok(Message::Recording) => {
                    if self.phase == Phase::Starting {
                        self.phase = Phase::Recording;
                        self.started = now;
                    }
                }
                Ok(Message::Transcribing) => self.phase = Phase::Transcribing,
                Ok(Message::Result { text }) => {
                    let text: String = text
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(/*n*/ 16_000)
                        .collect();
                    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    self.phase = if text.is_empty() {
                        Phase::Empty
                    } else {
                        self.insertion = Some(text);
                        Phase::Complete
                    };
                    break;
                }
                Ok(Message::Error { message }) => {
                    self.phase = Phase::Failed(message);
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.phase =
                        Phase::Failed("Orator stopped unexpectedly. Try /record again.".into());
                    break;
                }
            }
        }
        if matches!(self.phase, Phase::Failed(_))
            && let Some(child) = self.child.as_mut()
        {
            drop(child.stdin.take());
        }
        true
    }

    fn next_frame_delay(&self) -> Option<Duration> {
        match self.phase {
            Phase::Starting | Phase::Recording | Phase::Transcribing => {
                Some(Duration::from_millis(/*millis*/ 100))
            }
            Phase::Empty | Phase::Failed(_) | Phase::Complete => None,
        }
    }
}

impl Renderable for OratorView {
    fn desired_height(&self, width: u16) -> u16 {
        let inner_width = menu_surface_inset(Rect::new(
            /*x*/ 0, /*y*/ 0, width, /*height*/ 0,
        ))
        .width;
        menu_surface_padding_height()
            + Paragraph::new(self.lines())
                .wrap(Wrap { trim: false })
                .line_count(inner_width.max(1)) as u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let area = render_menu_surface(area, buf);
        Paragraph::new(self.lines())
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

impl OratorView {
    fn lines(&self) -> Vec<Line<'static>> {
        let (status, hint) = match &self.phase {
            Phase::Starting => ("Starting microphone…".into(), "esc cancel"),
            Phase::Recording => {
                let seconds = self.started.elapsed().as_secs();
                (
                    format!("● Recording  {}:{:02}", seconds / 60, seconds % 60),
                    "enter stop  ·  esc cancel",
                )
            }
            Phase::Transcribing => ("Transcribing locally…".into(), "esc cancel"),
            Phase::Empty => ("No speech detected.".into(), "enter close  ·  esc cancel"),
            Phase::Failed(message) => (message.clone(), "enter close  ·  esc cancel"),
            Phase::Complete => (String::new(), ""),
        };
        let status = if matches!(self.phase, Phase::Failed(_)) {
            status.red()
        } else {
            status.cyan()
        };
        vec![
            Line::from(vec!["Orator".bold(), " · speech to text".dim()]),
            Line::from(status),
            Line::from("Transcript goes into your draft for review.".dim()),
            Line::from(hint.dim()),
        ]
    }
}

impl Drop for OratorView {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // EOF cancels the worker; reap off the UI thread.
            drop(child.stdin.take());
            std::thread::spawn(move || {
                for _ in 0..50 {
                    if child.try_wait().ok().flatten().is_some() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(/*millis*/ 100));
                }
                let _ = child.kill();
                let _ = child.wait();
            });
        }
    }
}

#[cfg(test)]
#[path = "orator_tests.rs"]
mod tests;
