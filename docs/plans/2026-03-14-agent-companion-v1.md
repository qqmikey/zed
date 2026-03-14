# Agent Companion V1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an on-demand companion service inside Zed that exposes the active agent session over the local network so a lightweight client can view progress, stream messages, send replies, and stop the current run.

**Architecture:** Keep all session truth inside Zed and project a compact, platform-neutral companion model outward. Implement a small embedded HTTP/WebSocket service plus a session mirror that subscribes to `assistant_text_thread` and `acp_thread` updates from `agent_ui`, without changing ACP or mirroring the full GPUI tree.

**Tech Stack:** Rust, GPUI entities/subscriptions, axum/WebSocket, serde, existing agent session state from `agent_ui`, `assistant_text_thread`, and `acp_thread`.

---

### Task 1: Define Companion Protocol And Service Boundary

**Files:**
- Create: `crates/agent_companion/Cargo.toml`
- Create: `crates/agent_companion/src/agent_companion.rs`
- Create: `crates/agent_companion/src/protocol.rs`
- Modify: `Cargo.toml`

**Step 1: Define companion DTOs**

Add protocol types for:
- `CompanionSnapshot`
- `CompanionEvent`
- `CompanionCommand`
- `CompanionSessionSummary`
- `CompanionMessage`
- `CompanionToolCall`
- `CompanionRunStatus`

Include only v1 fields:
- active session id/title
- connection token metadata
- messages
- streaming text
- tool timeline summary
- run status
- `send_message` and `stop_run` commands

**Step 2: Add the new crate to the workspace**

Wire `crates/agent_companion` into the workspace and keep dependencies minimal.

**Step 3: Add protocol serialization tests**

Create focused unit tests for snapshot/event/command round-tripping so the public contract is stable before server code lands.

### Task 2: Build A Session Mirror For Active Agent State

**Files:**
- Create: `crates/agent_companion/src/session_mirror.rs`
- Modify: `crates/agent_companion/src/agent_companion.rs`
- Modify: `crates/agent_ui/src/agent_panel.rs`
- Modify: `crates/agent_ui/src/connection_view.rs`
- Modify: `crates/assistant_text_thread/src/text_thread.rs`
- Modify: `crates/acp_thread/src/acp_thread.rs`

**Step 1: Introduce a session mirror abstraction**

Create `CompanionSessionMirror` that can:
- attach to one active native text thread or ACP thread
- build an initial `CompanionSnapshot`
- emit incremental `CompanionEvent`s when source entities change

**Step 2: Subscribe to native thread events**

Map `TextThreadEvent` updates into companion state:
- message edits
- streamed completion
- summary/error state

**Step 3: Subscribe to ACP thread events**

Map `AcpThreadEvent` updates into companion state:
- new/updated entries
- stopped/error states
- tool authorization waiting
- retry/turn status

**Step 4: Add a thin AgentPanel hook for “active session changed”**

Expose enough active-session information from `AgentPanel` to let the companion manager retarget the mirror when the visible session changes.

**Step 5: Add mirror-focused tests**

Cover:
- initial snapshot creation
- native thread streaming update propagation
- ACP tool status propagation
- switching active session

### Task 3: Add The Embedded Companion HTTP And WebSocket Server

**Files:**
- Create: `crates/agent_companion/src/server.rs`
- Modify: `crates/agent_companion/src/agent_companion.rs`
- Modify: `crates/zed/Cargo.toml`
- Modify: `crates/zed/src/main.rs`

**Step 1: Add a global companion manager**

Implement `CompanionManager` with:
- `start(active_session_target)`
- `stop()`
- `status()`
- generated token
- bound port/url

**Step 2: Implement embedded server routes**

Provide:
- `GET /companion/snapshot`
- `GET /companion` for a minimal built-in client page
- `WS /companion/events`
- `POST /companion/command`

Scope command handling to:
- `send_message`
- `stop_run`

**Step 3: Restrict the server to v1 local use**

Add:
- token gate on every endpoint
- localhost/LAN binding only
- explicit on-demand startup

**Step 4: Add server tests**

Cover:
- token enforcement
- snapshot retrieval
- websocket event fanout
- command routing to the active session

### Task 4: Add Agent UI Controls For Starting The Companion

**Files:**
- Modify: `crates/agent_ui/src/agent_ui.rs`
- Modify: `crates/agent_ui/src/agent_panel.rs`
- Create: `crates/agent_ui/src/ui/companion_modal.rs`
- Modify: `crates/zed/src/zed.rs`

**Step 1: Add actions**

Define actions for:
- `StartCompanion`
- `StopCompanion`
- `ShowCompanion`

**Step 2: Add a small modal or popover**

Show:
- running/stopped status
- local URL
- QR code or copyable connection token URL
- active session target

**Step 3: Wire actions into the panel**

Start and stop the companion from the agent panel without auto-starting on app launch.

**Step 4: Add UI tests**

Cover:
- modal opens from the panel
- status changes after start/stop
- URL/token is shown

### Task 5: Add A Minimal Built-In Companion Client

**Files:**
- Create: `crates/agent_companion/src/static_client.rs`
- Modify: `crates/agent_companion/src/server.rs`

**Step 1: Add a tiny built-in client page**

Serve a minimal HTML/CSS/JS client that can:
- fetch snapshot
- connect websocket
- render message list
- render run status
- show tool timeline
- send message
- stop run

**Step 2: Keep the client protocol-first**

Do not bind it to browser-only assumptions beyond basic HTML/JS; keep the transport contract generic so a native client can reuse it unchanged.

**Step 3: Add smoke tests**

Verify the static page is served and the command endpoints accept the v1 payloads.

### Task 6: Verify The Full V1 Flow

**Files:**
- Test: `crates/agent_companion/src/*`
- Test: `crates/agent_ui/src/*`

**Step 1: Run focused unit and integration tests**

Run targeted tests for:
- protocol
- session mirror
- companion server
- agent UI integration

**Step 2: Run an end-to-end local smoke flow**

Verify manually or with an integration harness:
- start companion from agent panel
- open built-in companion page
- receive active session snapshot
- observe streaming update
- send message from companion
- stop current run from companion

**Step 3: Commit in slices**

Use separate commits for:
- protocol + crate scaffolding
- session mirror
- server
- UI integration
- built-in client + final verification
