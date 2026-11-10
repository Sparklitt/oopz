<script lang="ts">
  // The in-app mark: a curled "restore" arrow on the accent's rounded square. Decorative: the name is always next to it.
  let { size = 28 }: { size?: number } = $props();
</script>

<svg class="mark" width={size} height={size} viewBox="0 0 64 64" aria-hidden="true" focusable="false">
  <rect width="64" height="64" rx="15" fill="var(--mark-bg)" />
  <path d="M17.5 32a14.5 14.5 0 1 0 4.25-10.25" fill="none" stroke="#fff" stroke-width="5" stroke-linecap="round" />
  <path d="M15.5 14.5v10.25h10.25" fill="none" stroke="#fff" stroke-width="5" stroke-linecap="round" stroke-linejoin="round" />
  <circle cx="32" cy="32" r="3.6" fill="#fff" />
</svg>

<style>
  .mark {
    flex: none;
    display: block;
  }
</style>
