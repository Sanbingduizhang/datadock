import { invokeCommand } from './index';

import type { TableDataRequest, TableDataResponse } from '@/types/table';

/** 分页读取表数据（无排序列时按主键升序） */
export function getTableData(request: TableDataRequest): Promise<TableDataResponse> {
  return invokeCommand<TableDataResponse>('get_table_data', { request });
}
