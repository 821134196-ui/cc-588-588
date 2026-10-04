import { get } from 'svelte/store';
import { token, clearSession } from './session.js';

async function request(method, path, body) {
  const t = get(token);
  const headers = {};
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  if (t) headers['Authorization'] = `Bearer ${t}`;

  const resp = await fetch(path, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });

  let data = null;
  const text = await resp.text();
  if (text) {
    try {
      data = JSON.parse(text);
    } catch {
      data = { raw: text };
    }
  }

  if (!resp.ok) {
    if (resp.status === 401) clearSession();
    const err = new Error(data?.message || `请求失败 (${resp.status})`);
    err.status = resp.status;
    err.code = data?.error;
    throw err;
  }
  return data;
}

export const api = {
  get: (p) => request('GET', p),
  post: (p, b) => request('POST', p, b ?? {}),
  patch: (p, b) => request('PATCH', p, b),
  del: (p) => request('DELETE', p),
};

export async function uploadPhoto(file, onProgress) {
  const t = get(token);
  const form = new FormData();
  form.append('file', file);
  // 本地模拟：简单 XHR 以便扩展进度
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open('POST', '/api/uploads');
    xhr.setRequestHeader('Authorization', `Bearer ${t}`);
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        resolve(JSON.parse(xhr.responseText));
      } else {
        let msg = '上传失败';
        try {
          msg = JSON.parse(xhr.responseText).message || msg;
        } catch { /* ignore */ }
        reject(new Error(msg));
      }
    };
    xhr.onerror = () => reject(new Error('上传网络错误'));
    if (onProgress) xhr.upload.onprogress = (e) => e.lengthComputable && onProgress(e.loaded / e.total);
    xhr.send(form);
  });
}
