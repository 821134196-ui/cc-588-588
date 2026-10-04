<script>
  import TaskBoard from './TaskBoard.svelte';
  import PetCards from './PetCards.svelte';
  import PickupRedeem from './PickupRedeem.svelte';

  const tabs = [
    { key: 'board', label: '今日照护看板' },
    { key: 'pets', label: '宠物卡片' },
    { key: 'redeem', label: '取宠核销' },
  ];
  let tab = 'board';
</script>

<div class="tabs">
  {#each tabs as t}
    <button class="tab" class:on={tab === t.key} on:click={() => (tab = t.key)}>
      {t.label}
    </button>
  {/each}
</div>

{#if tab === 'board'}
  <TaskBoard />
{:else if tab === 'pets'}
  <PetCards />
{:else}
  <PickupRedeem />
{/if}

<style>
  .tabs { display: flex; gap: 8px; margin-bottom: 16px; flex-wrap: wrap; }
  .tab { border-radius: 999px; padding: 6px 18px; }
  .tab.on { background: var(--brand); border-color: var(--brand); color: #fff; }
</style>
