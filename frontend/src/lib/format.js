export const KIND_LABEL = { feed: '喂食', medication: '给药' };
export const STATUS_LABEL = { scheduled: '待执行', done: '已完成', cancelled: '已取消' };
export const INCIDENT_LABEL = {
  refused_food: '拒食',
  missed_medication: '漏服药物',
  food_change: '临时换粮',
  other: '其他异常',
};
export const PET_STATUS_LABEL = { boarding: '寄养中', picked_up: '已接走' };

export function fmtTime(s) {
  if (!s) return '—';
  return s.replace('T', ' ').slice(0, 16);
}
export function today() {
  // 后端 scheduled_at 用 "YYYY-MM-DD HH:MM" 本地时间，这里取本地日期
  const d = new Date();
  const p = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}
