<script>
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { api } from '../lib/api.js';
  import { toast } from '../lib/toast.js';
  import { fmtDateTime, fmtTime, STATUS_LABELS, INCIDENT_LABELS } from '../lib/format.js';
  import PickupAuthPanel from './PickupAuthPanel.svelte';

  export let petId;
  const dispatch = createEventDispatcher();

  let detail = null;
  let timeline = null;
  let loading = true;
  let filterKind = '';

  onMount(load);
  async function load() {
    loading = true;
    try {
      const [d, t] = await Promise.all([
        api.get(`/api/pets/${petId}`),
        api.get(`/api/pets/${petId}/timeline`),
      ]);
      detail = d;
      timeline = t;
    } catch (e) {
      toast(e.message, 'error');
    } finally {
      loading = false;
    }
  }

  // 按日期分组
  $: groups = (() => {
    if (!timeline) return [];
    let items = timeline.tasks;
    if (filterKind) items = items.filter((t) => t.kind === filterKind);
    const map = new Map();
    for (const t of items) {
      const day = t.scheduled_at.slice(0, 10);
      if (!map.has(day)) map.set(day, []);
      map.get(day).push(t);
    }
    return [...map.entries()].sort((a, b) => (a[0] < b[0] ? 1 : -1));
  })();

  function onTime(s) {
    // 是否按时：执行时间与计划时间相差 30 分钟内
    if (!s.execution) return false;
    const plan = new Date(s.scheduled_at).getTime();
    const done = new Date(s.execution.executed_at).getTime();
    return Math.abs(done - plan) <= 30 * 60 * 1000;
  }
</script>

