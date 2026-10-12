use super::*;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::tests::test_pane;
use pretty_assertions::assert_eq;

fn fixture() -> (OratorView, mpsc::Sender<Message>) {
    let (sender, messages) = mpsc::channel();
    (
        OratorView {
            phase: Phase::Recording,
            started: Instant::now(),
            child: None,
            messages,
            insertion: None,
        },
        sender,
    )
}

#[test]
fn transcript_is_inserted_in_existing_draft_without_submission() {
    let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(events));
    pane.insert_str("Existing draft: ");
    let (view, sender) = fixture();
    pane.show_view(Box::new(view));
    sender
        .send(Message::Result {
            text: "dictated words".into(),
        })
        .unwrap();
    pane.pre_draw_tick();
    assert_eq!(pane.composer_text(), "Existing draft: dictated words");
    assert!(!pane.has_active_view());
    assert!(
        received.try_recv().is_err(),
        "dictation must never submit an agent event"
    );
}

#[test]
fn escape_discards_a_queued_transcript_and_restores_typing() {
    let (events, _received) = tokio::sync::mpsc::unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(events));
    let (view, sender) = fixture();
    pane.show_view(Box::new(view));
    sender
        .send(Message::Result {
            text: "cancelled speech".into(),
        })
        .unwrap();
    pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    pane.pre_draw_tick();
    pane.insert_str("日本語 café 🗣");
    assert_eq!(pane.composer_text(), "日本語 café 🗣");
    assert!(!pane.has_active_view());
}

#[test]
fn silence_and_errors_leave_draft_unchanged() {
    for message in [
        Message::Result {
            text: " \n\t".into(),
        },
        Message::Error {
            message: "Microphone unavailable".into(),
        },
    ] {
        let (mut view, sender) = fixture();
        sender.send(message).unwrap();
        view.pre_draw_tick(Instant::now());
        assert!(view.take_composer_insertion().is_none());
        assert!(!view.is_complete());
        view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(view.is_complete());
    }
}

#[test]
fn key_releases_and_repeats_do_not_stop_recording() {
    let (mut view, _sender) = fixture();
    for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
        view.handle_key_event(KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            kind,
        ));
    }
    assert_eq!(view.phase, Phase::Recording);
}

#[test]
fn recording_and_transcribing_panels() {
    let (mut view, _sender) = fixture();
    let mut screens = Vec::new();
    for phase in [Phase::Recording, Phase::Transcribing, Phase::Empty] {
        view.phase = phase;
        let height = view.desired_height(/*width*/ 65);
        let mut buffer = Buffer::empty(Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 65, height,
        ));
        view.render(buffer.area, &mut buffer);
        let rows: Vec<String> = (0..height)
            .map(|y| {
                (0..65)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect();
        screens.push(rows.join("\n"));
    }
    insta::assert_snapshot!(screens.join("\n\n"));
}
