<script>
  import { api } from '../lib/api.js';
  import { toast } from '../lib/toast.js';

  let code = '';
  let result = null;
  let submitting = false;

  async function redeem() {
    if (!code.trim()) return toast('请输入 8 位取宠码', 'error');
    submitting = true;
    result = null;
    try {
      const res = await api.post('/api/pickup/redeem', { code: code.trim() });
      result = { ok: true, ...res };
      toast(`核销成功：${res.pet_name} 已交给 ${res.proxy_name}`, 'success');
      code = '';
    } catch (e) {
      // 重点提示：撤销/已用/错误码都会被明确拦截
      result = { ok: false, message: e.message, code: e.code };
      toast(e.message, 'error');
    } finally {
      submitting = false;
    }
  }
</script>

<div class="card redeem">
  <h2 style="font-size:17px">取宠码核销</h2>
  <p class="muted">代接人到店后，请其出示主人下发的 8 位取宠码。授权被撤销或已使用的码会被立即拦截。</p>
  <form class="row" on:submit|preventDefault={redeem}>
    <input
      class="code-input mono"
      bind:value={code}
      placeholder="8 位取宠码"
      maxlength="8"
      inputmode="numeric"
    />
    <button class="primary" disabled={submitting}>{submitting ? '核销中…' : '核销放宠'}</button>
  </form>

  {#if result}
    <div class="result {result.ok ? 'ok' : 'fail'}">
      {#if result.ok}
        ✅ 核销成功：<b>{result.pet_name}</b> 已由代接人 <b>{result.proxy_name}</b> 接走；
        已取消 {result.cancelled_tasks} 项后续照护任务，并通知主人。
      {:else}
        🚫 核销被拦截：{result.message}
        {#if result.code}<span class="muted">（{result.code}）</span>{/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .redeem { max-width: 560px; }
  .code-input { font-size: 22px; letter-spacing: 6px; text-align: center; }
  .result { margin-top: 14px; padding: 10px 14px; border-radius: 10px; font-size: 13.5px; }
  .result.ok { background: var(--green-soft); color: var(--green); }
  .result.fail { background: var(--red-soft); color: var(--red); }
</style>
