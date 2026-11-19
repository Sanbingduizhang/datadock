import { readonly, ref } from 'vue';

export type ToastType = 'info' | 'success' | 'warning' | 'error';

export interface ToastItem {
  id: number;
  message: string;
  type: ToastType;
}

const toasts = ref<ToastItem[]>([]);
let toastSequence = 0;

const DEFAULT_DURATION = 2600;

/**
 * 轻量 Toast：用于操作成功、失败提示。
 * 不阻塞操作，提示自动消失；同一时间最多保留 3 条。
 */
export function useToast() {
  function show(message: string, type: ToastType = 'info', duration = DEFAULT_DURATION): number {
    toastSequence += 1;
    const id = toastSequence;
    toasts.value = [...toasts.value.slice(-2), { id, message, type }];

    window.setTimeout(() => {
      dismiss(id);
    }, duration);

    return id;
  }

  function dismiss(id: number): void {
    toasts.value = toasts.value.filter((item) => item.id !== id);
  }

  return {
    toasts: readonly(toasts),
    show,
    dismiss,
    success: (message: string) => show(message, 'success'),
    info: (message: string) => show(message, 'info'),
    warning: (message: string) => show(message, 'warning'),
    error: (message: string) => show(message, 'error'),
  };
}
