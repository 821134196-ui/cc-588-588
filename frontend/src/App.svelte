<script>
  import { onMount } from 'svelte';
  import { api, saveSession, loadUser, clearSession, getToken } from './lib/api.js';
  import StaffView from './components/StaffView.svelte';
  import OwnerView from './components/OwnerView.svelte';

  let user = loadUser();
  let tab = user?.role === 'staff' ? 'staff' : 'owner';
  let username = '';
  let password = '';
  let loginErr = '';
  let busy = false;

  onMount(async () => {
    // 已有令牌则校验一次
    if (getToken() && user) {
      try {
        const r = await api.get('/me');
        user = r.user;
      } catch {
        doLogout();
      }
    }
  });

  async function login() {
    loginErr = '';
    busy = true;
    try {
      const r = await api.post('/auth/login', { username: username.trim(), password });
      saveSession(r.token, r.user);
      user = r.user;
      tab = r.user.role === 'staff' ? 'staff' : 'owner';
    } catch (e) {
      loginErr = e.message;
    } finally {
      busy = false;
    }
  }

  async function doLogout() {
    try { await api.post('/auth/logout'); } catch {}
    clearSession();
    user = null;
  }

  function quickFill(u, p) { username = u; password = p; }
</script>

{#if !user}
  <div class="login-wrap">
    <div class="brand"><span class="paw">🐾</span>萌宠寄养交接</div>
    <div class="card">
      <h2 style="text-align:center">登录</h2>
      <div class="field"><label>账号</label><input bind:value={username} placeholder="alice / bob / wang / li" on:keydown={(e) => e.key === 'Enter' && login()} /></div>
      <div class="field"><label>密码</label><input type="password" bind:value={password} placeholder="staff123 / owner123" on:keydown={(e) => e.key === 'Enter' && login()} /></div>
      {#if loginErr}<div class="notice error">{loginErr}</div>{/if}
      <button style="width:100%" disabled={busy} on:click={login}>{busy ? '登录中…' : '登录'}</button>
      <div class="login-hint">
        演示账号（点击自动填充）：<br />
        员工：<a href="#" on:click|preventDefault={() => quickFill('alice', 'staff123')}>alice / staff123</a>
        、<a href="#" on:click|preventDefault={() => quickFill('bob', 'staff123')}>bob / staff123</a><br />
        主人：<a href="#" on:click|preventDefault={() => quickFill('wang', 'owner123')}>wang / owner123</a>（咪咪）
        、<a href="#" on:click|preventDefault={() => quickFill('li', 'owner123')}>li / owner123</a>（豆豆）
      </div>
    </div>
  </div>
{:else}
  <div class="app">
    <div class="topbar">
      <div class="brand"><span class="paw">🐾</span>萌宠寄养照护交接系统</div>
      <div class="userbox">
        <span>{user.display_name}（{user.role === 'staff' ? '门店员工' : '宠物主人'}）</span>
        <button class="ghost small" on:click={doLogout}>退出</button>
      </div>
    </div>

    {#if user.role === 'staff'}
      <StaffView />
    {:else}
      <OwnerView />
    {/if}
  </div>
{/if}
