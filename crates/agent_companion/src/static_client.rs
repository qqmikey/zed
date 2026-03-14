pub fn default_client_html() -> &'static str {
    r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
  <title>Agent Companion</title>
  <style>
    :root {
      color-scheme: light;
      --page-bg: #f4ede1;
      --page-accent: #d6b487;
      --panel-bg: rgba(255, 251, 245, 0.88);
      --panel-border: rgba(107, 78, 49, 0.14);
      --text-strong: #1f1a17;
      --text-muted: #65584b;
      --text-soft: #847262;
      --success: #2e6b4e;
      --warning: #a56523;
      --danger: #8f3434;
      --shadow: 0 18px 40px rgba(74, 53, 32, 0.12);
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
        radial-gradient(circle at top left, rgba(214, 180, 135, 0.5), transparent 34%),
        linear-gradient(180deg, #fbf5ea 0%, var(--page-bg) 58%, #efe4d4 100%);
      color: var(--text-strong);
      font-family: "Avenir Next", "Segoe UI", sans-serif;
    }

    body {
      padding: 20px 14px 28px;
    }

    .shell {
      width: min(100%, 760px);
      margin: 0 auto;
      display: grid;
      gap: 14px;
    }

    .hero,
    .panel,
    .composer {
      background: var(--panel-bg);
      border: 1px solid var(--panel-border);
      border-radius: var(--radius-xl);
      box-shadow: var(--shadow);
      backdrop-filter: blur(12px);
    }

    .hero {
      padding: 18px;
      overflow: hidden;
      position: relative;
    }

    .hero::after {
      content: "";
      position: absolute;
      inset: auto -18% -42% auto;
      width: 180px;
      height: 180px;
      border-radius: 999px;
      background: radial-gradient(circle, rgba(214, 180, 135, 0.42), transparent 66%);
      pointer-events: none;
    }

    .eyebrow {
      letter-spacing: 0.14em;
      text-transform: uppercase;
      font-size: 11px;
      color: var(--text-soft);
      margin-bottom: 10px;
    }

    .hero-top {
      display: flex;
      gap: 12px;
      align-items: flex-start;
      justify-content: space-between;
    }

    h1 {
      margin: 0;
      font-size: clamp(28px, 4vw, 38px);
      line-height: 0.95;
      font-family: "Iowan Old Style", "Palatino Linotype", serif;
      max-width: 12ch;
    }

    .subtitle {
      margin: 10px 0 0;
      color: var(--text-muted);
      line-height: 1.45;
      font-size: 14px;
      max-width: 48ch;
    }

    .pill {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 8px 12px;
      border-radius: 999px;
      background: rgba(28, 25, 22, 0.08);
      color: var(--text-strong);
      font-size: 12px;
      font-weight: 600;
      white-space: nowrap;
    }

    .pill::before {
      content: "";
      width: 8px;
      height: 8px;
      border-radius: 999px;
      background: currentColor;
      opacity: 0.82;
    }

    .pill.connected {
      color: var(--success);
      background: rgba(46, 107, 78, 0.12);
    }

    .pill.reconnecting,
    .pill.connecting {
      color: var(--warning);
      background: rgba(165, 101, 35, 0.14);
    }

    .pill.disconnected,
    .pill.error {
      color: var(--danger);
      background: rgba(143, 52, 52, 0.12);
    }

    .run-card {
      margin-top: 16px;
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 12px;
    }

    .run-stat {
      padding: 14px;
      border-radius: var(--radius-lg);
      background: rgba(255, 255, 255, 0.6);
      border: 1px solid rgba(107, 78, 49, 0.08);
    }

    .run-label {
      display: block;
      font-size: 11px;
      text-transform: uppercase;
      letter-spacing: 0.12em;
      color: var(--text-soft);
      margin-bottom: 6px;
    }

    .run-value {
      font-size: 17px;
      font-weight: 700;
    }

    .stack {
      display: grid;
      gap: 14px;
    }

    .panel {
      padding: 16px;
    }

    .panel-header {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
      gap: 10px;
      margin-bottom: 12px;
    }

    .panel-header h2 {
      margin: 0;
      font-size: 15px;
      letter-spacing: 0.04em;
      text-transform: uppercase;
      color: var(--text-soft);
    }

    .meta {
      font-size: 12px;
      color: var(--text-muted);
    }

    .messages,
    .tools {
      display: grid;
      gap: 10px;
    }

    .message,
    .tool {
      border-radius: var(--radius-lg);
      padding: 14px;
      border: 1px solid rgba(107, 78, 49, 0.08);
      background: rgba(255, 255, 255, 0.72);
    }

    .message.user {
      background: rgba(214, 180, 135, 0.26);
    }

    .message.assistant.streaming {
      border-style: dashed;
      background: rgba(255, 248, 238, 0.92);
    }

    .message-header,
    .tool-header {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      align-items: flex-start;
      margin-bottom: 8px;
    }

    .badge {
      display: inline-flex;
      align-items: center;
      border-radius: 999px;
      padding: 4px 8px;
      font-size: 11px;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      background: rgba(31, 26, 23, 0.08);
      color: var(--text-muted);
    }

    .badge.success {
      color: var(--success);
      background: rgba(46, 107, 78, 0.12);
    }

    .badge.warning {
      color: var(--warning);
      background: rgba(165, 101, 35, 0.14);
    }

    .badge.danger {
      color: var(--danger);
      background: rgba(143, 52, 52, 0.12);
    }

    .message-body,
    .tool-body,
    .tool-output {
      line-height: 1.5;
      font-size: 14px;
      color: var(--text-strong);
      white-space: pre-wrap;
      word-break: break-word;
    }

    .tool-output {
      margin-top: 10px;
      padding: 10px 12px;
      border-radius: 12px;
      background: #201914;
      color: #f5ecdf;
      font-family: "JetBrains Mono", "SF Mono", monospace;
      font-size: 12px;
      overflow-x: auto;
    }

    .empty {
      padding: 18px 14px;
      border-radius: var(--radius-lg);
      border: 1px dashed rgba(107, 78, 49, 0.22);
      color: var(--text-muted);
      text-align: center;
      background: rgba(255, 255, 255, 0.42);
    }

    .composer {
      padding: 14px;
      display: grid;
      gap: 12px;
      position: sticky;
      bottom: 10px;
    }

    textarea {
      width: 100%;
      min-height: 108px;
      resize: vertical;
      border: 1px solid rgba(107, 78, 49, 0.18);
      border-radius: var(--radius-lg);
      padding: 14px;
      font: inherit;
      color: var(--text-strong);
      background: rgba(255, 255, 255, 0.92);
      box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.55);
    }

    textarea:focus {
      outline: 2px solid rgba(165, 101, 35, 0.24);
      border-color: rgba(165, 101, 35, 0.28);
    }

    .composer-actions {
      display: flex;
      gap: 10px;
    }

    button {
      appearance: none;
      border: none;
      border-radius: 999px;
      padding: 12px 16px;
      font: inherit;
      font-weight: 700;
      cursor: pointer;
      transition: transform 120ms ease, opacity 120ms ease, background 120ms ease;
    }

    button:disabled {
      opacity: 0.42;
      cursor: not-allowed;
    }

    button:not(:disabled):active {
      transform: translateY(1px);
    }

    .primary {
      background: linear-gradient(135deg, #a65d2c, #c7834e);
      color: #fff8ef;
    }

    .secondary {
      background: rgba(31, 26, 23, 0.08);
      color: var(--text-strong);
    }

    .feedback {
      min-height: 18px;
      font-size: 13px;
      color: var(--text-muted);
    }

    .feedback.success {
      color: var(--success);
    }

    .feedback.error {
      color: var(--danger);
    }

    @media (max-width: 560px) {
      body {
        padding: 12px 10px 18px;
      }

      .hero,
      .panel,
      .composer {
        border-radius: 18px;
      }

      .hero-top {
        flex-direction: column;
        align-items: stretch;
      }

      .run-card {
        grid-template-columns: 1fr;
      }

      .composer-actions {
        flex-direction: column;
      }
    }
  </style>
</head>
<body>
  <div class="shell">
    <section class="hero">
      <div class="eyebrow">Agent Companion</div>
      <div class="hero-top">
        <div>
          <h1 id="session-title">Waiting for a session</h1>
          <p class="subtitle" id="session-subtitle">Open a thread in Zed to stream progress, messages, and tool activity here.</p>
        </div>
        <div class="pill connecting" id="connection-pill">Connecting</div>
      </div>
      <div class="run-card">
        <div class="run-stat">
          <span class="run-label">Run status</span>
          <div class="run-value" id="run-status">Idle</div>
        </div>
        <div class="run-stat">
          <span class="run-label">Available commands</span>
          <div class="run-value" id="command-summary">0</div>
        </div>
      </div>
    </section>

    <section class="panel">
      <div class="panel-header">
        <h2>Conversation</h2>
        <div class="meta" id="message-count">0 messages</div>
      </div>
      <div class="messages" id="messages"></div>
    </section>

    <section class="panel">
      <div class="panel-header">
        <h2>Tool Timeline</h2>
        <div class="meta" id="tool-count">0 steps</div>
      </div>
      <div class="tools" id="tools"></div>
    </section>

    <section class="composer">
      <textarea id="composer-input" placeholder="Send a follow-up to the active Zed session"></textarea>
      <div class="composer-actions">
        <button class="primary" id="send-button">Send</button>
        <button class="secondary" id="stop-button">Stop</button>
      </div>
      <div class="feedback" id="feedback"></div>
    </section>
  </div>

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
        socket: null
      };

      const elements = {
        sessionTitle: document.getElementById("session-title"),
        sessionSubtitle: document.getElementById("session-subtitle"),
        connectionPill: document.getElementById("connection-pill"),
        runStatus: document.getElementById("run-status"),
        commandSummary: document.getElementById("command-summary"),
        messageCount: document.getElementById("message-count"),
        messages: document.getElementById("messages"),
        toolCount: document.getElementById("tool-count"),
        tools: document.getElementById("tools"),
        input: document.getElementById("composer-input"),
        sendButton: document.getElementById("send-button"),
        stopButton: document.getElementById("stop-button"),
        feedback: document.getElementById("feedback")
      };

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

      function titleCase(value) {
        return value
          .split("_")
          .map(word => word.charAt(0).toUpperCase() + word.slice(1))
          .join(" ");
      }

      function badgeClass(status) {
        if (["completed", "succeeded", "done", "connected"].includes(status)) {
          return "success";
        }
        if (["failed", "error", "canceled", "stopped", "disconnected"].includes(status)) {
          return "danger";
        }
        if (["running", "running_tools", "thinking", "waiting_for_input", "reconnecting", "connecting", "pending"].includes(status)) {
          return "warning";
        }
        return "";
      }

      function commandAvailable(kind) {
        return state.snapshot.available_commands.includes(kind);
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

      function renderMessages() {
        const entries = state.snapshot.messages.slice();
        if (state.snapshot.streaming_text) {
          entries.push({
            id: "streaming-preview",
            role: "assistant",
            status: "pending",
            text: state.snapshot.streaming_text,
            streaming: true
          });
        }

        elements.messageCount.textContent = `${entries.length} message${entries.length === 1 ? "" : "s"}`;

        if (!entries.length) {
          elements.messages.innerHTML = '<div class="empty">No messages yet.</div>';
          return;
        }

        elements.messages.innerHTML = entries.map(message => {
          const role = escapeHtml(titleCase(message.role));
          const status = escapeHtml(titleCase(message.status));
          const statusClass = badgeClass(message.status);
          const streamingClass = message.streaming ? " streaming" : "";

          return `
            <article class="message ${escapeHtml(message.role)}${streamingClass}">
              <div class="message-header">
                <span class="badge">${role}</span>
                <span class="badge ${statusClass}">${status}</span>
              </div>
              <div class="message-body">${escapeHtml(message.text)}</div>
            </article>
          `;
        }).join("");
      }

      function renderTools() {
        const toolCalls = state.snapshot.tool_calls || [];
        elements.toolCount.textContent = `${toolCalls.length} step${toolCalls.length === 1 ? "" : "s"}`;

        if (!toolCalls.length) {
          elements.tools.innerHTML = '<div class="empty">No tool activity for this run yet.</div>';
          return;
        }

        elements.tools.innerHTML = toolCalls.map(tool => {
          const summary = tool.summary ? `<div class="tool-body">${escapeHtml(tool.summary)}</div>` : "";
          const output = tool.output_preview
            ? `<div class="tool-output">${escapeHtml(tool.output_preview)}</div>`
            : "";

          return `
            <article class="tool">
              <div class="tool-header">
                <strong>${escapeHtml(tool.title)}</strong>
                <span class="badge ${badgeClass(tool.status)}">${escapeHtml(titleCase(tool.status))}</span>
              </div>
              ${summary}
              ${output}
            </article>
          `;
        }).join("");
      }

      function render() {
        const session = state.snapshot.session;
        elements.sessionTitle.textContent = session ? session.title : "Waiting for a session";
        elements.sessionSubtitle.textContent = session
          ? `Mirroring ${session.id} from the active Zed thread.`
          : "Open a thread in Zed to stream progress, messages, and tool activity here.";

        elements.connectionPill.textContent = titleCase(state.connectionState);
        elements.connectionPill.className = `pill ${state.connectionState}`;
        elements.runStatus.textContent = titleCase(state.snapshot.run_status || "idle");
        elements.commandSummary.textContent = `${state.snapshot.available_commands.length}`;

        const canSend = commandAvailable("send_message") && !state.pendingSend;
        const canStop = commandAvailable("stop_run");
        elements.sendButton.disabled = !canSend;
        elements.stopButton.disabled = !canStop;
        elements.input.disabled = !commandAvailable("send_message");

        renderMessages();
        renderTools();
        renderFeedback();
      }

      async function postCommand(command) {
        const response = await fetch(apiPath("/companion/command"), {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(command)
        });

        if (!response.ok) {
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
        if (!text || !commandAvailable("send_message")) {
          return;
        }

        state.pendingSend = true;
        render();

        try {
          await postCommand({ type: "send_message", text });
          elements.input.value = "";
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

      elements.sendButton.addEventListener("click", () => {
        sendMessage();
      });

      elements.stopButton.addEventListener("click", () => {
        stopRun();
      });

      elements.input.addEventListener("keydown", event => {
        if (event.key === "Enter" && !event.shiftKey && !event.metaKey) {
          event.preventDefault();
          sendMessage();
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
