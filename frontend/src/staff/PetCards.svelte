<script>
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { api } from '../lib/api.js';
  import { toast } from '../lib/toast.js';
  import PetDetail from './PetDetail.svelte';
  import CreatePetModal from './CreatePetModal.svelte';

  let pets = [];
  let loading = true;
  let selectedId = null;
  let showCreate = false;

  onMount(load);
  async function load() {
    loading = true;
    try {
      const res = await api.get('/api/pets');
      pets = res.pets;
    } catch (e) {
      toast(e.message, 'error');
    } finally {
      loading = false;
    }
  }
</script>

<div class="row wrap" style="margin-bottom:14px">
  <h2 style="font-size:17px">寄养宠物卡片</h2>
  <div class="spacer"></div>
  <button class="primary" on:click={() => (showCreate = true)}>+ 新建寄养卡片</button>
  <button class="ghost" on:click={load}>刷新</button>
</div>

{#if loading}
  <div class="empty">加载中…</div>
{:else if pets.length === 0}
  <div class="card empty">暂无寄养宠物</div>
{:else}
  <div class="grid">
    {#each pets as pet (pet.id)}
      <div class="card pet-card" class:off={pet.status === 'picked_up'}>
        <div class="row">
          <h3>{pet.name}</h3>
          <span class="badge {pet.status === 'boarding' ? 'done' : 'cancelled'}">
            {pet.status === 'boarding' ? '寄养中' : '已接走'}
          </span>
        </div>
        <div class="muted species">{pet.species || '—'}</div>
        <dl>
          <dt>主人</dt><dd>{pet.owner_name}</dd>
          <dt>每餐份量</dt><dd>{pet.food_portion || '—'}</dd>
          <dt>主粮</dt><dd>{pet.food_brand || '—'}</dd>
          <dt>禁忌</dt><dd class="danger-text">{pet.contraindications || '无'}</dd>
          <dt>喂食时刻</dt><dd class="mono">{pet.feeding_times}</dd>
          <dt>寄养期</dt><dd>{pet.boarding_start} ~ {pet.boarding_end}</dd>
        </dl>
        <button class="small primary" on:click={() => (selectedId = pet.id)}>
          查看照护详情 / 用药 / 接走
        </button>
      </div>
    {/each}
  </div>
{/if}

{#if selectedId}
  <PetDetail petId={selectedId} on:close={() => (selectedId = null)} on:changed={load} />
{/if}
{#if showCreate}
  <CreatePetModal
    on:close={() => (showCreate = false)}
    on:created={() => { showCreate = false; load(); }}
  />
{/if}

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(290px, 1fr)); gap: 14px; }
  .pet-card h3 { font-size: 16px; }
  .pet-card.off { opacity: 0.7; }
  .species { font-size: 12.5px; margin: 2px 0 10px; }
  dl { margin: 0 0 12px; display: grid; grid-template-columns: 64px 1fr; gap: 4px 8px; font-size: 13px; }
  dt { color: var(--ink-soft); }
  dd { margin: 0; }
  .danger-text { color: var(--red); }
</style>
