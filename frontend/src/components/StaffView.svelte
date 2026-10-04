<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';
  import { today, KIND_LABEL, PET_STATUS_LABEL } from '../lib/format.js';
  import PetCard from './PetCard.svelte';
  import TaskList from './TaskList.svelte';
  import IncidentForm from './IncidentForm.svelte';
  import IncidentList from './IncidentList.svelte';

  let pets = [];
  let owners = [];
  let selectedId = null;
  let detail = null;
  let tasks = [];
  let incidents = [];
  let date = today();
  let globalErr = '';
  let globalOk = '';

  // 登记宠物
  let np = { name: '', species: '猫', owner_id: '', food_brand: '', food_portion: '', meals_per_day: 2, contraindications: '', notes: '' };
  // 加药
  let med = { name: '', dosage: '', instructions: '' };
  // 排任务
  let nt = { kind: 'feed', med_id: '', scheduled_date: today(), scheduled_time: '08:00', food_brand: '', portion: '', note: '' };
  // 异常
  let incidentTask = null;
  let showIncident = false;
  // 核销
  let redeemCode = '';
  let redeemMsg = '';

  onMount(loadAll);

  async function loadAll() {
    try {
      const [p, o] = await Promise.all([api.get('/pets'), api.get('/users')]);
      pets = p;
      owners = o;
      if (!selectedId && pets.length) await selectPet(pets[0].id);
      else if (selectedId) await selectPet(selectedId);
    } catch (e) {
      globalErr = e.message;
    }
  }

  async function selectPet(id) {
    selectedId = id;
    incidentTask = null;
    showIncident = false;
    await refreshDetail();
  }

  async function refreshDetail() {
    if (!selectedId) return;
    try {
      const [d, t, inc] = await Promise.all([
        api.get(`/pets/${selectedId}`),
        api.get(`/tasks?pet_id=${selectedId}&date=${date}`),
        api.get(`/incidents?pet_id=${selectedId}`),
      ]);
      detail = d;
      tasks = t.tasks;
      incidents = inc;
      np.food_brand = np.food_brand || d.pet.food_brand;
      np.food_portion = np.food_portion || d.pet.food_portion;
    } catch (e) {
      globalErr = e.message;
    }
  }

  async function registerPet() {
    globalErr = ''; globalOk = '';
    try {
      const pet = await api.post('/pets', { ...np, owner_id: Number(np.owner_id) });
      globalOk = `已登记「${pet.name}」`;
      np = { name: '', species: '猫', owner_id: '', food_brand: '', food_portion: '', meals_per_day: 2, contraindications: '', notes: '' };
      await loadAll();
      selectPet(pet.id);
    } catch (e) { globalErr = e.message; }
  }

  async function addMedication() {
    globalErr = ''; globalOk = '';
    try {
      await api.post(`/pets/${selectedId}/medications`, med);
      globalOk = '药品已加入宠物卡片';
      med = { name: '', dosage: '', instructions: '' };
      await refreshDetail();
    } catch (e) { globalErr = e.message; }
  }

  async function scheduleTask() {
    globalErr = ''; globalOk = '';
    try {
      await api.post('/tasks', {
        pet_id: selectedId,
        kind: nt.kind,
        med_id: nt.kind === 'medication' ? Number(nt.med_id) : null,
        scheduled_at: `${nt.scheduled_date} ${nt.scheduled_time}`,
        food_brand: nt.food_brand,
        portion: nt.portion,
        note: nt.note,
      });
      globalOk = '照护任务已排入日历';
      nt.note = '';
      await refreshDetail();
    } catch (e) { globalErr = e.message; }
  }

  async function checkout() {
    globalErr = ''; globalOk = '';
    if (!confirm('确认主人提前接走？将取消该宠物后续全部待执行任务，并作废全部代接码。')) return;
    try {
      const r = await api.post(`/pets/${selectedId}/checkout`);
      globalOk = `已办理接走，取消后续任务 ${r.cancelled_tasks} 个`;
      await loadAll();
    } catch (e) { globalErr = e.message; }
  }

  async function doRedeem() {
    redeemMsg = '';
    try {
      const r = await api.post('/pickup/redeem', { code: redeemCode });
      redeemMsg = `ok: 放宠成功 —— ${r.pet_name} 已交给代接人「${r.authorized_name}」，后续任务已取消`;
      redeemCode = '';
      await loadAll();
    } catch (e) {
      redeemMsg = `blocked(${e.status}): ${e.message}`;
    }
  }

  function reportIncident(e) {
    incidentTask = e.detail;
    showIncident = true;
  }
</script>

