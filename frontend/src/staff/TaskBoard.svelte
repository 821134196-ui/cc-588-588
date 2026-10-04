<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';
  import { toast } from '../lib/toast.js';
  import { currentUser } from '../lib/session.js';
  import { fmtTime, fmtDateTime, STATUS_LABELS, INCIDENT_LABELS } from '../lib/format.js';
  import ExecuteModal from './ExecuteModal.svelte';
  import AnomalyModal from './AnomalyModal.svelte';

  let tasks = [];
  let pets = [];
  let staff = [];
  let loading = false;

  let filterStatus = 'pending';
  let filterKind = '';
  let filterPet = '';
  const today = new Date().toISOString().slice(0, 10);
  let filterDate = '';

  onMount(load);

  async function load() {
    loading = true;
    try {
      const params = new URLSearchParams();
      if (filterStatus) params.set('status', filterStatus);
      if (filterKind) params.set('kind', filterKind);
      if (filterPet) params.set('pet_id', filterPet);
      if (filterDate) params.set('date', filterDate);
      const [t, p, s] = await Promise.all([
        api.get(`/api/tasks?${params}`),
        api.get('/api/pets'),
        api.get('/api/staff'),
      ]);
      tasks = t.tasks;
      pets = p.pets;
      staff = s.staff;
    } catch (e) {
      toast(e.message, 'error');
    } finally {
      loading = false;
    }
  }

  // 执行 / 异常弹窗
  let execTask = null;
  let anomalyTask = null;

  const INCIDENT_TYPES = ['refused_food', 'missed_medication', 'food_swap', 'other'];
</script>

<div class="filters card">
  <div class="row wrap">
    <div>
      <label>状态</label>
      <select bind:value={filterStatus} on:change={load}>
        <option value="pending">待执行</option>
        <option value="done">已完成</option>
        <option value="anomaly">异常</option>
        <option value="cancelled">已取消</option>
        <option value="">全部</option>
      </select>
    </div>
    <div>
      <label>类型</label>
      <select bind:value={filterKind} on:change={load}>
        <option value="">全部</option>
        <option value="feeding">喂食</option>
        <option value="medication">给药</option>
      </select>
    </div>
    <div>
      <label>宠物</label>
      <select bind:value={filterPet} on:change={load}>
        <option value="">全部</option>
        {#each pets as pet}
          <option value={pet.id}>{pet.name}</option>
        {/each}
      </select>
    </div>
    <div>
      <label>日期</label>
      <input type="date" bind:value={filterDate} on:change={load} />
    </div>
    <button on:click={() => (filterDate = today, load())}>今天</button>
    <div class="spacer"></div>
    <button class="ghost" on:click={load}>刷新</button>
  </div>
  <div class="muted hint">
    提示：今天 = {today}；默认只看待执行任务。给药任务完成时必须由<b>另一名员工</b>核对药名与剂量。
  </div>
</div>

{#if loading}
  <div class="empty">加载中…</div>
{:else if tasks.length === 0}
  <div class="card empty">
    {filterStatus === 'pending' ? '当前筛选下没有待执行任务 🎉' : '没有符合条件的任务'}
  </div>
{:else}
  <div class="task-list">
    {#each tasks as task (task.id)}
      <div class="card task" class:danger={task.status === 'anomaly'}>
        <div class="time-col">
          <div class="time">{fmtTime(task.scheduled_at)}</div>
          <div class="muted date">{fmtDateTime(task.scheduled_at).slice(0, 10)}</div>
        </div>
        <div class="main-col">
          <div class="row wrap">
            <strong>{task.pet_name}</strong>
            {#if task.kind === 'feeding'}
              <span class="badge kind-feed">🍚 喂食</span>
            {:else}
              <span class="badge kind-med">💊 给药</span>
            {/if}
            <span class="badge {task.status}">{STATUS_LABELS[task.status]}</span>
            {#if task.kind === 'medication'}
              <span class="med-info mono">{task.drug_name} · {task.dosage}</span>
            {/if}
          </div>

          {#if task.execution}
            <div class="exec muted">
              实际执行：{fmtDateTime(task.execution.executed_at)} ·
              执行人 <b>{task.execution.performer_name}</b>
              {#if task.verifier_name || task.execution.verifier_name}
                · 核对人 <b class="verify">{task.execution.verifier_name}</b>
              {/if}
              {#if task.execution.notes}· 备注：{task.execution.notes}{/if}
            </div>
          {:else if task.status === 'cancelled'}
            <div class="exec muted">任务已取消（宠物接走或用药停用）</div>
          {/if}
        </div>

        <div class="actions">
          {#if task.status === 'pending'}
            <button class="primary small" on:click={() => (execTask = task)}>
              {task.kind === 'medication' ? '核对并完成' : '记录执行'}
            </button>
            <button class="danger small" on:click={() => (anomalyTask = task)}>异常上报</button>
          {:else if task.status === 'anomaly'}
            <span class="muted small">已转异常流程</span>
          {/if}
        </div>
      </div>
    {/each}
  </div>
{/if}

{#if execTask}
  <ExecuteModal
    {staff}
    task={execTask}
    currentUserId={$currentUser.id}
    on:close={() => (execTask = null)}
    on:done={() => { execTask = null; load(); }}
  />
{/if}

{#if anomalyTask}
  <AnomalyModal
    task={anomalyTask}
    on:close={() => (anomalyTask = null)}
    on:done={() => { anomalyTask = null; load(); }}
  />
{/if}

<style>
  .filters { margin-bottom: 14px; }
  .filters select, .filters input { width: auto; min-width: 130px; }
  .hint { margin-top: 10px; font-size: 12.5px; }
  .task-list { display: flex; flex-direction: column; gap: 10px; }
  .task { display: flex; gap: 16px; align-items: center; padding: 12px 16px; }
  .task.danger { border-color: #ecc4be; background: #fff8f7; }
  .time-col { min-width: 64px; text-align: center; }
  .time { font-size: 18px; font-weight: 700; }
  .date { font-size: 11.5px; }
  .main-col { flex: 1; }
  .med-info { font-size: 12.5px; background: #f6f0fb; padding: 1px 8px; border-radius: 6px; }
  .exec { margin-top: 6px; font-size: 12.5px; }
  .verify { color: #7c4bb0; }
  .actions { display: flex; flex-direction: column; gap: 6px; }
</style>
