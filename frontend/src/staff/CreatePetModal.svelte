<script>
  import { onMount, createEventDispatcher } from 'svelte';
  import { api } from '../lib/api.js';
  import { toast } from '../lib/toast.js';

  const dispatch = createEventDispatcher();
  let owners = [];
  let submitting = false;

  const today = new Date().toISOString().slice(0, 10);
  const endDefault = new Date(Date.now() + 5 * 864e5).toISOString().slice(0, 10);

  let form = {
    owner_id: '',
    name: '',
    species: '',
    food_portion: '',
    food_brand: '',
    contraindications: '',
    feeding_times: '08:00,18:00',
    notes: '',
    boarding_start: today,
    boarding_end: endDefault,
  };

  onMount(async () => {
    try {
      const res = await api.get('/api/owners');
      owners = res.owners;
      if (owners[0]) form.owner_id = owners[0].id;
    } catch (e) {
      toast(e.message, 'error');
    }
  });

  async function submit() {
    if (!form.name.trim()) return toast('请填写宠物名字', 'error');
    if (!form.owner_id) return toast('请选择主人', 'error');
    submitting = true;
    try {
      await api.post('/api/pets', form);
      toast('卡片已创建，照护日历已自动生成', 'success');
      dispatch('created');
    } catch (e) {
      toast(e.message, 'error');
    } finally {
      submitting = false;
    }
  }
</script>

<div class="modal-backdrop" on:click|self={() => dispatch('close')}>
  <div class="modal">
    <h3>新建寄养宠物卡片</h3>
    <div class="grid2">
      <div class="field">
        <label>主人</label>
        <select bind:value={form.owner_id}>
          {#each owners as o}
            <option value={o.id}>{o.display_name}（{o.username}）</option>
          {/each}
        </select>
      </div>
      <div class="field"><label>宠物名字</label><input bind:value={form.name} placeholder="豆豆" /></div>
      <div class="field"><label>品种 / 年龄</label><input bind:value={form.species} placeholder="金毛 · 3岁" /></div>
      <div class="field"><label>每餐份量</label><input bind:value={form.food_portion} placeholder="150g/餐，温水泡软" /></div>
      <div class="field"><label>主粮</label><input bind:value={form.food_brand} placeholder="品牌/换粮比例" /></div>
      <div class="field"><label>禁忌</label><input bind:value={form.contraindications} placeholder="巧克力、葡萄…" /></div>
      <div class="field"><label>每日喂食时刻</label><input bind:value={form.feeding_times} placeholder="08:00,12:30,18:30" /></div>
      <div class="field"><label>寄养开始</label><input type="date" bind:value={form.boarding_start} /></div>
      <div class="field"><label>寄养结束</label><input type="date" bind:value={form.boarding_end} /></div>
    </div>
    <div class="field">
      <label>照护备注</label>
      <textarea bind:value={form.notes} placeholder="性格、喂药技巧等"></textarea>
    </div>
    <div class="muted small" style="margin-bottom:12px">
      保存后系统会按喂食时刻与用药自动排好整个寄养期的照护日历。
    </div>
    <div class="row" style="justify-content:flex-end">
      <button on:click={() => dispatch('close')}>取消</button>
      <button class="primary" disabled={submitting} on:click={submit}>{submitting ? '保存中…' : '创建卡片'}</button>
    </div>
  </div>
</div>
