<script>
  import { createEventDispatcher } from 'svelte';
  import { KIND_LABEL, fmtTime } from '../lib/format.js';
  import { api } from '../lib/api.js';

  export let petId;
  export let task = null; // 关联任务（可选）

  const dispatch = createEventDispatcher();
  let incidentType = 'refused_food';
  let description = '';
  let handling = '';
  let err = '';
  let busy = false;

  const types = [
    ['refused_food', '拒食'],
    ['missed_medication', '漏服药物'],
    ['food_change', '临时换粮'],
    ['other', '其他异常'],
  ];

  async function submit() {
    err = '';
    if (!description.trim()) return (err = '请描述异常情况');
    if (!handling.trim()) return (err = '必须填写处理经过，不能将异常直接勾选完成');
    busy = true;
    try {
      await api.post('/incidents', {
        pet_id: petId,
        task_id: task?.id ?? null,
        incident_type: incidentType,
        description,
        handling,
      });
      description = '';
      handling = '';
      dispatch('done');
    } catch (e) {
      err = e.message;
    } finally {
      busy = false;
    }
  }
</script>

<div class="card">
  <h3>⚠ 异常处理记录（拒食 / 漏服 / 临时换粮）</h3>
  <p class="muted" style="margin-top:-6px">
    异常不能通过勾选「完成」掩盖，必须单独记录处理经过并通知主人。
    {#if task}关联任务：{fmtTime(task.scheduled_at)} {KIND_LABEL[task.kind]}{/if}
  </p>
  <div class="field">
    <label>异常类型</label>
    <select bind:value={incidentType}>
      {#each types as [v, l]}<option value={v}>{l}</option>{/each}
    </select>
  </div>
  <div class="field"><label>情况说明</label><textarea bind:value={description} placeholder="例如：早餐闻了一下就走开，未进食"></textarea></div>
  <div class="field"><label>处理经过（必填，将通知主人）</label><textarea bind:value={handling} placeholder="例如：30 分钟后少量多餐补喂，吃掉 20g，已持续观察精神状态正常"></textarea></div>
  {#if err}<div class="notice error">{err}</div>{/if}
  <div class="row-inline">
    <button class="small" disabled={busy} on:click={submit}>{busy ? '提交中…' : '提交并通知主人'}</button>
    <button class="ghost small" on:click={() => dispatch('cancel')}>取消</button>
  </div>
</div>
