use agent_companion::{
    CompanionAccessMode, CompanionManager, CompanionManagerEvent, CompanionServiceState,
    CompanionSessionMode,
};
use gpui::{
    Action, App, ClickEvent, ClipboardItem, Context, DismissEvent, Entity, EventEmitter,
    FocusHandle, Focusable, Subscription, Window,
};
use ui::{
    Modal, ModalFooter, ModalHeader, Section, SectionHeader, Switch, TintColor, ToggleState,
    prelude::*,
};
use workspace::{ModalView, Toast, Workspace, notifications::NotificationId};

pub struct MobileCompanionModal {
    workspace: Entity<Workspace>,
    companion_manager: Entity<CompanionManager>,
    focus_handle: FocusHandle,
    _manager_subscription: Subscription,
}

impl MobileCompanionModal {
    pub fn show(workspace: &Entity<Workspace>, window: &mut Window, cx: &mut App) {
        let workspace = workspace.clone();
        window.defer(cx, move |window, cx| {
            workspace.update(cx, |workspace, cx| {
                if let Some(existing) = workspace.active_modal::<Self>(cx) {
                    window.focus(&existing.focus_handle(cx), cx);
                    return;
                }

                let workspace_entity = cx.entity().clone();
                workspace.toggle_modal(window, cx, |_window, cx| Self::new(workspace_entity, cx));
            });
        });
    }

    fn new(workspace: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        let companion_manager = CompanionManager::global(cx);
        let manager_subscription =
            cx.subscribe(&companion_manager, |_, _, _: &CompanionManagerEvent, cx| {
                cx.notify();
            });

        Self {
            workspace,
            companion_manager,
            focus_handle: cx.focus_handle(),
            _manager_subscription: manager_subscription,
        }
    }

    fn show_toast(&self, message: &'static str, cx: &mut App) {
        self.workspace.update(cx, |workspace, cx| {
            struct MobileCompanionToast;
            workspace.show_toast(
                Toast::new(NotificationId::unique::<MobileCompanionToast>(), message).autohide(),
                cx,
            );
        });
    }

    fn copy_url(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(url) = self
            .companion_manager
            .read(cx)
            .status()
            .access_info
            .as_ref()
            .map(|info| info.url.clone())
        else {
            return;
        };

        cx.write_to_clipboard(ClipboardItem::new_string(url));
        self.show_toast("Mobile companion link copied", cx);
    }

    fn copy_persistent_url(
        &mut self,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(url) = self
            .companion_manager
            .read(cx)
            .status()
            .persistent_access_info
            .as_ref()
            .map(|info| info.url.clone())
        else {
            return;
        };

        cx.write_to_clipboard(ClipboardItem::new_string(url));
        self.show_toast("Stable mobile companion link copied", cx);
    }

    fn open_url(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(url) = self
            .companion_manager
            .read(cx)
            .status()
            .access_info
            .as_ref()
            .map(|info| info.url.clone())
        else {
            return;
        };

        cx.open_url(&url);
    }

    fn open_persistent_url(
        &mut self,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(url) = self
            .companion_manager
            .read(cx)
            .status()
            .persistent_access_info
            .as_ref()
            .map(|info| info.url.clone())
        else {
            return;
        };

        cx.open_url(&url);
    }

    fn stop(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let companion_manager = self.companion_manager.clone();

        window
            .spawn(cx, async move |cx| {
                companion_manager
                    .update(cx, |manager, cx| manager.stop(cx))
                    .await
            })
            .detach_and_log_err(cx);
    }

