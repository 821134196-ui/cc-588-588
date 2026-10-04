<script>
  import { createEventDispatcher } from 'svelte';
  import { api } from '../lib/api.js';
  import { KIND_LABEL, STATUS_LABEL, fmtTime } from '../lib/format.js';

  export let task;
  export let isStaff = false;

  const dispatch = createEventDispatcher();

  let note = '';
  let photo = null;
  let verifier_username = '';
  let verifier_password = '';
  let busy = false;
  let err = '';
  let ok = '';

  function pickFile(e) {
    photo = e.target.files?.[0] || null;
  }

  async function execute() {
    err = ''; ok = '';
    if (task.kind === 'medication') {
      if (!verifier_username || !verifier_password) {
        err = '给药任务必须由另一名员工输入账号密码，核对药名与剂量后才能完成';
        return;
      }
    }
    busy = true;
    try {
      if (photo) {
        const fd = new FormData();
        fd.append('note', note || '');
        if (task.kind === 'medication') {
          fd.append('verifier_username', verifier_username);
          fd.append('verifier_password', verifier_password);
        }
        fd.append('photo', photo);
        await api.postForm(`/tasks/${task.id}/execute-photo`, fd);
      } else {
        const body = { note: note || '' };
        if (task.kind === 'medication') {
          body.verifier_username = verifier_username;
          body.verifier_password = verifier_password;
        }
        await api.post(`/tasks/${task.id}/execute`, body);
      }
      ok = '已记录执行';
      dispatch('refresh');
    } catch (e) {
      err = e.status === 409 ? `已拦截重复提交：${e.message}` : e.message;
    } finally {
      busy = false;
    }
  }

  async function cancel() {
    err = '';
    if (!confirm('确认取消该任务？')) return;
    try {
      await api.post(`/tasks/${task.id}/cancel`);
      dispatch('refresh');
    } catch (e) {
      err = e.message;
    }
  }
</script>

<div class="task-item">
  <div class="task-head">
    <span class="task-time">🕐 {fmtTime(task.scheduled_at)}</span>
    <span class="badge {task.kind}">{KIND_LABEL[task.kind]}</span>
    <span class="badge {task.status}">{STATUS_LABEL[task.status]}</span>
    {#if task.medication}
      <span class="muted">💊 {task.medication.name} · {task.medication.dosage}</span>
    {/if}
  </div>

  {#if task.kind === 'feed'}
    <div class="muted">计划：{task.food_brand || '—'} · {task.portion || '—'} {task.note ? `· ${task.note}` : ''}</div>
  {:else}
    <div class="muted">{task.note || '给药任务'}{task.medication?.instructions ? ` · ${task.medication.instructions}` : ''}</div>
  {/if}

  {#if task.execution}
    <div class="exec-box">
      ✅ 实际执行：{fmtTime(task.execution.executed_at)} 由 {task.execution.executor_name} 记录
      {#if task.execution.note}<br />备注：{task.execution.note}{/if}
      {#if task.execution.photo_path}
        <br /><a href={task.execution.photo_path} target="_blank"><img class="photo-thumb" src={task.execution.photo_path} alt="执行照片" /></a>
      {/if}
      {#if task.kind === 'medication'}
        <div class="verifier-box">
          🔎 双人核对：{task.execution.verifier_name} 于 {fmtTime(task.execution.verified_at)}
          核对药名「{task.execution.verifier_med_name}」、剂量「{task.execution.verifier_dosage}」无误
        </div>
      {/if}
    </div>
  {:else if task.status === 'scheduled' && isStaff}
    <div class="card" style="margin:10px 0 0; padding:12px 14px; box-shadow:none">
      {#if task.kind === 'medication'}
        <div class="notice info" style="margin-top:0">
          💊 给药双人核对：完成前必须由<b>另一名员工</b>输入其账号密码，核对药名与剂量（不能由执行本人核对）
        </div>
        <div class="grid2">
          <div class="field"><label>核对员工号</label><input placeholder="如 bob" bind:value={verifier_username} /></div>
          <div class="field"><label>核对人密码</label><input type="password" placeholder="核对人登录密码" bind:value={verifier_password} /></div>
        </div>
      {/if}
      <div class="field">
        <label>执行备注（可选；拒食/漏服/换粮请走「上报异常」，不能只勾完成）</label>
        <textarea bind:value={note} placeholder="正常情况可留空；异常请点下方“上报异常”"></textarea>
      </div>
      <div class="row-inline">
        <label class="ghost small" style="padding:7px 14px; border:1px solid var(--border); border-radius:8px; cursor:pointer">
          📷 附照片（可选）
          <input type="file" accept="image/*" style="display:none" on:change={pickFile} />
        </label>
        {#if photo}<span class="muted">{photo.name}</span>{/if}
        <button class="small" disabled={busy} on:click={execute}>
          {busy ? '提交中…' : '确认执行并记录'}
        </button>
        <button class="ghost small" on:click={cancel}>取消任务</button>
        <button class="danger small" on:click={() => dispatch('reportincident', task)}>⚠ 上报异常</button>
      </div>
    </div>
  {/if}

  {#if err}<div class="notice error">{err}</div>{/if}
  {#if ok}<div class="notice ok">{ok}</div>{/if}
</div>
