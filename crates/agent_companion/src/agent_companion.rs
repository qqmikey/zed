mod protocol;
mod session_mirror;

pub use protocol::{
    CompanionCommand, CompanionCommandKind, CompanionConnectionMetadata, CompanionEvent,
    CompanionMessage, CompanionMessageRole, CompanionMessageStatus, CompanionRunStatus,
    CompanionSessionSummary, CompanionSnapshot, CompanionToolCall, CompanionToolCallStatus,
};
pub use session_mirror::{CompanionSessionMirror, CompanionSessionSource};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompanionManagerStatus {
    pub state: CompanionServiceState,
    pub active_session: Option<CompanionSessionSummary>,
    pub access_info: Option<CompanionAccessInfo>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum CompanionServiceState {
    Starting,
    Running,
    Stopping,
    Failed {
        message: String,
    },
    #[default]
    Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompanionAccessInfo {
    pub url: String,
    pub token_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompanionSessionTarget {
    pub id: String,
    pub title: String,
}
