use acp_thread::{
    AcpThread, AcpThreadEvent, AgentThreadEntry, PermissionOptions, ThreadStatus, ToolCallStatus,
};
use agent_client_protocol as acp;
use anyhow::{Result, anyhow};
use assistant_text_thread::{
    MessageStatus as TextMessageStatus, TextThread, TextThreadEvent, TextThreadSummary,
};
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Subscription, Task};
use language_model::Role;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use url::Url;
use uuid::Uuid;

use crate::{
    CompanionAsset, CompanionAssetDisposition, CompanionAssetSource, CompanionAttachment,
    CompanionCommandKind, CompanionConnectionMetadata, CompanionEvent, CompanionMessage,
    CompanionMessageRole, CompanionMessageStatus, CompanionPermissionChoice,
    CompanionPermissionOption, CompanionPermissionRequest, CompanionRunStatus,
    CompanionSessionSummary, CompanionSnapshot, CompanionToolCall, CompanionToolCallStatus,
    CompanionUpload,
};

#[derive(Clone)]
pub enum CompanionSessionSource {
    AcpThread(Entity<AcpThread>),
    TextThread(Entity<TextThread>),
}

pub struct CompanionSessionMirror {
    connection: CompanionConnectionMetadata,
    source: Option<CompanionSessionSource>,
    snapshot: CompanionSnapshot,
    assets: Vec<CompanionAsset>,
    _source_subscription: Option<Subscription>,
}

impl EventEmitter<CompanionEvent> for CompanionSessionMirror {}

impl CompanionSessionMirror {
    pub fn new(connection: CompanionConnectionMetadata) -> Self {
        let state = empty_state(connection.clone());
        Self {
            connection,
            source: None,
            snapshot: state.snapshot,
            assets: state.assets,
            _source_subscription: None,
        }
    }

    pub fn snapshot(&self) -> &CompanionSnapshot {
        &self.snapshot
    }

    pub fn source(&self) -> Option<&CompanionSessionSource> {
        self.source.as_ref()
    }

    pub fn assets(&self) -> &[CompanionAsset] {
        &self.assets
    }

    pub fn set_connection(
        &mut self,
        connection: CompanionConnectionMetadata,
        cx: &mut Context<Self>,
    ) {
        if self.connection == connection {
            return;
        }

        self.connection = connection;
        let state = self.build_state(cx);
        self.snapshot = state.snapshot;
        self.assets = state.assets;
        cx.emit(CompanionEvent::SnapshotReplaced {
            snapshot: self.snapshot.clone(),
        });
        cx.notify();
    }

    pub fn set_source(&mut self, source: Option<CompanionSessionSource>, cx: &mut Context<Self>) {
        self.source = source.clone();
        self._source_subscription = source
            .as_ref()
            .map(|source| subscribe_to_source(source, cx));
        let state = self.build_state(cx);
        self.snapshot = state.snapshot;
        self.assets = state.assets;
        cx.emit(CompanionEvent::SnapshotReplaced {
            snapshot: self.snapshot.clone(),
        });
        cx.notify();
    }

    pub fn send_message(
        &mut self,
        text: String,
        attachments: Vec<CompanionUpload>,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let Some(source) = self.source.clone() else {
            return Task::ready(Err(anyhow!("no active companion session")));
        };

        match source {
            CompanionSessionSource::AcpThread(thread) => cx.spawn(async move |_this, cx| {
                let message = build_uploaded_message(text, attachments, cx).await?;
                let send = thread.update(cx, |thread, cx| thread.send(message, cx));
                send.await?;
                Ok(())
            }),
            CompanionSessionSource::TextThread(thread) => thread.update(cx, move |thread, cx| {
                if !attachments.is_empty() {
                    return Task::ready(Err(anyhow!(
                        "text thread companion uploads are not supported yet"
                    )));
                }
                let Some(last_message) = thread.messages(cx).last() else {
                    return Task::ready(Err(anyhow!("text thread has no draft message")));
                };

                if last_message.role != Role::User {
                    return Task::ready(Err(anyhow!(
                        "text thread cannot accept a companion message right now"
                    )));
                }

                let existing_text: String = thread
                    .buffer()
                    .read(cx)
                    .text_for_range(last_message.offset_range.clone())
                    .collect();
                if !existing_text.trim().is_empty() {
                    return Task::ready(Err(anyhow!("text thread already has a local draft")));
                }

                let message_range = last_message.offset_range;
                thread.buffer().update(cx, |buffer, cx| {
                    buffer.edit([(message_range, text)], None, cx)
                });

                if thread.assist(cx).is_none() {
                    return Task::ready(Err(anyhow!("failed to start text thread completion")));
                }

                Task::ready(Ok(()))
            }),
        }
    }

    pub fn stop_run(&mut self, cx: &mut Context<Self>) -> Task<Result<()>> {
        let Some(source) = self.source.clone() else {
            return Task::ready(Err(anyhow!("no active companion session")));
        };

        match source {
            CompanionSessionSource::AcpThread(thread) => {
                let cancel = thread.update(cx, |thread, cx| thread.cancel(cx));
                cx.spawn(async move |_this, _cx| {
                    cancel.await;
                    Ok(())
                })
            }
            CompanionSessionSource::TextThread(thread) => thread.update(cx, |thread, cx| {
                if !thread.cancel_last_assist(cx) {
                    return Task::ready(Err(anyhow!("text thread is not generating")));
                }
                Task::ready(Ok(()))
            }),
        }
    }

