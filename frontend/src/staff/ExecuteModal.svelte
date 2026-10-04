<script>
  import { api, uploadPhoto } from '../lib/api.js';
  import { toast } from '../lib/toast.js';

  export let staff = [];
  export let task;
  export let currentUserId;
  import { createEventDispatcher } from 'svelte';
  const dispatch = createEventDispatcher();

  let notes = '';
  let photoPath = null;
  let photoName = '';
  let uploading = false;
  let submitting = false;

  // 给药核对字段（预填处方值，核对人需肉眼确认后保持一致提交）
  let verifierId = '';
  let verifyDrugName = task.drug_name || '';
  let verifyDosage = task.dosage || '';

  const otherStaff = staff.filter((s) => s.id !== currentUserId);

  async function pickPhoto(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    uploading = true;
    try {
      const res = await uploadPhoto(file);
      photoPath = res.path;
      photoName = file.name;
      toast('照片已保存到本地', 'success');
    } catch (err) {
      toast(err.message, 'error');
    } finally {
      uploading = false;
    }
  }

  async function submit() {
    if (task.kind === 'medication') {
      if (!verifierId) return toast('请选择另一名核对员工', 'error');
      if (!verifyDrugName.trim() || !verifyDosage.trim())
        return toast('核对员工必须填写药名与剂量', 'error');
    }
    submitting = true;
    try {
      const body = { notes, photo_path: photoPath };
      if (task.kind === 'medication') {
        body.verifier_id = verifierId;
        body.verify_drug_name = verifyDrugName.trim();
        body.verify_dosage = verifyDosage.trim();
      }
      await api.post(`/api/tasks/${task.id}/execute`, body);
      toast(task.kind === 'medication' ? '给药已完成并记录双人核对' : '执行已记录', 'success');
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
    <h3>
      {task.kind === 'medication' ? '💊 给药执行 · 双人核对' : '🍚 记录喂食执行'}
    </h3>
    <p class="muted" style="margin-top:-6px">
      宠物：<b>{task.pet_name}</b> · 计划 {task.scheduled_at.replace('T', ' ').slice(0, 16)}
    </p>

    {#if task.kind === 'medication'}
      <div class="rx card-inner">
        <div><span class="muted">处方药名：</span><b>{task.drug_name}</b></div>
        <div><span class="muted">处方剂量：</span><b>{task.dosage}</b></div>
        <div class="warn">⚠️ 必须由<b>另一名员工</b>亲眼核对药品与剂量，执行人不能核对自己的任务。</div>
      </div>
      <div class="field">
        <label>核对员工（须与执行人不同）</label>
        <select bind:value={verifierId}>
          <option value="">请选择核对人…</option>
          {#each otherStaff as s}
            <option value={s.id}>{s.display_name}（{s.username}）</option>
          {/each}
        </select>
      </div>
      <div class="grid2">
        <div class="field">
          <label>核对确认：药名</label>
          <input bind:value={verifyDrugName} placeholder="核对人确认的药名" />
        </div>
        <div class="field">
          <label>核对确认：剂量</label>
          <input bind:value={verifyDosage} placeholder="如 50mg" />
        </div>
      </div>
    {/if}

    <div class="field">
      <label>执行备注（食量/实际情况，可选）</label>
      <textarea bind:value={notes} placeholder="如：150g 全部吃完，状态正常"></textarea>
    </div>

    <div class="field">
      <label>现场照片（可选，保存在本地）</label>
      <input type="file" accept="image/*" on:change={pickPhoto} />
      {#if photoName}<div class="muted small">已上传：{photoName}</div>{/if}
    </div>

    <div class="row" style="justify-content:flex-end">
      <button on:click={() => dispatch('close')}>取消</button>
      <button class="primary" disabled={submitting || uploading} on:click={submit}>
        {submitting ? '提交中…' : task.kind === 'medication' ? '核对无误，完成给药' : '确认执行'}
      </button>
    </div>
  </div>
</div>

<style>
  .card-inner { background: var(--bg); padding: 12px 14px; margin-bottom: 14px; border-radius: 10px; border: 1px solid var(--line); }
  .card-inner > div { margin: 2px 0; }
  .warn { margin-top: 8px; font-size: 12.5px; color: var(--amber); }
  .small { font-size: 12.5px; margin-top: 4px; }
</style>
