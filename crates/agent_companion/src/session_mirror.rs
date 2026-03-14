use acp_thread::{AcpThread, AcpThreadEvent, AgentThreadEntry, ThreadStatus, ToolCallStatus};
use agent_client_protocol as acp;
use anyhow::{Result, anyhow};
use assistant_text_thread::{
    MessageStatus as TextMessageStatus, TextThread, TextThreadEvent, TextThreadSummary,
};
use gpui::{App, Context, Entity, EventEmitter, Subscription, Task};
use language_model::Role;

use crate::{
    CompanionCommandKind, CompanionConnectionMetadata, CompanionEvent, CompanionMessage,
    CompanionMessageRole, CompanionMessageStatus, CompanionRunStatus, CompanionSessionSummary,
    CompanionSnapshot, CompanionToolCall, CompanionToolCallStatus,
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
    _source_subscription: Option<Subscription>,
}

impl EventEmitter<CompanionEvent> for CompanionSessionMirror {}

impl CompanionSessionMirror {
    pub fn new(connection: CompanionConnectionMetadata) -> Self {
        let snapshot = empty_snapshot(connection.clone());
        Self {
            connection,
            source: None,
            snapshot,
            _source_subscription: None,
        }
    }

    pub fn snapshot(&self) -> &CompanionSnapshot {
        &self.snapshot
    }

    pub fn source(&self) -> Option<&CompanionSessionSource> {
        self.source.as_ref()
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
        self.snapshot = self.build_snapshot(cx);
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
        self.snapshot = self.build_snapshot(cx);
        cx.emit(CompanionEvent::SnapshotReplaced {
            snapshot: self.snapshot.clone(),
        });
        cx.notify();
    }

    pub fn send_message(&mut self, text: String, cx: &mut Context<Self>) -> Task<Result<()>> {
        let Some(source) = self.source.clone() else {
            return Task::ready(Err(anyhow!("no active companion session")));
        };

        match source {
            CompanionSessionSource::AcpThread(thread) => {
                let send = thread.update(cx, |thread, cx| {
                    thread.send(
                        vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
                        cx,
                    )
                });
                cx.spawn(async move |_this, _cx| {
                    send.await?;
                    Ok(())
                })
            }
            CompanionSessionSource::TextThread(thread) => thread.update(cx, move |thread, cx| {
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
        let new_snapshot = self.build_snapshot(cx);

        if self.snapshot.session != new_snapshot.session
            || self.snapshot.connection != new_snapshot.connection
            || self.snapshot.available_commands != new_snapshot.available_commands
            || self.snapshot.protocol_version != new_snapshot.protocol_version
        {
            self.snapshot = new_snapshot;
            cx.emit(CompanionEvent::SnapshotReplaced {
                snapshot: self.snapshot.clone(),
            });
            cx.notify();
            return;
        }

        if self.snapshot.messages != new_snapshot.messages {
            cx.emit(CompanionEvent::MessagesChanged {
                messages: new_snapshot.messages.clone(),
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
        cx.notify();
    }

    fn build_snapshot(&self, cx: &App) -> CompanionSnapshot {
        let Some(source) = &self.source else {
            return empty_snapshot(self.connection.clone());
        };

        match source {
            CompanionSessionSource::AcpThread(thread) => {
                snapshot_for_acp_thread(thread.read(cx), self.connection.clone(), cx)
            }
            CompanionSessionSource::TextThread(thread) => {
                snapshot_for_text_thread(thread.read(cx), self.connection.clone(), cx)
            }
        }
    }
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

fn empty_snapshot(connection: CompanionConnectionMetadata) -> CompanionSnapshot {
    CompanionSnapshot {
        protocol_version: 1,
        session: None,
        connection,
        messages: Vec::new(),
        streaming_text: None,
        tool_calls: Vec::new(),
        run_status: CompanionRunStatus::Idle,
        available_commands: Vec::new(),
    }
}

fn snapshot_for_acp_thread(
    thread: &AcpThread,
    connection: CompanionConnectionMetadata,
    cx: &App,
) -> CompanionSnapshot {
    let messages = thread
        .entries()
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| match entry {
            AgentThreadEntry::UserMessage(message) => Some(CompanionMessage {
                id: format!("user-{index}"),
                role: CompanionMessageRole::User,
                status: CompanionMessageStatus::Done,
                text: message.content.to_markdown(cx).to_string(),
            }),
            AgentThreadEntry::AssistantMessage(message) => {
                let text = assistant_message_text(message, cx);
                let status = if thread.status() == ThreadStatus::Generating
                    && index + 1 == thread.entries().len()
                {
                    CompanionMessageStatus::Pending
                } else {
                    CompanionMessageStatus::Done
                };
                Some(CompanionMessage {
                    id: format!("assistant-{index}"),
                    role: CompanionMessageRole::Assistant,
                    status,
                    text,
                })
            }
            AgentThreadEntry::ToolCall(_) => None,
        })
        .collect::<Vec<_>>();

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
                output_preview: tool_call_output_preview(tool_call, cx),
                started_at_unix_ms: None,
                finished_at_unix_ms: None,
            })
        })
        .collect::<Vec<_>>();

    let streaming_text = if thread.status() == ThreadStatus::Generating {
        messages
            .iter()
            .rev()
            .find(|message| message.role == CompanionMessageRole::Assistant)
            .map(|message| message.text.clone())
            .filter(|text| !text.is_empty())
    } else {
        None
    };

    CompanionSnapshot {
        protocol_version: 1,
        session: Some(CompanionSessionSummary {
            id: thread.session_id().to_string(),
            title: thread.title().to_string(),
        }),
        connection,
        messages,
        streaming_text,
        tool_calls,
        run_status: map_acp_run_status(thread),
        available_commands: available_commands_for_acp_thread(thread),
    }
}

fn snapshot_for_text_thread(
    thread: &TextThread,
    connection: CompanionConnectionMetadata,
    cx: &App,
) -> CompanionSnapshot {
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
        })
        .collect::<Vec<_>>();

