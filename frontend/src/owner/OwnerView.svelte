<script>
  import MyPets from './MyPets.svelte';
  import Notifications from './Notifications.svelte';

  const tabs = [
    { key: 'pets', label: '我的宠物 · 照护记录' },
    { key: 'notifications', label: '消息通知' },
  ];
  let tab = 'pets';
</script>

<div class="tabs">
  {#each tabs as t}
    <button class="tab" class:on={tab === t.key} on:click={() => (tab = t.key)}>{t.label}</button>
  {/each}
</div>

{#if tab === 'pets'}
  <MyPets />
{:else}
  <Notifications />
{/if}

<style>
  .tabs { display: flex; gap: 8px; margin-bottom: 16px; }
  .tab { border-radius: 999px; padding: 6px 18px; }
  .tab.on { background: var(--brand); border-color: var(--brand); color: #fff; }
</style>
