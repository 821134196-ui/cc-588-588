import { writable } from 'svelte/store';

export const toasts = writable([]);

export function toast(message, type = 'info', timeout = 3200) {
  const id = Math.random().toString(36).slice(2);
  toasts.update((list) => [...list, { id, message, type }]);
  setTimeout(() => {
    toasts.update((list) => list.filter((t) => t.id !== id));
  }, timeout);
}
