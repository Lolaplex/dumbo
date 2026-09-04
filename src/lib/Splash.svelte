<script lang="ts">
  import { onMount } from "svelte";

  let visible = $state(false);

  onMount(() => {
    visible = true;
    const timer = window.setTimeout(async () => {
      visible = false;
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        await getCurrentWindow().close();
      } catch {
        /* window already gone */
      }
    }, 2000);
    return () => window.clearTimeout(timer);
  });
</script>

{#if visible}
  <div class="splash" aria-hidden="true">
    <img src="/splash-icon.png" alt="" width="128" height="128" />
  </div>
{/if}

<style>
  .splash {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    place-items: center;
    background: transparent;
    pointer-events: none;
  }

  .splash img {
    width: 128px;
    height: 128px;
    display: block;
    animation: splash-pop 2s ease forwards;
  }

  @keyframes splash-pop {
    0% {
      transform: scale(0.92);
      opacity: 0;
    }
    14% {
      transform: scale(1);
      opacity: 1;
    }
    85% {
      transform: scale(1);
      opacity: 1;
    }
    100% {
      transform: scale(1.04);
      opacity: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .splash img {
      animation-duration: 0.01s;
    }
  }
</style>
