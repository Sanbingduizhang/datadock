/** SQL 文件读写类型，与 src-tauri/src/models/file.rs 一致 */

export interface ReadSqlFileResponse {
  path: string;
  name: string;
  content: string;
}

export interface WriteSqlFileResponse {
  path: string;
  name: string;
  bytes: number;
}
