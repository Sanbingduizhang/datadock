<script setup lang="ts">
import * as monaco from 'monaco-editor/esm/vs/editor/editor.api';
import 'monaco-editor/esm/vs/editor/editor.all.js';
import 'monaco-editor/esm/vs/basic-languages/sql/sql.contribution';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';

import { onBeforeUnmount, onMounted, ref, watch } from 'vue';

/**
 * Monaco SQL 编辑器封装。
 * 只保留编辑器核心能力（查找、括号匹配等），不引入语言服务 worker。
 */
const props = withDefaults(
  defineProps<{
    modelValue: string;
    theme?: 'light' | 'dark';
    fontSize?: number;
    tabSize?: number;
    wordWrap?: boolean;
    readOnly?: boolean;
  }>(),
  {
    theme: 'light',
    fontSize: 13,
    tabSize: 2,
    wordWrap: false,
    readOnly: false,
  },
);

const emit = defineEmits<{
  'update:modelValue': [value: string];
  execute: [];
  cancel: [];
}>();

const container = ref<HTMLDivElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let applyingExternalValue = false;

function themeName(): string {
  return props.theme === 'dark' ? 'vs-dark' : 'vs';
}

onMounted(() => {
  // 必须在创建编辑器前注册 worker，否则 Monaco 会尝试从网络加载
  self.MonacoEnvironment = {
    getWorker: () => new EditorWorker(),
  };

  if (!container.value) {
    return;
  }

  editor = monaco.editor.create(container.value, {
    value: props.modelValue,
    language: 'sql',
    theme: themeName(),
    fontSize: props.fontSize,
    tabSize: props.tabSize,
    wordWrap: props.wordWrap ? 'on' : 'off',
    readOnly: props.readOnly,
    minimap: { enabled: false },
    automaticLayout: true,
    scrollBeyondLastLine: false,
    renderLineHighlight: 'line',
    lineNumbersMinChars: 3,
    glyphMargin: false,
    folding: false,
    padding: { top: 10, bottom: 10 },
    fontFamily:
      "'JetBrains Mono', 'SF Mono', Menlo, Consolas, 'Liberation Mono', monospace",
    fontLigatures: false,
    smoothScrolling: true,
    contextmenu: true,
  });

  editor.onDidChangeModelContent(() => {
    if (applyingExternalValue || !editor) {
      return;
    }
    emit('update:modelValue', editor.getValue());
  });

  // ⌘/Ctrl + Enter 执行；Esc 取消正在执行的查询
  editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
    emit('execute');
  });
  editor.addCommand(monaco.KeyCode.Escape, () => {
    emit('cancel');
  });

  editor.focus();
});

// 外部（历史回填、打开文件）改动时同步到编辑器
watch(
  () => props.modelValue,
  (value) => {
    if (!editor || editor.getValue() === value) {
      return;
    }
    applyingExternalValue = true;
    const position = editor.getPosition();
    editor.setValue(value);
    if (position) {
      editor.setPosition(position);
    }
    applyingExternalValue = false;
  },
);

watch(
  () => props.theme,
  () => {
    monaco.editor.setTheme(themeName());
  },
);

watch(
  () => props.fontSize,
  (value) => {
    editor?.updateOptions({ fontSize: value });
  },
);

watch(
  () => props.tabSize,
  (value) => {
    editor?.updateOptions({ tabSize: value });
  },
);

watch(
  () => props.wordWrap,
  (value) => {
    editor?.updateOptions({ wordWrap: value ? 'on' : 'off' });
  },
);

onBeforeUnmount(() => {
  editor?.dispose();
  editor = null;
});

defineExpose({
  focus: () => editor?.focus(),
});
</script>

<template>
  <div ref="container" class="sql-editor"></div>
</template>

<style scoped>
.sql-editor {
  width: 100%;
  height: 100%;
  overflow: hidden;
}
</style>
