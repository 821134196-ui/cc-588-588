<script>
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { api } from '../lib/api.js';
  import { toast } from '../lib/toast.js';
  import { fmtDateTime, fmtTime, STATUS_LABELS, INCIDENT_LABELS } from '../lib/format.js';

  export let petId;
  const dispatch = createEventDispatcher();

  let detail = null;
  let timeline = null;
  let auths = [];
  let loading = true;

  let showMed = false;
  let med = { drug_name: '', dosage: '', instructions: '', times: '08:00,20:00' };

  let checkingOut = false;

  onMount(load);
  async function load() {
    loading = true;
    try {
      const [d, t, a] = await Promise.all([
        api.get(`/api/pets/${petId}`),
        api.get(`/api/pets/${petId}/timeline`),
        api.get(`/api/pets/${petId}/pickup-authorizations`),
      ]);
      detail = d;
      timeline = t;
      auths = a.authorizations;
    } catch (e) {
      toast(e.message, 'error');
    } finally {
      loading = false;
    }
  }

  async function addMed() {
    if (!med.drug_name.trim() || !med.dosage.trim() || !med.times.trim())
      return toast('药名、剂量和给药时刻都要填', 'error');
    try {
      await api.post(`/api/pets/${petId}/medications`, med);
      toast('用药已添加，照护日历已更新', 'success');
      showMed = false;
      med = { drug_name: '', dosage: '', instructions: '', times: '08:00,20:00' };
      load();
    } catch (e) {
      toast(e.message, 'error');
    }
  }

  async function stopMed(id) {
    if (!confirm('停用该药物？未执行的给药任务将取消，历史记录保留。')) return;
    try {
      await api.del(`/api/pets/${petId}/medications/${id}`);
      toast('药物已停用', 'success');
      load();
    } catch (e) {
      toast(e.message, 'error');
    }
  }

  async function checkout() {
    if (!confirm('确认主人提前接走？所有尚未执行的喂食/给药任务将立即取消，已完成记录保留。')) return;
    checkingOut = true;
    try {
      const res = await api.post(`/api/pets/${petId}/checkout`, {});
      toast(`已办理接走，取消 ${res.cancelled_tasks} 项后续任务，并通知主人`, 'success');
      dispatch('changed');
      load();
    } catch (e) {
      toast(e.message, 'error');
    } finally {
      checkingOut = false;
    }
  }

  const AUTH_STATUS = { active: '有效', revoked: '已撤销', used: '已使用' };
</script>