{#if loading || !detail}
  <div class="empty">加载中…</div>
{:else}
  <button class="ghost small" on:click={() => dispatch('back')}>← 返回我的宠物</button>

  <div class="card head">
    <div class="row wrap">
      <h2>{detail.pet.name}</h2>
      <span class="badge {detail.pet.status === 'boarding' ? 'done' : 'cancelled'}">
        {detail.pet.status === 'boarding' ? '寄养中' : '已接走'}
      </span>
    </div>
    <div class="grid2 info">
      <div><span class="muted">每餐份量：</span><b>{detail.pet.food_portion || '—'}</b></div>
      <div><span class="muted">主粮：</span>{detail.pet.food_brand || '—'}</div>
      <div class="full"><span class="muted">禁忌：</span><b class="danger-text">{detail.pet.contraindications || '无'}</b></div>
      <div><span class="muted">喂食时刻：</span><span class="mono">{detail.pet.feeding_times}</span></div>
      <div><span class="muted">寄养期：</span>{detail.pet.boarding_start} ~ {detail.pet.boarding_end}</div>
      {#each detail.medications.filter((m) => m.active) as m}
        <div class="full med"><span class="badge kind-med">💊</span> {m.drug_name} · {m.dosage} · <span class="mono">{m.times}</span> {#if m.instructions}<span class="muted">（{m.instructions}）</span>{/if}</div>
      {/each}
    </div>
    <div class="readonly-hint">🔒 本页所有照护记录由门店员工填写，您只能查看，不能修改。</div>
  </div>

  <div class="card stats-card">
    <div class="stat"><span class="num green">{timeline.summary.done}</span><span>已完成</span></div>
    <div class="stat"><span class="num amber">{timeline.summary.pending}</span><span>待执行</span></div>
    <div class="stat"><span class="num red">{timeline.summary.anomaly}</span><span>异常</span></div>
    <div class="stat"><span class="num muted-n">{timeline.summary.cancelled}</span><span>已取消</span></div>
  </div>

  <!-- 异常与处理经过 -->
  <div class="card block">
    <h3>异常与处理经过</h3>
    {#if timeline.incidents.length === 0}
      <div class="muted small">寄养期间暂无拒食、漏服或换粮异常 👍</div>
    {:else}
      {#each timeline.incidents as inc (inc.id)}
        <div class="incident">
          <div class="row wrap">
            <span class="badge anomaly">{INCIDENT_LABELS[inc.incident_type]}</span>
            <b>{inc.staff_name}</b>
            <span class="muted small">{fmtDateTime(inc.created_at)}</span>
          </div>
          <div class="small"><span class="muted">情况：</span>{inc.description}</div>
          <div class="small"><span class="muted">处理经过：</span>{inc.handling}</div>
          {#if inc.photo_path}
            <a href="/uploads/{inc.photo_path}" target="_blank" class="small">查看现场照片</a>
          {/if}
        </div>
      {/each}
    {/if}
  </div>

  <!-- 代接授权 -->
  <PickupAuthPanel petId={petId} petStatus={detail.pet.status} on:changed={load} />

  <!-- 照护日历 -->
  <div class="card block">
    <div class="row">
      <h3>照护日历 · 实际执行记录</h3>
      <div class="spacer"></div>
      <select class="mini" bind:value={filterKind}>
        <option value="">喂食+给药</option>
        <option value="feeding">只看喂食</option>
        <option value="medication">只看给药</option>
      </select>
    </div>

    {#each groups as [day, items]}
      <div class="day">
        <div class="day-title">{day}</div>
        {#each items as t (t.id)}
          <div class="task-row" class:anomaly={t.status === 'anomaly'}>
            <span class="t-time mono">{fmtTime(t.scheduled_at)}</span>
            {#if t.kind === 'feeding'}
              <span class="badge kind-feed">喂食</span>
            {:else}
              <span class="badge kind-med">给药 {t.drug_name} {t.dosage}</span>
            {/if}
            <span class="badge {t.status}">{STATUS_LABELS[t.status]}</span>
            <div class="exec">
              {#if t.execution}
                {fmtDateTime(t.execution.executed_at)}
                {#if onTime(t)}<span class="ok">· 按时</span>{:else}<span class="late">· 与计划时间有偏差</span>{/if}
                · {t.execution.performer_name}
                {#if t.execution.verifier_name} · <span class="verify">双人核对：{t.execution.verifier_name}</span>{/if}
                {#if t.execution.notes} · {t.execution.notes}{/if}
              {:else if t.status === 'cancelled'}
                <span class="muted">已取消</span>
              {:else if t.status === 'anomaly'}
                <span class="muted">见上方异常记录</span>
              {:else}
                <span class="muted">等待门店执行</span>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/each}
  </div>
{/if}

<style>
  .head { margin: 10px 0 14px; }
  .info { margin-top: 10px; }
  .full { grid-column: 1 / -1; }
  .danger-text { color: var(--red); }
  .med { font-size: 13px; }
  .readonly-hint { margin-top: 12px; font-size: 12.5px; color: var(--ink-soft); background: var(--bg); border-radius: 8px; padding: 6px 10px; }
  .stats-card { display: flex; gap: 10px; margin-bottom: 14px; }
  .stat { flex: 1; text-align: center; padding: 12px; background: var(--surface); border: 1px solid var(--line); border-radius: 12px; display: flex; flex-direction: column; gap: 2px; font-size: 12.5px; color: var(--ink-soft); }
  .num { font-size: 24px; font-weight: 700; }
  .green { color: var(--green); } .amber { color: var(--amber); } .red { color: var(--red); } .muted-n { color: var(--ink-soft); }
  .block { margin-bottom: 14px; }
  .block h3 { font-size: 15px; margin-bottom: 10px; }
  .incident { border-left: 3px solid var(--red); padding: 4px 0 8px 10px; margin-bottom: 10px; display: flex; flex-direction: column; gap: 3px; }
  .small { font-size: 12.5px; }
  .day { margin-bottom: 12px; }
  .day-title { font-weight: 650; font-size: 13px; color: var(--ink-soft); border-bottom: 1px solid var(--line); padding-bottom: 4px; margin-bottom: 6px; }
  .task-row { display: flex; gap: 10px; align-items: baseline; flex-wrap: wrap; padding: 5px 0; font-size: 13px; }
  .task-row.anomaly { background: var(--red-soft); border-radius: 6px; padding: 5px 8px; }
  .t-time { width: 48px; font-weight: 600; }
  .exec { color: var(--ink-soft); font-size: 12.5px; }
  .ok { color: var(--green); } .late { color: var(--amber); } .verify { color: #7c4bb0; }
  .mini { width: auto; }
</style>
