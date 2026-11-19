import { invokeCommand } from './index';

import type { ReadSqlFileResponse, WriteSqlFileResponse } from '@/types/file';

/** 读取本地 SQL 文件（UTF-8，上限 5 MB） */
export function readSqlFile(path: string): Promise<ReadSqlFileResponse> {
  return invokeCommand<ReadSqlFileResponse>('read_sql_file', { request: { path } });
}

/** 将 SQL 写入本地文件（覆盖写入） */
export function writeSqlFile(path: string, content: string): Promise<WriteSqlFileResponse> {
  return invokeCommand<WriteSqlFileResponse>('write_sql_file', {
    request: { path, content },
  });
}
