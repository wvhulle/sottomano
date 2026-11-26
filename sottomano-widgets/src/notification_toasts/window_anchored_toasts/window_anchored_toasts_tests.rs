use corredo::transitions::MotionState;
use iced::{
    Event, Padding, Settings, Size,
    time::Duration,
    widget::{container, text},
    window,
};
use iced_test::Simulator;
use sottomano_toml_config::{
    Config,
    screen_edges::{HEdge, VEdge},
};

use super::WindowAnchoredToasts;
use crate::{
    clock::Clock,
    notification_toasts::{
        toast_messages::NotificationMessage,
        toast_placement::{AnchoredToast, ToastAnchor},
    },
    widgets::{shell_layer_appearance::ShellLayer, typing_under_cards::CardBackgroundFade},
};

#[test]
fn unchanged_card_bounds_are_published_once() {
    let config = Config::default();
    let single_toast = AnchoredToast {
        id: 1,
        anchor: ToastAnchor::Edge {
            vertical: VEdge::Top,
            horizontal: HEdge::Right,
        },
        card: container(text("build finished")).padding(20).into(),
        motion: MotionState::Settled,
    };
    let toasts = WindowAnchoredToasts::new(
        "DP-1",
        vec![single_toast],
        Padding::ZERO,
        &ShellLayer::Overlays.default_appearance(),
        CardBackgroundFade::Resting,
        &config,
    );
    let mut simulator = Simulator::with_size(Settings::default(), Size::new(1200.0, 800.0), toasts);
    let first_frame = Clock::real().instant;
    let _redrawn = simulator.simulate((0..3).map(|frame_index| {
        Event::Window(window::Event::RedrawRequested(
            first_frame + Duration::from_millis(16 * frame_index),
        ))
    }));
    let published_card_counts: Vec<usize> = simulator
        .into_messages()
        .filter_map(|message| match message {
            NotificationMessage::CardsLaidOut { toast_cards, .. } => Some(toast_cards.len()),
            _ => None,
        })
        .collect();
    assert_eq!(published_card_counts, [1]);
}

#[test]
fn a_settled_card_is_reported_once_across_frames() {
    let config = Config::default();
    let single_toast = AnchoredToast {
        id: 7,
        anchor: ToastAnchor::Edge {
            vertical: VEdge::Top,
            horizontal: HEdge::Right,
        },
        card: container(text("device plugged in")).padding(20).into(),
        motion: MotionState::Settled,
    };
    let toasts = WindowAnchoredToasts::new(
        "DP-1",
        vec![single_toast],
        Padding::ZERO,
        &ShellLayer::Overlays.default_appearance(),
        CardBackgroundFade::Resting,
        &config,
    );
    let mut simulator = Simulator::with_size(Settings::default(), Size::new(1200.0, 800.0), toasts);
    let first_frame = Clock::real().instant;
    let _redrawn = simulator.simulate((0..3).map(|frame_index| {
        Event::Window(window::Event::RedrawRequested(
            first_frame + Duration::from_millis(16 * frame_index),
        ))
    }));
    let settled_reports: Vec<Vec<u32>> = simulator
        .into_messages()
        .filter_map(|message| match message {
            NotificationMessage::CardsSettled { toast_ids } => Some(toast_ids),
            _ => None,
        })
        .collect();
    assert_eq!(settled_reports, [vec![7]]);
}

#[test]
fn a_card_still_appearing_is_not_reported_settled() {
    let config = Config::default();
    let appearing_toast = AnchoredToast {
        id: 9,
        anchor: ToastAnchor::Edge {
            vertical: VEdge::Top,
            horizontal: HEdge::Right,
        },
        card: container(text("download complete")).padding(20).into(),
        motion: MotionState::Moving,
    };
    let toasts = WindowAnchoredToasts::new(
        "DP-1",
        vec![appearing_toast],
        Padding::ZERO,
        &ShellLayer::Overlays.default_appearance(),
        CardBackgroundFade::Resting,
        &config,
    );
    let mut simulator = Simulator::with_size(Settings::default(), Size::new(1200.0, 800.0), toasts);
    let first_frame = Clock::real().instant;
    let _redrawn = simulator.simulate((0..3).map(|frame_index| {
        Event::Window(window::Event::RedrawRequested(
            first_frame + Duration::from_millis(16 * frame_index),
        ))
    }));
    let settled_report_count = simulator
        .into_messages()
        .filter(|message| matches!(message, NotificationMessage::CardsSettled { .. }))
        .count();
    assert_eq!(settled_report_count, 0);
}
