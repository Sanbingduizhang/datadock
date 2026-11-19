<script setup lang="ts">
import { computed, ref, watch } from 'vue';

/**
 * 可输入 + 下拉建议的组合框。
 *
 * - 输入时下方实时过滤显示候选；
 * - 选中候选回填输入框；也允许自由输入任意值（不会被强制匹配）；
 * - 用于字段类型（可手动输入自定义类型）与索引列（可自由输入列名）。
 */
const props = withDefaults(
  defineProps<{
    /** 输入框当前值 */
    modelValue: string;
    /** 全部候选（扁平） */
    options: string[];
    /** 占位文案 */
    placeholder?: string;
    /** 是否禁用 */
    disabled?: boolean;
    /** 输入框宽度 */
    width?: number;
    /** 候选为空时是否仍显示下拉（自由输入提示） */
    hideWhenEmpty?: boolean;
  }>(),
  {
    placeholder: '',
    disabled: false,
    width: undefined,
    hideWhenEmpty: true,
  },
);

const emit = defineEmits<{
  'update:model-value': [value: string];
  /** 选中某个候选（用于组合类型+长度等场景） */
  select: [value: string];
}>();

const open = ref(false);
const highlightIndex = ref(-1);

/** 输入框显示文本 */
const inputValue = ref(props.modelValue);

// 外部值变化时同步（如撤销、程序回填）
watch(
  () => props.modelValue,
  (next) => {
    inputValue.value = next;
  },
);

/** 根据输入过滤候选（大小写不敏感，前缀优先） */
const filteredOptions = computed<string[]>(() => {
  const keyword = inputValue.value.trim().toLowerCase();
  if (keyword.length === 0) {
    return props.options;
  }
  const startsWith = props.options.filter((item) =>
    item.toLowerCase().startsWith(keyword),
  );
  const contains = props.options.filter(
    (item) => !startsWith.includes(item) && item.toLowerCase().includes(keyword),
  );
  return [...startsWith, ...contains];
});

/** 是否展示下拉面板 */
const showDropdown = computed(
  () => open.value && !(props.hideWhenEmpty && filteredOptions.value.length === 0),
);

function handleFocus(): void {
  if (!props.disabled) {
    open.value = true;
    highlightIndex.value = -1;
  }
}

function handleInput(event: Event): void {
  const value = (event.target as HTMLInputElement).value;
  inputValue.value = value;
  open.value = true;
  highlightIndex.value = -1;
  // 自由输入实时回传（类型场景需要实时拼长度）
  emit('update:model-value', value);
}

function choose(value: string): void {
  inputValue.value = value;
  emit('update:model-value', value);
  emit('select', value);
  open.value = false;
}

function handleKeydown(event: KeyboardEvent): void {
  if (!showDropdown.value) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      open.value = true;
    }
    return;
  }

  if (event.key === 'ArrowDown') {
    event.preventDefault();
    highlightIndex.value = Math.min(
      highlightIndex.value + 1,
      filteredOptions.value.length - 1,
    );
  } else if (event.key === 'ArrowUp') {
    event.preventDefault();
    highlightIndex.value = Math.max(highlightIndex.value - 1, -1);
  } else if (event.key === 'Enter') {
    event.preventDefault();
    const target =
      highlightIndex.value >= 0
        ? filteredOptions.value[highlightIndex.value]
        : filteredOptions.value[0];
    if (target) {
      choose(target);
    } else {
      open.value = false;
    }
  } else if (event.key === 'Escape') {
    open.value = false;
  }
}

function handleBlur(): void {
  // 延迟关闭，避免点击候选时先触发 blur
  window.setTimeout(() => {
    open.value = false;
  }, 150);
}
</script>

<template>
  <div class="combo" :style="width ? { width: `${width}px` } : {}">
    <input
      class="combo__input"
      type="text"
      :value="inputValue"
      :placeholder="placeholder"
      :disabled="disabled"
      spellcheck="false"
      autocomplete="off"
      @focus="handleFocus"
      @input="handleInput"
      @keydown="handleKeydown"
      @blur="handleBlur"
    />
    <div v-if="showDropdown" class="combo__dropdown">
      <button
        v-for="(option, index) in filteredOptions"
        :key="option"
        type="button"
        class="combo__option"
        :class="{ 'is-highlight': index === highlightIndex }"
        @mousedown.prevent="choose(option)"
        @mouseenter="highlightIndex = index"
      >
        {{ option }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.combo {
  position: relative;
  display: inline-flex;
  min-width: 0;
}

.combo__input {
  width: 100%;
  min-width: 0;
}

.combo__dropdown {
  position: absolute;
  top: calc(100% + 2px);
  left: 0;
  z-index: 40;
  display: flex;
  flex-direction: column;
  max-height: 200px;
  min-width: 100%;
  overflow-y: auto;
  padding: 3px;
  background-color: var(--surface-raised);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  box-shadow: var(--shadow-md);
}

.combo__option {
  display: block;
  width: 100%;
  padding: 4px 8px;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  text-align: left;
  border-radius: var(--radius-xs);
}

.combo__option:hover,
.combo__option.is-highlight {
  color: var(--text-primary);
  background-color: var(--surface-hover);
}
</style>
