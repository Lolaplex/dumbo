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

  onMount(() => {
    let dead = false;
    const pulse = () => {
      if (dead) return;
      void import("@tauri-apps/api/core")
        .then(({ invoke }) => invoke("webview_pulse"))
        .catch(() => {});
    };
    pulse();
    const timer = setInterval(pulse, 1500);
    const onWake = () => pulse();
    document.addEventListener("visibilitychange", onWake);
    window.addEventListener("pageshow", onWake);
    let unlisten: (() => void) | undefined;
    void import("@tauri-apps/api/event").then(({ listen }) =>
      listen("dumbo-probe", () => pulse()).then((stop) => {
        if (dead) stop();
        else unlisten = stop;
      }),
    );
    return () => {
      dead = true;
      clearInterval(timer);
      document.removeEventListener("visibilitychange", onWake);
      window.removeEventListener("pageshow", onWake);
      unlisten?.();
    };
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
