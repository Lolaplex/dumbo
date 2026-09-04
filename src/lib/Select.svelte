<script lang="ts">
  type Option = { value: string; label: string };

  let {
    value = $bindable(""),
    options = [],
    onchange,
  }: {
    value?: string;
    options?: Option[];
    onchange?: () => void;
  } = $props();

  let open = $state(false);

  let current = $derived(options.find((item) => item.value === value)?.label ?? value);

  function pick(next: string) {
    value = next;
    open = false;
    onchange?.();
  }
</script>

<div class="wrap">
  <button class="trigger" type="button" onclick={() => (open = !open)}>
    <span>{current}</span>
    <svg width="10" height="6" viewBox="0 0 10 6" fill="none" aria-hidden="true">
      <path d="M1 1l4 4 4-4" stroke="currentColor" stroke-width="1.3" />
    </svg>
  </button>
  {#if open}
    <div class="menu">
      {#each options as option}
        <button
          class="item"
          class:on={option.value === value}
          type="button"
          onclick={() => pick(option.value)}
        >
          {option.label}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }

  .trigger {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--bg-elev);
    color: var(--fg);
    padding: 0.55rem 0.75rem;
    border-radius: 10px;
    font-size: 0.88rem;
    text-align: left;
  }

  .menu {
    position: absolute;
    z-index: 8;
    left: 0;
    right: 0;
    top: calc(100% + 6px);
    padding: 6px;
    background: #161616;
    border: 1px solid var(--line);
    border-radius: 12px;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.45);
    max-height: 240px;
    overflow: auto;
  }

  .item {
    display: block;
    width: 100%;
    text-align: left;
    border: 0;
    background: transparent;
    color: var(--fg);
    padding: 0.5rem 0.65rem;
    border-radius: 8px;
    font-size: 0.88rem;
  }

  .item.on,
  .item:hover {
    background: #2a2a2a;
  }
</style>
