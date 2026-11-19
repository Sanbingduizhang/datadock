/** 事务类型，与 src-tauri/src/models/query.rs 一致 */

export interface BeginTransactionRequest {
  connectionId: string;
  databaseName?: string;
}

export interface BeginTransactionResponse {
  transactionId: string;
}

export interface TransactionRequest {
  connectionId: string;
  transactionId: string;
}

export interface TransactionResponse {
  success: boolean;
  message: string;
}
