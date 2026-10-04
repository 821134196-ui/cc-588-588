<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';
  import { toast } from '../lib/toast.js';
  import { fmtDateTime } from '../lib/format.js';

  let notifications = [];
  let unread = 0;
  let loading = true;

  const KIND_ICON = {
    refused_food: '🍽️',
    missed_medication: '💊',
    food_swap: '🥣',
    other: '⚠️',
    checkout: '🏠',
    pickup_redeemed: '🔑',
    pickup_revoked: '🚫',
  };

  onMount(load);
  async function load() {
    loading = true;
    try {
      const res = await api.get('/api/notifications');
      notifications = res.notifications;
      unread = res.unread;
    } catch (e) {
      toast(e.message, 'error');
    } finally {
      loading = false;
    }
  }

  async function readAll() {
    await api.post('/api/notifications/read-all');
    load();
  }

  async function open(n) {
    if (!n.read_at) {
      await api.post(`/api/notifications/${n.id}/read`);
      load();
    }
  }
</script>

<div class="row wrap" style="margin-bottom:12px">
  <h2 style="font-size:17px">消息通知（本地模拟）</h2>
  {#if unread > 0}<span class="badge anomaly">{unread} 条未读</span>{/if}
  <div class="spacer"></div>
  <button class="small" on:click={readAll}>全部标为已读</button>
  <button class="ghost small" on:click={load}>刷新</button>
</div>

{#if loading}
  <div class="empty">加载中…</div>
{:else if notifications.length === 0}
  <div class="card empty">暂无消息。拒食、漏服、换粮或接走等情况门店会第一时间在这里通知您。</div>
{:else}
  <div class="list">
    {#each notifications as n (n.id)}
      <button class="card note" class:unread={!n.read_at} on:click={() => open(n)}>
        <div class="row">
          <span class="ico">{KIND_ICON[n.kind] || '🔔'}</span>
          <b>{n.title}</b>
          <div class="spacer"></div>
          <span class="muted small">{fmtDateTime(n.created_at)}</span>
          {#if !n.read_at}<span class="dot"></span>{/if}
        </div>
        <div class="body">{n.body}</div>
      </button>
    {/each}
  </div>
{/if}

<style>
  .list { display: flex; flex-direction: column; gap: 10px; }
  .note { text-align: left; width: 100%; font: inherit; color: inherit; }
  .note.unread { border-left: 4px solid var(--brand); background: #fffaf7; }
  .ico { font-size: 18px; }
  .body { margin-top: 4px; font-size: 13px; color: var(--ink-soft); }
  .small { font-size: 12px; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--brand); }
</style>
