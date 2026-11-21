use iced::Task;
use sottomano_desktop_sinks::{
    media_playback_audio::mpris_player_commands, sink_write_errors::BusWriteError,
};
use sottomano_desktop_sources::{
    desktop_windows_apps::window_edge_placement::WindowOwner,
    media_playback_audio::mpris_players::{Command, CommandOffer, Media},
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};

use super::{Action, MediaMessage, PlayerInteraction, player_fallback_message};
use crate::widgets::app_window_focus::AppWindowFocus;

pub(super) fn raise(media: &Media) -> Action {
    let OwnerProcess::Identified(pid) = media.owner else {
        return Action::Run(raise_by_mpris(media));
    };
    Action::FocusAppWindow(AppWindowFocus {
        owner: WindowOwner::Process(pid),
        unmatched: player_fallback_message(&media.bus, PlayerInteraction::RaiseByMpris),
    })
}

pub(super) fn raise_by_mpris(media: &Media) -> Task<MediaMessage> {
    match media.capabilities.raise {
        CommandOffer::Offered => send(media, Command::Raise),
        CommandOffer::NotOffered => Task::none(),
    }
}

pub(super) fn send(media: &Media, command: Command) -> Task<MediaMessage> {
    let bus = media.bus.clone();
    Task::future(mpris_player_commands::control(bus.clone(), command)).then(move |dispatched| {
        match dispatched {
            Ok(()) => Task::none(),
            Err(bus_failure) => Task::done(MediaMessage::CommandRefused {
                bus: bus.clone(),
                refusal: Box::new(command_refusal(bus_failure)),
            }),
        }
    })
}

fn command_refusal(bus_failure: BusWriteError) -> zbus::Error {
    match bus_failure {
        BusWriteError::Call(call_failure) => call_failure,
        BusWriteError::ServiceUnreachable(_) | BusWriteError::Refused => {
            zbus::Error::Failure(bus_failure.to_string())
        }
    }
}
