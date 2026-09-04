<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { ipc } from "$lib/ipc";
  import { renderMarkdown } from "$lib/markdown";
  import MarkdownView from "$lib/MarkdownView.svelte";
  import Switch from "$lib/Switch.svelte";
  import Select from "$lib/Select.svelte";
  import HotkeyInput from "$lib/HotkeyInput.svelte";
  import type { AppSettings, ChatDetail, ChatSummary, ProviderView } from "$lib/types";
  import { t, applyLanguage, localeTag } from "$lib/i18n.svelte";

  let settings = $state<AppSettings | null>(null);
  let providers = $state<ProviderView[]>([]);
  let chats = $state<ChatSummary[]>([]);
  let selectedChat = $state<ChatDetail | null>(null);
  let status = $state("");
  let statusTimer: any;

  function showStatus(msg: string) {
    status = msg;
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => {
      status = "";
    }, 2000);
  }

  let keyDrafts = $state<Record<string, string>>({});
  let ttsKeyDrafts = $state<Record<string, string>>({
    azure: "",
    elevenlabs: "",
    gemini: "",
    openai: "",
  });
  let ttsKeyStatus = $state<Record<string, boolean>>({});
  let ttsTesting = $state(false);
  let ttsBusy = $state(false);
  let ttsError = $state("");
  let customStatus = $state<import("$lib/types").LocalTtsStatus | null>(null);


  const openaiVoices = [
    { value: "alloy", label: "Alloy" },
    { value: "echo", label: "Echo" },
    { value: "fable", label: "Fable" },
    { value: "onyx", label: "Onyx" },
    { value: "nova", label: "Nova" },
    { value: "shimmer", label: "Shimmer" },
  ];

  const geminiVoices = [
    { value: "Puck", label: "Puck" },
    { value: "Charon", label: "Charon" },
    { value: "Kore", label: "Kore" },
    { value: "Fenrir", label: "Fenrir" },
    { value: "Aoede", label: "Aoede" },
  ];

  type VoiceMeta = {
    value: string;
    name: string;
    lang: "voiceLangDe" | "voiceLangEn";
    gender: "voiceMale" | "voiceFemale";
    multilingual?: boolean;
  };

  const elevenVoiceSpecs: VoiceMeta[] = [
    { value: "PhufIH7nYh2Up1uej6aY", name: "Moritz", lang: "voiceLangDe", gender: "voiceMale" },
    { value: "21m00Tcm4TlvDq8ikWAM", name: "Rachel", lang: "voiceLangEn", gender: "voiceFemale" },
    { value: "AZnzlk1XvdvUeBnXmlld", name: "Domi", lang: "voiceLangEn", gender: "voiceFemale" },
    { value: "EXAVITQu4vr4xnSDxMaL", name: "Sarah", lang: "voiceLangEn", gender: "voiceFemale" },
    { value: "ErXwobaYiN019PkySvjV", name: "Antoni", lang: "voiceLangEn", gender: "voiceMale" },
    { value: "MF3mGyEYCl7XYWbV9V6O", name: "Elli", lang: "voiceLangEn", gender: "voiceFemale" },
    { value: "TxGEqnHWrfWFTfGW9XjX", name: "Josh", lang: "voiceLangEn", gender: "voiceMale" },
    { value: "VR6AewLTigWG4xSOukaG", name: "Arnold", lang: "voiceLangEn", gender: "voiceMale" },
    { value: "pNInz6obpgDQGcFmaJgB", name: "Adam", lang: "voiceLangEn", gender: "voiceMale" },
    { value: "yoZ06aMxZJJ28mfd3POQ", name: "Sam", lang: "voiceLangEn", gender: "voiceMale" },
  ];

  const azureVoiceSpecs: VoiceMeta[] = [
    { value: "de-DE-ConradNeural", name: "Conrad", lang: "voiceLangDe", gender: "voiceMale" },
    { value: "de-DE-KatjaNeural", name: "Katja", lang: "voiceLangDe", gender: "voiceFemale" },
    { value: "de-DE-FlorianMultilingualNeural", name: "Florian", lang: "voiceLangDe", gender: "voiceMale", multilingual: true },
    { value: "de-DE-SeraphinaMultilingualNeural", name: "Seraphina", lang: "voiceLangDe", gender: "voiceFemale", multilingual: true },
    { value: "de-DE-AmalaNeural", name: "Amala", lang: "voiceLangDe", gender: "voiceFemale" },
    { value: "de-DE-KillianNeural", name: "Killian", lang: "voiceLangDe", gender: "voiceMale" },
    { value: "en-US-JennyNeural", name: "Jenny", lang: "voiceLangEn", gender: "voiceFemale" },
    { value: "en-US-GuyNeural", name: "Guy", lang: "voiceLangEn", gender: "voiceMale" },
  ];

  function formatVoice(v: VoiceMeta): { value: string; label: string } {
    const detail = v.multilingual ? t("voiceMultilingual") : `${t(v.lang)} · ${t(v.gender)}`;
    return { value: v.value, label: `${v.name} (${detail})` };
  }

  let elevenVoices = $derived(elevenVoiceSpecs.map(formatVoice));
  let azureVoices = $derived(azureVoiceSpecs.map(formatVoice));

  let selectedChatProvider = $derived(
    providers.find((p) => p.id === settings?.activeProviderId) ?? providers[0] ?? null
  );

  async function checkTtsKey(provider?: string) {
    const list = provider ? [provider] : ["azure", "elevenlabs", "gemini", "openai"];
    for (const p of list) {
      try {
        ttsKeyStatus[p] = await ipc.getTtsKeyStatus(p);
      } catch {
        ttsKeyStatus[p] = false;
      }
    }
  }

  async function saveTtsKey(provider?: string, valueToSave?: string) {
    const targetProvider = provider ?? settings?.ttsProvider;
    if (!targetProvider) return;
    const toSave = valueToSave !== undefined ? valueToSave.trim() : (ttsKeyDrafts[targetProvider] ?? "").trim();
    await ipc.setTtsKey(targetProvider, toSave);
    ttsKeyDrafts[targetProvider] = "";
    await checkTtsKey(targetProvider);
    showStatus(t("saved"));
  }

  async function testTtsVoice() {
    if (!settings) return;
    const currentProvider = settings.ttsProvider.toLowerCase();
    if ((ttsKeyDrafts[currentProvider] ?? "").trim()) {
      await saveTtsKey(currentProvider);
    }
    ttsTesting = true;
    ttsError = "";

    const provider = settings.ttsProvider.toLowerCase();
    let voice = "";
    let model = "";
    let azureRegion = "";

    if (provider === "custom") {
      voice = settings.ttsCustomVoice || "af_bella";
      model = settings.ttsCustomModel || "kokoro";
      azureRegion = settings.ttsCustomUrl || "http://127.0.0.1:8880";
    } else if (provider === "openai") {
      voice = settings.ttsOpenaiVoice || "alloy";
      model = settings.ttsOpenaiModel || "tts-1";
    } else if (provider === "elevenlabs") {
      voice = settings.ttsElevenVoice || "PhufIH7nYh2Up1uej6aY";
      model = settings.ttsElevenModel || "eleven_multilingual_v2";
    } else if (provider === "azure") {
      voice = settings.ttsAzureVoice || "de-DE-ConradNeural";
      azureRegion = settings.ttsAzureRegionSetting || settings.ttsAzureRegion || "westeurope";
    } else {
      voice = settings.ttsGeminiVoice || "Puck";
      model = settings.ttsGeminiModel || "gemini-2.0-flash";
    }

    try {
      await ipc.testTts({
        provider,
        voice,
        model,
        azureRegion,
        text: t("ttsSampleText"),
      });
    } catch (err: any) {
      ttsError = typeof err === "string" ? err : err?.message || t("ttsErrorDefault");
    } finally {
      ttsTesting = false;
    }
  }

  async function stopTtsVoice() {
    await ipc.stopTts();
    ttsTesting = false;
  }

  async function refreshCustomStatus() {
    if (settings?.ttsProvider === "custom") {
      try {
        customStatus = await ipc.getLocalTtsStatus(settings.ttsCustomUrl);
      } catch {
        customStatus = null;
      }
    }
  }

  async function load() {
    const next = await ipc.getSettings();
    applyLanguage(next?.language);
    settings = next;
    providers = await ipc.listProviders();
    await checkTtsKey();
    await refreshCustomStatus();
    if (settings.historyEnabled) {
      chats = await ipc.listChats();
    } else {
      chats = [];
      selectedChat = null;
    }
  }

  async function persistSettings() {
    if (!settings) return;
    applyLanguage(settings.language);
    settings = await ipc.saveSettings(settings);
    applyLanguage(settings.language);
    showStatus(t("saved"));
    if (settings.historyEnabled) {
      chats = await ipc.listChats();
    } else {
      chats = [];
      selectedChat = null;
    }
  }

  async function saveProvider(provider: ProviderView) {
    await ipc.upsertProvider({
      id: provider.id,
      name: provider.name,
      kind: provider.kind,
      baseUrl: provider.baseUrl,
      model: provider.model,
    });
    const draft = keyDrafts[provider.id]?.trim();
    if (draft !== undefined) {
      await ipc.setProviderKey(provider.id, draft);
      keyDrafts[provider.id] = "";
    }
    providers = await ipc.listProviders();
    showStatus(t("saved"));
  }

  async function addCustom() {
    const created = await ipc.upsertProvider({
      id: "",
      name: "Custom",
      kind: "openai",
      baseUrl: "https://api.openai.com/v1",
      model: "",
    });
    providers = await ipc.listProviders();
    if (settings) {
      settings.activeProviderId = created.id;
      await persistSettings();
    }
  }

  async function addPreset(preset: "openrouter" | "gemini" | "openai" | "ollama") {
    let spec = {
      id: "",
      name: "OpenRouter",
      kind: "openai",
      baseUrl: "https://openrouter.ai/api/v1",
      model: "anthropic/claude-3.7-sonnet",
    };

    if (preset === "gemini") {
      spec = {
        id: "",
        name: "Google Gemini",
        kind: "gemini",
        baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai",
        model: "gemini-3.5-flash-lite",
      };
    } else if (preset === "openai") {
      spec = {
        id: "",
        name: "OpenAI",
        kind: "openai",
        baseUrl: "https://api.openai.com/v1",
        model: "gpt-4o",
      };
    } else if (preset === "ollama") {
      spec = {
        id: "",
        name: "Ollama",
        kind: "ollama",
        baseUrl: "http://localhost:11434/v1",
        model: "llama3.2",
      };
    }

    const created = await ipc.upsertProvider(spec);
    providers = await ipc.listProviders();
    if (settings) {
      settings.activeProviderId = created.id;
      await persistSettings();
    }
    showStatus(`${spec.name} ${t("providerAdded")}`);
  }

  async function removeProvider(id: string) {
    await ipc.deleteProvider(id);
    providers = await ipc.listProviders();
    if (settings && settings.activeProviderId === id) {
      if (providers.length > 0) {
        settings.activeProviderId = providers[0].id;
        await persistSettings();
      }
    }
    showStatus(t("removed"));
  }

  const dateFormat = $derived(
    new Intl.DateTimeFormat(localeTag(), {
      day: "2-digit",
      month: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    })
  );

  function stamp(seconds: number) {
    return dateFormat.format(new Date(seconds * 1000));
  }

  let copiedHistoryText = $state(false);

  async function copyHistoryText(text: string) {
    if (!text) return;
    await ipc.copyText(text);
    copiedHistoryText = true;
    setTimeout(() => {
      copiedHistoryText = false;
    }, 1200);
  }

  async function openChat(id: string) {
    copiedHistoryText = false;
    selectedChat = selectedChat?.chat.id === id ? null : await ipc.getChat(id);
  }

  async function dropChat(id: string) {
    await ipc.deleteChat(id);
    if (selectedChat?.chat.id === id) selectedChat = null;
    chats = await ipc.listChats();
  }

  async function wipeHistory() {
    await ipc.clearHistory();
    chats = [];
    selectedChat = null;
  }

  onMount(() => {
    void load();
    void listen<boolean>("tts-busy", (event) => {
      ttsBusy = event.payload;
    });
    void listen<string>("tts-error", (event) => {
      if (event.payload) {
        ttsError = event.payload;
      }
    });
    void listen<string>("hotkey-error", (event) => {
      if (event.payload) {
        ttsError = event.payload;
      }
    });
    void listen("settings-opened", () => {
      void load();
    });
    const handleFocus = () => {
      void load();
    };
    window.addEventListener("focus", handleFocus);
    return () => {
      window.removeEventListener("focus", handleFocus);
    };
  });

  $effect(() => {
    const title = t("settingsTitle");
    void import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) => getCurrentWindow().setTitle(title))
      .catch(() => {});
  });

  async function closeSettings() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().hide();
  }