    fn start(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let companion_manager = self.companion_manager.clone();

        window
            .spawn(cx, async move |cx| {
                companion_manager
                    .update(cx, |manager, cx| manager.start(cx))
                    .await
                    .map(|_| ())
            })
            .detach_and_log_err(cx);
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn render_status_copy(
        &self,
        status: &agent_companion::CompanionManagerStatus,
        cx: &mut Context<Self>,
    ) -> Section {
        let status_label = match &status.state {
            CompanionServiceState::Running => "Running",
            CompanionServiceState::Starting => "Starting",
            CompanionServiceState::Stopping => "Stopping",
            CompanionServiceState::Stopped => "Stopped",
            CompanionServiceState::Failed { .. } => "Failed",
        };

        let session_label = status
            .shared_session
            .as_ref()
            .map(|session| session.title.clone())
            .unwrap_or_else(|| match status.selection_mode {
                CompanionSessionMode::FollowActive => {
                    "Following the active agent session".to_string()
                }
                CompanionSessionMode::Pinned => "No shared session selected".to_string(),
            });

        let body = match &status.state {
            CompanionServiceState::Running => {
                let url = status
                    .access_info
                    .as_ref()
                    .map(|info| info.url.clone())
                    .unwrap_or_default();

                v_flex()
                    .gap_3()
                    .child(
                        div()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().colors().border_variant)
                            .bg(cx.theme().colors().editor_background)
                            .p_3()
                            .child(Label::new(url).buffer_font(cx).size(LabelSize::Small)),
                    )
                    .child(
                        Label::new(
                            "Open this link from your phone while it is on the same local network as Zed.",
                        )
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                    )
                    .when_some(
                        status
                            .persistent_access_info
                            .as_ref()
                            .map(|info| info.url.clone()),
                        |this, url| {
                            this.child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        Label::new("Stable link")
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    )
                                    .child(
                                        div()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(cx.theme().colors().border_variant)
                                            .bg(cx.theme().colors().editor_background)
                                            .p_3()
                                            .child(
                                                Label::new(url)
                                                    .buffer_font(cx)
                                                    .size(LabelSize::Small),
                                            ),
                                    )
                                    .child(
                                        Label::new(
                                            "This token survives restarts. The host and port still follow the current running companion session.",
                                        )
                                        .size(LabelSize::Small)
                                        .color(Color::Muted),
                                    ),
                            )
                        },
                    )
                    .into_any_element()
            }
            CompanionServiceState::Starting => {
                Label::new("Preparing a local companion link for the current thread…")
                    .color(Color::Muted)
                    .into_any_element()
            }
            CompanionServiceState::Stopping => {
                Label::new("Shutting down the local companion service…")
                    .color(Color::Muted)
                    .into_any_element()
            }
            CompanionServiceState::Stopped => {
                Label::new("The mobile companion service is not running for this workspace.")
                    .color(Color::Muted)
                    .into_any_element()
            }
            CompanionServiceState::Failed { message } => v_flex()
                .gap_2()
                .child(
                    Label::new("The mobile companion service failed to start.").color(Color::Error),
                )
                .child(
                    Label::new(message.clone())
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .into_any_element(),
        };