<div class="grid2" style="grid-template-columns: 280px 1fr; align-items:start">
  <div>
    <div class="card">
      <h3>在店宠物</h3>
      {#each pets as pet}
        <div class="pet-row {pet.id === selectedId ? 'selected' : ''}" on:click={() => selectPet(pet.id)} role="button">
          <div>
            <strong>{pet.name}</strong>
            <div class="muted">{pet.species} · {PET_STATUS_LABEL[pet.status]}</div>
          </div>
        </div>
      {/each}
    </div>

    <div class="card">
      <h3>🆕 登记宠物</h3>
      <div class="field"><label>名字</label><input bind:value={np.name} /></div>
      <div class="field"><label>种类</label><input bind:value={np.species} placeholder="猫 / 狗" /></div>
      <div class="field"><label>主人</label>
        <select bind:value={np.owner_id}>
          <option value="">请选择主人</option>
          {#each owners as o}<option value={o.id}>{o.display_name}（{o.username}）</option>{/each}
        </select>
      </div>
      <div class="field"><label>主粮</label><input bind:value={np.food_brand} /></div>
      <div class="field"><label>每餐份量</label><input bind:value={np.food_portion} placeholder="40g" /></div>
      <div class="field"><label>每日餐数</label><input type="number" min="1" max="8" bind:value={np.meals_per_day} /></div>
      <div class="field"><label>禁忌</label><textarea bind:value={np.contraindications} /></div>
      <div class="field"><label>备注</label><textarea bind:value={np.notes} /></div>
      <button class="small" on:click={registerPet}>登记</button>
    </div>
  </div>

  <div>
    {#if globalErr}<div class="notice error">{globalErr}</div>{/if}
    {#if globalOk}<div class="notice ok">{globalOk}</div>{/if}

    <div class="card">
      <h3>🎫 代接取宠核销</h3>
      <div class="row-inline">
        <input bind:value={redeemCode} placeholder="请代接人报出 8 位取宠码" style="max-width:220px" />
        <button class="small" on:click={doRedeem}>核验放宠</button>
      </div>
      {#if redeemMsg}
        <div class="notice {redeemMsg.startsWith('ok') ? 'ok' : 'error'}" style="white-space:pre-wrap">{redeemMsg}</div>
      {/if}
    </div>

    {#if detail}
      <PetCard {detail} />

      {#if detail.pet.status === 'boarding'}
        <div class="card">
          <h3>➕ 维护用药卡片</h3>
          <div class="grid2">
            <div class="field"><label>药名</label><input bind:value={med.name} /></div>
            <div class="field"><label>剂量</label><input bind:value={med.dosage} placeholder="5mg / 半片" /></div>
            <div class="field" style="grid-column:1/-1"><label>服用说明</label><input bind:value={med.instructions} /></div>
          </div>
          <button class="small" on:click={addMedication}>加入药品</button>
        </div>

        <div class="card">
          <h3>📅 安排照护任务</h3>
          <div class="grid2">
            <div class="field"><label>类型</label>
              <select bind:value={nt.kind}>
                <option value="feed">喂食</option>
                <option value="medication">给药</option>
              </select>
            </div>
            {#if nt.kind === 'medication'}
              <div class="field"><label>药品</label>
                <select bind:value={nt.med_id}>
                  <option value="">请选择药品</option>
                  {#each detail.medications as m}<option value={m.id}>{m.name}（{m.dosage}）</option>{/each}
                </select>
              </div>
            {/if}
            <div class="field"><label>日期</label><input type="date" bind:value={nt.scheduled_date} /></div>
            <div class="field"><label>时间</label><input type="time" bind:value={nt.scheduled_time} /></div>
            {#if nt.kind === 'feed'}
              <div class="field"><label>粮</label><input bind:value={nt.food_brand} placeholder={detail.pet.food_brand} /></div>
              <div class="field"><label>份量</label><input bind:value={nt.portion} placeholder={detail.pet.food_portion} /></div>
            {/if}
            <div class="field" style="grid-column:1/-1"><label>备注</label><input bind:value={nt.note} /></div>
          </div>
          <button class="small" on:click={scheduleTask}>排入日历</button>
        </div>
      {/if}

      <div class="card">
        <div class="row-inline" style="justify-content:space-between">
          <h3 style="margin:0">🗓 照护日历执行情况</h3>
          <div class="row-inline">
            <input type="date" bind:value={date} />
            <button class="ghost small" on:click={refreshDetail}>查看</button>
            {#if detail.pet.status === 'boarding'}
              <button class="danger small" on:click={checkout}>主人提前接走（取消后续任务）</button>
            {/if}
          </div>
        </div>
        <hr class="sep" />
        <TaskList {tasks} isStaff={true} on:refresh={refreshDetail} on:reportIncident={reportIncident} />
      </div>

      {#if showIncident}
        <IncidentForm petId={selectedId} task={incidentTask}
                      on:done={() => { showIncident = false; refreshDetail(); }}
                      on:cancel={() => (showIncident = false)} />
      {/if}

      <IncidentList {incidents} />
    {/if}
  </div>
</div>
