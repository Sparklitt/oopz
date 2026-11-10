<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import PathRow from "../components/PathRow.svelte";
  import { strings } from "../i18n";
  import { app, startOver } from "../state.svelte";

  let t = $derived(strings(app.lang));
</script>

<div class="page narrow done">
  <span class="mark"><Icon name="check" size={34} /></span>
  <h1>{t.doneTitle}</h1>
  <p class="muted">{t.doneLead}</p>
  {#if app.outcome}<PathRow folder={app.outcome.folder} />{/if}
  <button class="btn" onclick={startOver}>{t.startOver}</button>
</div>

<style>
  .done {
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 12px;
    padding-top: 48px;
  }
  .mark {
    width: 72px;
    height: 72px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--ok);
    color: #fff;
  }
  h1 {
    margin: 8px 0 0;
    font-size: 26px;
  }
  p {
    max-width: 52ch;
    margin: 0;
  }
  .done :global(.path-row) {
    width: 100%;
    max-width: 620px;
    text-align: left;
  }
</style>