</script>

<div class="frame">
  <header class="bar">
    <div class="drag" data-tauri-drag-region></div>
    <button class="x" type="button" aria-label={t("close")} onclick={closeSettings}>
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
        <path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" stroke-width="1.4" />
      </svg>
    </button>
  </header>
  <div class="scroll">
    <main class="page">
    {#if settings}
      <!-- SECTION 1: CHAT & MODELLE -->
      <section>
        <h2>{t("chatAndModel")}</h2>
        <label>
          {t("overlayHotkey")}
          <HotkeyInput bind:value={settings.hotkey} placeholder={t("overlayHotkeyPlaceholder")} onchange={() => persistSettings()} />
        </label>
        <p class="note">{t("overlayHotkeyNote")}</p>
        
        <label class="check">
          <span>{t("clipboardContext")}</span>
          <Switch bind:checked={settings.clipboardContext} onchange={() => persistSettings()} />
        </label>
        <label class="check">
          <span>{t("historySave")}</span>
          <Switch bind:checked={settings.historyEnabled} onchange={() => persistSettings()} />
        </label>
        <label class="check">
          <span>{t("autostart")}</span>
          <Switch bind:checked={settings.autostart} onchange={() => persistSettings()} />
        </label>

        <div style="margin-top: 1.2rem;">
          <div class="row" style="margin-bottom: 0.5rem;">
            <label style="flex: 1; margin-bottom: 0;">
              {t("activeProvider")}
              <Select
                bind:value={settings.activeProviderId}
                options={providers.map((p) => ({ value: p.id, label: `${p.name} (${p.model || p.kind})` }))}
                onchange={() => persistSettings()}
              />
            </label>
          </div>

          <div class="row" style="margin-top: 0.4rem; margin-bottom: 0.8rem;">
            <div style="display: flex; gap: 0.4rem; flex-wrap: wrap;">
              <button type="button" class="ghost small" onclick={() => addPreset("gemini")}>+ Gemini</button>
              <button type="button" class="ghost small" onclick={() => addPreset("openai")}>+ OpenAI</button>
              <button type="button" class="ghost small" onclick={() => addPreset("openrouter")}>+ OpenRouter</button>
              <button type="button" class="ghost small" onclick={() => addPreset("ollama")}>+ Ollama</button>
              <button type="button" class="ghost small" onclick={addCustom}>+ Custom</button>
            </div>
          </div>

          {#if selectedChatProvider}
            <article class="provider-card">
              <div class="grid">
                <label>
                  {t("providerName")}
                  <input bind:value={selectedChatProvider.name} onchange={() => saveProvider(selectedChatProvider)} />
                </label>
                <label>
                  {t("providerModel")}
                  <input bind:value={selectedChatProvider.model} placeholder={t("providerModelPlaceholder")} onchange={() => saveProvider(selectedChatProvider)} />
                </label>
                <label class="wide">
                  {t("providerBaseUrl")}
                  <input bind:value={selectedChatProvider.baseUrl} placeholder="https://api.openai.com/v1" onchange={() => saveProvider(selectedChatProvider)} />
                </label>
                <label class="wide">
                  {t("providerApiKey")}
                  <input
                    type="password"
                    value={keyDrafts[selectedChatProvider.id] ?? ""}
                    oninput={(e) => {
                      keyDrafts[selectedChatProvider.id] = e.currentTarget.value;
                    }}
                    placeholder={selectedChatProvider.hasKey ? t("keySaved") : t("keyEmpty")}
                    onkeydown={(e) => {
                      if (e.key === "Enter") void saveProvider(selectedChatProvider);
                    }}
                  />
                </label>
              </div>
              <div class="row" style="margin-top: 0.6rem;">
                <button type="button" onclick={() => saveProvider(selectedChatProvider)}>{t("saveProvider")}</button>
                <button type="button" class="ghost" onclick={() => removeProvider(selectedChatProvider.id)}>{t("removeProvider")}</button>
              </div>
            </article>
          {/if}
        </div>

        <div class="row" style="margin-top: 1.2rem; align-items: center;">
          <label style="flex: 1; margin-bottom: 0;">
            {t("languageLabel")}
            <Select
              bind:value={settings.language}
              options={[
                { value: "auto", label: t("languageAuto") },
                { value: "en", label: t("languageEn") },
                { value: "de", label: t("languageDe") },
              ]}
              onchange={() => {
                if (settings) {
                  applyLanguage(settings.language);
                  persistSettings();
                }
              }}
            />
          </label>
        </div>
        <p class="note" style="margin-top: 0.35rem;">{t("languageNote")}</p>
      </section>

      <!-- DIVIDER -->
      <div class="section-divider"></div>

      <!-- SECTION 2: TEXT-TO-SPEECH -->
      <section>
        <div class="tts-section-head">
          <h2>{t("ttsTitle")}</h2>
          {#if ttsBusy}
            <span class="tts-busy-indicator" aria-live="polite">
              <span class="tts-spinner" aria-hidden="true"></span>
              {t("ttsBusyGenerating")}
            </span>
          {/if}
        </div>
        <label>
          {t("ttsHotkey")}
          <HotkeyInput bind:value={settings.ttsHotkey} placeholder={t("ttsHotkeyPlaceholder")} onchange={() => persistSettings()} />
        </label>
        <p class="note">{t("ttsHotkeyNote")}</p>

        <label style="margin-top: 1rem;">
          {t("ttsProvider")}
          <Select
            bind:value={settings.ttsProvider}
            options={[
              { value: "azure", label: "Azure Speech Services" },
              { value: "elevenlabs", label: "ElevenLabs API" },
              { value: "gemini", label: "Google Gemini API (Audio)" },
              { value: "openai", label: "OpenAI TTS (tts-1 / tts-1-hd)" },
              { value: "custom", label: t("ttsProviderCustomLabel") },
            ]}
            onchange={() => {
              if (settings) {
                void persistSettings();
                void checkTtsKey();
              }
            }}
          />
        </label>

        <article class="provider-card" style="margin-top: 0.9rem;">
          <div class="grid">
            {#if settings.ttsProvider === "custom"}
              <div class="wide" style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 0.25rem;">
                <div style="display: flex; align-items: center; gap: 0.5rem;">
                  {#if customStatus?.ready}
                    <span class="badge badge-green">{t("ttsServerReachable")}</span>
                  {:else if customStatus?.running}
                    <span class="badge badge-yellow">{t("ttsServerNotReady")}</span>
                  {:else}
                    <span class="badge badge-red">{t("ttsServerUnreachable")}</span>
                  {/if}
                </div>
                <button type="button" class="ghost small" onclick={refreshCustomStatus}>{t("ttsCheckStatus")}</button>
              </div>

              <label class="wide">
                {t("ttsCustomUrl")}
                <input bind:value={settings.ttsCustomUrl} placeholder={t("ttsCustomUrlPlaceholder")} onchange={() => { persistSettings(); refreshCustomStatus(); }} />
              </label>
              <label>
                {t("ttsVoice")}
                <input bind:value={settings.ttsCustomVoice} placeholder="af_bella — Kokoro / alloy" onchange={() => persistSettings()} />
              </label>
              <label>
                {t("ttsModel")}
                <input bind:value={settings.ttsCustomModel} placeholder="kokoro" onchange={() => persistSettings()} />
              </label>
              <div class="wide local-info">
                <p><strong>Kokoro FastAPI / OpenAI-TTS</strong></p>
                <p class="note">{t("ttsCustomNote")}</p>
                {#if customStatus?.message}
                  <p class="note">{customStatus.message}</p>
                {/if}
              </div>
            {:else if settings.ttsProvider === "openai"}
              <label>
                {t("ttsVoice")} (OpenAI)
                <Select
                  bind:value={settings.ttsOpenaiVoice}
                  options={openaiVoices}
                  onchange={() => persistSettings()}
                />
              </label>
              <label>
                {t("ttsModel")}
                <input bind:value={settings.ttsOpenaiModel} placeholder="tts-1" onchange={() => persistSettings()} />
              </label>
              <label class="wide">
                OpenAI API Key
                <input
                  type="password"
                  bind:value={ttsKeyDrafts.openai}
                  placeholder={ttsKeyStatus.openai ? t("saved") : t("openaiKeyHint")}
                  onkeydown={(e) => {
                    if (e.key === "Enter") void saveTtsKey("openai");
                  }}
                  onblur={() => {
                    if (ttsKeyDrafts.openai.trim()) void saveTtsKey("openai");
                  }}
                />
              </label>
            {:else if settings.ttsProvider === "gemini"}
              <label>
                {t("ttsVoice")}
                <Select
                  bind:value={settings.ttsGeminiVoice}
                  options={geminiVoices}
                  onchange={() => persistSettings()}
                />
              </label>
              <label>
                {t("ttsModel")}
                <input bind:value={settings.ttsGeminiModel} placeholder="gemini-2.0-flash" onchange={() => persistSettings()} />
              </label>
              <label class="wide">
                Gemini API Key (optional)
                <input
                  type="password"
                  bind:value={ttsKeyDrafts.gemini}
                  placeholder={ttsKeyStatus.gemini ? t("saved") : t("keyEmpty")}
                  onkeydown={(e) => {
                    if (e.key === "Enter") void saveTtsKey("gemini");
                  }}
                  onblur={() => {
                    if (ttsKeyDrafts.gemini.trim()) void saveTtsKey("gemini");
                  }}
                />
              </label>
            {:else if settings.ttsProvider === "elevenlabs"}
              <label>
                {t("ttsVoice")}
                <Select
                  bind:value={settings.ttsElevenVoice}
                  options={elevenVoices}
                  onchange={() => persistSettings()}
                />
              </label>
              <label>
                {t("ttsModel")}
                <input bind:value={settings.ttsElevenModel} placeholder="eleven_multilingual_v2" onchange={() => persistSettings()} />
              </label>
              <label class="wide">
                ElevenLabs API Key
                <input
                  type="password"
                  bind:value={ttsKeyDrafts.elevenlabs}
                  placeholder={ttsKeyStatus.elevenlabs ? t("saved") : t("keyEmpty")}
                  onkeydown={(e) => {
                    if (e.key === "Enter") void saveTtsKey("elevenlabs");
                  }}
                  onblur={() => {
                    if (ttsKeyDrafts.elevenlabs.trim()) void saveTtsKey("elevenlabs");
                  }}
                />
              </label>
            {:else if settings.ttsProvider === "azure"}
              <label>
                {t("ttsVoice")} (Azure Neural)
                <Select
                  bind:value={settings.ttsAzureVoice}
                  options={azureVoices}
                  onchange={() => persistSettings()}
                />
              </label>
              <label>
                {t("ttsAzureRegion")}
                <input bind:value={settings.ttsAzureRegionSetting} placeholder="westeurope" onchange={() => persistSettings()} />
              </label>
              <label class="wide">
                Azure Speech Key
                <input
                  type="password"
                  bind:value={ttsKeyDrafts.azure}
                  placeholder={ttsKeyStatus.azure ? t("saved") : t("keyEmpty")}
                  onkeydown={(e) => {
                    if (e.key === "Enter") void saveTtsKey("azure");
                  }}
                  onblur={() => {
                    if (ttsKeyDrafts.azure.trim()) void saveTtsKey("azure");
                  }}
                />
              </label>
            {/if}
          </div>

          <div class="row" style="margin-top: 0.9rem;">
            <div style="display: flex; gap: 0.5rem;">
              <button type="button" disabled={ttsTesting || ttsBusy} onclick={testTtsVoice}>
                {ttsTesting || ttsBusy ? t("ttsTesting") : t("ttsTestVoice")}
              </button>
              <button type="button" class="ghost" onclick={stopTtsVoice}>
                {t("ttsStop")}
              </button>
            </div>
            {#if settings && settings.ttsProvider !== "local" && settings.ttsProvider !== "custom"}
              <div style="display: flex; gap: 0.4rem;">
                {#if (ttsKeyDrafts[settings.ttsProvider] ?? "").trim()}
                  <button type="button" onclick={() => saveTtsKey(settings?.ttsProvider)}>{t("ttsSaveKey")}</button>
                {/if}
                {#if settings.ttsProvider && ttsKeyStatus[settings.ttsProvider]}
                  <button type="button" class="ghost" onclick={() => saveTtsKey(settings?.ttsProvider, "")}>{t("ttsDeleteKey")}</button>
                {/if}
              </div>
            {/if}
          </div>
          {#if ttsError}
            <p class="status" style="color: var(--danger); margin-top: 8px;">{ttsError}</p>
          {/if}
        </article>
      </section>

      <!-- DIVIDER -->
      <div class="section-divider"></div>

      <!-- SECTION 3: HISTORY -->
      <section>
        <div class="row">
          <h2>{t("historyTitle")}</h2>
          {#if settings?.historyEnabled && chats.length > 0}
            <button type="button" class="ghost small" onclick={wipeHistory}>{t("historyClear")}</button>
          {/if}
        </div>
        {#if !settings?.historyEnabled}
          <p class="note">
            {t("historyDisabledNote")}
          </p>
        {:else if chats.length === 0}
          <p class="note">{t("historyEmpty")}</p>
        {:else}
          <ul class="chats">
            {#each chats as chat}
              <li class:active={selectedChat?.chat.id === chat.id}>
                <button type="button" class="entry" onclick={() => openChat(chat.id)}>
                  <span class="title-row">
                    {#if chat.kind === "tts"}
                      <span class="kind-icon tts" title={t("ttsHistoryTitle")} aria-hidden="true">
                        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                          <line x1="2" y1="4" x2="2" y2="8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
                          <line x1="5" y1="2" x2="5" y2="10" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
                          <line x1="8" y1="3" x2="8" y2="9" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
                          <line x1="11" y1="5" x2="11" y2="7" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
                        </svg>
                      </span>
                    {:else}
                      <span class="kind-icon chat" title={t("historyTitle")} aria-hidden="true">
                        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                          <path d="M10 5.5c0 2.2-1.9 4-4.2 4-.6 0-1.2-.1-1.8-.4L2 9.5l.6-1.8C2.2 7.1 2 6.3 2 5.5 2 3.3 3.9 1.5 6.2 1.5S10 3.3 10 5.5z" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" />
                        </svg>
                      </span>
                    {/if}
                    <span class="title">{chat.title}</span>
                  </span>
                  <span class="meta">{stamp(chat.createdAt)} · {chat.model || chat.providerId}</span>
                </button>
                <button type="button" class="trash" aria-label={t("delete")} onclick={() => dropChat(chat.id)}>
                  <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
                    <path d="M2 3.5h8M4.8 3.5V2.4h2.4v1.1M3.2 3.5l.5 6h4.6l.5-6" stroke="currentColor" stroke-width="1.1" />
                  </svg>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
        {#if selectedChat}
          <div class="thread">
            <div class="row">
              <h2>{selectedChat.chat.title}</h2>
              <div class="thread-actions">
                {#if selectedChat.chat.kind === "tts"}
                  {@const fullText = selectedChat.messages.map((m) => m.content).join("\n\n")}
                  <button type="button" class="ghost small copy-btn" onclick={() => copyHistoryText(fullText)}>
                    {copiedHistoryText ? t("copiedText") : t("copyText")}
                  </button>
                {/if}
                <button type="button" class="ghost" onclick={() => (selectedChat = null)}>{t("close")}</button>
              </div>
            </div>
            {#if selectedChat.chat.kind === "tts"}
              <div class="tts-reading-block">
                {#each selectedChat.messages as message}
                  <p class="said spoken">{message.content}</p>
                {/each}
              </div>
            {:else}
              {#each selectedChat.messages as message}
                <p class="role">{message.role}</p>
                {#if message.role === "assistant"}
                  <MarkdownView html={renderMarkdown(message.content)} />
                {:else}
                  <p class="said">{message.content}</p>
                {/if}
              {/each}
            {/if}
          </div>
        {/if}
      </section>

      {#if status}
        <p class="status">{status}</p>
      {/if}
    {/if}
    </main>
  </div>
</div>

<style>
  .frame {
    position: relative;
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    overflow: hidden;
  }

  .bar {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
    height: 40px;
    padding: 0 6px 0 0;
  }

  .drag {
    flex: 1;
    align-self: stretch;
  }

  .x {
    flex: 0 0 auto;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--mute);
    display: grid;
    place-items: center;
    line-height: 0;
    cursor: pointer;
  }

  .x:hover {
    background: #1c1c1c;
    color: var(--fg);
  }

  .scroll {
    flex: 1;
    min-height: 0;
    width: 100%;
    overflow: auto;
    scrollbar-width: thin;
    scrollbar-color: #444 transparent;
  }

  .scroll::-webkit-scrollbar {
    width: 8px;
  }

  .scroll::-webkit-scrollbar-track {
    margin: 0;
    background: transparent;
  }

  .scroll::-webkit-scrollbar-thumb {
    background: #3f3f3f;
    border: 0;
    border-radius: 4px;
  }

  .page {
    max-width: 600px;
    margin: 0 auto;
    padding: 0 1.25rem 2.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    color: var(--fg);
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .section-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 0.5rem 0;
  }

  h2 {
    font-size: 0.95rem;
    font-weight: 600;
    color: #fff;
    margin: 0 0 0.25rem;
    letter-spacing: -0.01em;
  }

  .provider-card {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 0.9rem;
  }

  .badge {
    font-size: 0.72rem;
    padding: 0.2rem 0.5rem;
    border-radius: 999px;
    font-weight: 500;
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
  }

  .badge-green {
    background: rgba(34, 197, 94, 0.12);
    color: #4ade80;
    border: 1px solid rgba(34, 197, 94, 0.25);
  }

  .badge-red {
    background: rgba(239, 68, 68, 0.12);
    color: #f87171;
    border: 1px solid rgba(239, 68, 68, 0.25);
  }

  .badge-yellow {
    background: rgba(234, 179, 8, 0.12);
    color: #facc15;
    border: 1px solid rgba(234, 179, 8, 0.25);
  }

  .tts-section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    flex-wrap: wrap;
    margin-bottom: 0.25rem;
  }

  .tts-section-head h2 {
    margin: 0;
  }

  .tts-busy-indicator {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.78rem;
    color: #93c5fd;
    padding: 0.25rem 0.55rem;
    border-radius: 999px;
    background: rgba(59, 130, 246, 0.1);
    border: 1px solid rgba(59, 130, 246, 0.22);
  }

  .tts-spinner {
    width: 0.85rem;
    height: 0.85rem;
    border: 2px solid rgba(147, 197, 253, 0.25);
    border-top-color: #93c5fd;
    border-radius: 999px;
    animation: ttsSpin 0.8s linear infinite;
  }

  @keyframes ttsSpin {
    to {
      transform: rotate(360deg);
    }
  }

  .local-info {
    background: rgba(255, 255, 255, 0.02);
    border-radius: 6px;
    padding: 0.6rem 0.75rem;
    border-left: 2px solid #3b82f6;
  }

  .local-info p {
    margin: 0;
    font-size: 0.85rem;
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
  }

  .wide {
    grid-column: span 2;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.8rem;
    color: var(--mute);
  }

  label.check {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    padding: 0.4rem 0;
    color: var(--fg);
  }

  input {
    background: #141414;
    border: 1px solid #282828;
    border-radius: 6px;
    padding: 0.5rem 0.65rem;
    color: #eee;
    font-size: 0.85rem;
    outline: none;
    transition: border-color 0.15s;
  }

  input:focus {
    border-color: #555;
  }

  input::placeholder {
    color: #555;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  button {
    background: #222;
    border: 1px solid #333;
    border-radius: 6px;
    color: #eee;
    padding: 0.45rem 0.85rem;
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  button:hover:not(:disabled) {
    background: #2c2c2c;
    border-color: #444;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  button.ghost {
    background: transparent;
    border-color: transparent;
    color: var(--mute);
  }

  button.ghost:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #eee;
  }

  button.small {
    padding: 0.3rem 0.6rem;
    font-size: 0.75rem;
  }

  .note {
    font-size: 0.75rem;
    color: #666;
    margin: 0;
    line-height: 1.4;
  }

  .status {
    position: fixed;
    bottom: 1rem;
    right: 1.5rem;
    background: #222;
    border: 1px solid #333;
    border-radius: 6px;
    padding: 0.4rem 0.8rem;
    font-size: 0.75rem;
    color: #aaa;
    animation: fadeIn 0.2s ease-out;
  }

  .chats {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .chats li {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: #141414;
    border: 1px solid #222;
    border-radius: 6px;
    padding: 0.25rem 0.5rem;
  }

  .chats li.active {
    border-color: #444;
    background: #1a1a1a;
  }

  .chats .entry {
    flex: 1;
    background: transparent;
    border: 0;
    padding: 0.3rem;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }

  .chats .title-row {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    min-width: 0;
  }

  .kind-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 13px;
    height: 13px;
    flex-shrink: 0;
  }

  .kind-icon.chat {
    color: rgba(255, 255, 255, 0.4);
  }

  .kind-icon.tts {
    color: rgba(255, 255, 255, 0.55);
  }

  .chats li:hover .kind-icon.chat,
  .chats li.active .kind-icon.chat {
    color: rgba(255, 255, 255, 0.7);
  }

  .chats li:hover .kind-icon.tts,
  .chats li.active .kind-icon.tts {
    color: rgba(255, 255, 255, 0.85);
  }

  .chats .title {
    font-size: 0.85rem;
    color: #eee;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .chats .meta {
    font-size: 0.7rem;
    color: #666;
  }

  .trash {
    background: transparent;
    border: 0;
    color: #666;
    padding: 0.3rem;
    display: grid;
    place-items: center;
  }

  .trash:hover {
    color: #ff5555;
    background: transparent;
  }

  .thread {
    margin-top: 1rem;
    padding: 1rem;
    background: #141414;
    border: 1px solid #282828;
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .thread-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .copy-btn {
    border: 1px solid rgba(255, 255, 255, 0.12) !important;
    background: rgba(255, 255, 255, 0.04) !important;
    color: #ccc !important;
  }

  .copy-btn:hover {
    background: rgba(255, 255, 255, 0.09) !important;
    color: #fff !important;
  }

  .tts-reading-block {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0.4rem 0.2rem;
  }

  .thread .role {
    font-size: 0.7rem;
    text-transform: uppercase;
    color: #666;
    margin: 0;
    letter-spacing: 0.05em;
  }

  .thread .said {
    margin: 0;
    font-size: 0.85rem;
    color: #ddd;
    line-height: 1.45;
  }

  .thread .said.spoken {
    font-size: 0.9rem;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    color: #e0e0e0;
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
