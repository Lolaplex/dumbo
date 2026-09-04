<script lang="ts">
  let {
    checked = $bindable(false),
    onchange,
  }: {
    checked?: boolean;
    onchange?: () => void;
  } = $props();

  function toggle() {
    checked = !checked;
    onchange?.();
  }
</script>

<button
  class="switch"
  class:on={checked}
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label="Umschalten"
  onclick={toggle}
>
  <span class="knob"></span>
</button>

<style>
  .switch {
    position: relative;
    width: 36px;
    height: 22px;
    flex-shrink: 0;
    border: 0;
    border-radius: 999px;
    background: #2a2a2a;
    padding: 0;
  }

  .switch.on {
    background: #f4f4f4;
  }

  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    border-radius: 999px;
    background: #111;
    transition: transform 160ms ease;
  }

  .switch.on .knob {
    transform: translateX(14px);
    background: #111;
  }

  @media (prefers-reduced-motion: reduce) {
    .knob {
      transition: none;
    }
  }
</style>
