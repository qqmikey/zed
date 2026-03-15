pub fn default_client_html() -> &'static str {
    r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
  <title>Agent Companion</title>
  <style>
    :root {
      color-scheme: dark;
      --page-bg: #17191f;
      --shell-bg: rgba(31, 33, 40, 0.94);
      --panel-border: rgba(255, 255, 255, 0.08);
      --text-strong: #e7eaf0;
      --text-muted: #a3acba;
      --text-soft: #737c8d;
      --surface: rgba(42, 45, 54, 0.92);
      --surface-strong: rgba(47, 50, 61, 0.98);
      --user-bubble: #1f3c63;
      --assistant-bubble: rgba(42, 45, 54, 0.98);
      --success: #5cbf89;
      --warning: #d0a14a;
      --danger: #d66b6b;
      --action: #2f6feb;
      --action-text: #f8fbff;
      --shadow: 0 20px 44px rgba(0, 0, 0, 0.42);
      --radius-xl: 24px;
      --radius-lg: 18px;
      --radius-md: 14px;
    }

    * {
      box-sizing: border-box;
    }

    html,
    body {
      min-height: 100%;
      margin: 0;
      background:
        radial-gradient(circle at top left, rgba(47, 111, 235, 0.16), transparent 26%),
        radial-gradient(circle at bottom right, rgba(104, 84, 255, 0.1), transparent 24%),
        linear-gradient(180deg, #1b1d24 0%, var(--page-bg) 52%, #14161c 100%);
      color: var(--text-strong);
      font-family: "Avenir Next", "Segoe UI", sans-serif;
    }

    body {
      padding: 12px;
    }

    .app {
      width: min(100%, 760px);
      min-height: calc(100svh - 24px);
      margin: 0 auto;
      display: grid;
      grid-template-rows: auto minmax(0, 1fr) auto;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius-xl);
      background: var(--shell-bg);
      box-shadow: var(--shadow);
      backdrop-filter: blur(14px);
      overflow: hidden;
    }

    .topbar {
      padding: 16px 16px 14px;
      border-bottom: 1px solid rgba(255, 255, 255, 0.06);
      background: linear-gradient(180deg, rgba(255, 255, 255, 0.04), rgba(255, 255, 255, 0));
    }

    .topbar-row {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: 12px;
    }

    .kicker {
      font-size: 11px;
      line-height: 1;
      letter-spacing: 0.14em;
      text-transform: uppercase;
      color: var(--text-soft);
      margin-bottom: 8px;
    }

    h1 {
      margin: 0;
      font-size: 24px;
      line-height: 1.05;
      font-family: "Iowan Old Style", "Palatino Linotype", serif;
      max-width: 16ch;
    }

    .subtitle {
      margin: 6px 0 0;
      color: var(--text-muted);
      font-size: 13px;
      line-height: 1.45;
    }

    .connection {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 8px 11px;
      border-radius: 999px;
      background: rgba(255, 255, 255, 0.04);
      color: var(--text-strong);
      font-size: 11px;
      font-weight: 700;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      white-space: nowrap;
    }

    .connection::before {
      content: "";
      width: 8px;
      height: 8px;
      border-radius: 999px;
      background: currentColor;
      opacity: 0.85;
    }

    .connection.connected {
      color: var(--success);
      background: rgba(92, 191, 137, 0.12);
    }

    .connection.connecting,
    .connection.reconnecting {
      color: var(--warning);
      background: rgba(208, 161, 74, 0.12);
    }

    .connection.error,
    .connection.disconnected {
      color: var(--danger);
      background: rgba(214, 107, 107, 0.12);
    }

    .activity {
      margin-top: 12px;
      padding: 10px 12px;
      border-radius: var(--radius-md);
      background: var(--surface);
      border: 1px solid rgba(255, 255, 255, 0.05);
      color: var(--text-muted);
      font-size: 13px;
      line-height: 1.4;
    }

    .messages-shell {
      min-height: 0;
      overflow: hidden;
    }

    .messages {
      height: 100%;
      overflow-y: auto;
      padding: 14px 12px 6px;
      display: grid;
      gap: 10px;
      align-content: start;
    }

    .empty {
      margin: auto;
      padding: 18px 16px;
      border-radius: var(--radius-lg);
      border: 1px dashed rgba(255, 255, 255, 0.12);
      background: rgba(255, 255, 255, 0.03);
      color: var(--text-muted);
      text-align: center;
      max-width: 28ch;
      font-size: 14px;
      line-height: 1.45;
    }

    .message-row {
      display: flex;
      width: 100%;
    }

    .message-row.user {
      justify-content: flex-end;
    }

    .message-row.assistant,
    .message-row.streaming {
      justify-content: flex-start;
    }

    .message-row.system,
    .message-row.tool {
      justify-content: center;
    }

    .message {
      max-width: min(92%, 560px);
      padding: 12px 14px;
      border-radius: 20px;
      border: 1px solid rgba(255, 255, 255, 0.06);
      background: var(--assistant-bubble);
      box-shadow: 0 10px 24px rgba(0, 0, 0, 0.14);
    }

    .message-row.user .message {
      background: var(--user-bubble);
      border-bottom-right-radius: 8px;
    }

    .message-row.assistant .message,
    .message-row.streaming .message {
      border-bottom-left-radius: 8px;
    }

    .message-row.system .message,
    .message-row.tool .message {
      background: rgba(255, 255, 255, 0.04);
      color: var(--text-muted);
    }

    .message-meta {
      display: flex;
      justify-content: space-between;
      gap: 10px;
      margin-bottom: 6px;
      font-size: 11px;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      color: var(--text-soft);
    }

    .message-body {
      white-space: pre-wrap;
      word-break: break-word;
      line-height: 1.5;
      font-size: 14px;
    }

    .message-attachments {
      display: grid;
      gap: 10px;
      margin-top: 10px;
    }

    .attachment-image {
      display: block;
      width: 100%;
      max-width: 100%;
      border-radius: 14px;
      border: 1px solid rgba(255, 255, 255, 0.08);
      background: rgba(255, 255, 255, 0.03);
      overflow: hidden;
    }

    .attachment-image img {
      display: block;
      width: 100%;
      height: auto;
      max-height: 420px;
      object-fit: contain;
      background: rgba(255, 255, 255, 0.02);
    }

    .attachment-file,
    .attachment-link {
      display: grid;
      gap: 8px;
      padding: 10px 12px;
      border-radius: 14px;
      background: rgba(255, 255, 255, 0.04);
      border: 1px solid rgba(255, 255, 255, 0.06);
    }

    .attachment-title {
      font-size: 13px;
      font-weight: 700;
      color: var(--text-strong);
      word-break: break-word;
    }

    .attachment-meta {
      font-size: 12px;
      color: var(--text-soft);
    }

    .attachment-actions {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
    }

    .attachment-action {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      min-height: 34px;
      padding: 0 12px;
      border-radius: 999px;
      color: var(--action-text);
      background: rgba(47, 111, 235, 0.86);
      text-decoration: none;
      font-size: 12px;
      font-weight: 700;
    }

    .attachment-action.secondary {
      background: rgba(255, 255, 255, 0.08);
      color: var(--text-strong);
    }

    .streaming-indicator {
      display: inline-flex;
      align-items: center;
      gap: 6px;
    }

    .streaming-indicator::before {
      content: "";
      width: 6px;
      height: 6px;
      border-radius: 999px;
      background: currentColor;
      animation: pulse 1.2s ease-in-out infinite;
    }

    @keyframes pulse {
      0%, 100% {
        opacity: 0.35;
        transform: scale(0.9);
      }
      50% {
        opacity: 1;
        transform: scale(1);
      }
    }

    .composer {
      padding: 10px 12px 12px;
      border-top: 1px solid rgba(255, 255, 255, 0.06);
      background: linear-gradient(180deg, rgba(255, 255, 255, 0.01), rgba(255, 255, 255, 0.03));
      display: grid;
      gap: 10px;
    }

    textarea {
      width: 100%;
      min-height: 86px;
      max-height: 180px;
      resize: vertical;
      border: 1px solid rgba(255, 255, 255, 0.08);
      border-radius: var(--radius-lg);
      padding: 13px 14px;
      font: inherit;
      color: var(--text-strong);
      background: var(--surface-strong);
      box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
    }

    textarea:focus {
      outline: 2px solid rgba(47, 111, 235, 0.22);
      border-color: rgba(47, 111, 235, 0.38);
    }

    textarea:disabled {
      color: var(--text-soft);
      background: rgba(255, 255, 255, 0.04);
      cursor: not-allowed;
    }

    .selected-attachments {
      display: grid;
      gap: 8px;
    }

    .selected-attachments:empty {
      display: none;
    }

    .selected-attachment {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      padding: 10px 12px;
      border-radius: 14px;
      background: rgba(255, 255, 255, 0.04);
      border: 1px solid rgba(255, 255, 255, 0.06);
    }

    .selected-attachment-copy {
      min-width: 0;
      display: grid;
      gap: 2px;
    }

    .selected-attachment-name {
      font-size: 13px;
      font-weight: 700;
      color: var(--text-strong);
      word-break: break-word;
    }

    .selected-attachment-meta {
      font-size: 12px;
      color: var(--text-soft);
    }

    .composer-bar {
      display: flex;
      align-items: center;
      gap: 10px;
    }

    .feedback {
      flex: 1;
      min-height: 18px;
      font-size: 12px;
      color: var(--text-muted);
      line-height: 1.4;
    }

    .feedback.success {
      color: var(--success);
    }

    .feedback.error {
      color: var(--danger);
    }

    .action-button {
      appearance: none;
      border: none;
      border-radius: 999px;
      min-width: 104px;
      padding: 12px 18px;
      background: var(--action);
      color: var(--action-text);
      font: inherit;
      font-weight: 700;
      cursor: pointer;
      transition: opacity 120ms ease, transform 120ms ease, background 120ms ease;
    }

    .action-button.stop {
      background: linear-gradient(135deg, #a44a4a, #c36262);
    }

    .action-button:disabled {
      opacity: 0.4;
      cursor: not-allowed;
    }

    .action-button:not(:disabled):active {
      transform: translateY(1px);
    }

    .composer-tools {
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .secondary-button {
      appearance: none;
      border: 1px solid rgba(255, 255, 255, 0.08);
      border-radius: 999px;
      min-width: 40px;
      min-height: 40px;
      padding: 0 14px;
      background: rgba(255, 255, 255, 0.04);
      color: var(--text-strong);
      font: inherit;
      font-weight: 700;
      cursor: pointer;
    }

    .secondary-button:disabled {
      opacity: 0.35;
      cursor: not-allowed;
    }

    .remove-attachment {
      appearance: none;
      border: none;
      border-radius: 999px;
      min-width: 28px;
      min-height: 28px;
      background: rgba(255, 255, 255, 0.08);
      color: var(--text-strong);
      font: inherit;
      font-weight: 700;
      cursor: pointer;
    }

    .file-input {
      display: none;
    }

    @media (max-width: 560px) {
      body {
        padding: 0;
      }

      .app {
        width: 100%;
        min-height: 100svh;
        border-radius: 0;
        border-left: none;
        border-right: none;
      }

      .topbar {
        padding-top: max(16px, env(safe-area-inset-top));
      }

      .composer {
        padding-bottom: max(12px, env(safe-area-inset-bottom));
      }

      .topbar-row,
      .composer-bar {
        flex-direction: column;
        align-items: stretch;
      }

      .action-button {
        width: 100%;
      }
    }
  </style>
</head>
<body>
  <main class="app">
    <header class="topbar">
      <div class="topbar-row">
        <div>
          <div class="kicker">Agent Companion</div>
          <h1 id="session-title">Waiting for a session</h1>
          <p class="subtitle" id="session-subtitle">Open a thread in Zed to mirror the conversation here.</p>
        </div>
        <div class="connection connecting" id="connection-pill">Connecting</div>
      </div>
      <div class="activity" id="activity-line">Idle</div>
    </header>

    <section class="messages-shell">
      <div class="messages" id="messages"></div>
    </section>

    <form class="composer" id="composer-form">
      <textarea id="composer-input" placeholder="Send a follow-up to the active Zed session"></textarea>
      <input class="file-input" id="attachment-input" type="file" multiple>
      <div class="selected-attachments" id="selected-attachments"></div>
      <div class="composer-bar">
        <div class="composer-tools">
          <button class="secondary-button" id="attach-button" type="button">Attach</button>
        </div>
        <div class="feedback" id="feedback"></div>
        <button class="action-button" id="action-button" type="submit">Send</button>
      </div>
    </form>
  </main>

  <script>
    (() => {
      const state = {
        snapshot: {
          session: null,
          messages: [],
          streaming_text: null,
          tool_calls: [],
          run_status: "idle",
          available_commands: []
        },
        connectionState: "connecting",
        feedback: "",
        feedbackKind: "info",
        pendingSend: false,
        pendingAttachments: [],
        socket: null,
        forceScrollToBottom: true
      };

      const elements = {
        sessionTitle: document.getElementById("session-title"),
        sessionSubtitle: document.getElementById("session-subtitle"),
        connectionPill: document.getElementById("connection-pill"),
        activityLine: document.getElementById("activity-line"),
        messages: document.getElementById("messages"),
        composerForm: document.getElementById("composer-form"),
        input: document.getElementById("composer-input"),
        attachmentInput: document.getElementById("attachment-input"),
        selectedAttachments: document.getElementById("selected-attachments"),
        attachButton: document.getElementById("attach-button"),
        actionButton: document.getElementById("action-button"),
        feedback: document.getElementById("feedback")
      };

      const maxAttachmentBytes = 10 * 1024 * 1024;

      function token() {
        return new URLSearchParams(window.location.search).get("token") || "";
      }

      function apiPath(path) {
        return `${path}?token=${encodeURIComponent(token())}`;
      }

      function websocketUrl() {
        const url = new URL(window.location.href);
        url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
        url.pathname = "/companion/events";
        url.search = `?token=${encodeURIComponent(token())}`;
        url.hash = "";
        return url.toString();
      }

      function escapeHtml(text) {
        return text
          .replaceAll("&", "&amp;")
          .replaceAll("<", "&lt;")
          .replaceAll(">", "&gt;")
          .replaceAll('"', "&quot;");
      }

      function assetUrl(assetId) {
        return apiPath(`/companion/assets/${encodeURIComponent(assetId)}`);
      }

      function titleCase(value) {
        return value
          .split("_")
          .map(word => word.charAt(0).toUpperCase() + word.slice(1))
          .join(" ");
      }

      function commandAvailable(kind) {
        return state.snapshot.available_commands.includes(kind);
      }

      function attachmentUploadAvailable() {
        return commandAvailable("send_attachments");
      }

      function activeToolSummary() {
        const runningTool = (state.snapshot.tool_calls || []).find(tool =>
          tool.status === "running" || tool.status === "pending"
        );

        if (!runningTool) {
          return null;
        }

        return runningTool.summary || runningTool.title || null;
      }

      function activityText() {
        const toolSummary = activeToolSummary();

        switch (state.snapshot.run_status) {
          case "running_tools":
            return toolSummary ? `Running: ${toolSummary}` : "Running tools";
          case "thinking":
            return "Thinking";
          case "waiting_for_input":
            return "Waiting for input";
          case "failed":
            return "Run failed";
          case "stopped":
            return "Run stopped";
          case "completed":
            return "Ready";
          case "idle":
          default:
            return "Idle";
        }
      }

      function setFeedback(message, kind = "info") {
        state.feedback = message;
        state.feedbackKind = kind;
        renderFeedback();
      }

      function renderFeedback() {
        elements.feedback.textContent = state.feedback;
        elements.feedback.className = `feedback ${state.feedbackKind}`;
      }

      function selectedAttachmentSummary(file) {
        const size = file.size >= 1024 * 1024
          ? `${(file.size / (1024 * 1024)).toFixed(1)} MB`
          : `${Math.max(1, Math.round(file.size / 1024))} KB`;
        const type = file.type || "file";
        return `${type} · ${size}`;
      }

      function renderSelectedAttachments() {
        if (!state.pendingAttachments.length) {
          elements.selectedAttachments.innerHTML = "";
          return;
        }

        elements.selectedAttachments.innerHTML = state.pendingAttachments.map((file, index) => `
          <div class="selected-attachment">
            <div class="selected-attachment-copy">
              <div class="selected-attachment-name">${escapeHtml(file.name)}</div>
              <div class="selected-attachment-meta">${escapeHtml(selectedAttachmentSummary(file))}</div>
            </div>
            <button class="remove-attachment" type="button" data-attachment-index="${index}">×</button>
          </div>
        `).join("");
      }

      function shouldAutoScroll() {
        const threshold = 64;
        const distanceFromBottom =
          elements.messages.scrollHeight - elements.messages.scrollTop - elements.messages.clientHeight;
        return state.forceScrollToBottom || distanceFromBottom <= threshold;
      }

      function scrollMessagesToBottom() {
        elements.messages.scrollTop = elements.messages.scrollHeight;
      }

      function lastPendingAssistantIndex(entries) {
        for (let index = entries.length - 1; index >= 0; index -= 1) {
          const entry = entries[index];
          if (entry.role === "assistant" && entry.status === "pending") {
            return index;
          }
        }

        return -1;
      }

      function renderAttachments(attachments) {
        if (!attachments || !attachments.length) {
          return "";
        }

        return `
          <div class="message-attachments">
            ${attachments.map(attachment => {
              switch (attachment.type) {
                case "image":
                  return `
                    <a class="attachment-image" href="${escapeHtml(assetUrl(attachment.asset_id))}" target="_blank" rel="noreferrer">
                      <img src="${escapeHtml(assetUrl(attachment.asset_id))}" alt="${escapeHtml(attachment.name)}" loading="lazy">
                    </a>
                  `;
                case "file":
                  return `
                    <div class="attachment-file">
                      <div class="attachment-title">${escapeHtml(attachment.name)}</div>
                      ${attachment.mime_type ? `<div class="attachment-meta">${escapeHtml(attachment.mime_type)}</div>` : ""}
                      <div class="attachment-actions">
                        <a class="attachment-action" href="${escapeHtml(assetUrl(attachment.asset_id))}" target="_blank" rel="noreferrer">Open</a>
                        <a class="attachment-action secondary" href="${escapeHtml(assetUrl(attachment.asset_id))}" download="${escapeHtml(attachment.name)}">Download</a>
                      </div>
                    </div>
                  `;
                case "link":
                  return `
                    <div class="attachment-link">
                      <div class="attachment-title">${escapeHtml(attachment.name)}</div>
                      <div class="attachment-actions">
                        <a class="attachment-action" href="${escapeHtml(attachment.url)}" target="_blank" rel="noreferrer">Open link</a>
                      </div>
                    </div>
                  `;
                default:
                  return "";
              }
            }).join("")}
          </div>
        `;
      }

      function renderMessages() {
        const shouldStick = shouldAutoScroll();
        const entries = state.snapshot.messages.slice();
        const pendingAssistantIndex = lastPendingAssistantIndex(entries);
        const showThinkingIndicator =
          !state.snapshot.streaming_text && state.snapshot.run_status === "thinking";

        if (state.snapshot.streaming_text) {
          if (pendingAssistantIndex >= 0) {
            entries[pendingAssistantIndex] = {
              ...entries[pendingAssistantIndex],
              text: state.snapshot.streaming_text,
              streaming: true,
              placeholder: false
            };
          } else {
            entries.push({
              id: "streaming-preview",
              role: "assistant",
              status: "pending",
              text: state.snapshot.streaming_text,
              attachments: [],
              streaming: true
            });
          }
        }

        if (showThinkingIndicator) {
          if (pendingAssistantIndex >= 0) {
            entries[pendingAssistantIndex] = {
              ...entries[pendingAssistantIndex],
              text: "",
              streaming: true,
              placeholder: true
            };
          } else {
            entries.push({
              id: "thinking-preview",
              role: "assistant",
              status: "pending",
              text: "",
              attachments: [],
              streaming: true,
              placeholder: true
            });
          }
        }

        if (!entries.length) {
          elements.messages.innerHTML = '<div class="empty">The current thread has no messages yet.</div>';
          if (shouldStick) {
            scrollMessagesToBottom();
          }
          return;
        }

        elements.messages.innerHTML = entries.map(message => {
          const roleLabel = message.role === "user" ? "You" : titleCase(message.role);
          const statusLabel = message.streaming
            ? '<span class="streaming-indicator">Streaming</span>'
            : escapeHtml(titleCase(message.status));
          const textBody = message.placeholder
            ? '<div class="message-body"><span class="streaming-indicator">Thinking</span></div>'
            : (message.text ? `<div class="message-body">${escapeHtml(message.text)}</div>` : "");
          const attachments = renderAttachments(message.attachments || []);
          const body = `${textBody}${attachments}`;

          return `
            <div class="message-row ${escapeHtml(message.streaming ? "streaming" : message.role)}">
              <article class="message">
                <div class="message-meta">
                  <span>${escapeHtml(roleLabel)}</span>
                  <span>${statusLabel}</span>
                </div>
                ${body}
              </article>
            </div>
          `;
        }).join("");

        if (shouldStick) {
          scrollMessagesToBottom();
        }
        state.forceScrollToBottom = false;
      }

      function renderActionButton() {
        const hasStop = commandAvailable("stop_run");
        const canSend = commandAvailable("send_message") && !state.pendingSend;
        const attachmentsAllowed =
          state.pendingAttachments.length === 0 || attachmentUploadAvailable();
        const sendReady = canSend &&
          attachmentsAllowed &&
          (elements.input.value.trim().length > 0 || state.pendingAttachments.length > 0);

        if (hasStop) {
          elements.actionButton.textContent = "Stop";
          elements.actionButton.className = "action-button stop";
          elements.actionButton.disabled = false;
          elements.input.disabled = true;
          elements.attachButton.disabled = true;
          elements.input.placeholder = "Wait for the current run to finish or stop it.";
          return;
        }

        elements.actionButton.textContent = state.pendingSend ? "Sending..." : "Send";
        elements.actionButton.className = "action-button";
        elements.actionButton.disabled = !sendReady;
        elements.input.disabled = !commandAvailable("send_message");
        elements.attachButton.disabled = !attachmentUploadAvailable() || state.pendingSend;
        elements.input.placeholder = commandAvailable("send_message")
          ? "Send a follow-up to the active Zed session"
          : "This session is not ready for input";
      }

      function render() {
        const session = state.snapshot.session;

        elements.sessionTitle.textContent = session ? session.title : "Waiting for a session";
        elements.sessionSubtitle.textContent = session
          ? "Live view of the active Zed thread."
          : "Open a thread in Zed to mirror the conversation here.";
        elements.connectionPill.textContent = titleCase(state.connectionState);
        elements.connectionPill.className = `connection ${state.connectionState}`;
        elements.activityLine.textContent = activityText();

        renderMessages();
        renderSelectedAttachments();
        renderActionButton();
        renderFeedback();
      }

      function fileToUpload(file) {
        return new Promise((resolve, reject) => {
          const reader = new FileReader();
          reader.onerror = () => reject(new Error(`Failed to read ${file.name}`));
          reader.onload = () => {
            const result = typeof reader.result === "string" ? reader.result : "";
            const [, dataBase64 = ""] = result.split(",", 2);
            resolve({
              name: file.name,
              mime_type: file.type || "application/octet-stream",
              data_base64: dataBase64
            });
          };
          reader.readAsDataURL(file);
        });
      }

      async function postCommand(command) {
        const response = await fetch(apiPath("/companion/command"), {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(command)
        });

        if (!response.ok) {
          if (response.status === 413) {
            throw new Error("Attachments exceed the companion upload request limit. Try fewer or smaller files.");
          }
          throw new Error(`Command failed with status ${response.status}`);
        }
      }

      async function loadSnapshot() {
        const response = await fetch(apiPath("/companion/snapshot"), { cache: "no-store" });
        if (!response.ok) {
          throw new Error(`Snapshot failed with status ${response.status}`);
        }

        state.snapshot = await response.json();
        render();
      }

      function applyEvent(event) {
        switch (event.type) {
          case "snapshot_replaced":
            state.snapshot = event.snapshot;
            break;
          case "messages_changed":
            state.snapshot.messages = event.messages;
            break;
          case "streaming_text_changed":
            state.snapshot.streaming_text = event.streaming_text;
            break;
          case "tool_calls_changed":
            state.snapshot.tool_calls = event.tool_calls;
            break;
          case "run_status_changed":
            state.snapshot.run_status = event.run_status;
            break;
          default:
            break;
        }

        state.forceScrollToBottom = true;
        render();
      }

      function connectSocket() {
        if (!token()) {
          state.connectionState = "error";
          setFeedback("Missing token in companion URL.", "error");
          render();
          return;
        }

        if (state.socket) {
          state.socket.close();
        }

        state.connectionState = "connecting";
        render();

        const socket = new WebSocket(websocketUrl());
        state.socket = socket;

        socket.addEventListener("open", () => {
          if (state.socket !== socket) {
            socket.close();
            return;
          }

          state.connectionState = "connected";
          render();
        });

        socket.addEventListener("message", event => {
          try {
            applyEvent(JSON.parse(event.data));
          } catch (error) {
            setFeedback(`Failed to decode update: ${error}`, "error");
          }
        });

        socket.addEventListener("error", () => {
          if (state.socket !== socket) {
            return;
          }

          state.connectionState = "reconnecting";
          render();
        });

        socket.addEventListener("close", () => {
          if (state.socket !== socket) {
            return;
          }

          state.connectionState = "reconnecting";
          render();
          window.setTimeout(async () => {
            try {
              await loadSnapshot();
            } catch (error) {
              setFeedback(`Reconnect snapshot failed: ${error}`, "error");
            }
            connectSocket();
          }, 1500);
        });
      }

      async function sendMessage() {
        const text = elements.input.value.trim();
        if (
          (!text && state.pendingAttachments.length === 0) ||
          !commandAvailable("send_message") ||
          (state.pendingAttachments.length > 0 && !attachmentUploadAvailable())
        ) {
          return;
        }

        state.pendingSend = true;
        render();

        try {
          const attachments = await Promise.all(state.pendingAttachments.map(fileToUpload));
          await postCommand({ type: "send_message", text, attachments });
          elements.input.value = "";
          state.pendingAttachments = [];
          elements.attachmentInput.value = "";
          state.forceScrollToBottom = true;
          setFeedback("Message sent to the active Zed session.", "success");
        } catch (error) {
          setFeedback(`Failed to send message: ${error}`, "error");
        } finally {
          state.pendingSend = false;
          render();
        }
      }

      async function stopRun() {
        if (!commandAvailable("stop_run")) {
          return;
        }

        try {
          await postCommand({ type: "stop_run" });
          setFeedback("Stop requested.", "success");
        } catch (error) {
          setFeedback(`Failed to stop run: ${error}`, "error");
        }
      }

      async function handlePrimaryAction() {
        if (commandAvailable("stop_run")) {
          await stopRun();
          return;
        }

        await sendMessage();
      }

      elements.composerForm.addEventListener("submit", event => {
        event.preventDefault();
        handlePrimaryAction();
      });

      elements.input.addEventListener("input", () => {
        renderActionButton();
      });

      elements.attachButton.addEventListener("click", () => {
        if (!attachmentUploadAvailable() || state.pendingSend || commandAvailable("stop_run")) {
          return;
        }
        elements.attachmentInput.click();
      });

      elements.attachmentInput.addEventListener("change", event => {
        const files = Array.from(event.target.files || []);
        if (!files.length) {
          return;
        }

        const accepted = [];
        for (const file of files) {
          if (file.size > maxAttachmentBytes) {
            setFeedback(`${file.name} exceeds the 10 MB companion upload limit.`, "error");
            continue;
          }
          accepted.push(file);
        }

        state.pendingAttachments = state.pendingAttachments.concat(accepted);
        render();
      });

      elements.selectedAttachments.addEventListener("click", event => {
        const button = event.target.closest("[data-attachment-index]");
        if (!button) {
          return;
        }

        const index = Number(button.getAttribute("data-attachment-index"));
        if (!Number.isFinite(index)) {
          return;
        }

        state.pendingAttachments.splice(index, 1);
        render();
      });

      elements.input.addEventListener("keydown", event => {
        if (event.key === "Enter" && !event.shiftKey && !event.metaKey && !commandAvailable("stop_run")) {
          event.preventDefault();
          handlePrimaryAction();
        }
      });

      window.addEventListener("focus", () => {
        loadSnapshot().catch(error => {
          setFeedback(`Refresh failed: ${error}`, "error");
        });
      });

      loadSnapshot()
        .then(() => {
          setFeedback("Connected to the active Zed session.", "success");
        })
        .catch(error => {
          setFeedback(`Failed to load snapshot: ${error}`, "error");
          state.connectionState = "error";
          render();
        })
        .finally(() => {
          connectSocket();
          render();
        });
    })();
  </script>
</body>
</html>
"##
}
