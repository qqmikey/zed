use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionSnapshot {
    pub protocol_version: u32,
    pub session: Option<CompanionSessionSummary>,
    pub connection: CompanionConnectionMetadata,
    pub messages: Vec<CompanionMessage>,
    pub streaming_text: Option<String>,
    pub tool_calls: Vec<CompanionToolCall>,
    pub run_status: CompanionRunStatus,
    pub available_commands: Vec<CompanionCommandKind>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionSessionSummary {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionConnectionMetadata {
    pub token_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionMessage {
    pub id: String,
    pub role: CompanionMessageRole,
    pub status: CompanionMessageStatus,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionMessageRole {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionMessageStatus {
    Pending,
    Done,
    Error,
    Canceled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionToolCall {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub status: CompanionToolCallStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionToolCallStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompanionEvent {
    SnapshotReplaced { snapshot: CompanionSnapshot },
    MessagesChanged { messages: Vec<CompanionMessage> },
    StreamingTextChanged { streaming_text: Option<String> },
    ToolCallsChanged { tool_calls: Vec<CompanionToolCall> },
    RunStatusChanged { run_status: CompanionRunStatus },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionRunStatus {
    Idle,
    Thinking,
    RunningTools,
    WaitingForInput,
    Completed,
    Failed,
    Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompanionCommand {
    SendMessage { text: String },
    StopRun,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionCommandKind {
    SendMessage,
    StopRun,
}

#[cfg(test)]
mod tests {
    use super::{
        CompanionCommand, CompanionCommandKind, CompanionConnectionMetadata, CompanionEvent,
        CompanionMessage, CompanionMessageRole, CompanionMessageStatus, CompanionRunStatus,
        CompanionSessionSummary, CompanionSnapshot, CompanionToolCall, CompanionToolCallStatus,
    };

    #[test]
    fn snapshot_round_trips_with_expected_shape() -> anyhow::Result<()> {
        let snapshot = CompanionSnapshot {
            protocol_version: 1,
            session: Some(CompanionSessionSummary {
                id: "thread-1".into(),
                title: "Fix flaky tests".into(),
            }),
            connection: CompanionConnectionMetadata {
                token_id: "token-1".into(),
                issued_at_unix_ms: Some(1_710_000_000_000),
                expires_at_unix_ms: Some(1_710_000_300_000),
            },
            messages: vec![
                CompanionMessage {
                    id: "message-1".into(),
                    role: CompanionMessageRole::User,
                    status: CompanionMessageStatus::Done,
                    text: "Run the test suite".into(),
                },
                CompanionMessage {
                    id: "message-2".into(),
                    role: CompanionMessageRole::Assistant,
                    status: CompanionMessageStatus::Pending,
                    text: "Running tests now".into(),
                },
            ],
            streaming_text: Some("cargo test -p agent_ui".into()),
            tool_calls: vec![CompanionToolCall {
                id: "tool-1".into(),
                title: "Run tests".into(),
                summary: Some("Executing focused suite".into()),
                status: CompanionToolCallStatus::Running,
                output_preview: Some("3/12 complete".into()),
                started_at_unix_ms: Some(1_710_000_000_100),
                finished_at_unix_ms: None,
            }],
            run_status: CompanionRunStatus::RunningTools,
            available_commands: vec![
                CompanionCommandKind::SendMessage,
                CompanionCommandKind::StopRun,
            ],
        };

        let json = serde_json::to_value(&snapshot)?;
        assert_eq!(json["protocol_version"], 1);
        assert_eq!(json["session"]["id"], "thread-1");
        assert_eq!(json["messages"][1]["role"], "assistant");
        assert_eq!(json["tool_calls"][0]["status"], "running");
        assert_eq!(json["run_status"], "running_tools");

        let round_trip: CompanionSnapshot = serde_json::from_value(json)?;
        assert_eq!(round_trip, snapshot);
        Ok(())
    }

    #[test]
    fn event_round_trips_with_tagged_payloads() -> anyhow::Result<()> {
        let event = CompanionEvent::StreamingTextChanged {
            streaming_text: Some("Searching codebase".into()),
        };

        let json = serde_json::to_value(&event)?;
        assert_eq!(json["type"], "streaming_text_changed");
        assert_eq!(json["streaming_text"], "Searching codebase");

        let round_trip: CompanionEvent = serde_json::from_value(json)?;
        assert_eq!(round_trip, event);
        Ok(())
    }

    #[test]
    fn command_round_trips_with_tagged_payloads() -> anyhow::Result<()> {
        let command = CompanionCommand::SendMessage {
            text: "Stop after this test run".into(),
        };

        let json = serde_json::to_value(&command)?;
        assert_eq!(json["type"], "send_message");
        assert_eq!(json["text"], "Stop after this test run");

        let round_trip: CompanionCommand = serde_json::from_value(json)?;
        assert_eq!(round_trip, command);
        Ok(())
    }
}
