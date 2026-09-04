<script lang="ts">
  let { html }: { html: string } = $props();
</script>

<div class="md">{@html html}</div>

<style>
  .md {
    font-size: 0.95rem;
    line-height: 1.55;
    color: var(--ivory);
  }

  .md :global(p) {
    margin: 0 0 0.8em;
  }

  .md :global(p:last-child) {
    margin-bottom: 0;
  }

  .md :global(a) {
    color: var(--brass);
  }

  .md :global(ul),
  .md :global(ol) {
    margin: 0 0 0.8em;
    padding-left: 1.2em;
  }

  .md :global(code) {
    font-family: var(--mono);
    font-size: 0.82em;
    background: rgba(255, 255, 255, 0.06);
    padding: 0.1em 0.35em;
    border-radius: 3px;
  }

  .md :global(pre.code-block) {
    position: relative;
    margin: 0.8em 0;
    padding: 1.1rem 0.9rem 0.85rem;
    background: #0b0d12;
    border: 1px solid var(--hairline);
    border-radius: 12px;
    overflow-x: auto;
  }

  .md :global(pre.code-block code) {
    background: transparent;
    padding: 0;
    font-size: 0.8rem;
    line-height: 1.5;
    color: var(--ivory);
  }

  .md :global(.code-lang) {
    position: absolute;
    top: 0.35rem;
    right: 0.5rem;
    font-family: var(--mono);
    font-size: 0.65rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ivory-mute);
  }

  .md :global(.hljs-keyword),
  .md :global(.hljs-selector-tag) {
    color: var(--brass);
  }

  .md :global(.hljs-string),
  .md :global(.hljs-attr) {
    color: #d8c4a0;
  }

  .md :global(.hljs-comment) {
    color: var(--ivory-mute);
    font-style: italic;
  }

  .md :global(.hljs-number),
  .md :global(.hljs-literal) {
    color: #e4c9a5;
  }
</style>