    fn refresh_from_source(&mut self, cx: &mut Context<Self>) {
        let new_state = self.build_state(cx);
        let new_snapshot = new_state.snapshot;

        if self.snapshot.session != new_snapshot.session
            || self.snapshot.connection != new_snapshot.connection
            || self.snapshot.available_commands != new_snapshot.available_commands
            || self.snapshot.protocol_version != new_snapshot.protocol_version
            || self.assets != new_state.assets
        {
            self.snapshot = new_snapshot;
            self.assets = new_state.assets;
            cx.emit(CompanionEvent::SnapshotReplaced {
                snapshot: self.snapshot.clone(),
            });
            cx.notify();
            return;
        }

        if self.snapshot.messages != new_snapshot.messages {
            cx.emit(CompanionEvent::MessagesChanged {
                messages: new_snapshot.messages.clone(),
                has_more_before: new_snapshot.has_more_messages_before,
            });
        }
        if self.snapshot.streaming_text != new_snapshot.streaming_text {
            cx.emit(CompanionEvent::StreamingTextChanged {
                streaming_text: new_snapshot.streaming_text.clone(),
            });
        }
        if self.snapshot.tool_calls != new_snapshot.tool_calls {
            cx.emit(CompanionEvent::ToolCallsChanged {
                tool_calls: new_snapshot.tool_calls.clone(),
            });
        }
        if self.snapshot.run_status != new_snapshot.run_status {
            cx.emit(CompanionEvent::RunStatusChanged {
                run_status: new_snapshot.run_status.clone(),
            });
        }

        self.snapshot = new_snapshot;
        self.assets = new_state.assets;
        cx.notify();
    }

    fn build_state(&self, cx: &App) -> CompanionMirrorState {
        let Some(source) = &self.source else {
            return empty_state(self.connection.clone());
        };

        match source {
            CompanionSessionSource::AcpThread(thread) => {
                state_for_acp_thread(thread.read(cx), self.connection.clone(), cx)
            }
            CompanionSessionSource::TextThread(thread) => {
                state_for_text_thread(thread.read(cx), self.connection.clone(), cx)
            }
        }
    }

    pub fn authorize_tool_call(
        &mut self,
        tool_call_id: String,
        option_id: String,
        option_kind: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let Some(source) = self.source.clone() else {
            return Task::ready(Err(anyhow!("no active companion session")));
        };

        let option_kind = match permission_option_kind_from_name(&option_kind) {
            Some(kind) => kind,
            None => {
                return Task::ready(Err(anyhow!(
                    "unknown permission option kind: {option_kind}"
                )));
            }
        };

        match source {
            CompanionSessionSource::AcpThread(thread) => {
                let tool_call_id = acp::ToolCallId::new(tool_call_id);
                let option_id = acp::PermissionOptionId::new(option_id);
                let result = thread.update(cx, |thread, cx| {
                    let Some((_, tool_call)) = thread.tool_call(&tool_call_id) else {
                        return Err(anyhow!("tool call no longer exists"));
                    };

                    if !matches!(
                        tool_call.status,
                        ToolCallStatus::WaitingForConfirmation { .. }
                    ) {
                        return Err(anyhow!("tool call is no longer waiting for confirmation"));
                    }

                    thread.authorize_tool_call(tool_call_id, option_id, option_kind, cx);
                    Ok(())
                });
                Task::ready(result)
            }
            CompanionSessionSource::TextThread(_) => Task::ready(Err(anyhow!(
                "text thread companion authorization is not supported"
            ))),
        }
    }
}

struct CompanionMirrorState {
    snapshot: CompanionSnapshot,
    assets: Vec<CompanionAsset>,
}

fn subscribe_to_source(
    source: &CompanionSessionSource,
    cx: &mut Context<CompanionSessionMirror>,
) -> Subscription {
    match source {
        CompanionSessionSource::AcpThread(thread) => cx.subscribe(thread, |this, _, event, cx| {
            if matches!(
                event,
                AcpThreadEvent::NewEntry
                    | AcpThreadEvent::TitleUpdated
                    | AcpThreadEvent::EntryUpdated(_)
                    | AcpThreadEvent::EntriesRemoved(_)
                    | AcpThreadEvent::Retry(_)
                    | AcpThreadEvent::Stopped(_)
                    | AcpThreadEvent::Error
                    | AcpThreadEvent::Refusal
            ) {
                this.refresh_from_source(cx);
            }
        }),
        CompanionSessionSource::TextThread(thread) => cx.subscribe(thread, |this, _, event, cx| {
            if matches!(
                event,
                TextThreadEvent::MessagesEdited
                    | TextThreadEvent::SummaryChanged
                    | TextThreadEvent::ShowAssistError(_)
                    | TextThreadEvent::ShowPaymentRequiredError
                    | TextThreadEvent::StreamedCompletion
            ) {
                this.refresh_from_source(cx);
            }
        }),
    }
}

fn empty_state(connection: CompanionConnectionMetadata) -> CompanionMirrorState {
    CompanionMirrorState {
        snapshot: CompanionSnapshot {
            protocol_version: 1,
            session: None,
            connection,
            messages: Vec::new(),
            has_more_messages_before: false,
            streaming_text: None,
            tool_calls: Vec::new(),
            run_status: CompanionRunStatus::Idle,
            available_commands: Vec::new(),
        },
        assets: Vec::new(),
    }
}

