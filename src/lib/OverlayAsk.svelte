<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { ipc } from "$lib/ipc";
  import { renderMarkdown } from "$lib/markdown";
  import MarkdownView from "$lib/MarkdownView.svelte";
  import type { AppSettings, ChatAttachment, ContextPayload, Exchange, ProviderView, SessionTurn, TtsLiveState } from "$lib/types";
  import { t, applyLanguage, localeTag } from "$lib/i18n.svelte";

  type Live = {
    query: string;
    session: SessionTurn[];
    error: string;
    chatId: string | null;
    attachments: ChatAttachment[];
  };

  let root = $state<HTMLElement | null>(null);
  let sheetEl = $state<HTMLElement | null>(null);
  let inputEl = $state<HTMLTextAreaElement | null>(null);
  let markEl = $state<HTMLSpanElement | null>(null);
  let caretX = $state(0);
  let caretY = $state(0);
  let caretHeight = $state(18);
  let query = $state("");
  let attachments = $state<ChatAttachment[]>([]);
  let error = $state("");
  let streaming = $state(false);
  let copied = $state(false);
  let focused = $state(false);
  let hasSelection = $state(false);
  let caretIndex = $state(0);
  let requestId = $state<string | null>(null);
  let providers = $state<ProviderView[]>([]);
  let settings = $state<AppSettings | null>(null);
  let selection = $state<string | null>(null);
  let clipboard = $state<string | null>(null);
  let history = $state<Exchange[]>([]);
  /// -1 is the live draft, 0 the newest stored exchange, higher values older ones.
  let cursor = $state(-1);
  let session = $state<SessionTurn[]>([]);
  let sessionChatId = $state<string | null>(null);
  let browseTurns = $state<SessionTurn[]>([]);
  let live = $state<Live>({ query: "", session: [], error: "", chatId: null, attachments: [] });
  let ttsState = $state<TtsLiveState>({ synthesizing: false, playing: false, busy: false });
  let hiddenAt = 0;
  const SESSION_TTL_MS = 60_000;

  let activeProvider = $derived(
    providers.find((item) => item.id === settings?.activeProviderId) ?? providers[0] ?? null,
  );
  let before = $derived(query.slice(0, caretIndex));
  let after = $derived(query.slice(caretIndex));
  let activeHistory = $derived(
    sessionChatId ? history.filter((item) => item.id !== sessionChatId) : history
  );
  let canBrowse = $derived(!streaming && (activeHistory.length > 0 || session.length > 0));
  let browsing = $derived(cursor >= 0);
  let entry = $derived(cursor >= 0 ? (activeHistory[cursor] ?? null) : null);
  let turns = $derived(browsing ? browseTurns : session);
  let expanded = $derived(turns.length > 0 || Boolean(error) || streaming);
  let lastAssistant = $derived.by(() => {
    for (let index = turns.length - 1; index >= 0; index -= 1) {
      const turn = turns[index];
      if (turn?.role === "assistant" && turn.content) return turn.content;
    }
    return "";
  });

  const stamp = $derived(
    new Intl.DateTimeFormat(localeTag(), {
      day: "2-digit",
      month: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    })
  );

  async function refreshHeight() {
    if (!root) return;
    await ipc.setOverlayHeight(Math.ceil(root.getBoundingClientRect().height));
  }

  function autoSize() {
    if (!inputEl) return;
    inputEl.style.height = "auto";
    inputEl.style.height = `${Math.min(inputEl.scrollHeight, 220)}px`;
  }

  function placeCaret() {
    if (!markEl) return;
    const h = markEl.offsetHeight;
    caretHeight = Math.round(h * 0.9);
    caretX = markEl.offsetLeft;
    caretY = markEl.offsetTop + Math.round((h - caretHeight) / 2) + 1;
  }

  function syncCaret() {
    if (!inputEl) return;
    caretIndex = inputEl.selectionEnd ?? query.length;
    hasSelection = inputEl.selectionStart !== inputEl.selectionEnd;
    void tick().then(placeCaret);
  }

  async function loadBase() {
    const next = await ipc.getSettings();
    applyLanguage(next?.language);
    settings = next;
    providers = await ipc.listProviders();
    await loadHistory();
  }

  async function loadHistory() {
    if (!settings?.historyEnabled) {
      history = [];
      cursor = -1;
      return;
    }
    try {
      history = await ipc.listExchanges(40);
    } catch {
      history = [];
    }
    if (cursor >= activeHistory.length) cursor = activeHistory.length - 1;
  }

  function snapshotLive(): Live {
    return {
      query,
      session: session.slice(),
      error,
      chatId: sessionChatId,
      attachments: attachments.slice(),
    };
  }

  function showLive() {
    cursor = -1;
    query = live.query;
    error = live.error;
    session = live.session.slice();
    sessionChatId = live.chatId;
    attachments = live.attachments.slice();
    browseTurns = [];
    copied = false;
  }

  function resetSession() {
    cursor = -1;
    query = "";
    attachments = [];
    error = "";
    copied = false;
    session = [];
    sessionChatId = null;
    browseTurns = [];
    live = { query: "", session: [], error: "", chatId: null, attachments: [] };
  }

  /// Keep the live draft + session for a short window after hide
  /// (accidental Esc). After that, blank — history still has the chat.
  let expiryTimer: number | null = null;

  function scheduleExpiry() {
    if (expiryTimer !== null) window.clearTimeout(expiryTimer);
    expiryTimer = window.setTimeout(() => {
      expiryTimer = null;
      if (!streaming) resetSession();
    }, SESSION_TTL_MS);
  }

  function expireIfStale() {
    if (streaming) return;
    if (hiddenAt > 0 && Date.now() - hiddenAt >= SESSION_TTL_MS) {
      resetSession();
    }
    hiddenAt = 0;
  }

  function beginFreshDraft() {
    cursor = -1;
    session = [];
    sessionChatId = null;
    browseTurns = [];
    attachments = [];
    error = "";
    copied = false;
    live = { query: "", session: [], error: "", chatId: null, attachments: [] };
  }

  function adoptBrowsedChat() {
    if (cursor >= 0 && entry) {
      session = browseTurns.slice();
      sessionChatId = entry.id;
      browseTurns = [];
      cursor = -1;
    }
  }

  async function showEntry(index: number) {
    const item = activeHistory[index];
    if (!item) return;
    if (cursor < 0) live = snapshotLive();
    cursor = index;
    query = "";
    attachments = [];
    error = "";
    copied = false;
    try {
      const detail = await ipc.getChat(item.id);
      browseTurns = detail.messages.map((message) => ({
        role: message.role,
        content: message.content,
        createdAt: message.createdAt,
      }));
    } catch {
      browseTurns = [
        { role: "user", content: item.prompt, createdAt: item.createdAt },
        { role: "assistant", content: item.answer, createdAt: item.createdAt },
      ];
    }
  }

  /// delta +1 walks into the past, -1 back towards the live draft / fresh chat.
  function stepHistory(delta: number) {
    if (!canBrowse) return;
    
    // Scrolling down while already at live session -> clear and start fresh chat
    if (delta < 0 && cursor === -1) {
      if (session.length > 0) {
        beginFreshDraft();
        void tick().then(() => {
          autoSize();
          caretIndex = 0;
          placeCaret();
          void refreshHeight();
        });
      }
      return;
    }

    const next = cursor + delta;
    if (next < -1 || next >= activeHistory.length) return;
    if (next < 0) showLive();
    else void showEntry(next);
    void tick().then(() => {
      autoSize();
      caretIndex = query.length;
      placeCaret();
      void refreshHeight();
    });
  }

  let wheelAcc = 0;
  let wheelAt = 0;
  const WHEEL_STEP = 60;
  const WHEEL_RESET_MS = 320;

  function onWheel(event: WheelEvent) {
    if (!canBrowse) return;
    event.preventDefault();
    const now = performance.now();
    if (now - wheelAt > WHEEL_RESET_MS) wheelAcc = 0;
    wheelAt = now;
    wheelAcc += event.deltaY;
    while (wheelAcc <= -WHEEL_STEP) {
      wheelAcc += WHEEL_STEP;
      stepHistory(1);
    }
    while (wheelAcc >= WHEEL_STEP) {
      wheelAcc -= WHEEL_STEP;
      stepHistory(-1);
    }
  }

  /// Over the answer sheet the wheel always scrolls the text itself —
  /// history browsing stays off here to avoid conflicts.
  function onSheetWheel(event: WheelEvent) {
    event.preventDefault();
    const el = event.currentTarget as HTMLElement;
    el.scrollTop += event.deltaY;
  }

  function insertText(text: string) {
    if (cursor >= 0) adoptBrowsedChat();
    const el = inputEl;
    const start = el?.selectionStart ?? query.length;
    const end = el?.selectionEnd ?? query.length;
    query = query.slice(0, start) + text + query.slice(end);
    const next = start + text.length;
    void tick().then(() => {
      el?.setSelectionRange(next, next);
      caretIndex = next;
      placeCaret();
      autoSize();
    });
  }

  function onInput() {
    if (cursor >= 0) adoptBrowsedChat();
    syncCaret();
  }

  const timeOnlyFormat = $derived(
    new Intl.DateTimeFormat(localeTag(), {
      hour: "2-digit",
      minute: "2-digit",
    })
  );

  function isSameCalendarDay(d1: Date, d2: Date): boolean {
    return (
      d1.getFullYear() === d2.getFullYear() &&
      d1.getMonth() === d2.getMonth() &&
      d1.getDate() === d2.getDate()
    );
  }

  function shouldShowDivider(currTurn: SessionTurn, prevTurn?: SessionTurn): boolean {
    if (!currTurn.createdAt || !prevTurn?.createdAt) return false;
    const prevDate = new Date(prevTurn.createdAt * 1000);
    const currDate = new Date(currTurn.createdAt * 1000);
    const diffSec = currTurn.createdAt - prevTurn.createdAt;
    return !isSameCalendarDay(prevDate, currDate) || diffSec >= 900;
  }

  function formatDivider(ts?: number): string {
    if (!ts) return "";
    const date = new Date(ts * 1000);
    const now = new Date();
    if (isSameCalendarDay(date, now)) {
      return `${t("today")} · ${timeOnlyFormat.format(date)}`;
    }
    const yesterday = new Date(now);
    yesterday.setDate(yesterday.getDate() - 1);
    if (isSameCalendarDay(date, yesterday)) {
      return `${t("yesterday")} · ${timeOnlyFormat.format(date)}`;
    }
    return stamp.format(date);
  }

  function readFileAsDataUrl(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = reject;
      reader.readAsDataURL(file);
    });
  }

  function readFileAsText(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = reject;
      reader.readAsText(file);
    });
  }

  function isCodeFile(name: string): boolean {
    const ext = name.split(".").pop()?.toLowerCase() ?? "";
    return [
      "js", "ts", "jsx", "tsx", "py", "rs", "go", "c", "cpp", "h", "hpp",
      "cs", "java", "kt", "rb", "php", "html", "css", "scss", "json",
      "yaml", "yml", "toml", "sql", "sh", "bash", "ps1", "svelte", "vue"
    ].includes(ext);
  }

  async function addFile(file: File) {
    if (file.type.startsWith("image/")) {
      try {
        const dataUrl = await readFileAsDataUrl(file);
        const name = file.name && file.name !== "image.png" ? file.name : `Bild ${attachments.filter((a) => a.kind === "image").length + 1}.png`;
        attachments = [
          ...attachments,
          {
            id: crypto.randomUUID(),
            name,
            kind: "image",
            mimeType: file.type || "image/png",
            dataUrl,
            size: file.size,
          },
        ];
      } catch (e) {
        console.error("Fehler beim Laden des Bildes", e);
      }
    } else {
      try {
        const textContent = await readFileAsText(file);
        attachments = [
          ...attachments,
          {
            id: crypto.randomUUID(),
            name: file.name || "Dokument.txt",
            kind: "text",
            mimeType: file.type || "text/plain",
            textContent,
            size: file.size,
          },
        ];
      } catch (e) {
        console.error("Fehler beim Laden der Datei", e);
      }
    }
  }

  function removeAttachment(id: string) {
    attachments = attachments.filter((a) => a.id !== id);
    void tick().then(() => {
      autoSize();
      void refreshHeight();
    });
  }

  async function onPaste(event: ClipboardEvent) {
    const items = event.clipboardData?.items;
    if (!items || items.length === 0) return;

    const fileList: File[] = [];
    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      if (item.kind === "file") {
        const file = item.getAsFile();
        if (file) {
          fileList.push(file);
        }
      }
    }

    if (fileList.length > 0) {
      event.preventDefault();
      if (cursor >= 0) adoptBrowsedChat();
      for (const file of fileList) {
        await addFile(file);
      }
      void tick().then(() => {
        autoSize();
        inputEl?.focus();
        void refreshHeight();
      });
    }
  }

  async function ask(detailed: boolean) {
    if (cursor >= 0) adoptBrowsedChat();
    let prompt = query.trim();
    const currentAttachments = attachments.slice();
    if (!prompt && currentAttachments.length === 0) {
      prompt = selection?.trim() || clipboard?.trim() || "";
    }
    if ((!prompt && currentAttachments.length === 0) || !activeProvider || streaming) return;
    cursor = -1;
    const prior = session
      .filter((turn) => turn.content.trim().length > 0 || (turn.attachments && turn.attachments.length > 0))
      .map((turn) => ({ role: turn.role, content: turn.content }));
    const first = prior.every((turn) => turn.role !== "user");
    const nowSec = Math.floor(Date.now() / 1000);
    session = [
      ...session,
      { role: "user", content: prompt, createdAt: nowSec, attachments: currentAttachments },
      { role: "assistant", content: "", createdAt: nowSec },
    ];
    query = "";
    attachments = [];
    error = "";
    copied = false;
    streaming = true;
    requestId = crypto.randomUUID();
    caretIndex = 0;
    void tick().then(() => {
      autoSize();
      inputEl?.focus();
      placeCaret();
      void refreshHeight();
    });
    try {
      await ipc.startChat({
        requestId,
        providerId: activeProvider.id,
        model: activeProvider.model,
        prompt,
        selection: first ? selection : null,
        clipboard: first && settings?.clipboardContext ? clipboard : null,
        detailed,
        prior,
        chatId: sessionChatId,
        attachments: currentAttachments,
      });
    } catch (err) {
      streaming = false;
      error = String(err);
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      if (browsing) {
        showLive();
      } else if (session.length > 0 && !query.trim()) {
        beginFreshDraft();
        void tick().then(() => {
          autoSize();
          caretIndex = 0;
          placeCaret();
          void refreshHeight();
        });
      } else {
        void ipc.hideOverlay();
      }
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "n") {
      event.preventDefault();
      beginFreshDraft();
      void tick().then(() => {
        autoSize();
        caretIndex = 0;
        placeCaret();
        void refreshHeight();
      });
      return;
    }
    if (event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault();
      stepHistory(event.key === "ArrowUp" ? 1 : -1);
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key === ",") {
      event.preventDefault();
      void ipc.openSettings();
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "c" && lastAssistant && !streaming) {
      if (!inputEl || inputEl.selectionStart === inputEl.selectionEnd) {
        event.preventDefault();
        void copyAnswer();
        return;
      }
    }
    if (event.key === "Enter") {
      event.preventDefault();
      void ask(event.shiftKey);
    }
  }

  /// Alt still held after Alt+Space → next letter is a menu mnemonic (beep, no char).
  function onCaptureKey(event: KeyboardEvent) {
    if (!document.hasFocus()) return;
    if (!event.altKey || event.ctrlKey || event.metaKey) return;
    if (event.code === "Space" || event.key === " ") {
      event.preventDefault();
      return;
    }
    if (event.key.length === 1) {
      event.preventDefault();
      event.stopPropagation();
      if (!event.repeat) {
        inputEl?.focus();
        insertText(event.key);
      }
    }
  }

  async function copyAnswer() {
    if (!lastAssistant) return;
    await ipc.copyText(lastAssistant);
    copied = true;
    setTimeout(() => {
      copied = false;
    }, 1000);
  }

  async function handleStopTts() {
    try {
      await ipc.stopTts();
    } catch {}
    ttsState = { synthesizing: false, playing: false, busy: false };
  }

  onMount(() => {
    const unsubs: Array<() => void> = [];
    void loadBase();
    document.addEventListener("keydown", onCaptureKey, true);

    void ipc.getTtsState().then((state) => {
      if (state) ttsState = state;
    }).catch(() => {});

    void listen<TtsLiveState>("tts-state", (event) => {
      if (event.payload) {
        ttsState = event.payload;
      }
    }).then((unlisten) => unsubs.push(unlisten));

    void listen<boolean>("tts-busy", (event) => {
      ttsState.busy = event.payload;
      if (!event.payload) {
        ttsState.synthesizing = false;
        ttsState.playing = false;
      }
    }).then((unlisten) => unsubs.push(unlisten));

    void listen<string>("tts-error", (event) => {
      if (event.payload) {
        error = event.payload;
        void refreshHeight();
      }
    }).then((unlisten) => unsubs.push(unlisten));

    void listen<string>("hotkey-error", (event) => {
      if (event.payload) {
        error = event.payload;
        void refreshHeight();
      }
    }).then((unlisten) => unsubs.push(unlisten));

    void listen("overlay-hidden", () => {
      hiddenAt = Date.now();
      scheduleExpiry();
    }).then((unlisten) => unsubs.push(unlisten));

    void listen<AppSettings>("settings-changed", (event) => {
      applyLanguage(event.payload?.language);
      void loadBase();
    }).then((unlisten) => unsubs.push(unlisten));

    void listen("providers-changed", () => {
      void loadBase();
    }).then((unlisten) => unsubs.push(unlisten));

    void listen<ContextPayload>("overlay-ready", async (event) => {
      selection = event.payload.selection;
      clipboard = event.payload.clipboard;
      copied = false;
      expireIfStale();
      await loadBase();
      await tick();
      autoSize();
      inputEl?.focus();
      if (query) {
        inputEl?.select();
        caretIndex = query.length;
      } else {
        caretIndex = 0;
      }
      syncCaret();
      void refreshHeight();
    }).then((unlisten) => unsubs.push(unlisten));

    void listen<{ requestId: string; text: string }>("chat-chunk", (event) => {
      if (event.payload.requestId !== requestId) return;
      const next = session.slice();
      const last = next[next.length - 1];
      if (!last || last.role !== "assistant") return;
      next[next.length - 1] = { ...last, content: last.content + event.payload.text };
      session = next;
    }).then((unlisten) => unsubs.push(unlisten));

    void listen<{ requestId: string; chatId?: string | null }>("chat-done", (event) => {
      if (event.payload.requestId !== requestId) return;
      streaming = false;
      if (event.payload.chatId) sessionChatId = event.payload.chatId;
      void loadHistory();
    }).then((unlisten) => unsubs.push(unlisten));

    void listen<{ requestId: string; message: string }>("chat-error", (event) => {
      if (event.payload.requestId !== requestId) return;
      streaming = false;
      error = event.payload.message;
      session = session.filter((turn) => turn.role !== "assistant" || turn.content.trim().length > 0);
    }).then((unlisten) => unsubs.push(unlisten));

    const onSelection = () => {
      if (focused) syncCaret();
    };
    document.addEventListener("selectionchange", onSelection);

    return () => {
      if (expiryTimer !== null) window.clearTimeout(expiryTimer);
      document.removeEventListener("keydown", onCaptureKey, true);
      document.removeEventListener("selectionchange", onSelection);
      unsubs.forEach((fn) => fn());
    };
  });

  $effect(() => {
    query;
    attachments;
    void tick().then(autoSize);
  });

  $effect(() => {
    query;
    caretIndex;
    void tick().then(placeCaret);
  });

  $effect(() => {
    session;
    streaming;
    if (browsing || !sheetEl) return;
    sheetEl.scrollTop = sheetEl.scrollHeight;
  });

  $effect(() => {
    if (!root) return;
    const observer = new ResizeObserver(() => void refreshHeight());
    observer.observe(root);
    return () => observer.disconnect();
  });
