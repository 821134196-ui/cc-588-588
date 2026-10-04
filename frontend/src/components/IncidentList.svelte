<script>
  import { INCIDENT_LABEL, fmtTime } from '../lib/format.js';
  export let incidents = [];
</script>

<div class="card">
  <h3>📕 异常处理台账</h3>
  {#if incidents.length === 0}
    <p class="muted">暂无异常记录 🎉</p>
  {:else}
    {#each incidents as row}
      <div class="incident {row.incident.incident_type === 'food_change' ? 'food_change' : row.incident.incident_type === 'other' ? 'other' : ''}">
        <strong>{INCIDENT_LABEL[row.incident.incident_type]}</strong>
        <span class="muted"> · {fmtTime(row.incident.created_at)} · 记录人 {row.reporter_name}</span>
        <div style="margin-top:6px">情况：{row.incident.description}</div>
        <div style="margin-top:4px"><strong>处理经过：</strong>{row.incident.handling}</div>
        <div class="muted" style="margin-top:4px">🔔 已通知宠物主人</div>
      </div>
    {/each}
  {/if}
</div>
