use super::*;
use crate::media_playback_audio::pipewire_audio::{
    DeviceDescription,
    audio_graph_state::{AudioNode, KindHint, Levels, NodeDevice, NodePlacement},
};

fn session_with_undefaulted_sink(enumeration: GraphEnumeration) -> BoundAudioSession {
    let mut graph = AudioGraph::default();
    graph.add_node(
        56,
        AudioNode {
            direction: Direction::Output,
            name: "speaker".to_owned(),
            description: DeviceDescription::Undescribed,
            device: NodeDevice::Deviceless,
            props_kind: KindHint::Unhinted,
            serial: 0,
            placement: NodePlacement::Unrouted,
            levels: Levels::default(),
        },
    );
    BoundAudioSession {
        graph,
        enumeration,
        ..BoundAudioSession::default()
    }
}

#[test]
fn missing_defaults_stay_unreported_while_enumerating() {
    let mut session = session_with_undefaulted_sink(GraphEnumeration::NotStarted);
    report_missing_defaults(&mut session);
    assert!(session.reported_missing.is_empty());
}

#[test]
fn missing_defaults_are_reported_once_settled() {
    let mut session = session_with_undefaulted_sink(GraphEnumeration::Settled);
    report_missing_defaults(&mut session);
    assert!(session.reported_missing.contains(&Direction::Output));
}

#[test]
fn enumeration_settles_after_two_matching_sync_rounds() {
    let first_sync = AsyncSeq::from_seq(1);
    let second_sync = AsyncSeq::from_seq(2);
    let unrelated_sync = AsyncSeq::from_seq(9);
    let listing = GraphEnumeration::started(Ok(first_sync));
    let unchanged = listing.advanced(unrelated_sync, || Ok(second_sync));
    assert!(
        matches!(unchanged, GraphEnumeration::ListingGlobals(pending) if pending == first_sync)
    );
    let answering = unchanged.advanced(first_sync, || Ok(second_sync));
    assert!(
        matches!(answering, GraphEnumeration::AnsweringBinds(pending) if pending == second_sync)
    );
    let still_answering = answering.advanced(first_sync, || Ok(unrelated_sync));
    assert!(matches!(
        still_answering,
        GraphEnumeration::AnsweringBinds(_)
    ));
    assert!(matches!(
        still_answering.advanced(second_sync, || Ok(unrelated_sync)),
        GraphEnumeration::Settled
    ));
}
