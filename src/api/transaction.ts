import { invokeCommand } from './index';

import type {
  BeginTransactionRequest,
  BeginTransactionResponse,
  TransactionRequest,
  TransactionResponse,
} from '@/types/transaction';

/** 在独立会话中开启手动事务 */
export function beginTransaction(
  request: BeginTransactionRequest,
): Promise<BeginTransactionResponse> {
  return invokeCommand<BeginTransactionResponse>('begin_transaction', { request });
}

export function commitTransaction(request: TransactionRequest): Promise<TransactionResponse> {
  return invokeCommand<TransactionResponse>('commit_transaction', { request });
}

export function rollbackTransaction(request: TransactionRequest): Promise<TransactionResponse> {
  return invokeCommand<TransactionResponse>('rollback_transaction', { request });
}