        Section::new_contained()
            .header(
                SectionHeader::new("Connection").end_slot(
                    Label::new(status_label)
                        .size(LabelSize::Small)
                        .color(match &status.state {
                            CompanionServiceState::Running => Color::Success,
                            CompanionServiceState::Failed { .. } => Color::Error,
                            _ => Color::Muted,
                        }),
                ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .p_3()
                    .child(
                        Label::new(session_label)
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .child(body),
            )
    }

    fn render_access_controls(
        &self,
        status: &agent_companion::CompanionManagerStatus,
        _cx: &mut Context<Self>,
    ) -> Section {
        let is_running = matches!(status.state, CompanionServiceState::Running);
        let toggle_state = if status.access_mode == CompanionAccessMode::ReadOnly {
            ToggleState::Selected
        } else {
            ToggleState::Unselected
        };
        let companion_manager = self.companion_manager.clone();

        Section::new_contained()
            .header(SectionHeader::new("Access"))
            .child(
                v_flex()
                    .gap_2()
                    .p_3()
                    .child(
                        Switch::new("mobile-companion-read-only", toggle_state)
                            .label("Read-only")
                            .disabled(!is_running)
                            .on_click(move |state, _window, cx| {
                                let access_mode = if *state == ToggleState::Selected {
                                    CompanionAccessMode::ReadOnly
                                } else {
                                    CompanionAccessMode::Control
                                };
                                companion_manager
                                    .update(cx, |manager, cx| manager.set_access_mode(access_mode, cx));
                            }),
                    )
                    .child(
                        Label::new(
                            "When enabled, the current shared link becomes view-only and can no longer reply, approve, or stop runs.",
                        )
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                    ),
            )
    }
}

impl EventEmitter<DismissEvent> for MobileCompanionModal {}

impl Focusable for MobileCompanionModal {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ModalView for MobileCompanionModal {}

impl Render for MobileCompanionModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.companion_manager.read(cx).status().clone();
        let is_running = matches!(status.state, CompanionServiceState::Running);
        let is_stopped = matches!(
            status.state,
            CompanionServiceState::Stopped | CompanionServiceState::Failed { .. }
        );
        let stop_button = is_running.then(|| {
            Button::new("stop-mobile-companion", "Stop Sharing")
                .style(ButtonStyle::Tinted(TintColor::Warning))
                .on_click(cx.listener(Self::stop))
        });
        let start_button = is_stopped.then(|| {
            Button::new("start-mobile-companion", "Start Sharing")
                .style(ButtonStyle::Tinted(TintColor::Accent))
                .on_click(cx.listener(Self::start))
        });

        div()
            .key_context("MobileCompanionModal")
            .occlude()
            .elevation_3(cx)
            .w(rems(34.))
            .on_action(cx.listener(Self::cancel))
            .track_focus(&self.focus_handle)
            .child(
                Modal::new("mobile-companion", None)
                    .header(
                        ModalHeader::new()
                            .headline("Mobile Companion")
                            .description(
                                "Inspect and control the mobile companion service for agent sessions on your local network.",
                            )
                            .show_dismiss_button(true),
                    )
                    .section(self.render_status_copy(&status, cx))
                    .section(self.render_access_controls(&status, cx))
                    .footer(
                        ModalFooter::new()
                            .start_slot(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("copy-mobile-companion-link", "Copy Link")
                                            .on_click(cx.listener(Self::copy_url))
                                            .disabled(!is_running),
                                    )
                                    .child(
                                        Button::new("open-mobile-companion-link", "Open Locally")
                                            .end_icon(
                                                Icon::new(IconName::ArrowUpRight)
                                                    .size(IconSize::Small)
                                                    .color(Color::Muted),
                                            )
                                            .on_click(cx.listener(Self::open_url))
                                            .disabled(!is_running),
                                    )
                                    .child(
                                        Button::new(
                                            "copy-mobile-companion-stable-link",
                                            "Copy Stable Link",
                                        )
                                        .on_click(cx.listener(Self::copy_persistent_url))
                                        .disabled(!is_running),
                                    )
                                    .child(
                                        Button::new(
                                            "open-mobile-companion-stable-link",
                                            "Open Stable Link",
                                        )
                                        .end_icon(
                                            Icon::new(IconName::ArrowUpRight)
                                                .size(IconSize::Small)
                                                .color(Color::Muted),
                                        )
                                        .on_click(cx.listener(Self::open_persistent_url))
                                        .disabled(!is_running),
                                    ),
                            )
                            .end_slot(
                                h_flex()
                                    .gap_2()
                                    .when_some(start_button, |this, start_button| {
                                        this.child(start_button)
                                    })
                                    .when_some(stop_button, |this, stop_button| {
                                        this.child(stop_button)
                                    })
                                    .child(
                                        Button::new("dismiss-mobile-companion", "Done")
                                            .on_click(|_: &ClickEvent, window, cx| {
                                                window.dispatch_action(menu::Cancel.boxed_clone(), cx);
                                            }),
                                    ),
                            ),
                    ),
            )
    }
}