fn state_for_acp_thread(
    thread: &AcpThread,
    connection: CompanionConnectionMetadata,
    cx: &App,
) -> CompanionMirrorState {
    let mut messages = Vec::new();
    let mut assets = Vec::new();

    for (index, entry) in thread.entries().iter().enumerate() {
        match entry {
            AgentThreadEntry::UserMessage(message) => {
                let message_id = format!("user-{index}");
                let extracted = companion_content_from_acp_blocks(
                    &message.chunks,
                    message_id.as_str(),
                    ResourceBlockFallback::Uri,
                );
                assets.extend(extracted.assets);
                messages.push(CompanionMessage {
                    id: message_id,
                    role: CompanionMessageRole::User,
                    status: CompanionMessageStatus::Done,
                    text: extracted.text,
                    attachments: extracted.attachments,
                });
            }
            AgentThreadEntry::AssistantMessage(message) => {
                let message_id = format!("assistant-{index}");
                let extracted =
                    companion_content_from_assistant_message(message, message_id.as_str(), cx);
                let status = if thread.status() == ThreadStatus::Generating
                    && index + 1 == thread.entries().len()
                {
                    CompanionMessageStatus::Pending
                } else {
                    CompanionMessageStatus::Done
                };
                assets.extend(extracted.assets);
                messages.push(CompanionMessage {
                    id: message_id,
                    role: CompanionMessageRole::Assistant,
                    status,
                    text: extracted.text,
                    attachments: extracted.attachments,
                });
            }
            AgentThreadEntry::ToolCall(_) => {}
        }
    }

    let tool_calls = thread
        .entries()
        .iter()
        .filter_map(|entry| {
            let AgentThreadEntry::ToolCall(tool_call) = entry else {
                return None;
            };

            Some(CompanionToolCall {
                id: tool_call.id.to_string(),
                title: tool_call.label.read(cx).source().to_string(),
                summary: tool_call.tool_name.as_ref().map(ToString::to_string),
                status: map_acp_tool_call_status(&tool_call.status),
                permission_request: companion_permission_request_for_tool_call_status(
                    &tool_call.status,
                ),
                output_preview: tool_call_output_preview(tool_call, cx),
                started_at_unix_ms: None,
                finished_at_unix_ms: None,
            })
        })
        .collect::<Vec<_>>();

    let streaming_text = if thread.status() == ThreadStatus::Generating {
        streaming_text_for_messages(&messages)
    } else {
        None
    };

    CompanionMirrorState {
        snapshot: CompanionSnapshot {
            protocol_version: 1,
            session: Some(CompanionSessionSummary {
                id: thread.session_id().to_string(),
                title: thread.title().to_string(),
            }),
            connection,
            messages,
            has_more_messages_before: false,
            streaming_text,
            tool_calls,
            run_status: map_acp_run_status(thread),
            available_commands: available_commands_for_acp_thread(thread),
        },
        assets,
    }
}

fn state_for_text_thread(
    thread: &TextThread,
    connection: CompanionConnectionMetadata,
    cx: &App,
) -> CompanionMirrorState {
    let buffer = thread.buffer().read(cx);
    let messages = thread
        .messages(cx)
        .enumerate()
        .map(|(index, message)| CompanionMessage {
            id: format!("text-{index}"),
            role: map_role(message.role),
            status: map_text_message_status(&message.status),
            text: buffer
                .text_for_range(message.offset_range.clone())
                .collect(),
            attachments: Vec::new(),
        })
        .collect::<Vec<_>>();

    let streaming_text = streaming_text_for_messages(&messages);

    CompanionMirrorState {
        snapshot: CompanionSnapshot {
            protocol_version: 1,
            session: Some(CompanionSessionSummary {
                id: thread.id().to_proto(),
                title: thread.summary().or_default().to_string(),
            }),
            connection,
            messages,
            has_more_messages_before: false,
            streaming_text,
            tool_calls: Vec::new(),
            run_status: map_text_run_status(thread, cx),
            available_commands: available_commands_for_text_thread(thread, cx),
        },
        assets: Vec::new(),
    }
}

fn map_role(role: Role) -> CompanionMessageRole {
    match role {
        Role::User => CompanionMessageRole::User,
        Role::Assistant => CompanionMessageRole::Assistant,
        Role::System => CompanionMessageRole::System,
    }
}

fn streaming_text_for_messages(messages: &[CompanionMessage]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|message| {
            message.role == CompanionMessageRole::Assistant
                && message.status == CompanionMessageStatus::Pending
        })
        .map(|message| message.text.clone())
        .filter(|text| !text.is_empty())
}

fn map_text_message_status(status: &TextMessageStatus) -> CompanionMessageStatus {
    match status {
        TextMessageStatus::Pending => CompanionMessageStatus::Pending,
        TextMessageStatus::Done => CompanionMessageStatus::Done,
        TextMessageStatus::Error(_) => CompanionMessageStatus::Error,
        TextMessageStatus::Canceled => CompanionMessageStatus::Canceled,
    }
}

fn map_text_run_status(thread: &TextThread, cx: &App) -> CompanionRunStatus {
    let Some(last_message) = thread.messages(cx).last() else {
        return CompanionRunStatus::Idle;
    };

    match &last_message.status {
        TextMessageStatus::Pending => CompanionRunStatus::Thinking,
        TextMessageStatus::Error(_) => CompanionRunStatus::Failed,
        TextMessageStatus::Canceled => CompanionRunStatus::Stopped,
        TextMessageStatus::Done => match thread.summary() {
            TextThreadSummary::Error => CompanionRunStatus::Failed,
            TextThreadSummary::Pending => CompanionRunStatus::Idle,
            TextThreadSummary::Content(_) => CompanionRunStatus::Completed,
        },
    }
}

fn available_commands_for_text_thread(thread: &TextThread, cx: &App) -> Vec<CompanionCommandKind> {
    let mut commands = vec![CompanionCommandKind::SendMessage];
    if matches!(
        map_text_run_status(thread, cx),
        CompanionRunStatus::Thinking
    ) {
        commands.push(CompanionCommandKind::StopRun);
    }
    commands
}

fn map_acp_tool_call_status(status: &ToolCallStatus) -> CompanionToolCallStatus {
    match status {
        ToolCallStatus::Pending => CompanionToolCallStatus::Pending,
        ToolCallStatus::WaitingForConfirmation { .. } => {
            CompanionToolCallStatus::WaitingForConfirmation
        }
        ToolCallStatus::InProgress => CompanionToolCallStatus::Running,
        ToolCallStatus::Completed => CompanionToolCallStatus::Succeeded,
        ToolCallStatus::Failed | ToolCallStatus::Rejected => CompanionToolCallStatus::Failed,
        ToolCallStatus::Canceled => CompanionToolCallStatus::Canceled,
    }
}

fn companion_permission_request_for_tool_call_status(
    status: &ToolCallStatus,
) -> Option<CompanionPermissionRequest> {
    let ToolCallStatus::WaitingForConfirmation { options, .. } = status else {
        return None;
    };

    companion_permission_request_for_options(options)
}

