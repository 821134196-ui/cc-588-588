<script>
  import { createEventDispatcher } from 'svelte';
  import { api, uploadPhoto } from '../lib/api.js';
  import { toast } from '../lib/toast.js';
  import { INCIDENT_LABELS } from '../lib/format.js';

  export let task;
  const dispatch = createEventDispatcher();

  let incidentType = task.kind === 'medication' ? 'missed_medication' : 'refused_food';
  let description = '';
  let handling = '';
  let photoPath = null;
  let photoName = '';
  let uploading = false;
  let submitting = false;

  const types = ['refused_food', 'missed_medication', 'food_swap', 'other'];

  async function pickPhoto(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    uploading = true;
    try {
      const res = await uploadPhoto(file);
      photoPath = res.path;
      photoName = file.name;
    } catch (err) {
      toast(err.message, 'error');
    } finally {
      uploading = false;
    }
  }

  async function submit() {
    if (!description.trim() || !handling.trim()) {
      return toast('异常情况与处理经过都必须填写，不能直接勾选完成', 'error');
    }
    submitting = true;
    try {
      await api.post(`/api/tasks/${task.id}/anomaly`, {
        incident_type: incidentType,
        description: description.trim(),
        handling: handling.trim(),
        photo_path: photoPath,
      });
      toast('异常已记录，并已通知主人', 'success');
      dispatch('done');
    } catch (e) {
      toast(e.message, 'error');
    } finally {
      submitting = false;
    }
  }
</script>

<div class="modal-backdrop" on:click|self={() => dispatch('close')}>
  <div class="modal">
    <h3>⚠️ 异常情况上报</h3>
    <p class="muted" style="margin-top:-6px">
      {task.pet_name} · {task.kind === 'medication' ? `给药 ${task.drug_name || ''}` : '喂食'}
    </p>

    <div class="field">
      <label>异常类型</label>
      <select bind:value={incidentType}>
        {#each types as t}
          <option value={t}>{INCIDENT_LABELS[t]}</option>
        {/each}
      </select>
    </div>

    <div class="field">
      <label>异常情况（发生了什么）</label>
      <textarea
        bind:value={description}
        placeholder="如：早餐闻了一下就走开，完全没吃；精神略显萎靡"
      ></textarea>
    </div>

    <div class="field">
      <label>处理经过（门店采取的措施）<span class="req">*</span></label>
      <textarea
        bind:value={handling}
        placeholder="如：30 分钟后少量多次投喂，吃下约 80g；电话告知主人并持续观察"
      ></textarea>
    </div>

    <div class="field">
      <label>现场照片（可选）</label>
      <input type="file" accept="image/*" on:change={pickPhoto} />
      {#if photoName}<div class="muted small">已上传：{photoName}</div>{/if}
    </div>

    <div class="notice">
      异常任务<b>不会</b>被标记为“已完成”，而是单独留档并立即通知主人。
    </div>

    <div class="row" style="justify-content:flex-end">
      <button on:click={() => dispatch('close')}>取消</button>
      <button class="primary" disabled={submitting || uploading} on:click={submit}>
        {submitting ? '提交中…' : '记录异常并通知主人'}
      </button>
    </div>
  </div>
</div>

<style>
  .req { color: var(--red); }
  .notice {
    background: var(--amber-soft);
    color: var(--amber);
    border-radius: 8px;
    padding: 8px 12px;
    font-size: 12.5px;
    margin-bottom: 14px;
  }
  .small { font-size: 12.5px; margin-top: 4px; }
</style>
