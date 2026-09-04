<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "$lib/i18n.svelte";

  type Props = {
    value?: string;
    placeholder?: string;
    presets?: string[];
    onchange?: () => void;
  };

  let {
    value = $bindable(""),
    placeholder = "",
    presets = ["Alt+Space", "Alt+Shift+S", "F8", "F9", "F10", "Ctrl+Space"],
    onchange,
  }: Props = $props();

  const pressHint = $derived(placeholder || t("hotkeyPress"));

  let recording = $state(false);
  let manualEdit = $state(false);
  let heldModifiers = $state<string[]>([]);

  function mapKeyToStandard(e: KeyboardEvent): string {
    const { code, key } = e;

    // Space
    if (code === "Space" || key === " " || key === "Space") {
      return "Space";
    }

    // Function Keys (F1-F24)
    if (/^F\d{1,2}$/i.test(code) || /^F\d{1,2}$/i.test(key)) {
      return (code || key).toUpperCase();
    }

    // Numpad Keys
    if (code.startsWith("Numpad") && code.length === 7 && /\d/.test(code.slice(6))) {
      return `Num${code.slice(6)}`;
    }

    // Navigation & Editing
    switch (code) {
      case "Escape":
        return "Escape";
      case "Tab":
        return "Tab";
      case "Backspace":
        return "Backspace";
      case "Delete":
        return "Delete";
      case "Insert":
        return "Insert";
      case "Home":
        return "Home";
      case "End":
        return "End";
      case "PageUp":
        return "PageUp";
      case "PageDown":
        return "PageDown";
      case "ArrowUp":
        return "ArrowUp";
      case "ArrowDown":
        return "ArrowDown";
      case "ArrowLeft":
        return "ArrowLeft";
      case "ArrowRight":
        return "ArrowRight";
      case "Enter":
        return "Enter";
    }

    // Layout-Aware Characters (QWERTZ / International: Z, Y, Ä, Ö, Ü, ß, etc.)
    if (key && key.length === 1) {
      return key.toUpperCase();
    }

    return key || code;
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (!recording) return;

    e.preventDefault();
    e.stopPropagation();
    e.stopImmediatePropagation();

    const isModifierOnly = ["Control", "Alt", "Shift", "Meta"].includes(e.key);

    const mods: string[] = [];
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.altKey) mods.push("Alt");
    if (e.shiftKey) mods.push("Shift");
    if (e.metaKey) mods.push("Super");

    if (isModifierOnly) {
      heldModifiers = mods;
      return;
    }

    if ((e.code === "Escape" || e.key === "Escape") && mods.length === 0) {
      recording = false;
      heldModifiers = [];
      return;
    }

    const mainKey = mapKeyToStandard(e);
    const finalShortcut = mods.length > 0 ? `${mods.join("+")}+${mainKey}` : mainKey;

    value = finalShortcut;
    recording = false;
    heldModifiers = [];
    onchange?.();
  }

  function handleKeyUp(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    e.stopImmediatePropagation();

    const mods: string[] = [];
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.altKey) mods.push("Alt");
    if (e.shiftKey) mods.push("Shift");
    if (e.metaKey) mods.push("Super");
    heldModifiers = mods;
  }

  function handlePointerDown(e: MouseEvent) {
    if (!recording) return;

    // Mouse button mapping:
    // 1: Middle Click -> Mouse3
    // 2: Right Click -> Mouse2
    // 3: Back Button -> Mouse4
    // 4: Forward Button -> Mouse5
    let mouseKey = "";
    if (e.button === 1) mouseKey = "Mouse3";
    else if (e.button === 2) mouseKey = "Mouse2";
    else if (e.button === 3) mouseKey = "Mouse4";
    else if (e.button === 4) mouseKey = "Mouse5";

    if (mouseKey) {
      e.preventDefault();
      e.stopPropagation();
      e.stopImmediatePropagation();

      const mods: string[] = [];
      if (e.ctrlKey) mods.push("Ctrl");
      if (e.altKey) mods.push("Alt");
      if (e.shiftKey) mods.push("Shift");
      if (e.metaKey) mods.push("Super");

      value = mods.length > 0 ? `${mods.join("+")}+${mouseKey}` : mouseKey;
      recording = false;
      heldModifiers = [];
      onchange?.();
    }
  }

  onMount(() => {
    const onKeyDown = (e: KeyboardEvent) => handleKeyDown(e);
    const onKeyUp = (e: KeyboardEvent) => handleKeyUp(e);
    const onPointer = (e: MouseEvent) => handlePointerDown(e);
    const onContextMenu = (e: MouseEvent) => {
      if (recording) {
        e.preventDefault();
        e.stopPropagation();
      }
    };

    window.addEventListener("keydown", onKeyDown, { capture: true });
    window.addEventListener("keyup", onKeyUp, { capture: true });
    window.addEventListener("pointerdown", onPointer, { capture: true });
    window.addEventListener("contextmenu", onContextMenu, { capture: true });

    return () => {
      window.removeEventListener("keydown", onKeyDown, { capture: true });
      window.removeEventListener("keyup", onKeyUp, { capture: true });
      window.removeEventListener("pointerdown", onPointer, { capture: true });
      window.removeEventListener("contextmenu", onContextMenu, { capture: true });
    };
  });

  function startRecording() {
    manualEdit = false;
    recording = true;
    heldModifiers = [];
  }

  function stopRecording() {
    recording = false;
    heldModifiers = [];
  }

  function clearShortcut(e: MouseEvent) {
    e.stopPropagation();
    value = "";
    recording = false;
    onchange?.();
  }

  function applyPreset(p: string, e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    value = p;
    recording = false;
    manualEdit = false;
    onchange?.();
  }
