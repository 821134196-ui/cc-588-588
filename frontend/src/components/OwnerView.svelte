<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';
  import { today, PET_STATUS_LABEL, fmtTime } from '../lib/format.js';
  import PetCard from './PetCard.svelte';
  import TaskList from './TaskList.svelte';
  import IncidentList from './IncidentList.svelte';

  let pets = [];
  let selectedId = null;
  let detail = null;
  let tasks = [];
  let incidents = [];
  let codes = [];
  let notifications = [];
  let date = today();
  let err = '';
  let ok = '';

  let authName = '';
  let newCode = null;

  onMount(loadAll);

  async function loadAll() {
    try {
      pets = await api.get('/pets');
      notifications = await api.get('/notifications');
      if (!selectedId && pets.length) await selectPet(pets[0].id);
      else if (selectedId) await selectPet(selectedId);
    } catch (e) { err = e.message; }
  }

  async function selectPet(id) {
    selectedId = id;
    try {
      const [d, t, inc, c] = await Promise.all([
        api.get(`/pets/${id}`),
        api.get(`/tasks?pet_id=${id}&date=${date}`),
        api.get(`/incidents?pet_id=${id}`),
        api.get(`/pickup-codes?pet_id=${id}`),
      ]);
      detail = d; tasks = t.tasks; incidents = inc; codes = c;
    } catch (e) { err = e.message; }
  }

  async function changeDate() {
    const t = await api.get(`/tasks?pet_id=${selectedId}&date=${date}`);
    tasks = t.tasks;
  }

  async function createCode() {
    err = ''; ok = '';
    if (!authName.trim()) { err = '请填写代接人姓名'; return; }
    try {
      const r = await api.post('/pickup-codes', { pet_id: selectedId, authorized_name: authName });
      newCode = r;
      ok = '代接授权已创建；撤销后此码立即失效';
      authName = '';
      await selectPet(selectedId);
    } catch (e) { err = e.message; }
  }

  async function revoke(c) {
    err = ''; ok = '';
    if (!confirm(`确定撤销给「${c.authorized_name}」的授权？旧取宠码将立即失效，无法再凭旧码接走宠物。`)) return;
    try {
      const r = await api.post(`/pickup-codes/${c.id}/revoke`);
      ok = `已撤销，旧码 ${r.invalidated_code} 立即失效`;
      newCode = null;
      await selectPet(selectedId);
    } catch (e) { err = e.message; }
  }

  async function markRead(n) {
    await api.post(`/notifications/${n.id}/read`);
    await loadAll();
  }

  const STATUS_CN = { active: '有效', revoked: '已撤销（旧码失效）', redeemed: '已使用' };
</script>

<div class="read-only-hint">
  🔒 主人端为只读视图：您可以查看宠物卡片、照护日历的实际执行记录与异常处理，
  门店填写的内容不可修改。代接授权可由您创建或撤销。
</div>

<div class="grid2" style="grid-template-columns: 240px 1fr; align-items:start">
  <div>
    <div class="card">
      <h3>我的宠物</h3>
      {#each pets as pet}
        <div class="pet-row {pet.id === selectedId ? 'selected' : ''}" on:click={() => selectPet(pet.id)} role="button">
          <div>
            <strong>{pet.name}</strong>
            <div class="muted">{pet.species} · {PET_STATUS_LABEL[pet.status]}</div>
          </div>
        </div>
      {/each}
      {#if pets.length === 0}<p class="muted">暂无寄养宠物</p>{/if}
    </div>

    <div class="card">
      <h3>🔔 通知{notifications.filter(n => !n.is_read).length ? `（${notifications.filter(n => !n.is_read).length} 未读）` : ''}</h3>
      {#each notifications.slice(0, 8) as n}
        <div class="incident {n.is_read ? 'other' : ''}" style="padding:8px 10px">
          <strong style="font-size:13px">{n.title}</strong>
          <div class="muted" style="font-size:12px; white-space:pre-wrap; margin:4px 0">{n.body}</div>
          <div class="row-inline" style="justify-content:space-between">
            <span class="muted" style="font-size:11px">{fmtTime(n.created_at)}</span>
            {#if !n.is_read}<button class="ghost small" on:click={() => markRead(n)}>标为已读</button>{/if}
          </div>
        </div>
      {/each}
      {#if notifications.length === 0}<p class="muted">暂无通知</p>{/if}
    </div>
  </div>

  <div>
    {#if err}<div class="notice error">{err}</div>{/if}
    {#if ok}<div class="notice ok">{ok}</div>{/if}

    {#if newCode}
      <div class="card" style="border:2px solid var(--green)">
        <h3 style="margin-top:0">✅ 代接授权已创建</h3>
        <p>代接人：<strong>{newCode.authorized_name}</strong></p>
        <p>取宠码（请转交代接人，到店报此 8 位码）：</p>
        <p><span class="code-chip">{newCode.code}</span></p>
        <p class="muted">如需取消代接，请在下方点「撤销授权」，旧码会立即失效。</p>
      </div>
    {/if}

    {#if detail}
      <PetCard {detail} />

      <div class="card">
        <h3>🎫 代接授权管理</h3>
        {#if detail.pet.status === 'boarding'}
          <div class="row-inline">
            <input bind:value={authName} placeholder="被授权代接人姓名" style="max-width:220px" />
            <button class="small" on:click={createCode}>生成取宠码</button>
          </div>
        {:else}
          <p class="muted">宠物已接走。</p>
        {/if}
        <hr class="sep" />
        {#each codes as c}
          <div class="task-item" style="margin-bottom:8px">
            <div class="row-inline" style="justify-content:space-between">
              <div>
                <strong>{c.authorized_name}</strong>
                <span class="muted"> · {fmtTime(c.created_at)} 创建 · 状态：{STATUS_CN[c.status]}</span>
              </div>
              {#if c.status === 'active'}
                <button class="danger small" on:click={() => revoke(c)}>撤销授权（旧码立即失效）</button>
              {/if}
            </div>
            <div style="margin-top:6px">取宠码：<span class="code-chip" style="font-size:15px; letter-spacing:3px">{c.code}</span></div>
          </div>
        {/each}
        {#if codes.length === 0}<p class="muted">暂无代接授权</p>{/if}
      </div>

      <div class="card">
        <div class="row-inline" style="justify-content:space-between">
          <h3 style="margin:0">🗓 照护日历（实际执行记录）</h3>
          <div class="row-inline">
            <input type="date" bind:value={date} on:change={changeDate} />
          </div>
        </div>
        <hr class="sep" />
        <TaskList {tasks} isStaff={false} />
      </div>

      <IncidentList {incidents} />
    {/if}
  </div>
</div>