fn companion_permission_request_for_options(
    options: &PermissionOptions,
) -> Option<CompanionPermissionRequest> {
    match options {
        PermissionOptions::Dropdown(choices) => {
            let choices = choices
                .iter()
                .map(|choice| CompanionPermissionChoice {
                    label: choice.label().to_string(),
                    allow_option_id: choice.allow.option_id.0.to_string(),
                    allow_option_kind: permission_option_kind_name(choice.allow.kind).to_string(),
                    deny_option_id: choice.deny.option_id.0.to_string(),
                    deny_option_kind: permission_option_kind_name(choice.deny.kind).to_string(),
                })
                .collect::<Vec<_>>();

            if choices.is_empty() {
                return None;
            }

            Some(CompanionPermissionRequest::Dropdown {
                default_choice_index: choices.len().saturating_sub(1),
                choices,
            })
        }
        PermissionOptions::Flat(options) => {
            let options = options
                .iter()
                .map(|option| CompanionPermissionOption {
                    label: option.name.to_string(),
                    option_id: option.option_id.0.to_string(),
                    option_kind: permission_option_kind_name(option.kind).to_string(),
                })
                .collect::<Vec<_>>();

            if options.is_empty() {
                return None;
            }

            Some(CompanionPermissionRequest::Flat { options })
        }
    }
}

fn permission_option_kind_name(kind: acp::PermissionOptionKind) -> &'static str {
    match kind {
        acp::PermissionOptionKind::AllowOnce => "AllowOnce",
        acp::PermissionOptionKind::AllowAlways => "AllowAlways",
        acp::PermissionOptionKind::RejectOnce => "RejectOnce",
        acp::PermissionOptionKind::RejectAlways => "RejectAlways",
        _ => "AllowOnce",
    }
}

fn permission_option_kind_from_name(name: &str) -> Option<acp::PermissionOptionKind> {
    match name {
        "AllowOnce" => Some(acp::PermissionOptionKind::AllowOnce),
        "AllowAlways" => Some(acp::PermissionOptionKind::AllowAlways),
        "RejectOnce" => Some(acp::PermissionOptionKind::RejectOnce),
        "RejectAlways" => Some(acp::PermissionOptionKind::RejectAlways),
        _ => None,
    }
}

fn map_acp_run_status(thread: &AcpThread) -> CompanionRunStatus {
    if thread.is_waiting_for_confirmation() {
        return CompanionRunStatus::WaitingForInput;
    }
    if thread.status() == ThreadStatus::Generating {
        if thread.has_in_progress_tool_calls() {
            return CompanionRunStatus::RunningTools;
        }
        return CompanionRunStatus::Thinking;
    }
    if thread.had_error() {
        return CompanionRunStatus::Failed;
    }

    match thread.entries().iter().rev().find_map(|entry| match entry {
        AgentThreadEntry::ToolCall(tool_call) => Some(match tool_call.status {
            ToolCallStatus::Failed | ToolCallStatus::Rejected => CompanionRunStatus::Failed,
            ToolCallStatus::Canceled => CompanionRunStatus::Stopped,
            ToolCallStatus::Completed => CompanionRunStatus::Completed,
            ToolCallStatus::Pending
            | ToolCallStatus::WaitingForConfirmation { .. }
            | ToolCallStatus::InProgress => CompanionRunStatus::RunningTools,
        }),
        AgentThreadEntry::AssistantMessage(_) | AgentThreadEntry::UserMessage(_) => None,
    }) {
        Some(status) => status,
        None if thread.entries().is_empty() => CompanionRunStatus::Idle,
        None => CompanionRunStatus::Completed,
    }
}

fn available_commands_for_acp_thread(thread: &AcpThread) -> Vec<CompanionCommandKind> {
    let mut commands = vec![
        CompanionCommandKind::SendMessage,
        CompanionCommandKind::SendAttachments,
    ];
    if thread.is_waiting_for_confirmation() {
        commands.push(CompanionCommandKind::AuthorizeToolCall);
    }
    if thread.status() == ThreadStatus::Generating {
        commands.push(CompanionCommandKind::StopRun);
    }
    commands
}

async fn build_uploaded_message(
    text: String,
    uploads: Vec<CompanionUpload>,
    cx: &mut gpui::AsyncApp,
) -> Result<Vec<acp::ContentBlock>> {
    let uploaded_blocks = cx
        .background_spawn(async move { uploaded_blocks_for_companion_uploads(uploads) })
        .await?;
    let mut message = Vec::new();

    if !text.trim().is_empty() {
        message.push(acp::ContentBlock::Text(acp::TextContent::new(text)));
    }
    message.extend(uploaded_blocks);

    if message.is_empty() {
        return Err(anyhow!("companion message is empty"));
    }

    Ok(message)
}

#[derive(Default)]
struct CompanionExtractedContent {
    text_parts: Vec<String>,
    attachments: Vec<CompanionAttachment>,
    assets: Vec<CompanionAsset>,
}

impl CompanionExtractedContent {
    fn push_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if !text.trim().is_empty() {
            self.text_parts.push(text);
        }
    }

    fn finish(
        mut self,
        message_id: &str,
        attachment_index: &mut usize,
    ) -> CompanionFinishedContent {
        let text = normalize_message_text(self.text_parts.join("\n\n"));
        let fallback = extract_local_markdown_attachments(&text, message_id, attachment_index);
        self.attachments.extend(fallback.attachments);
        self.assets.extend(fallback.assets);

        CompanionFinishedContent {
            text: fallback.text,
            attachments: self.attachments,
            assets: self.assets,
        }
    }
}

struct CompanionFinishedContent {
    text: String,
    attachments: Vec<CompanionAttachment>,
    assets: Vec<CompanionAsset>,
}

