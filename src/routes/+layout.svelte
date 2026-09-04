<script lang="ts">
  import "../app.css";
  import { page } from "$app/stores";
  import { onMount } from "svelte";

  let { children } = $props();
  let label = $state("");

  onMount(async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      label = getCurrentWindow().label;
    } catch {
      label = $page.url.pathname.startsWith("/settings") ? "settings" : "quick-ask";
    }
  });

  let settings = $derived(label === "settings" || $page.url.pathname.startsWith("/settings"));

  $effect(() => {
    if (typeof document === "undefined") return;
    document.documentElement.classList.toggle("is-settings", settings);
    document.documentElement.classList.toggle("is-overlay", !settings);
    document.body.classList.toggle("is-settings", settings);
    document.body.classList.toggle("is-overlay", !settings);
  });
</script>

<div class={settings ? "shell-settings" : "shell-overlay"}>
  {@render children()}
</div>
