<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { ipc } from "$lib/ipc";
  import type { TrayAction } from "$lib/types";
  import { t, applyLanguage } from "$lib/i18n.svelte";

  type Entry = {
    action: TrayAction;
    key: "trayAsk" | "traySettings" | "trayQuit";
    hint: string;
    danger?: boolean;
  };

  const entries: Entry[] = [
    { action: "open", key: "trayAsk", hint: "Alt+Space" },
    { action: "settings", key: "traySettings", hint: "Ctrl+," },
    { action: "quit", key: "trayQuit", hint: "", danger: true },
  ];

  let root = $state<HTMLElement | null>(null);
  let shell = $state<HTMLElement | null>(null);
  let active = $state(0);
  let busy = $state(false);

  async function run(action: TrayAction) {
    if (busy) return;
    busy = true;
    try {
      await ipc.trayAction(action);
    } finally {
      busy = false;
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      void ipc.hideTrayMenu();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : entries.length - 1;
      active = (active + step) % entries.length;
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      void run(entries[active].action);
    }
  }

  onMount(() => {
    const unsubs: Array<() => void> = [];
    shell?.focus();

    void ipc.getSettings().then((s) => {
      applyLanguage(s?.language);
    }).catch(() => {});

    void listen("tray-menu-shown", () => {
      active = 0;
      busy = false;
      shell?.focus();
      void ipc.getSettings().then((s) => {
        applyLanguage(s?.language);
      }).catch(() => {});
    }).then((unlisten) => unsubs.push(unlisten));

    void listen<{ language?: string }>("settings-changed", (event) => {
      applyLanguage(event.payload?.language);
    }).then((unlisten) => unsubs.push(unlisten));

    const observer = new ResizeObserver(() => {
      if (!root) return;
      void ipc.resizeTrayMenu(Math.ceil(root.getBoundingClientRect().height));
    });
    if (root) observer.observe(root);

    return () => {
      observer.disconnect();
      unsubs.forEach((fn) => fn());
    };
  });
</script>

<svelte:window onkeydown={onKeydown} />

<div class="stage" bind:this={root}>
  <div class="menu" bind:this={shell} tabindex="-1" role="menu">
    {#each entries as entry, index}
      {#if entry.danger}
        <div class="sep" role="separator"></div>
      {/if}
      <button
        type="button"
        role="menuitem"
        class="item"
        class:on={active === index}
        class:danger={entry.danger}
        onmouseenter={() => (active = index)}
        onclick={() => run(entry.action)}
      >
        <span class="glyph" aria-hidden="true">
          {#if entry.action === "open"}
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <path d="M7 1.5l1.5 3.6L12 6.6l-3.5 1.5L7 11.7 5.5 8.1 2 6.6l3.5-1.5L7 1.5z" stroke="currentColor" stroke-width="1.1" />
            </svg>
          {:else if entry.action === "settings"}
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <circle cx="7" cy="7" r="2.1" stroke="currentColor" stroke-width="1.1" />
              <path d="M7 1.4v1.5M7 11.1v1.5M1.4 7h1.5M11.1 7h1.5M3 3l1.1 1.1M9.9 9.9L11 11M11 3L9.9 4.1M4.1 9.9L3 11" stroke="currentColor" stroke-width="1.1" />
            </svg>
          {:else}
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <path d="M7 1.8v4.4" stroke="currentColor" stroke-width="1.1" />
              <path d="M10.4 3.6a4.6 4.6 0 1 1-6.8 0" stroke="currentColor" stroke-width="1.1" />
            </svg>
          {/if}
        </span>
        <span class="label">{t(entry.key)}</span>
        {#if entry.hint}
          <span class="hint">{entry.hint}</span>
        {/if}
      </button>
    {/each}
  </div>
</div>

<style>
  /* Padding leaves room for the shadow to fade out inside the transparent
     window instead of being clipped into a rectangle. */
  .stage {
    padding: 8px 14px 20px;
    background: transparent;
  }

  .menu {
    padding: 6px;
    border-radius: 16px;
    background: #202020;
    outline: none;
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.07),
      0 0 18px rgba(255, 255, 255, 0.05),
      0 12px 28px rgba(0, 0, 0, 0.55);
  }

  .item {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    height: 34px;
    padding: 0 10px;
    border: 0;
    border-radius: 11px;
    background: transparent;
    color: var(--fg);
    font-size: 0.86rem;
    letter-spacing: -0.01em;
    text-align: left;
    transition: background 120ms ease;
  }

  .item.on {
    background: rgba(255, 255, 255, 0.09);
  }

  .item.danger.on {
    background: rgba(255, 255, 255, 0.06);
    color: var(--danger);
  }

  .glyph {
    display: grid;
    place-items: center;
    color: var(--mute);
    line-height: 0;
  }

  .item.on .glyph {
    color: inherit;
  }

  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hint {
    font-family: var(--mono);
    font-size: 0.68rem;
    color: var(--ivory-mute);
  }

  .item.on .hint {
    color: var(--mute);
  }

  .sep {
    height: 1px;
    margin: 5px 8px;
    background: var(--line);
  }
</style>