fn companion_content_from_acp_blocks(
    blocks: &[acp::ContentBlock],
    message_id: &str,
    resource_fallback: ResourceBlockFallback,
) -> CompanionFinishedContent {
    let mut content = CompanionExtractedContent::default();
    let mut attachment_index = 0;

    for block in blocks {
        match block {
            acp::ContentBlock::Text(text_content) => content.push_text(text_content.text.clone()),
            acp::ContentBlock::Image(image_content) => {
                if let Some((attachment, asset)) =
                    image_attachment_from_acp(image_content, message_id, attachment_index)
                {
                    content.attachments.push(attachment);
                    content.assets.push(asset);
                    attachment_index += 1;
                }
            }
            acp::ContentBlock::ResourceLink(resource_link) => {
                append_resource_link_content(
                    &mut content,
                    &resource_link.name,
                    &resource_link.uri,
                    resource_link.mime_type.as_deref(),
                    message_id,
                    &mut attachment_index,
                );
            }
            acp::ContentBlock::Resource(acp::EmbeddedResource {
                resource: acp::EmbeddedResourceResource::TextResourceContents(resource_contents),
                ..
            }) => {
                append_resource_link_content(
                    &mut content,
                    attachment_name_for_uri(&resource_contents.uri).as_str(),
                    &resource_contents.uri,
                    resource_contents.mime_type.as_deref(),
                    message_id,
                    &mut attachment_index,
                );
                let _ = resource_fallback;
            }
            acp::ContentBlock::Resource(acp::EmbeddedResource {
                resource: acp::EmbeddedResourceResource::BlobResourceContents(resource_contents),
                ..
            }) => {
                if let Some((attachment, asset)) =
                    blob_attachment_from_acp(resource_contents, message_id, attachment_index)
                {
                    content.attachments.push(attachment);
                    content.assets.push(asset);
                    attachment_index += 1;
                } else {
                    content.push_text(resource_contents.uri.clone());
                }
            }
            _ => {}
        }
    }

    content.finish(message_id, &mut attachment_index)
}

fn companion_content_from_assistant_message(
    message: &acp_thread::AssistantMessage,
    message_id: &str,
    cx: &App,
) -> CompanionFinishedContent {
    let mut content = CompanionExtractedContent::default();
    let mut attachment_index = 0;

    for chunk in &message.chunks {
        let block = match chunk {
            acp_thread::AssistantMessageChunk::Message { block }
            | acp_thread::AssistantMessageChunk::Thought { block } => block,
        };

        match block {
            acp_thread::ContentBlock::Empty => {}
            acp_thread::ContentBlock::Markdown { markdown } => {
                content.push_text(markdown.read(cx).source().to_string());
            }
            acp_thread::ContentBlock::ResourceLink { resource_link } => {
                append_resource_link_content(
                    &mut content,
                    &resource_link.name,
                    &resource_link.uri,
                    resource_link.mime_type.as_deref(),
                    message_id,
                    &mut attachment_index,
                );
            }
            acp_thread::ContentBlock::Image { image } => {
                let (attachment, asset) =
                    image_attachment_from_rendered(image, message_id, attachment_index);
                content.attachments.push(attachment);
                content.assets.push(asset);
                attachment_index += 1;
            }
        }
    }

    content.finish(message_id, &mut attachment_index)
}

#[derive(Clone, Copy)]
enum ResourceBlockFallback {
    Uri,
}

fn append_resource_link_content(
    content: &mut CompanionExtractedContent,
    preferred_name: &str,
    uri: &str,
    mime_type: Option<&str>,
    message_id: &str,
    attachment_index: &mut usize,
) {
    if let Some(path) = local_file_path_from_uri(uri) {
        let (attachment, asset) = file_attachment_from_local_path(
            preferred_name,
            path,
            mime_type,
            message_id,
            *attachment_index,
        );
        content.attachments.push(attachment);
        content.assets.push(asset);
        *attachment_index += 1;
        return;
    }

    content.attachments.push(CompanionAttachment::Link {
        id: attachment_id(message_id, *attachment_index),
        name: attachment_display_name(preferred_name, uri),
        url: uri.to_string(),
    });
    *attachment_index += 1;
}

fn image_attachment_from_acp(
    image_content: &acp::ImageContent,
    message_id: &str,
    attachment_index: usize,
) -> Option<(CompanionAttachment, CompanionAsset)> {
    use base64::Engine as _;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image_content.data.as_bytes())
        .ok()?;
    let asset_id = asset_id(message_id, attachment_index);
    let name = image_content
        .uri
        .as_deref()
        .map(attachment_name_for_uri)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| default_name_for_mime_type(&image_content.mime_type));

    Some((
        CompanionAttachment::Image {
            id: attachment_id(message_id, attachment_index),
            name: name.clone(),
            mime_type: image_content.mime_type.clone(),
            asset_id: asset_id.clone(),
        },
        CompanionAsset {
            id: asset_id,
            name,
            mime_type: image_content.mime_type.clone(),
            disposition: CompanionAssetDisposition::Inline,
            source: CompanionAssetSource::Bytes(bytes),
        },
    ))
}

fn image_attachment_from_rendered(
    image: &gpui::Image,
    message_id: &str,
    attachment_index: usize,
) -> (CompanionAttachment, CompanionAsset) {
    let mime_type = image.format().mime_type().to_string();
    let name = default_name_for_mime_type(&mime_type);
    let asset_id = asset_id(message_id, attachment_index);

    (
        CompanionAttachment::Image {
            id: attachment_id(message_id, attachment_index),
            name: name.clone(),
            mime_type: mime_type.clone(),
            asset_id: asset_id.clone(),
        },
        CompanionAsset {
            id: asset_id,
            name,
            mime_type,
            disposition: CompanionAssetDisposition::Inline,
            source: CompanionAssetSource::Bytes(image.bytes().to_vec()),
        },
    )
}

fn blob_attachment_from_acp(
    resource_contents: &acp::BlobResourceContents,
    message_id: &str,
    attachment_index: usize,
) -> Option<(CompanionAttachment, CompanionAsset)> {
    use base64::Engine as _;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(resource_contents.blob.as_bytes())
        .ok()?;
    let mime_type = resource_contents.mime_type.clone().unwrap_or_else(|| {
        mime_type_for_uri(&resource_contents.uri).unwrap_or_else(default_binary_mime_type)
    });
    let name = attachment_name_for_uri(&resource_contents.uri);
    let asset_id = asset_id(message_id, attachment_index);

    let attachment = if is_inline_image_mime_type(&mime_type) {
        CompanionAttachment::Image {
            id: attachment_id(message_id, attachment_index),
            name: name.clone(),
            mime_type: mime_type.clone(),
            asset_id: asset_id.clone(),
        }
    } else {
        CompanionAttachment::File {
            id: attachment_id(message_id, attachment_index),
            name: name.clone(),
            mime_type: Some(mime_type.clone()),
            asset_id: asset_id.clone(),
        }
    };
    let disposition = if is_inline_image_mime_type(&mime_type) {
        CompanionAssetDisposition::Inline
    } else {
        CompanionAssetDisposition::Attachment
    };

    Some((
        attachment,
        CompanionAsset {
            id: asset_id,
            name,
            mime_type,
            disposition,
            source: CompanionAssetSource::Bytes(bytes),
        },
    ))
}

