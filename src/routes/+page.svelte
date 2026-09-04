<script lang="ts">
  import { onMount } from "svelte";
  import OverlayAsk from "$lib/OverlayAsk.svelte";
  import SettingsPage from "$lib/SettingsPage.svelte";
  import TrayMenu from "$lib/TrayMenu.svelte";
  import Splash from "$lib/Splash.svelte";

  let label = $state("");

  onMount(async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      label = getCurrentWindow().label;
    } catch {
      label = "quick-ask";
    }
  });
</script>

{#if label === "settings"}
  <SettingsPage />
{:else if label === "tray-menu"}
  <TrayMenu />
{:else if label === "splash"}
  <Splash />
{:else if label}
  <OverlayAsk />
{/if}
