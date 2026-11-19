/** 通用格式化工具 */

/** 相对时间：刚刚 / n 分钟前 / n 小时前 / MM-DD HH:mm */
export function formatRelativeTime(timestamp: number): string {
  const diff = Date.now() - timestamp;
  if (diff < 60_000) {
    return '刚刚';
  }
  if (diff < 3_600_000) {
    return `${Math.floor(diff / 60_000)} 分钟前`;
  }
  if (diff < 86_400_000) {
    return `${Math.floor(diff / 3_600_000)} 小时前`;
  }

  const date = new Date(timestamp);
  const pad = (value: number) => value.toString().padStart(2, '0');
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** 单行压缩 SQL 预览 */
export function compactSql(sql: string, limit = 140): string {
  const compact = sql.split(/\s+/).join(' ').trim();
  return compact.length > limit ? `${compact.slice(0, limit)}…` : compact;
}