fn file_attachment_from_local_path(
    preferred_name: &str,
    path: PathBuf,
    mime_type: Option<&str>,
    message_id: &str,
    attachment_index: usize,
) -> (CompanionAttachment, CompanionAsset) {
    let asset_id = asset_id(message_id, attachment_index);
    let effective_mime_type = mime_type
        .map(ToString::to_string)
        .or_else(|| mime_type_for_path(&path));
    let name = if preferred_name.trim().is_empty() {
        file_name_for_path(&path)
    } else {
        preferred_name.to_string()
    };

    if let Some(mime_type) = effective_mime_type
        .clone()
        .filter(|mime| is_inline_image_mime_type(mime))
    {
        (
            CompanionAttachment::Image {
                id: attachment_id(message_id, attachment_index),
                name: name.clone(),
                mime_type: mime_type.clone(),
                asset_id: asset_id.clone(),
            },
            CompanionAsset {
                id: asset_id,
                name,
                mime_type,
                disposition: CompanionAssetDisposition::Inline,
                source: CompanionAssetSource::File(path),
            },
        )
    } else {
        (
            CompanionAttachment::File {
                id: attachment_id(message_id, attachment_index),
                name: name.clone(),
                mime_type: effective_mime_type.clone(),
                asset_id: asset_id.clone(),
            },
            CompanionAsset {
                id: asset_id,
                name,
                mime_type: effective_mime_type.unwrap_or_else(default_binary_mime_type),
                disposition: CompanionAssetDisposition::Attachment,
                source: CompanionAssetSource::File(path),
            },
        )
    }
}

fn local_file_path_from_uri(uri: &str) -> Option<PathBuf> {
    let url = Url::parse(uri).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path().ok()
}

fn uploaded_blocks_for_companion_uploads(
    uploads: Vec<CompanionUpload>,
) -> Result<Vec<acp::ContentBlock>> {
    let mut blocks = Vec::with_capacity(uploads.len());

    for upload in uploads {
        blocks.push(uploaded_block_for_companion_upload(upload)?);
    }

    Ok(blocks)
}

fn uploaded_block_for_companion_upload(upload: CompanionUpload) -> Result<acp::ContentBlock> {
    let path = persist_companion_upload(&upload)?;
    let uri = companion_upload_uri(&path)?;

    if is_inline_image_mime_type(&upload.mime_type) {
        Ok(acp::ContentBlock::Image(
            acp::ImageContent::new(upload.data_base64, upload.mime_type).uri(Some(uri)),
        ))
    } else {
        Ok(acp::ContentBlock::ResourceLink(
            acp::ResourceLink::new(upload.name, uri).mime_type(Some(upload.mime_type)),
        ))
    }
}

fn persist_companion_upload(upload: &CompanionUpload) -> Result<PathBuf> {
    use base64::Engine as _;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(upload.data_base64.as_bytes())
        .map_err(|error| anyhow!("invalid companion upload data: {error}"))?;
    let directory = companion_upload_directory();
    std::fs::create_dir_all(&directory)?;

    let file_name = sanitized_upload_file_name(&upload.name);
    let path = directory.join(format!("{}-{file_name}", Uuid::new_v4()));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

fn companion_upload_directory() -> PathBuf {
    std::env::temp_dir().join("zed-companion-uploads")
}

fn companion_upload_uri(path: &Path) -> Result<String> {
    Url::from_file_path(path)
        .map(|url| url.to_string())
        .map_err(|_| anyhow!("failed to create upload file uri"))
}

fn sanitized_upload_file_name(name: &str) -> String {
    let trimmed = name.trim();
    let candidate = if trimmed.is_empty() {
        "attachment".to_string()
    } else {
        trimmed
            .chars()
            .map(|character| match character {
                '/' | '\\' | '\n' | '\r' | '\t' => '_',
                other => other,
            })
            .collect::<String>()
    };

    let sanitized = candidate
        .trim_matches('.')
        .trim()
        .chars()
        .take(160)
        .collect::<String>();
    if sanitized.is_empty() {
        "attachment".into()
    } else {
        sanitized
    }
}

fn local_file_path_from_link_target(target: &str) -> Option<PathBuf> {
    let trimmed = target.trim().trim_start_matches('<').trim_end_matches('>');
    local_file_path_from_uri(trimmed).or_else(|| {
        let path = PathBuf::from(trimmed);
        path.is_absolute().then_some(path)
    })
}

fn attachment_id(message_id: &str, attachment_index: usize) -> String {
    format!("{message_id}-attachment-{attachment_index}")
}

fn asset_id(message_id: &str, attachment_index: usize) -> String {
    format!("{message_id}-asset-{attachment_index}")
}

fn attachment_display_name(preferred_name: &str, uri: &str) -> String {
    if !preferred_name.trim().is_empty() {
        return preferred_name.to_string();
    }
    attachment_name_for_uri(uri)
}

fn attachment_name_for_uri(uri: &str) -> String {
    if let Some(path) = local_file_path_from_uri(uri) {
        return file_name_for_path(&path);
    }

    if let Ok(url) = Url::parse(uri) {
        if let Some(name) = url
            .path_segments()
            .and_then(|segments| segments.filter(|segment| !segment.is_empty()).next_back())
        {
            return name.to_string();
        }
        if let Some(host) = url.host_str() {
            return host.to_string();
        }
    }

    "attachment".into()
}

fn file_name_for_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("attachment")
        .to_string()
}

fn mime_type_for_uri(uri: &str) -> Option<String> {
    local_file_path_from_uri(uri).and_then(|path| mime_type_for_path(&path))
}

