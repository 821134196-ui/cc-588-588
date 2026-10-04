<script>
  import { api } from './lib/api.js';
  import { setSession } from './lib/session.js';
  import { toast } from './lib/toast.js';

  let username = '';
  let password = '';
  let loading = false;

  const demo = [
    { role: '员工', u: 'alice', p: 'staff123', name: '王护工' },
    { role: '员工', u: 'bob', p: 'staff123', name: '陈护工（给药核对）' },
    { role: '主人', u: 'lisi', p: 'owner123', name: '李小姐 · 金毛豆豆' },
    { role: '主人', u: 'zhangsan', p: 'owner123', name: '张先生 · 英短咪咪' },
  ];

  async function submit() {
    if (!username || !password) return toast('请输入用户名和密码', 'error');
    loading = true;
    try {
      const res = await api.post('/api/auth/login', { username, password });
      setSession(res.token, res.user);
      toast('登录成功', 'success');
    } catch (e) {
      toast(e.message, 'error');
    } finally {
      loading = false;
    }
  }

  function fill(u, p) {
    username = u;
    password = p;
  }
</script>

<div class="login-page">
  <div class="login-card card">
    <h1 style="font-size:22px">🐾 宠物寄养喂食与用药交接</h1>
    <p class="muted" style="margin:6px 0 20px">门店照护执行 · 主人随时查看 · 给药双人核对</p>

    <form on:submit|preventDefault={submit}>
      <div class="field">
        <label>用户名</label>
        <input bind:value={username} placeholder="如 alice" autocomplete="username" />
      </div>
      <div class="field">
        <label>密码</label>
        <input bind:value={password} type="password" placeholder="演示密码见下方" autocomplete="current-password" />
      </div>
      <button class="primary" style="width:100%" disabled={loading}>
        {loading ? '登录中…' : '登 录'}
      </button>
    </form>

    <div class="demo">
      <div class="muted" style="font-size:12.5px;margin-bottom:6px">演示账号（点击填充）：</div>
      {#each demo as d}
        <button class="small demo-btn" on:click={() => fill(d.u, d.p)}>
          {d.role} · {d.name}
        </button>
      {/each}
    </div>
  </div>
</div>

<style>
  .login-page {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }
  .login-card { width: 100%; max-width: 420px; }
  .demo { margin-top: 18px; border-top: 1px dashed var(--line); padding-top: 12px; }
  .demo-btn { margin: 0 6px 6px 0; }
</style>
