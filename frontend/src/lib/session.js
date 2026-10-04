import { writable } from 'svelte/store';

const stored = JSON.parse(localStorage.getItem('petcare_session') || 'null');
export const token = writable(stored?.token || '');
export const currentUser = writable(stored?.user || null);

export function setSession(t, user) {
  token.set(t);
  currentUser.set(user);
  localStorage.setItem('petcare_session', JSON.stringify({ token: t, user }));
}

export function clearSession() {
  token.set('');
  currentUser.set(null);
  localStorage.removeItem('petcare_session');
}
