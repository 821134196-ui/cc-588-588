<script>
  import { currentUser, clearSession } from './lib/session.js';
  import Login from './Login.svelte';
  import Toasts from './Toasts.svelte';
  import StaffView from './staff/StaffView.svelte';
  import OwnerView from './owner/OwnerView.svelte';

  function logout() {
    clearSession();
  }
</script>

<Toasts />

{#if !$currentUser}
  <Login />
{:else}
  <div class="shell">
    <header class="topbar">
      <div class="brand">🐾 宠物寄养照护交接</div>
      <div class="spacer"></div>
      <span class="muted">
        {$currentUser.role === 'staff' ? '门店员工' : '宠物主人'} · {$currentUser.display_name}
      </span>
      <button class="small" on:click={logout}>退出登录</button>
    </header>
    <main class="content">
      {#if $currentUser.role === 'staff'}
        <StaffView />
      {:else}
        <OwnerView />
      {/if}
    </main>
  </div>
{/if}

<style>
  .shell { min-height: 100vh; }
  .topbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 22px;
    background: var(--surface);
    border-bottom: 1px solid var(--line);
    position: sticky;
    top: 0;
    z-index: 20;
  }
  .brand { font-weight: 700; font-size: 16px; color: var(--brand-dark); }
  .spacer { flex: 1; }
  .content { max-width: 1100px; margin: 0 auto; padding: 20px 16px 60px; }
</style>
