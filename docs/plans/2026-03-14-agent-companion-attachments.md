# Agent Companion Attachments Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let the Agent Companion mobile/web client render inline images and offer downloadable/openable files from mirrored ACP thread messages.

**Architecture:** Preserve ACP message structure inside `agent_companion` instead of flattening everything to plain text. Map ACP `Image` blocks to served companion assets, map local `file://` resource links to downloadable file attachments, and keep the mobile client protocol-first with authenticated asset URLs.

**Tech Stack:** Rust, GPUI entities/subscriptions, axum HTTP/WebSocket, serde, ACP thread content blocks, local asset streaming.

---

### Task 1: Extend The Companion Protocol For Attachments

**Files:**
- Modify: `crates/agent_companion/src/protocol.rs`

**Step 1: Add structured message attachments**

Add attachment types that let a message carry:
- inline images
- downloadable files
- external links when no local asset proxy is needed

**Step 2: Keep protocol round-trip coverage**

Update serialization tests so snapshots/events round-trip with attachment metadata.

### Task 2: Preserve ACP Attachments In The Session Mirror

**Files:**
- Modify: `crates/agent_companion/src/session_mirror.rs`
- Test: `crates/agent_companion/src/session_mirror.rs`

**Step 1: Extract ACP content blocks without losing structure**

Map:
- `ContentBlock::Image` -> image attachment
- `ContentBlock::ResourceLink(file://...)` -> downloadable file attachment
- everything else -> text content

**Step 2: Keep text-thread behavior unchanged**

Do not expand scope into text-thread attachments in this pass.

**Step 3: Add mirror tests**

Cover image and file attachment extraction from ACP entries.

### Task 3: Add Authenticated Asset Serving To The Companion Server

**Files:**
- Modify: `crates/agent_companion/src/agent_companion.rs`
- Modify: `crates/agent_companion/src/server.rs`
- Test: `crates/agent_companion/src/server.rs`

**Step 1: Add an in-memory asset registry**

Register:
- image bytes generated from ACP image blocks
- local file paths for downloadable attachments

**Step 2: Add an authenticated asset route**

Serve assets through `GET /companion/assets/:id?token=...` with:
- token enforcement
- correct `Content-Type`
- inline vs attachment disposition based on asset kind

**Step 3: Add server tests**

Cover:
- unauthorized asset access
- successful image serving
- successful file download serving

### Task 4: Render Attachments In The Embedded Mobile Client

**Files:**
- Modify: `crates/agent_companion/src/static_client.rs`

**Step 1: Render inline images**

Show image attachments directly inside the message body.

**Step 2: Render downloadable file chips**

Show file attachments as action rows with download/open links.

**Step 3: Preserve existing chat-first layout**

Keep the new UI minimal and avoid reintroducing dense tool chrome.

### Task 5: Verify The Attachment Flow

**Files:**
- Test: `crates/agent_companion/src/*`

**Step 1: Run focused tests**

Run:
- `cargo test -p agent_companion`

**Step 2: Smoke the end-to-end companion page**

Verify manually:
- inline image shows in the web client
- non-image file offers download/open
- existing text-only messages still render correctly