<div class="modal-backdrop" on:click|self={() => dispatch('close')}>
  <div class="modal big">
    {#if loading || !detail}
      <div class="empty">加载中…</div>
    {:else}
      <div class="row">
        <h2>{detail.pet.name}</h2>
        <span class="badge {detail.pet.status === 'boarding' ? 'done' : 'cancelled'}">
          {detail.pet.status === 'boarding' ? '寄养中' : '已接走'}
        </span>
        <div class="spacer"></div>
        {#if detail.pet.status === 'boarding'}
          <button class="danger small" disabled={checkingOut} on:click={checkout}>
            主人提前接走
          </button>
        {/if}
        <button class="small" on:click={() => dispatch('close')}>关闭</button>
      </div>

      <div class="card-inner">
        <div class="grid2">
          <div><span class="muted">品种：</span>{detail.pet.species || '—'}</div>
          <div><span class="muted">主人：</span>{detail.pet.owner_name}</div>
          <div><span class="muted">每餐份量：</span><b>{detail.pet.food_portion || '—'}</b></div>
          <div><span class="muted">主粮：</span>{detail.pet.food_brand || '—'}</div>
          <div class="full"><span class="muted">禁忌：</span><b class="danger-text">{detail.pet.contraindications || '无'}</b></div>
          <div><span class="muted">喂食时刻：</span><span class="mono">{detail.pet.feeding_times}</span></div>
          <div><span class="muted">寄养期：</span>{detail.pet.boarding_start} ~ {detail.pet.boarding_end}</div>
          {#if detail.pet.notes}<div class="full"><span class="muted">备注：</span>{detail.pet.notes}</div>{/if}
        </div>
      </div>

      <!-- 用药 -->
      <div class="section">
        <div class="row">
          <h3>用药</h3>
          <div class="spacer"></div>
          <button class="small" on:click={() => (showMed = !showMed)}>
            {showMed ? '收起' : '+ 添加用药'}
          </button>
        </div>
        {#if showMed}
          <div class="card-inner">
            <div class="grid2">
              <div class="field"><label>药名</label><input bind:value={med.drug_name} placeholder="阿莫西林" /></div>
              <div class="field"><label>剂量</label><input bind:value={med.dosage} placeholder="50mg" /></div>
              <div class="field"><label>每日给药时刻</label><input bind:value={med.times} placeholder="08:30,19:00" /></div>
              <div class="field"><label>服用说明</label><input bind:value={med.instructions} placeholder="餐后服用" /></div>
            </div>
            <div class="row" style="justify-content:flex-end">
              <button on:click={() => (showMed = false)}>取消</button>
              <button class="primary" on:click={addMed}>保存并生成给药任务</button>
            </div>
          </div>
        {/if}
        {#if detail.medications.length === 0}
          <div class="muted small">暂无用药</div>
        {:else}
          {#each detail.medications as m (m.id)}
            <div class="med-row">
              <div>
                <b>{m.drug_name}</b> · {m.dosage}
                <span class="muted small">（{m.times}）{m.instructions ? '· ' + m.instructions : ''}</span>
              </div>
              {#if m.active}
                <button class="small danger" on:click={() => stopMed(m.id)}>停用</button>
              {:else}
                <span class="badge cancelled">已停用</span>
              {/if}
            </div>
          {/each}
        {/if}
      </div>

      <!-- 汇总 -->
      <div class="section">
        <h3>照护汇总</h3>
        <div class="stats">
          <div class="stat"><span class="num amber">{timeline.summary.pending}</span><span>待执行</span></div>
          <div class="stat"><span class="num green">{timeline.summary.done}</span><span>已完成</span></div>
          <div class="stat"><span class="num red">{timeline.summary.anomaly}</span><span>异常</span></div>
          <div class="stat"><span class="num muted-n">{timeline.summary.cancelled}</span><span>已取消</span></div>
        </div>
      </div>

      <!-- 异常记录 -->
      {#if timeline.incidents.length > 0}
        <div class="section">
          <h3>异常与处理经过</h3>
          {#each timeline.incidents as inc (inc.id)}
            <div class="incident">
              <div class="row wrap">
                <span class="badge anomaly">{INCIDENT_LABELS[inc.incident_type]}</span>
                <b>{inc.staff_name}</b>
                <span class="muted small">{fmtDateTime(inc.created_at)}</span>
              </div>
              <div class="small"><span class="muted">情况：</span>{inc.description}</div>
              <div class="small"><span class="muted">处理：</span>{inc.handling}</div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- 代接授权状态（员工只读） -->
      <div class="section">
        <h3>代接授权</h3>
        {#if auths.length === 0}
          <div class="muted small">主人尚未授权代接人；请到「取宠核销」页扫码/输码办理。</div>
        {:else}
          {#each auths as a (a.id)}
            <div class="med-row">
              <div>
                {a.proxy_name}
                {#if a.proxy_contact}<span class="muted small"> · {a.proxy_contact}</span>{/if}
                <span class="badge {a.status}" style="margin-left:6px">{AUTH_STATUS[a.status]}</span>
              </div>
              <span class="muted small">{fmtDateTime(a.created_at)}</span>
            </div>
          {/each}
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .modal.big { max-width: 720px; }
  .card-inner { background: var(--bg); border: 1px solid var(--line); border-radius: 10px; padding: 12px 14px; margin: 12px 0; }
  .full { grid-column: 1 / -1; }
  .danger-text { color: var(--red); }
  .section { margin-top: 18px; }
  .section h3 { font-size: 15px; margin-bottom: 8px; }
  .med-row, .incident {
    display: flex; justify-content: space-between; align-items: center; gap: 10px;
    padding: 8px 2px; border-bottom: 1px dashed var(--line);
  }
  .incident { flex-direction: column; align-items: stretch; gap: 3px; }
  .small { font-size: 12.5px; }
  .stats { display: flex; gap: 10px; }
  .stat {
    flex: 1; text-align: center; background: var(--bg); border-radius: 10px; padding: 10px;
    display: flex; flex-direction: column; gap: 2px; font-size: 12.5px; color: var(--ink-soft);
  }
  .num { font-size: 22px; font-weight: 700; }
  .amber { color: var(--amber); } .green { color: var(--green); } .red { color: var(--red); } .muted-n { color: var(--ink-soft); }
</style>
