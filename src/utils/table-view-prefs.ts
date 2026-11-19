/** 表视图偏好（列顺序/显隐/列宽）的本地持久化 */

export interface TableViewPrefs {
  version: number;
  order: string[];
  hidden: string[];
  widths: Record<string, number>;
}

const KEY_PREFIX = 'datadock.tableView.';
const VERSION = 1;

export function tableViewScope(
  connectionId: string,
  databaseName: string,
  tableName: string,
): string {
  return `${connectionId}|${databaseName}|${tableName}`;
}

function storageKey(scope: string): string {
  return `${KEY_PREFIX}${scope}`;
}

export function loadTableViewPrefs(scope: string): TableViewPrefs | null {
  try {
    const raw = localStorage.getItem(storageKey(scope));
    if (!raw) {
      return null;
    }

    const parsed = JSON.parse(raw) as Partial<TableViewPrefs>;
    if (!parsed || parsed.version !== VERSION) {
      return null;
    }

    return {
      version: VERSION,
      order: Array.isArray(parsed.order) ? parsed.order.filter((item) => typeof item === 'string') : [],
      hidden: Array.isArray(parsed.hidden)
        ? parsed.hidden.filter((item) => typeof item === 'string')
        : [],
      widths:
        parsed.widths && typeof parsed.widths === 'object'
          ? Object.fromEntries(
              Object.entries(parsed.widths).filter(
                ([, value]) => typeof value === 'number' && Number.isFinite(value),
              ),
            )
          : {},
    };
  } catch {
    return null;
  }
}

export function saveTableViewPrefs(scope: string, prefs: Omit<TableViewPrefs, 'version'>): void {
  try {
    const payload: TableViewPrefs = { version: VERSION, ...prefs };
    localStorage.setItem(storageKey(scope), JSON.stringify(payload));
  } catch {
    // 存储不可用（隐私模式/配额满）时静默失败，仅影响偏好记忆
  }
}
