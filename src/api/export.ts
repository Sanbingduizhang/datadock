import { invokeCommand } from './index';

import type {
  DumpDatabaseRequest,
  DumpDatabaseResponse,
  ExportQueryRequest,
  ExportResponse,
  ExportTableRequest,
  ImportDatabaseRequest,
  ImportDatabaseResponse,
} from '@/types/export';

/**
 * 导出表数据到本地文件。
 * 文件由 Rust 侧写入（分批读取、流式落盘），前端只负责选择路径。
 */
export function exportTableData(request: ExportTableRequest): Promise<ExportResponse> {
  return invokeCommand<ExportResponse>('export_table_data', { request });
}

/** 导出只读查询的结果（会重新执行该 SQL，仅允许 SELECT/SHOW/WITH 等） */
export function exportQueryResult(request: ExportQueryRequest): Promise<ExportResponse> {
  return invokeCommand<ExportResponse>('export_query_result', { request });
}

/** 导出数据库（结构 / 数据 / 结构+数据）为 SQL 脚本 */
export function dumpDatabase(request: DumpDatabaseRequest): Promise<DumpDatabaseResponse> {
  return invokeCommand<DumpDatabaseResponse>('dump_database', { request });
}

/** 从 SQL 脚本导入到目标库（整体一个事务，失败回滚） */
export function importDatabase(request: ImportDatabaseRequest): Promise<ImportDatabaseResponse> {
  return invokeCommand<ImportDatabaseResponse>('import_database', { request });
}
