use super::*;

#[test]
fn hover_keeps_sanitized_prose_outside_a_source_derived_safe_fence() {
    let source = "public component C { // ``` hostile fence\n}";
    let prose = "Summary&#46;\n\n\\[run\\](command&#58;delete)\n\\<script\\>";
    let rendered = markdown_hover(EditorSymbolKind::Component, "C", source, Some(prose));
    assert!(rendered.starts_with(prose));
    assert!(rendered.contains("\n````eqiora\npublic component C"));
    assert!(rendered.ends_with("\n````"));
    assert!(!rendered.contains("[run](command:"));
}

fn cancellation(id: i32) -> Message {
    Notification::new("$/cancelRequest".to_owned(), serde_json::json!({"id": id})).into()
}

#[test]
fn pending_request_cancellation_preserves_unrelated_messages() {
    for already_buffered in [false, true] {
        let (connection, client) = Connection::memory();
        let mut state = ServerState::new(vec![]);
        state.pending.insert(
            "group".to_owned(),
            PendingAnalysis {
                version: 1,
                cancelled: Arc::new(AtomicBool::new(false)),
            },
        );
        // No worker can complete analysis. A missed cancellation fails rather than hanging.
        let (sender, receiver) = crossbeam_channel::unbounded();
        drop(sender);
        let mut buffered = VecDeque::new();
        for message in [cancellation(3), cancellation(2)] {
            if already_buffered {
                buffered.push_back(message);
            } else {
                client.sender.send(message).unwrap();
            }
        }
        let mut input_closed = false;
        assert!(
            settle_group(
                &connection,
                &mut state,
                &receiver,
                "group",
                &RequestId::from(2),
                &mut buffered,
                &mut input_closed,
            )
            .expect("cancel without waiting for analysis")
        );
        assert_eq!(buffered.len(), 1);
        assert!(cancels_request(&buffered[0], &RequestId::from(3)));
        assert!(state.pending.contains_key("group"));
        assert!(!input_closed);
    }
}

#[test]
fn completed_analysis_does_not_wait_for_a_future_cancellation() {
    let (connection, _client) = Connection::memory();
    let mut state = ServerState::new(vec![]);
    let (_sender, receiver) = crossbeam_channel::unbounded();
    assert!(
        !settle_group(
            &connection,
            &mut state,
            &receiver,
            "group",
            &RequestId::from(2),
            &mut VecDeque::new(),
            &mut false,
        )
        .unwrap()
    );
}
