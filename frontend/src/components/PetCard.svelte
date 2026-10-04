<script>
  export let detail; // { pet, medications }
  import { PET_STATUS_LABEL } from '../lib/format.js';
</script>

<div class="card">
  <div class="task-head" style="justify-content:space-between">
    <h2 style="margin:0">🐾 {detail.pet.name}（{detail.pet.species}）</h2>
    <span class="badge {detail.pet.status}">{PET_STATUS_LABEL[detail.pet.status]}</span>
  </div>
  <dl class="kv" style="margin-top:12px">
    <dt>当前主粮</dt><dd><strong>{detail.pet.food_brand || '—'}</strong></dd>
    <dt>每餐份量</dt><dd><strong>{detail.pet.food_portion || '—'}</strong>（每日 {detail.pet.meals_per_day} 餐）</dd>
    <dt>禁忌</dt>
    <dd>
      {#if detail.pet.contraindications}
        <span class="danger-note" style="display:inline-block">⚠ {detail.pet.contraindications}</span>
      {:else}—{/if}
    </dd>
    <dt>备注</dt><dd>{detail.pet.notes || '—'}</dd>
  </dl>

  <h3>💊 用药卡片</h3>
  {#if detail.medications.length === 0}
    <p class="muted">无在用药物</p>
  {:else}
    {#each detail.medications as med}
      <div class="task-item" style="margin-bottom:8px">
        <strong>{med.name}</strong>
        <span class="muted"> · 剂量 {med.dosage}</span>
        {#if med.instructions}<div class="muted" style="margin-top:4px">{med.instructions}</div>{/if}
      </div>
    {/each}
  {/if}
</div>
