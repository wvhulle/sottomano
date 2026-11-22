#[expect(
    clippy::disallowed_types,
    reason = "imported for the session the pipewire listener callbacks share"
)]
use std::{cell::RefCell, collections::HashSet, rc::Rc};

use pipewire::{core::CoreRc, spa::utils::result::AsyncSeq};
use tokio::sync::watch;

use super::{
    AudioState, DefaultChannel, Direction, audio_graph_state::AudioGraph,
    graph_proxy_listeners::Proxies,
};

#[derive(Clone, Copy, Default)]
pub(super) enum GraphEnumeration {
    #[default]
    NotStarted,
    ListingGlobals(AsyncSeq),
    AnsweringBinds(AsyncSeq),
    Settled,
}

impl GraphEnumeration {
    pub(super) fn started(sync_result: Result<AsyncSeq, pipewire::Error>) -> Self {
        sync_result
            .inspect_err(|sync_error| tracing::warn!("audio: pipewire sync failed: {sync_error}"))
            .map_or(Self::Settled, Self::ListingGlobals)
    }

    fn advanced(
        self,
        finished: AsyncSeq,
        sync_again: impl FnOnce() -> Result<AsyncSeq, pipewire::Error>,
    ) -> Self {
        match self {
            Self::ListingGlobals(pending) if pending == finished => sync_again()
                .inspect_err(|sync_error| {
                    tracing::warn!("audio: pipewire sync failed: {sync_error}");
                })
                .map_or(Self::Settled, Self::AnsweringBinds),
            Self::AnsweringBinds(pending) if pending == finished => Self::Settled,
            unfinished => unfinished,
        }
    }
}

#[derive(Default)]
pub(super) struct BoundAudioSession {
    pub(super) graph: AudioGraph,
    pub(super) proxies: Proxies,
    pub(super) enumeration: GraphEnumeration,
    reported_missing: HashSet<Direction>,
}

fn report_missing_defaults(session: &mut BoundAudioSession) {
    if !matches!(session.enumeration, GraphEnumeration::Settled) {
        return;
    }
    for direction in [Direction::Output, Direction::Input] {
        let graph = &session.graph;
        let present = graph.node_count(direction);
        if graph.default_node(direction).is_some() || present == 0 {
            session.reported_missing.remove(&direction);
        } else if session.reported_missing.insert(direction) {
            let label = direction.label();
            if let Some(name) = graph.default_name(direction) {
                tracing::warn!(
                    "audio: default {label} {name} matches none of the {present} {label} nodes"
                );
            } else {
                tracing::warn!(
                    "audio: no default {label} has been published while {present} {label} nodes \
                     are present"
                );
            }
        }
    }
}

fn report_unresolved(previous: &AudioState, next: &AudioState, graph: &AudioGraph) {
    for direction in [Direction::Output, Direction::Input] {
        if matches!(previous.channel(direction), DefaultChannel::Resolved(_))
            && *next.channel(direction) == DefaultChannel::Unresolved
        {
            tracing::warn!(
                "audio: default {} {:?} no longer resolves to a node",
                direction.label(),
                graph.default_name(direction)
            );
        }
    }
}

#[derive(Clone)]
pub(super) struct LiveAudioGraph {
    #[expect(
        clippy::disallowed_types,
        reason = "pipewire listener callbacks run on the pipewire loop thread and must share the \
                  bound session"
    )]
    pub(super) session: Rc<RefCell<BoundAudioSession>>,
    pub(super) states: watch::Sender<AudioState>,
}

impl LiveAudioGraph {
    pub(super) fn new(states: watch::Sender<AudioState>) -> Self {
        Self {
            session: Rc::default(),
            states,
        }
    }

    pub(super) fn update(&self, change: impl FnOnce(&mut AudioGraph)) {
        let mut session = self.session.borrow_mut();
        change(&mut session.graph);
        let next = session.graph.audio_state();
        self.states.send_if_modified(|current| {
            if *current == next {
                return false;
            }
            report_unresolved(current, &next, &session.graph);
            *current = next;
            true
        });
        report_missing_defaults(&mut session);
    }

    pub(super) fn settle_enumeration(&self, finished: AsyncSeq, core: &CoreRc) {
        let mut session = self.session.borrow_mut();
        session.enumeration = session.enumeration.advanced(finished, || core.sync(0));
        report_missing_defaults(&mut session);
    }

    pub(super) fn remove_object(&self, id: u32) {
        {
            let mut session = self.session.borrow_mut();
            session.proxies.nodes.remove(&id);
            session.proxies.devices.remove(&id);
        }
        self.update(|graph| graph.remove_object(id));
    }
}

#[cfg(test)]
mod live_session_tests;
