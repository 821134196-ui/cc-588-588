export function fmtDateTime(iso) {
  if (!iso) return '—';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

export function fmtTime(iso) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (n) => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}`;
}

export const INCIDENT_LABELS = {
  refused_food: '拒食',
  missed_medication: '漏服药物',
  food_swap: '临时换粮',
  other: '其他异常',
};

export const STATUS_LABELS = {
  pending: '待执行',
  done: '已完成',
  anomaly: '异常',
  cancelled: '已取消',
};