    let streaming_text = messages
        .iter()
        .rev()
        .find(|message| {
            message.role == CompanionMessageRole::Assistant
                && message.status == CompanionMessageStatus::Pending
        })
        .map(|message| message.text.clone())
        .filter(|text| !text.is_empty());

    CompanionSnapshot {
        protocol_version: 1,
        session: Some(CompanionSessionSummary {
            id: thread.id().to_proto(),
            title: thread.summary().or_default().to_string(),
        }),
        connection,
        messages,
        streaming_text,
        tool_calls: Vec::new(),
        run_status: map_text_run_status(thread, cx),
        available_commands: available_commands_for_text_thread(thread, cx),
    }
}

fn map_role(role: Role) -> CompanionMessageRole {
    match role {
        Role::User => CompanionMessageRole::User,
        Role::Assistant => CompanionMessageRole::Assistant,
        Role::System => CompanionMessageRole::System,
    }
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
        ToolCallStatus::Pending | ToolCallStatus::WaitingForConfirmation { .. } => {
            CompanionToolCallStatus::Pending
        }
        ToolCallStatus::InProgress => CompanionToolCallStatus::Running,
        ToolCallStatus::Completed => CompanionToolCallStatus::Succeeded,
        ToolCallStatus::Failed | ToolCallStatus::Rejected => CompanionToolCallStatus::Failed,
        ToolCallStatus::Canceled => CompanionToolCallStatus::Canceled,
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
    let mut commands = vec![CompanionCommandKind::SendMessage];
    if thread.status() == ThreadStatus::Generating {
        commands.push(CompanionCommandKind::StopRun);
    }
    commands
}

fn assistant_message_text(message: &acp_thread::AssistantMessage, cx: &App) -> String {
    let mut text = String::new();
    for (index, chunk) in message.chunks.iter().enumerate() {
        if index > 0 {
            text.push_str("\n\n");
        }
        match chunk {
            acp_thread::AssistantMessageChunk::Message { block }
            | acp_thread::AssistantMessageChunk::Thought { block } => {
                text.push_str(block.to_markdown(cx));
            }
        }
    }
    text
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

    use super::{CompanionSessionMirror, CompanionSessionSource, empty_snapshot, truncate_preview};
    use crate::{CompanionConnectionMetadata, CompanionEvent, CompanionRunStatus};

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
        let snapshot = empty_snapshot(CompanionConnectionMetadata {
            token_id: "token-1".into(),
            issued_at_unix_ms: None,
            expires_at_unix_ms: None,
        });

        assert!(snapshot.messages.is_empty());
        assert!(snapshot.tool_calls.is_empty());
        assert!(snapshot.available_commands.is_empty());
    }
}