</script>

<div
  class="stage"
  bind:this={root}
  role="region"
  aria-label="Dumbo Chat"
  ondragover={(e) => { e.preventDefault(); }}
  ondrop={async (e) => {
    e.preventDefault();
    if (e.dataTransfer?.files && e.dataTransfer.files.length > 0) {
      if (cursor >= 0) adoptBrowsedChat();
      for (let i = 0; i < e.dataTransfer.files.length; i++) {
        await addFile(e.dataTransfer.files[i]);
      }
      void tick().then(() => {
        autoSize();
        inputEl?.focus();
        void refreshHeight();
      });
    }
  }}
>
  <div class="pill" class:has-attachments={attachments.length > 0} onwheel={onWheel}>
    {#if attachments.length > 0}
      <div class="attachments-row">
        {#each attachments as att (att.id)}
          <div class="chip" title={att.name}>
            {#if att.kind === "image"}
              {#if att.dataUrl}
                <img class="chip-img" src={att.dataUrl} alt="" />
              {:else}
                <svg class="chip-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor">
                  <rect x="2" y="2" width="12" height="12" rx="2.5" stroke-width="1.3"/>
                  <circle cx="5.5" cy="5.5" r="1.2" fill="currentColor"/>
                  <path d="M14 11l-3.5-3.5-5 5" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              {/if}
            {:else if isCodeFile(att.name)}
              <svg class="chip-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor">
                <path d="M5.5 4.5L2 8l3.5 3.5M10.5 4.5L14 8l-3.5 3.5" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            {:else}
              <svg class="chip-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor">
                <path d="M3.5 2.5h6l3.5 3.5v7.5a1 1 0 0 1-1 1h-8.5a1 1 0 0 1-1-1v-10a1 1 0 0 1 1-1z" stroke-width="1.3"/>
                <path d="M9.5 2.5v3.5h3.5" stroke-width="1.3"/>
                <path d="M5.5 8h5M5.5 11h3.5" stroke-width="1.3" stroke-linecap="round"/>
              </svg>
            {/if}
            <span class="chip-name">{att.name}</span>
            <button
              type="button"
              class="chip-remove"
              onclick={(e) => { e.stopPropagation(); removeAttachment(att.id); }}
              aria-label={t("delete")}
            >
              <svg viewBox="0 0 10 10" fill="none" stroke="currentColor">
                <path d="M2 2l6 6M8 2l-6 6" stroke-width="1.3" stroke-linecap="round"/>
              </svg>
            </button>
          </div>
        {/each}
      </div>
    {/if}

    <div class="field">
      <textarea
        bind:this={inputEl}
        bind:value={query}
        placeholder={selection ? t("askSelection") : (clipboard ? t("askClipboard") : (attachments.length > 0 ? t("askAttachments") : t("askAnything")))}
        onkeydown={onKeydown}
        onkeyup={syncCaret}
        onclick={syncCaret}
        oninput={onInput}
        onpaste={onPaste}
        onselect={syncCaret}
        onfocus={() => {
          focused = true;
          syncCaret();
        }}
        onblur={() => (focused = false)}
        rows="1"
        autocomplete="off"
        spellcheck="false"
        aria-label={t("askAnything")}
      ></textarea>
      <pre class="mirror" aria-hidden="true">{before}<span class="mark" bind:this={markEl}>{"\u200b"}</span>{after}</pre>
      {#if focused && !hasSelection}
        <span class="caret" style="left: {caretX}px; top: {caretY}px; height: {caretHeight}px"></span>
      {/if}
    </div>
    {#if browsing}
      <span class="badge">{cursor + 1}/{activeHistory.length}</span>
    {/if}
    {#if ttsState.busy}
      <button
        type="button"
        class="tts-indicator"
        class:is-playing={ttsState.playing}
        onclick={handleStopTts}
        title={ttsState.playing ? t("stopTts") : t("loadingTts")}
        aria-label={t("stopTts")}
      >
        {#if ttsState.playing}
          <span class="soundwaves" aria-hidden="true">
            <span class="wave-bar wb-1"></span>
            <span class="wave-bar wb-2"></span>
            <span class="wave-bar wb-3"></span>
            <span class="wave-bar wb-4"></span>
          </span>
        {:else}
          <span class="tts-dots" aria-hidden="true">
            <span class="tdot td-1"></span>
            <span class="tdot td-2"></span>
            <span class="tdot td-3"></span>
          </span>
        {/if}
        <span class="tts-cancel" aria-hidden="true">
          <svg width="6" height="6" viewBox="0 0 8 8" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <path d="M1 1l6 6M7 1L1 7" />
          </svg>
        </span>
      </button>
    {/if}
  </div>

  {#if expanded}
    <section class="sheet" bind:this={sheetEl} onwheel={onSheetWheel}>
      {#if browsing}
        {#if browseTurns.length > 0 && browseTurns[0]?.createdAt}
          <p class="meta">{t("historyMeta")} · {stamp.format(new Date(browseTurns[0].createdAt * 1000))}</p>
        {:else if entry}
          <p class="meta">{t("historyMeta")} · {stamp.format(new Date(entry.createdAt * 1000))}</p>
        {/if}
      {/if}
      {#each turns as turn, index (index)}
        {#if index > 0 && shouldShowDivider(turn, turns[index - 1])}
          <div class="time-divider"><span>· {formatDivider(turn.createdAt)}</span></div>
        {/if}
        {#if turn.role === "user"}
          <div class="turn user">
            {#if turn.attachments && turn.attachments.length > 0}
              <div class="turn-attachments">
                {#each turn.attachments as att (att.id)}
                  <span class="chip chip-static" title={att.name}>
                    {#if att.kind === "image"}
                      {#if att.dataUrl}
                        <img class="chip-img" src={att.dataUrl} alt="" />
                      {:else}
                        <svg class="chip-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor">
                          <rect x="2" y="2" width="12" height="12" rx="2.5" stroke-width="1.3"/>
                          <circle cx="5.5" cy="5.5" r="1.2" fill="currentColor"/>
                          <path d="M14 11l-3.5-3.5-5 5" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                        </svg>
                      {/if}
                    {:else if isCodeFile(att.name)}
                      <svg class="chip-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor">
                        <path d="M5.5 4.5L2 8l3.5 3.5M10.5 4.5L14 8l-3.5 3.5" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>
                      </svg>
                    {:else}
                      <svg class="chip-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor">
                        <path d="M3.5 2.5h6l3.5 3.5v7.5a1 1 0 0 1-1 1h-8.5a1 1 0 0 1-1-1v-10a1 1 0 0 1 1-1z" stroke-width="1.3"/>
                        <path d="M9.5 2.5v3.5h3.5" stroke-width="1.3"/>
                        <path d="M5.5 8h5M5.5 11h3.5" stroke-width="1.3" stroke-linecap="round"/>
                      </svg>
                    {/if}
                    <span class="chip-name">{att.name}</span>
                  </span>
                {/each}
              </div>
            {/if}
            {#if turn.content}
              <div class="turn-text">{turn.content}</div>
            {/if}
          </div>
        {:else if streaming && index === turns.length - 1 && !turn.content.trim()}
          <div class="turn loading-turn">
            <div class="thinking-dots">
              <span class="dot"></span>
              <span class="dot"></span>
              <span class="dot"></span>
            </div>
          </div>
        {:else if turn.content}
          <div class="turn">
            <MarkdownView html={renderMarkdown(turn.content)} />
          </div>
        {/if}
      {/each}
      {#if error}
        <p class="err">{error}</p>
      {/if}
      {#if lastAssistant && !streaming}
        <button class="copy" class:done={copied} type="button" onclick={copyAnswer} aria-label={t("copyAnswer")}>
          <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <rect x="5.5" y="5.5" width="8" height="8" rx="1.5" stroke="currentColor" />
            <path d="M3 10.5V3.8c0-.7.6-1.3 1.3-1.3H10" stroke="currentColor" />
          </svg>
        </button>
      {/if}
    </section>
  {/if}
</div>

<style>
  /* Padding must exceed the largest shadow extent so the blur fades out
     inside the transparent window instead of being clipped rectangular. */
  .stage {
    padding: 22px 30px 38px;
    background: transparent;
  }

  .pill {
    display: flex;
    align-items: center;
    min-height: 48px;
    padding: 10px 22px;
    background: #202020;
    border-radius: 999px;
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.06),
      0 0 18px rgba(255, 255, 255, 0.06),
      0 2px 6px rgba(0, 0, 0, 0.35),
      0 10px 26px rgba(0, 0, 0, 0.45);
    transition: border-radius 0.15s ease, padding 0.15s ease;
  }

  .pill.has-attachments {
    flex-direction: column;
    align-items: stretch;
    border-radius: 20px;
    padding: 10px 18px 12px;
    gap: 8px;
  }

  .attachments-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    padding-bottom: 2px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px 3px 8px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.09);
    font-size: 0.76rem;
    color: rgba(255, 255, 255, 0.9);
    max-width: 220px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
    user-select: none;
    transition: background 0.15s ease, border-color 0.15s ease;
  }

  .chip:hover {
    background: rgba(255, 255, 255, 0.12);
    border-color: rgba(255, 255, 255, 0.16);
  }

  .chip-static {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.07);
    padding: 2px 8px;
  }

  .chip-static:hover {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.07);
  }

  .chip-img {
    width: 16px;
    height: 16px;
    border-radius: 999px;
    object-fit: cover;
    flex-shrink: 0;
  }

  .chip-icon {
    width: 13px;
    height: 13px;
    color: rgba(255, 255, 255, 0.7);
    flex-shrink: 0;
  }

  .chip-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
    letter-spacing: -0.01em;
  }

  .chip-remove {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border: 0;
    background: transparent;
    color: rgba(255, 255, 255, 0.45);
    border-radius: 999px;
    cursor: pointer;
    padding: 0;
    margin-left: 1px;
    flex-shrink: 0;
    transition: all 0.12s ease;
  }

  .chip-remove:hover {
    background: rgba(255, 255, 255, 0.2);
    color: #fff;
  }

  .chip-remove svg {
    width: 8px;
    height: 8px;
  }

  .turn-attachments {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 6px;
  }

  .turn-text {
    white-space: pre-wrap;
    word-break: break-word;
  }

  .field {
    position: relative;
    width: 100%;
  }

  .mirror,
  textarea {
    margin: 0;
    width: 100%;
    font: inherit;
    font-size: 1.12rem;
    line-height: 1.25;
    letter-spacing: -0.02em;
    white-space: pre-wrap;
    word-break: break-word;
    overflow-wrap: anywhere;
  }

  .mirror {
    position: absolute;
    inset: 0;
    visibility: hidden;
    pointer-events: none;
    overflow: hidden;
  }

  textarea {
    position: relative;
    display: block;
    min-height: 1.25em;
    max-height: 220px;
    resize: none;
    overflow: hidden;
    border: 0 !important;
    outline: none !important;
    box-shadow: none !important;
    background: transparent;
    color: var(--fg);
    caret-color: transparent;
    padding: 0;
    appearance: none;
  }

  .mark {
    display: inline;
  }

  textarea::selection {
    background: rgba(255, 255, 255, 0.22);
    color: var(--fg);
  }

  textarea::placeholder {
    color: var(--mute, #888);
    opacity: 0.55;
  }

  .caret {
    position: absolute;
    width: 2px;
    border-radius: 999px;
    background: #fff;
    pointer-events: none;
    animation: caret 1.15s ease-in-out infinite;
  }

  .sheet {
    position: relative;
    margin-top: 10px;
    max-height: 480px;
    overflow: auto;
    scrollbar-width: thin;
    scrollbar-color: #3f3f3f transparent;
    padding: 16px 20px 18px;
    background: #202020;
    border-radius: 22px;
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.06),
      0 0 18px rgba(255, 255, 255, 0.05),
      0 10px 26px rgba(0, 0, 0, 0.45);
  }

  .sheet::-webkit-scrollbar {
    width: 6px;
  }

  .sheet::-webkit-scrollbar-track {
    background: transparent;
  }

  .sheet::-webkit-scrollbar-thumb {
    background: #3f3f3f;
    border-radius: 0;
  }

  .sheet::-webkit-scrollbar-button {
    display: none;
    width: 0;
    height: 0;
  }

  .turn + .turn {
    margin-top: 12px;
  }

  .user {
    display: block;
    width: fit-content;
    max-width: 85%;
    margin: 0 0 0 auto;
    padding: 8px 14px;
    border-radius: 14px 14px 4px 14px;
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.78);
    font-size: 0.88rem;
    line-height: 1.4;
    word-break: break-word;
    text-align: left;
  }

  .err {
    margin: 10px 0 0;
    color: var(--danger);
    font-size: 0.92rem;
  }

  .badge {
    flex: 0 0 auto;
    margin-left: 12px;
    padding: 3px 9px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.62);
    font-size: 0.72rem;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
  }

  .tts-indicator {
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 24px;
    padding: 2px 8px;
    margin-left: 10px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.82);
    cursor: pointer;
    user-select: none;
    transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease;
  }

  .tts-indicator:hover {
    background: rgba(255, 255, 255, 0.13);
    border-color: rgba(255, 255, 255, 0.18);
    color: #fff;
  }

  .tts-dots {
    display: flex;
    align-items: center;
    gap: 3px;
    height: 10px;
  }

  .tdot {
    width: 3.5px;
    height: 3.5px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.7);
    animation: ttsPulse 1.2s infinite ease-in-out;
  }

  .td-1 { animation-delay: 0s; }
  .td-2 { animation-delay: 0.2s; }
  .td-3 { animation-delay: 0.4s; }

  @keyframes ttsPulse {
    0%, 100% { transform: scale(0.7); opacity: 0.35; }
    50% { transform: scale(1.15); opacity: 1; }
  }

  .soundwaves {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 12px;
  }

  .wave-bar {
    width: 2px;
    background: rgba(255, 255, 255, 0.9);
    border-radius: 1px;
    animation: waveJump 0.7s infinite ease-in-out alternate;
  }

  .wb-1 { height: 5px; animation-delay: 0.08s; }
  .wb-2 { height: 11px; animation-delay: 0.25s; }
  .wb-3 { height: 7px; animation-delay: 0.15s; }
  .wb-4 { height: 9px; animation-delay: 0.32s; }

  @keyframes waveJump {
    0% { height: 3px; opacity: 0.45; }
    100% { height: 12px; opacity: 1; }
  }

  .tts-cancel {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 10px;
    height: 10px;
    opacity: 0.4;
    transition: opacity 0.15s ease;
  }

  .tts-indicator:hover .tts-cancel {
    opacity: 0.95;
  }

  .meta {
    margin: 0 0 10px;
    color: rgba(255, 255, 255, 0.42);
    font-size: 0.72rem;
    letter-spacing: 0.02em;
  }

  .time-divider {
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 14px 0 10px;
    color: rgba(255, 255, 255, 0.42);
    font-size: 0.72rem;
    letter-spacing: 0.02em;
    user-select: none;
  }

  .loading-turn {
    padding: 8px 0;
  }

  .thinking-dots {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 999px;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.85);
    animation: bounce 1.4s ease-in-out infinite both;
  }

  .dot:nth-child(1) {
    animation-delay: -0.32s;
  }

  .dot:nth-child(2) {
    animation-delay: -0.16s;
  }

  .dot:nth-child(3) {
    animation-delay: 0s;
  }

  @keyframes bounce {
    0%, 80%, 100% {
      transform: scale(0.6);
      opacity: 0.3;
    }
    40% {
      transform: scale(1.1);
      opacity: 1;
    }
  }

  .copy {
    position: sticky;
    bottom: 0;
    float: right;
    margin-top: 10px;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 999px;
    background: #111;
    color: var(--fg);
    display: grid;
    place-items: center;
  }

  .copy:hover,
  .copy.done {
    background: #2a2a2a;
  }

  @keyframes caret {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.12;
    }
  }
</style>
