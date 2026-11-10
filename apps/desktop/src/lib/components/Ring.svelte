<script lang="ts">
  // A circular progress in thousandths; turns while the value is still unknown.
  let { value, size = 96, label }: { value: number | null; size?: number; label: string } = $props();
  const R = 42;
  const C = 2 * Math.PI * R;
</script>

<div
  class="ring"
  class:spin={value === null}
  style:width="{size}px"
  style:height="{size}px"
  role="progressbar"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={value === null ? undefined : Math.round(value / 10)}
>
  <svg viewBox="0 0 100 100" aria-hidden="true">
    <circle cx="50" cy="50" r={R} class="track" />
    <circle cx="50" cy="50" r={R} class="bar" stroke-dasharray={C} stroke-dashoffset={C * (1 - (value ?? 250) / 1000)} />
  </svg>
  {#if value !== null}<span class="pct">{Math.round(value / 10)}%</span>{/if}
</div>

<style>
  .ring {
    position: relative;
    display: grid;
    place-items: center;
  }
  svg {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    transform: rotate(-90deg);
  }
  circle {
    fill: none;
    stroke-width: 7;
  }
  .track {
    stroke: var(--surface-3);
  }
  .bar {
    stroke: var(--accent);
    stroke-linecap: round;
    transition: stroke-dashoffset 0.2s;
  }
  .spin svg {
    animation: spin 1.2s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(270deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spin svg {
      animation: none;
    }
  }
  .pct {
    font-size: 24px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
</style>
