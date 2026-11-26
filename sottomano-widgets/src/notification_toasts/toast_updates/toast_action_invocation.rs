use iced::{Task, window::Id};
use sottomano_compositor_model::compositor_requests::{CompositorRequest, FocusFollow};
use sottomano_desktop_sources::{
    desktop_notifications::notification_data::{Outbound, Residency, close_reason},
    desktop_windows_apps::window_edge_placement::Placement,
};

use super::super::{DEFAULT_ACTION, NotificationLayer, ToastContext, toast_lifecycle::ExitPace};
use crate::{
    background_work::compositor_request,
    hover_cards::PopupMessage,
    layer_surfaces::{ActivationTokenOutcome, activation_token_request},
    notification_toasts::{ToastLayerHost, toast_messages::NotificationMessage},
};

impl<Routed: ToastLayerHost> NotificationLayer<Routed> {
    fn previewed_file(&self, id: u32) -> Option<String> {
        self.showing()
            .find(|toast| toast.data.id == id)?
            .icons
            .previewed_file()
            .and_then(|file| {
                let file_text = file.to_str().map(str::to_owned);
                if file_text.is_none() {
                    tracing::warn!(
                        "toast {id}: previewed file {} is not unicode, so it cannot be opened or \
                         copied",
                        file.display()
                    );
                }
                file_text
            })
    }

    pub(super) fn open_previewed_file(&mut self, id: u32, context: &ToastContext) -> Task<Routed> {
        let Some(file) = self.previewed_file(id) else {
            return Task::none();
        };
        Task::batch([
            Task::done(Routed::from(PopupMessage::OpenUrl(file))),
            self.close_all(
                [id],
                close_reason::DISMISSED,
                context.config,
                context.clock.instant,
            ),
        ])
    }

    pub(super) fn copy_previewed_file_path(&self, id: u32) -> Task<Routed> {
        self.previewed_file(id).map_or_else(Task::none, |file| {
            Task::done(Routed::from(PopupMessage::CopyText(file)))
        })
    }

    pub(super) fn activate_action(
        &mut self,
        id: u32,
        key: &str,
        context: &ToastContext,
        toast_surface_on: &dyn Fn(&str) -> Option<Id>,
    ) -> Task<Routed> {
        let placement = &context.placement;
        if let Some(routed_message) = self.routed_message_for(id, key) {
            self.snooze_ongoing_toast(id);
            return Task::batch([
                Task::done(routed_message),
                self.close_all(
                    [id],
                    close_reason::DISMISSED,
                    context.config,
                    context.clock.instant,
                ),
            ]);
        }
        let declares_default_action = self.showing().any(|notif| {
            notif.data.id == id
                && notif
                    .data
                    .actions
                    .iter()
                    .any(|action| action.key == DEFAULT_ACTION)
        });
        if key == DEFAULT_ACTION && declares_default_action {
            return self.hand_default_action_back_to_sender(id, toast_surface_on);
        }

        let survives_action_invocation = self
            .showing()
            .any(|toast| toast.data.id == id && toast.data.residency == Residency::Resident);
        let window_action = self
            .anchored_window_id(id, placement)
            .and_then(|window| action_on_window(key, window, placement));
        let activation = match window_action {
            Some(task) => task,
            None if key == DEFAULT_ACTION => Task::none(),
            None => self.emit_in_order(vec![Outbound::ActionInvoked {
                id,
                key: key.to_owned(),
            }]),
        };

        if survives_action_invocation {
            activation
        } else {
            activation.chain(self.close_all(
                [id],
                close_reason::DISMISSED,
                context.config,
                context.clock.instant,
            ))
        }
    }

    fn hand_default_action_back_to_sender(
        &self,
        id: u32,
        toast_surface_on: &dyn Fn(&str) -> Option<Id>,
    ) -> Task<Routed> {
        let toast_surface = self
            .showing()
            .find(|toast| toast.data.id == id)
            .and_then(|toast| toast.place.assigned_output())
            .and_then(toast_surface_on);
        let Some(toast_surface) = toast_surface else {
            return Task::done(Routed::from(NotificationMessage::DefaultActivated {
                id,
                token: ActivationTokenOutcome::Unavailable,
            }));
        };

        let token_request = activation_token_request(toast_surface);
        Task::batch([
            Task::done(Routed::from(token_request.surface_request)),
            Task::future(token_request.token_arrival).map(move |token| {
                Routed::from(NotificationMessage::DefaultActivated { id, token })
            }),
        ])
    }

    pub(super) fn invoke_default(
        &mut self,
        id: u32,
        token: ActivationTokenOutcome,
        context: &ToastContext,
    ) -> Task<Routed> {
        let survives_action_invocation = self
            .showing()
            .any(|toast| toast.data.id == id && toast.data.residency == Residency::Resident);
        let action_invoked = Outbound::ActionInvoked {
            id,
            key: DEFAULT_ACTION.to_owned(),
        };
        let mut outbound = match token {
            ActivationTokenOutcome::Issued(token) => {
                vec![Outbound::ActivationToken { id, token }, action_invoked]
            }
            ActivationTokenOutcome::Unavailable => vec![action_invoked],
        };

        if survives_action_invocation {
            return self.emit_in_order(outbound);
        }

        outbound.extend(
            self.withdraw_showing(
                [id],
                ExitPace::Dismissed,
                context.config,
                context.clock.instant,
            )
            .into_iter()
            .map(|id| Outbound::Closed {
                id,
                reason: close_reason::DISMISSED,
            }),
        );
        self.emit_in_order(outbound)
    }
}

fn action_on_window<Routed: ToastLayerHost>(
    key: &str,
    window: u64,
    placement: &Placement,
) -> Option<Task<Routed>> {
    match key {
        DEFAULT_ACTION => Some(compositor_request(CompositorRequest::FocusWindow(window))),
        "quit" => Some(compositor_request(CompositorRequest::CloseWindow(window))),
        "move" => Some(
            placement
                .active_workspace_for_move()
                .map_or_else(Task::none, |target| {
                    compositor_request(CompositorRequest::MoveWindowToWorkspace {
                        window,
                        workspace: target,
                        focus_follow: FocusFollow::FollowWindow,
                    })
                }),
        ),
        _ => None,
    }
}
