use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionSnapshot {
    pub protocol_version: u32,
    pub session: Option<CompanionSessionSummary>,
    pub connection: CompanionConnectionMetadata,
    pub timeline: Vec<CompanionTimelineEntry>,
    #[serde(default)]
    pub has_more_timeline_before: bool,
    pub streaming_text: Option<String>,
    pub tool_calls: Vec<CompanionToolCall>,
    pub run_status: CompanionRunStatus,
    pub available_commands: Vec<CompanionCommandKind>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionTimelinePage {
    pub timeline: Vec<CompanionTimelineEntry>,
    pub has_more_before: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionSessionSummary {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionConnectionMetadata {
    pub token_id: String,
    #[serde(default)]
    pub access_mode: CompanionAccessMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionAccessMode {
    ReadOnly,
    #[default]
    Control,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionMessage {
    pub id: String,
    pub role: CompanionMessageRole,
    pub status: CompanionMessageStatus,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rendered_html: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<CompanionAttachment>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompanionTimelineEntry {
    Message {
        #[serde(flatten)]
        message: CompanionMessage,
    },
    ToolCall {
        #[serde(flatten)]
        tool_call: CompanionTimelineToolCall,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompanionAttachment {
    Image {
        id: String,
        name: String,
        mime_type: String,
        asset_id: String,
    },
    File {
        id: String,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        asset_id: String,
    },
    Link {
        id: String,
        name: String,
        url: String,
    },
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
    pub permission_request: Option<CompanionPermissionRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionTimelineToolCall {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub status: CompanionToolCallStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_preview: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<CompanionTimelineToolCallDetail>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompanionTimelineToolCallDetail {
    Markdown {
        id: String,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rendered_html: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<CompanionAttachment>,
    },
    Terminal {
        id: String,
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        working_directory: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        #[serde(default)]
        truncated: bool,
    },
    Edit {
        id: String,
        path: String,
        old_text: String,
        new_text: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionToolCallStatus {
    Pending,
    WaitingForConfirmation,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CompanionPermissionRequest {
    Flat {
        options: Vec<CompanionPermissionOption>,
    },
    Dropdown {
        default_choice_index: usize,
        choices: Vec<CompanionPermissionChoice>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionPermissionOption {
    pub label: String,
    pub option_id: String,
    pub option_kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionPermissionChoice {
    pub label: String,
    pub allow_option_id: String,
    pub allow_option_kind: String,
    pub deny_option_id: String,
    pub deny_option_kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompanionEvent {
    SnapshotReplaced {
        snapshot: CompanionSnapshot,
    },
    TimelineChanged {
        timeline: Vec<CompanionTimelineEntry>,
        has_more_before: bool,
    },
    StreamingTextChanged {
        streaming_text: Option<String>,
    },
    ToolCallsChanged {
        tool_calls: Vec<CompanionToolCall>,
    },
    RunStatusChanged {
        run_status: CompanionRunStatus,
    },
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
    SendMessage {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<CompanionUpload>,
    },
    AuthorizeToolCall {
        tool_call_id: String,
        option_id: String,
        option_kind: String,
    },
    StopRun,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionCommandKind {
    SendMessage,
    SendAttachments,
    AuthorizeToolCall,
    StopRun,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompanionUpload {
    pub name: String,
    pub mime_type: String,
    pub data_base64: String,
}

#[cfg(test)]
mod tests {
    use super::{
        CompanionAccessMode, CompanionAttachment, CompanionCommand, CompanionCommandKind,
        CompanionConnectionMetadata, CompanionEvent, CompanionMessage, CompanionMessageRole,
        CompanionMessageStatus, CompanionPermissionChoice, CompanionPermissionOption,
        CompanionPermissionRequest, CompanionRunStatus, CompanionSessionSummary, CompanionSnapshot,
        CompanionTimelineEntry, CompanionTimelineToolCall, CompanionTimelineToolCallDetail,
        CompanionToolCall, CompanionToolCallStatus, CompanionUpload,
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
                access_mode: CompanionAccessMode::Control,
                issued_at_unix_ms: Some(1_710_000_000_000),
                expires_at_unix_ms: Some(1_710_000_300_000),
            },
            timeline: vec![
                CompanionTimelineEntry::Message {
                    message: CompanionMessage {
                        id: "message-1".into(),
                        role: CompanionMessageRole::User,
                        status: CompanionMessageStatus::Done,
                        text: "Run the test suite".into(),
                        rendered_html: Some("<p>Run the test suite</p>\n".into()),
                        attachments: vec![CompanionAttachment::File {
                            id: "attachment-1".into(),
                            name: "test-output.txt".into(),
                            mime_type: Some("text/plain".into()),
                            asset_id: "asset-1".into(),
                        }],
                    },
                },
                CompanionTimelineEntry::ToolCall {
                    tool_call: CompanionTimelineToolCall {
                        id: "timeline-tool-1".into(),
                        title: "Run tests".into(),
                        inline_label: Some("cargo test -p agent_ui".into()),
                        summary: Some("Executing focused suite".into()),
                        status: CompanionToolCallStatus::Running,
                        output_preview: Some("cargo test -p agent_ui".into()),
                        details: vec![
                            CompanionTimelineToolCallDetail::Terminal {
                                id: "detail-terminal-1".into(),
                                command: "cargo test -p agent_ui".into(),
                                working_directory: Some("/Users/q/projects/utils/zed".into()),
                                output: Some("running 6 tests".into()),
                                truncated: false,
                            },
                            CompanionTimelineToolCallDetail::Edit {
                                id: "detail-edit-1".into(),
                                path: "crates/agent_ui/src/agent_panel.rs".into(),
                                old_text: "old".into(),
                                new_text: "new".into(),
                            },
                        ],
                    },
                },
                CompanionTimelineEntry::Message {
                    message: CompanionMessage {
                        id: "message-2".into(),
                        role: CompanionMessageRole::Assistant,
                        status: CompanionMessageStatus::Pending,
                        text: "Running tests now".into(),
                        rendered_html: Some("<p>Running tests now</p>\n".into()),
                        attachments: vec![CompanionAttachment::Image {
                            id: "attachment-2".into(),
                            name: "progress.png".into(),
                            mime_type: "image/png".into(),
                            asset_id: "asset-2".into(),
                        }],
                    },
                },
            ],
            has_more_timeline_before: true,
            streaming_text: Some("cargo test -p agent_ui".into()),
            tool_calls: vec![CompanionToolCall {
                id: "tool-1".into(),
                title: "Run tests".into(),
                summary: Some("Executing focused suite".into()),
                status: CompanionToolCallStatus::WaitingForConfirmation,
                permission_request: Some(CompanionPermissionRequest::Dropdown {
                    default_choice_index: 2,
                    choices: vec![
                        CompanionPermissionChoice {
                            label: "Always allow terminal".into(),
                            allow_option_id: "always_allow:terminal".into(),
                            allow_option_kind: "AllowAlways".into(),
                            deny_option_id: "always_deny:terminal".into(),
                            deny_option_kind: "RejectAlways".into(),
                        },
                        CompanionPermissionChoice {
                            label: "Always allow `npm` commands".into(),
                            allow_option_id: "always_allow_pattern:terminal\\nnpm".into(),
                            allow_option_kind: "AllowAlways".into(),
                            deny_option_id: "always_deny_pattern:terminal\\nnpm".into(),
                            deny_option_kind: "RejectAlways".into(),
                        },
                        CompanionPermissionChoice {
                            label: "Only this time".into(),
                            allow_option_id: "allow".into(),
                            allow_option_kind: "AllowOnce".into(),
                            deny_option_id: "deny".into(),
                            deny_option_kind: "RejectOnce".into(),
                        },
                    ],
                }),
                output_preview: Some("3/12 complete".into()),
                started_at_unix_ms: Some(1_710_000_000_100),
                finished_at_unix_ms: None,
            }],
            run_status: CompanionRunStatus::RunningTools,
            available_commands: vec![
                CompanionCommandKind::SendMessage,
                CompanionCommandKind::SendAttachments,
                CompanionCommandKind::AuthorizeToolCall,
                CompanionCommandKind::StopRun,
            ],
        };

        let json = serde_json::to_value(&snapshot)?;
        assert_eq!(json["protocol_version"], 1);
        assert_eq!(json["session"]["id"], "thread-1");
        assert_eq!(json["connection"]["access_mode"], "control");
        assert_eq!(json["timeline"][0]["type"], "message");
        assert_eq!(json["timeline"][1]["type"], "tool_call");
        assert_eq!(json["timeline"][2]["role"], "assistant");
        assert_eq!(
            json["timeline"][0]["rendered_html"],
            "<p>Run the test suite</p>\n"
        );
        assert_eq!(json["timeline"][1]["details"][0]["type"], "terminal");
        assert_eq!(json["timeline"][2]["attachments"][0]["type"], "image");
        assert_eq!(json["has_more_timeline_before"], true);
        assert_eq!(json["tool_calls"][0]["status"], "waiting_for_confirmation");
        assert_eq!(
            json["tool_calls"][0]["permission_request"]["kind"],
            "dropdown"
        );
        assert_eq!(
            json["tool_calls"][0]["permission_request"]["choices"][2]["label"],
            "Only this time"
        );
        assert_eq!(json["run_status"], "running_tools");

        let round_trip: CompanionSnapshot = serde_json::from_value(json)?;
        assert_eq!(round_trip, snapshot);
        Ok(())
    }

    #[test]
    fn flat_permission_request_round_trips_with_exact_options() -> anyhow::Result<()> {
        let request = CompanionPermissionRequest::Flat {
            options: vec![
                CompanionPermissionOption {
                    label: "Yes, proceed".into(),
                    option_id: "approve".into(),
                    option_kind: "AllowOnce".into(),
                },
                CompanionPermissionOption {
                    label: "Yes, and don't ask again".into(),
                    option_id: "approve_amendment".into(),
                    option_kind: "AllowAlways".into(),
                },
                CompanionPermissionOption {
                    label: "No, and tell Codex what to do differently".into(),
                    option_id: "abort".into(),
                    option_kind: "RejectOnce".into(),
                },
            ],
        };

        let json = serde_json::to_value(&request)?;
        assert_eq!(json["kind"], "flat");
        assert_eq!(json["options"][0]["label"], "Yes, proceed");
        assert_eq!(json["options"][2]["option_kind"], "RejectOnce");

        let round_trip: CompanionPermissionRequest = serde_json::from_value(json)?;
        assert_eq!(round_trip, request);
        Ok(())
    }

    #[test]
    fn event_round_trips_with_tagged_payloads() -> anyhow::Result<()> {
        let event = CompanionEvent::TimelineChanged {
            timeline: vec![CompanionTimelineEntry::Message {
                message: CompanionMessage {
                    id: "message-1".into(),
                    role: CompanionMessageRole::Assistant,
                    status: CompanionMessageStatus::Pending,
                    text: "Searching codebase".into(),
                    rendered_html: None,
                    attachments: Vec::new(),
                },
            }],
            has_more_before: true,
        };

        let json = serde_json::to_value(&event)?;
        assert_eq!(json["type"], "timeline_changed");
        assert_eq!(json["timeline"][0]["text"], "Searching codebase");
        assert_eq!(json["has_more_before"], true);

        let round_trip: CompanionEvent = serde_json::from_value(json)?;
        assert_eq!(round_trip, event);
        Ok(())
    }

    #[test]
    fn command_round_trips_with_tagged_payloads() -> anyhow::Result<()> {
        let send_message = CompanionCommand::SendMessage {
            text: "Stop after this test run".into(),
            attachments: vec![CompanionUpload {
                name: "calculator-window.png".into(),
                mime_type: "image/png".into(),
                data_base64: "AQID".into(),
            }],
        };

        let json = serde_json::to_value(&send_message)?;
        assert_eq!(json["type"], "send_message");
        assert_eq!(json["text"], "Stop after this test run");
        assert_eq!(json["attachments"][0]["name"], "calculator-window.png");

        let round_trip: CompanionCommand = serde_json::from_value(json)?;
        assert_eq!(round_trip, send_message);

        let authorize = CompanionCommand::AuthorizeToolCall {
            tool_call_id: "tool-1".into(),
            option_id: "allow".into(),
            option_kind: "AllowOnce".into(),
        };

        let json = serde_json::to_value(&authorize)?;
        assert_eq!(json["type"], "authorize_tool_call");
        assert_eq!(json["tool_call_id"], "tool-1");
        assert_eq!(json["option_kind"], "AllowOnce");

        let round_trip: CompanionCommand = serde_json::from_value(json)?;
        assert_eq!(round_trip, authorize);
        Ok(())
    }
}
