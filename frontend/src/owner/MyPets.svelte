<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';
  import { toast } from '../lib/toast.js';
  import PetRecord from './PetRecord.svelte';

  let pets = [];
  let loading = true;
  let selectedId = null;

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

{#if loading}
  <div class="empty">加载中…</div>
{:else if pets.length === 0}
  <div class="card empty">您还没有宠物在门店寄养</div>
{:else if selectedId}
  <PetRecord petId={selectedId} on:back={() => (selectedId = null)} />
{:else}
  <div class="grid">
    {#each pets as pet (pet.id)}
      <div class="card pet">
        <div class="row">
          <h3>{pet.name}</h3>
          <span class="badge {pet.status === 'boarding' ? 'done' : 'cancelled'}">
            {pet.status === 'boarding' ? '寄养中' : '已接走'}
          </span>
        </div>
        <div class="muted small">{pet.species}</div>
        <dl>
          <dt>每餐份量</dt><dd>{pet.food_portion || '—'}</dd>
          <dt>主粮</dt><dd>{pet.food_brand || '—'}</dd>
          <dt>禁忌</dt><dd class="danger-text">{pet.contraindications || '无'}</dd>
          <dt>寄养期</dt><dd>{pet.boarding_start} ~ {pet.boarding_end}</dd>
        </dl>
        <button class="primary small" on:click={() => (selectedId = pet.id)}>
          查看吃饭用药记录
        </button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 14px; }
  dl { margin: 8px 0 12px; display: grid; grid-template-columns: 64px 1fr; gap: 4px 8px; font-size: 13px; }
  dt { color: var(--ink-soft); }
  dd { margin: 0; }
  .danger-text { color: var(--red); }
</style>
