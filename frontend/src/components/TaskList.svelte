<script>
  import { createEventDispatcher } from 'svelte';
  import TaskItem from './TaskItem.svelte';

  export let tasks = [];
  export let isStaff = false;

  const dispatch = createEventDispatcher();
</script>

{#each tasks as task (task.id)}
  <TaskItem {task} {isStaff}
             on:refresh={() => dispatch('refresh')}
             on:reportincident={(e) => dispatch('reportIncident', e.detail)} />
{/each}

{#if tasks.length === 0}
  <p class="muted">当日无照护任务</p>
{/if}
