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
      --page-bg: #efe7da;
      --shell-bg: rgba(251, 247, 240, 0.9);
      --panel-border: rgba(75, 59, 40, 0.12);
      --text-strong: #1f1913;
      --text-muted: #706352;
      --text-soft: #948676;
      --surface: rgba(255, 255, 255, 0.76);
      --surface-strong: rgba(255, 255, 255, 0.94);
      --user-bubble: #d7bf9d;
      --assistant-bubble: rgba(255, 255, 255, 0.96);
      --success: #2f6a4b;
      --warning: #9b6429;
      --danger: #8f3535;
      --action: #1f1913;
      --action-text: #fff9f0;
      --shadow: 0 18px 40px rgba(57, 41, 22, 0.12);
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
        radial-gradient(circle at top left, rgba(215, 191, 157, 0.52), transparent 28%),
        linear-gradient(180deg, #f8f2e8 0%, var(--page-bg) 54%, #e9decd 100%);
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
      border-bottom: 1px solid rgba(75, 59, 40, 0.08);
      background: linear-gradient(180deg, rgba(255, 255, 255, 0.45), rgba(255, 255, 255, 0));
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
      background: rgba(31, 25, 19, 0.08);
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
      background: rgba(47, 106, 75, 0.12);
    }

    .connection.connecting,
    .connection.reconnecting {
      color: var(--warning);
      background: rgba(155, 100, 41, 0.12);
    }

    .connection.error,
    .connection.disconnected {
      color: var(--danger);
      background: rgba(143, 53, 53, 0.12);
    }

    .activity {
      margin-top: 12px;
      padding: 10px 12px;
      border-radius: var(--radius-md);
      background: rgba(255, 255, 255, 0.55);
      border: 1px solid rgba(75, 59, 40, 0.08);
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
      border: 1px dashed rgba(75, 59, 40, 0.18);
      background: rgba(255, 255, 255, 0.42);
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
      border: 1px solid rgba(75, 59, 40, 0.08);
      background: var(--assistant-bubble);
      box-shadow: 0 1px 0 rgba(255, 255, 255, 0.35);
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
      background: rgba(31, 25, 19, 0.06);
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
      border-top: 1px solid rgba(75, 59, 40, 0.08);
      background: linear-gradient(180deg, rgba(255, 255, 255, 0.02), rgba(255, 255, 255, 0.3));
      display: grid;
      gap: 10px;
    }

    textarea {
      width: 100%;
      min-height: 86px;
      max-height: 180px;
      resize: vertical;
      border: 1px solid rgba(75, 59, 40, 0.12);
      border-radius: var(--radius-lg);
      padding: 13px 14px;
      font: inherit;
      color: var(--text-strong);
      background: var(--surface-strong);
      box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.5);
    }

    textarea:focus {
      outline: 2px solid rgba(31, 25, 19, 0.12);
      border-color: rgba(31, 25, 19, 0.18);
    }

    textarea:disabled {
      color: var(--text-soft);
      background: rgba(255, 255, 255, 0.6);
      cursor: not-allowed;
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
      background: linear-gradient(135deg, #8f3535, #b45c4e);
    }

    .action-button:disabled {
      opacity: 0.4;
      cursor: not-allowed;
    }

    .action-button:not(:disabled):active {
      transform: translateY(1px);
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
      <div class="composer-bar">
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
        actionButton: document.getElementById("action-button"),
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

      function commandAvailable(kind) {
        return state.snapshot.available_commands.includes(kind);
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

      function shouldAutoScroll() {
        const threshold = 64;
        const distanceFromBottom =
          elements.messages.scrollHeight - elements.messages.scrollTop - elements.messages.clientHeight;
        return state.forceScrollToBottom || distanceFromBottom <= threshold;
      }

      function scrollMessagesToBottom() {
        elements.messages.scrollTop = elements.messages.scrollHeight;
      }

      function renderMessages() {
        const shouldStick = shouldAutoScroll();
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

          return `
            <div class="message-row ${escapeHtml(message.streaming ? "streaming" : message.role)}">
              <article class="message">
                <div class="message-meta">
                  <span>${escapeHtml(roleLabel)}</span>
                  <span>${statusLabel}</span>
                </div>
                <div class="message-body">${escapeHtml(message.text)}</div>
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
        const sendReady = canSend && elements.input.value.trim().length > 0;

        if (hasStop) {
          elements.actionButton.textContent = "Stop";
          elements.actionButton.className = "action-button stop";
          elements.actionButton.disabled = false;
          elements.input.disabled = true;
          elements.input.placeholder = "Wait for the current run to finish or stop it.";
          return;
        }

        elements.actionButton.textContent = state.pendingSend ? "Sending..." : "Send";
        elements.actionButton.className = "action-button";
        elements.actionButton.disabled = !sendReady;
        elements.input.disabled = !commandAvailable("send_message");
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
        renderActionButton();
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
        if (!text || !commandAvailable("send_message")) {
          return;
        }

        state.pendingSend = true;
        render();

        try {
          await postCommand({ type: "send_message", text });
          elements.input.value = "";
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