fn mime_type_for_path(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(
        match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "bmp" => "image/bmp",
            "tif" | "tiff" => "image/tiff",
            "ico" => "image/x-icon",
            "pdf" => "application/pdf",
            "json" => "application/json",
            "md" => "text/markdown",
            "txt" | "log" | "rs" | "js" | "ts" | "tsx" | "jsx" | "py" | "toml" | "yaml" | "yml" => {
                "text/plain"
            }
            _ => return None,
        }
        .to_string(),
    )
}

fn default_name_for_mime_type(mime_type: &str) -> String {
    let extension = match mime_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        _ => "bin",
    };
    format!("attachment.{extension}")
}

fn default_binary_mime_type() -> String {
    "application/octet-stream".into()
}

fn is_inline_image_mime_type(mime_type: &str) -> bool {
    mime_type.starts_with("image/")
}

static LOCAL_MARKDOWN_ATTACHMENT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?P<image>!)?\[(?P<label>[^\]]*)\]\((?P<target>[^)\s]+)\)"#)
        .expect("valid markdown attachment regex")
});

fn extract_local_markdown_attachments(
    text: &str,
    message_id: &str,
    attachment_index: &mut usize,
) -> CompanionFinishedContent {
    let mut cleaned_text = String::with_capacity(text.len());
    let mut attachments = Vec::new();
    let mut assets = Vec::new();
    let mut last_match_end = 0;

    for capture in LOCAL_MARKDOWN_ATTACHMENT_REGEX.captures_iter(text) {
        let Some(matched) = capture.get(0) else {
            continue;
        };
        cleaned_text.push_str(&text[last_match_end..matched.start()]);

        let label = capture
            .name("label")
            .map(|value| value.as_str())
            .unwrap_or_default();
        let target = capture
            .name("target")
            .map(|value| value.as_str())
            .unwrap_or_default();

        if let Some(path) = local_file_path_from_link_target(target) {
            let (attachment, asset) =
                file_attachment_from_local_path(label, path, None, message_id, *attachment_index);
            attachments.push(attachment);
            assets.push(asset);
            *attachment_index += 1;
        } else {
            cleaned_text.push_str(matched.as_str());
        }

        last_match_end = matched.end();
    }

    cleaned_text.push_str(&text[last_match_end..]);

    CompanionFinishedContent {
        text: normalize_message_text(cleaned_text),
        attachments,
        assets,
    }
}

fn normalize_message_text(text: String) -> String {
    let mut normalized = text.trim().to_string();
    while normalized.contains("\n\n\n") {
        normalized = normalized.replace("\n\n\n", "\n\n");
    }
    normalized
}

fn tool_call_output_preview(tool_call: &acp_thread::ToolCall, cx: &App) -> Option<String> {
    let preview = tool_call
        .content
        .iter()
        .map(|content| content.to_markdown(cx))
        .find(|content| !content.trim().is_empty())
        .or_else(|| tool_call.raw_output.as_ref().map(ToString::to_string))?;

    Some(truncate_preview(&preview, 240))
}