</script>

<div class="hotkey-wrap">
  {#if manualEdit}
    <div class="manual-row">
      <input
        class="manual-input"
        bind:value
        placeholder={t("overlayHotkeyPlaceholder")}
        onblur={() => {
          manualEdit = false;
          onchange?.();
        }}
        onkeydown={(e) => {
          if (e.key === "Enter") {
            manualEdit = false;
            onchange?.();
          }
        }}
      />
      <button
        type="button"
        class="btn-text-mode"
        title={t("hotkeyCapture")}
        onclick={() => (manualEdit = false)}
      >
        {t("hotkeyCapture")}
      </button>
    </div>
  {:else}
    <div
      role="button"
      tabindex="0"
      class="hotkey-btn"
      class:recording
      onclick={startRecording}
      onkeydown={(e) => {
        if (!recording && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          startRecording();
        }
      }}
    >
      {#if recording}
        <span class="recording-badge">
          <span class="dot"></span>
          {#if heldModifiers.length > 0}
            {heldModifiers.join(" + ")} + ...
          {:else}
            {t("hotkeyListening")}
          {/if}
        </span>
      {:else if value}
        <span class="keys">
          {#each value.split("+") as keyPart}
            <kbd>{keyPart.trim()}</kbd>
          {/each}
        </span>
      {:else}
        <span class="empty">{pressHint}</span>
      {/if}

      <div class="actions">
        {#if value && !recording}
          <button
            type="button"
            class="clear-btn"
            title={t("hotkeyClear")}
            aria-label={t("hotkeyClearAria")}
            onclick={clearShortcut}
          >
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
              <path d="M2 2l6 6M8 2l-6 6" stroke="currentColor" stroke-width="1.3" />
            </svg>
          </button>
        {/if}
        <button
          type="button"
          class="clear-btn"
          title={t("hotkeyManual")}
          aria-label={t("hotkeyManualAria")}
          onclick={(e) => {
            e.stopPropagation();
            manualEdit = true;
          }}
        >
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 20h9M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
          </svg>
        </button>
        <span class="hint">{recording ? t("hotkeyRecording") : t("hotkeyCapture")}</span>
      </div>
    </div>
  {/if}

  {#if presets.length > 0}
    <div class="presets-row">
      <span class="presets-label">{t("hotkeyPresets")}</span>
      {#each presets as preset}
        <button
          type="button"
          class="preset-chip"
          class:active={value.toLowerCase() === preset.toLowerCase()}
          onclick={(e) => applyPreset(preset, e)}
        >
          {preset}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .hotkey-wrap {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .hotkey-btn {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--bg-elev);
    color: var(--fg);
    padding: 0.5rem 0.75rem;
    border-radius: 10px;
    font-size: 0.85rem;
    cursor: pointer;
    text-align: left;
    transition: border-color 0.15s, background 0.15s;
    min-height: 40px;
  }

  .hotkey-btn:hover {
    border-color: #383838;
    background: #191919;
  }

  .hotkey-btn.recording {
    border-color: #ffffff;
    background: #141414;
    color: #ffffff;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.2);
  }

  .manual-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .manual-input {
    flex: 1;
    border: 1px solid var(--line);
    background: var(--bg-elev);
    color: var(--fg);
    padding: 0.5rem 0.75rem;
    border-radius: 10px;
    font-size: 0.85rem;
  }

  .manual-input:focus {
    border-color: #ffffff;
    outline: none;
  }

  .btn-text-mode {
    border: 1px solid var(--line);
    background: #1f1f1f;
    color: var(--mute);
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    font-size: 0.78rem;
    cursor: pointer;
  }

  .btn-text-mode:hover {
    color: var(--fg);
    background: #282828;
  }

  .keys {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }

  kbd {
    display: inline-block;
    padding: 2px 7px;
    background: #242424;
    border: 1px solid #3a3a3a;
    border-radius: 6px;
    font-family: inherit;
    font-size: 0.76rem;
    font-weight: 500;
    color: var(--fg);
    box-shadow: 0 1px 0 rgba(0, 0, 0, 0.3);
  }

  .empty {
    color: var(--mute);
    font-size: 0.82rem;
  }

  .recording-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #ffffff;
    font-size: 0.8rem;
    font-weight: 500;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #ffffff;
    animation: pulse 1s infinite alternate;
  }

  @keyframes pulse {
    0% {
      opacity: 0.4;
      transform: scale(0.85);
    }
    100% {
      opacity: 1;
      transform: scale(1.15);
    }
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .hint {
    font-size: 0.72rem;
    color: var(--mute);
    padding: 2px 6px;
    border-radius: 4px;
    background: #1f1f1f;
  }

  .clear-btn {
    border: 0;
    background: transparent;
    color: var(--mute);
    padding: 4px;
    border-radius: 4px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }

  .clear-btn:hover {
    color: var(--fg);
    background: #282828;
  }

  .presets-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
    margin-top: 2px;
  }

  .presets-label {
    font-size: 0.72rem;
    color: var(--mute);
    margin-right: 2px;
  }

  .preset-chip {
    border: 1px solid #282828;
    background: #181818;
    color: var(--mute);
    padding: 2px 7px;
    border-radius: 6px;
    font-size: 0.72rem;
    cursor: pointer;
    transition: all 0.12s;
  }

  .preset-chip:hover {
    border-color: #444444;
    color: var(--fg);
    background: #222222;
  }

  .preset-chip.active {
    border-color: #ffffff;
    color: #000000;
    background: #ffffff;
    font-weight: 600;
  }
</style>
