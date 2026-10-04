<script>
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { api } from '../lib/api.js';
  import { toast } from '../lib/toast.js';
  import { fmtDateTime } from '../lib/format.js';

  export let petId;
  export let petStatus;
  const dispatch = createEventDispatcher();

  let auths = [];
  let showCreate = false;
  let proxyName = '';
  let proxyContact = '';
  let newCode = null; // 仅创建时展示一次
  let submitting = false;

  onMount(load);
  async function load() {
    try {
      const res = await api.get(`/api/pets/${petId}/pickup-authorizations`);
      auths = res.authorizations;
    } catch (e) {
      toast(e.message, 'error');
    }
  }

  async function create() {
    if (!proxyName.trim()) return toast('请填写代接人姓名', 'error');
    submitting = true;
    try {
      const res = await api.post(`/api/pets/${petId}/pickup-authorizations`, {
        proxy_name: proxyName.trim(),
        proxy_contact: proxyContact.trim(),
      });
      newCode = { code: res.pickup_code, proxy: proxyName.trim() };
      proxyName = '';
      proxyContact = '';
      showCreate = false;
      load();
      dispatch('changed');
    } catch (e) {
      toast(e.message, 'error');
    } finally {
      submitting = false;
    }
  }

  async function revoke(a) {
    const reason = prompt(`撤销「${a.proxy_name}」的代接授权？\n撤销后旧取宠码会立即失效，请填写原因（可留空）：`, '');
    if (reason === null) return;
    try {
      await api.post(`/api/pickup-authorizations/${a.id}/revoke`, { reason: reason || undefined });
      toast('已撤销，旧取宠码立即失效', 'success');
      load();
      dispatch('changed');
    } catch (e) {
      toast(e.message, 'error');
    }
  }

  const STATUS = { active: '有效', revoked: '已撤销', used: '已使用' };
</script>

<div class="card block">
  <div class="row">
    <h3>代接授权 / 取宠码</h3>
    <div class="spacer"></div>
    {#if petStatus === 'boarding'}
      <button class="small primary" on:click={() => (showCreate = !showCreate)}>
        {showCreate ? '收起' : '+ 授权他人代接'}
      </button>
    {/if}
  </div>

  {#if petStatus !== 'boarding'}
    <div class="muted small">宠物已接走，不能再创建代接授权。</div>
  {/if}

  {#if showCreate}
    <div class="create-box">
      <div class="grid2">
        <div class="field"><label>代接人姓名</label><input bind:value={proxyName} placeholder="如：王阿姨" /></div>
        <div class="field"><label>联系方式（可选）</label><input bind:value={proxyContact} placeholder="手机号" /></div>
      </div>
      <div class="row" style="justify-content:flex-end">
        <button on:click={() => (showCreate = false)}>取消</button>
        <button class="primary" disabled={submitting} on:click={create}>
          {submitting ? '生成中…' : '生成取宠码'}
        </button>
      </div>
    </div>
  {/if}

  {#if newCode}
    <div class="code-box">
      <div>代接人 <b>{newCode.proxy}</b> 的一次性取宠码：</div>
      <div class="code mono">{newCode.code}</div>
      <div class="muted small">
        请通过可信渠道把取宠码交给代接人。此码<b>仅显示一次</b>；一旦您撤销授权，旧码立即失效，无法再领取宠物。
      </div>
      <button class="small" on:click={() => (newCode = null)}>我已保存</button>
    </div>
  {/if}

  {#if auths.length === 0}
    <div class="muted small">尚未设置代接人；本人到店接宠无需取宠码。</div>
  {:else}
    {#each auths as a (a.id)}
      <div class="auth-row">
        <div>
          <b>{a.proxy_name}</b>
          {#if a.proxy_contact}<span class="muted small"> · {a.proxy_contact}</span>{/if}
          <span class="badge {a.status}" style="margin-left:6px">{STATUS[a.status]}</span>
          {#if a.status === 'revoked' && a.revoke_reason}
            <span class="muted small">（{a.revoke_reason}）</span>
          {/if}
          {#if a.status === 'used'}
            <span class="muted small">· {a.staff_name || ''} 于 {fmtDateTime(a.used_at)} 核销</span>
          {/if}
        </div>
        <div class="muted small">{fmtDateTime(a.created_at)}</div>
        {#if a.status === 'active'}
          <button class="small danger" on:click={() => revoke(a)}>撤销（旧码立即失效）</button>
        {/if}
      </div>
    {/each}
  {/if}
</div>

<style>
  .block h3 { font-size: 15px; margin: 0; }
  .create-box { background: var(--bg); border-radius: 10px; padding: 12px; margin: 10px 0; }
  .code-box { background: var(--green-soft); border: 1px dashed var(--green); border-radius: 10px; padding: 12px 14px; margin: 10px 0; }
  .code { font-size: 30px; font-weight: 700; letter-spacing: 8px; color: var(--brand-dark); margin: 6px 0; }
  .small { font-size: 12.5px; }
  .auth-row { display: flex; gap: 12px; align-items: center; padding: 8px 0; border-bottom: 1px dashed var(--line); flex-wrap: wrap; }
</style>