fn truncate_preview(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }

    text.chars().take(max_len).collect::<String>() + "..."
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use assistant_slash_command::SlashCommandWorkingSet;
    use gpui::{AppContext as _, TestAppContext};
    use language::LanguageRegistry;
    use language_model::LanguageModelRegistry;
    use prompt_store::PromptBuilder;
    use settings::SettingsStore;

    use assistant_text_thread::TextThread;

    use super::{
        CompanionSessionMirror, CompanionSessionSource, ResourceBlockFallback,
        companion_content_from_acp_blocks, companion_permission_request_for_options, empty_state,
        extract_local_markdown_attachments, streaming_text_for_messages, truncate_preview,
    };
    use crate::{
        CompanionAttachment, CompanionConnectionMetadata, CompanionEvent, CompanionMessage,
        CompanionMessageRole, CompanionMessageStatus, CompanionPermissionRequest,
        CompanionRunStatus,
    };

    #[gpui::test]
    async fn text_thread_source_replacement_emits_snapshot(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let settings_store = SettingsStore::test(cx);
            cx.set_global(settings_store);
        });
        cx.update(LanguageModelRegistry::test);
        let registry = Arc::new(LanguageRegistry::new(cx.executor()));
        let prompt_builder = match PromptBuilder::new(None) {
            Ok(builder) => Arc::new(builder),
            Err(error) => panic!("failed to create prompt builder: {error}"),
        };
        let thread = cx.new(|cx| {
            TextThread::local(
                registry,
                prompt_builder,
                Arc::new(SlashCommandWorkingSet::default()),
                cx,
            )
        });
        let mirror = cx.new(|_| {
            CompanionSessionMirror::new(CompanionConnectionMetadata {
                token_id: "token-1".into(),
                issued_at_unix_ms: None,
                expires_at_unix_ms: None,
            })
        });

        let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let _subscription = cx.update(|cx| {
            let events = events.clone();
            cx.subscribe(&mirror, move |_, event, _cx| {
                events.borrow_mut().push(event.clone());
            })
        });

        mirror.update(cx, |mirror, cx| {
            mirror.set_source(Some(CompanionSessionSource::TextThread(thread.clone())), cx);
        });

        let snapshot = cx.read(|cx| mirror.read(cx).snapshot().clone());
        assert_eq!(snapshot.session.unwrap().title, "New Text Thread");
        assert_eq!(snapshot.run_status, CompanionRunStatus::Idle);
        assert_eq!(events.borrow().len(), 1);
        assert!(matches!(
            events.borrow().first(),
            Some(CompanionEvent::SnapshotReplaced { .. })
        ));
    }

    #[test]
    fn truncate_preview_adds_ellipsis_only_when_needed() {
        assert_eq!(truncate_preview("abc", 8), "abc");
        assert_eq!(truncate_preview("abcdefghij", 5), "abcde...");
    }

    #[test]
    fn empty_snapshot_has_no_commands() {
        let snapshot = empty_state(CompanionConnectionMetadata {
            token_id: "token-1".into(),
            issued_at_unix_ms: None,
            expires_at_unix_ms: None,
        })
        .snapshot;

        assert!(snapshot.messages.is_empty());
        assert!(snapshot.tool_calls.is_empty());
        assert!(snapshot.available_commands.is_empty());
    }

    #[test]
    fn streaming_text_ignores_previous_completed_assistant_message() {
        let messages = vec![
            CompanionMessage {
                id: "assistant-1".into(),
                role: CompanionMessageRole::Assistant,
                status: CompanionMessageStatus::Done,
                text: "previous reply".into(),
                attachments: Vec::new(),
            },
            CompanionMessage {
                id: "user-2".into(),
                role: CompanionMessageRole::User,
                status: CompanionMessageStatus::Done,
                text: "new question".into(),
                attachments: Vec::new(),
            },
        ];

        assert_eq!(streaming_text_for_messages(&messages), None);
    }

    #[test]
    fn streaming_text_uses_pending_assistant_message() {
        let messages = vec![CompanionMessage {
            id: "assistant-1".into(),
            role: CompanionMessageRole::Assistant,
            status: CompanionMessageStatus::Pending,
            text: "streaming reply".into(),
            attachments: Vec::new(),
        }];

        assert_eq!(
            streaming_text_for_messages(&messages),
            Some("streaming reply".into())
        );
    }

    #[test]
    fn acp_image_blocks_become_inline_image_attachments() {
        let extracted = companion_content_from_acp_blocks(
            &[agent_client_protocol::ContentBlock::Image(
                agent_client_protocol::ImageContent::new("AQID", "image/png"),
            )],
            "message-1",
            ResourceBlockFallback::Uri,
        );

        assert!(extracted.text.is_empty());
        assert!(matches!(
            extracted.attachments.first(),
            Some(CompanionAttachment::Image { .. })
        ));
        assert_eq!(extracted.assets.len(), 1);
    }

    #[test]
    fn local_file_resource_links_become_downloadable_file_attachments() {
        let extracted = companion_content_from_acp_blocks(
            &[agent_client_protocol::ContentBlock::ResourceLink(
                agent_client_protocol::ResourceLink::new("report.txt", "file:///tmp/report.txt"),
            )],
            "message-2",
            ResourceBlockFallback::Uri,
        );

        assert!(extracted.text.is_empty());
        assert!(matches!(
            extracted.attachments.first(),
            Some(CompanionAttachment::File { .. })
        ));
        assert_eq!(extracted.assets.len(), 1);
    }

    #[test]
    fn markdown_image_paths_become_inline_attachments() {
        let mut attachment_index = 0;
        let extracted = extract_local_markdown_attachments(
            "Preview:\n\n![calculator](/tmp/calculator-window.png)",
            "message-3",
            &mut attachment_index,
        );

        assert_eq!(extracted.text, "Preview:");
        assert!(matches!(
            extracted.attachments.first(),
            Some(CompanionAttachment::Image { .. })
        ));
        assert_eq!(extracted.assets.len(), 1);
        assert_eq!(attachment_index, 1);
    }

    #[test]
    fn markdown_file_links_become_downloadable_attachments() {
        let mut attachment_index = 0;
        let extracted = extract_local_markdown_attachments(
            "Artifact: [report.txt](/tmp/report.txt)",
            "message-4",
            &mut attachment_index,
        );

        assert_eq!(extracted.text, "Artifact:");
        assert!(matches!(
            extracted.attachments.first(),
            Some(CompanionAttachment::File { .. })
        ));
        assert_eq!(extracted.assets.len(), 1);
        assert_eq!(attachment_index, 1);
    }

    #[test]
    fn dropdown_permission_options_become_companion_request() {
        let options = acp_thread::PermissionOptions::Dropdown(vec![
            acp_thread::PermissionOptionChoice {
                allow: agent_client_protocol::PermissionOption::new(
                    "always_allow:terminal",
                    "Always allow terminal",
                    agent_client_protocol::PermissionOptionKind::AllowAlways,
                ),
                deny: agent_client_protocol::PermissionOption::new(
                    "always_deny:terminal",
                    "Always deny terminal",
                    agent_client_protocol::PermissionOptionKind::RejectAlways,
                ),
            },
            acp_thread::PermissionOptionChoice {
                allow: agent_client_protocol::PermissionOption::new(
                    "allow",
                    "Allow",
                    agent_client_protocol::PermissionOptionKind::AllowOnce,
                ),
                deny: agent_client_protocol::PermissionOption::new(
                    "deny",
                    "Deny",
                    agent_client_protocol::PermissionOptionKind::RejectOnce,
                ),
            },
        ]);

        let request =
            companion_permission_request_for_options(&options).expect("permission request");

        let CompanionPermissionRequest::Dropdown {
            default_choice_index,
            choices,
        } = request
        else {
            panic!("expected dropdown permission request");
        };

        assert_eq!(default_choice_index, 1);
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].label, "Always allow terminal");
        assert_eq!(choices[0].allow_option_kind, "AllowAlways");
        assert_eq!(choices[1].label, "Allow");
        assert_eq!(choices[1].deny_option_kind, "RejectOnce");
    }

    #[test]
    fn flat_permission_options_preserve_exact_button_labels() {
        let options = acp_thread::PermissionOptions::Flat(vec![
            agent_client_protocol::PermissionOption::new(
                "approve",
                "Yes, proceed",
                agent_client_protocol::PermissionOptionKind::AllowOnce,
            ),
            agent_client_protocol::PermissionOption::new(
                "approve_amendment",
                "Yes, and don't ask again",
                agent_client_protocol::PermissionOptionKind::AllowAlways,
            ),
            agent_client_protocol::PermissionOption::new(
                "abort",
                "No, and tell Codex what to do differently",
                agent_client_protocol::PermissionOptionKind::RejectOnce,
            ),
        ]);

        let request =
            companion_permission_request_for_options(&options).expect("permission request");

        let CompanionPermissionRequest::Flat { options } = request else {
            panic!("expected flat permission request");
        };

        assert_eq!(options.len(), 3);
        assert_eq!(options[0].label, "Yes, proceed");
        assert_eq!(options[0].option_id, "approve");
        assert_eq!(options[1].option_kind, "AllowAlways");
        assert_eq!(
            options[2].label,
            "No, and tell Codex what to do differently"
        );
    }
}
