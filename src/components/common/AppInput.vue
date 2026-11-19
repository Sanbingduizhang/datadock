<script setup lang="ts">
withDefaults(
  defineProps<{
    modelValue: string;
    placeholder?: string;
    size?: 'sm' | 'md';
    type?: 'text' | 'password';
    disabled?: boolean;
    readonly?: boolean;
  }>(),
  {
    placeholder: '',
    size: 'md',
    type: 'text',
    disabled: false,
    readonly: false,
  },
);

const emit = defineEmits<{
  'update:modelValue': [value: string];
  enter: [];
}>();

function handleInput(event: Event): void {
  const target = event.target as HTMLInputElement;
  emit('update:modelValue', target.value);
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.key === 'Enter') {
    emit('enter');
  }
}
</script>

<template>
  <label class="input" :class="[`input--${size}`, { 'is-disabled': disabled }]">
    <span v-if="$slots.prefix" class="input__prefix">
      <slot name="prefix" />
    </span>
    <input
      class="input__control"
      :type="type"
      :value="modelValue"
      :placeholder="placeholder"
      :disabled="disabled"
      :readonly="readonly"
      @input="handleInput"
      @keydown="handleKeydown"
    />
    <span v-if="$slots.suffix" class="input__suffix">
      <slot name="suffix" />
    </span>
  </label>
</template>

<style scoped>
.input {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  height: 30px;
  padding: 0 var(--space-2);
  background-color: var(--surface-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  transition:
    border-color var(--transition-fast),
    box-shadow var(--transition-fast);
}

.input:focus-within {
  border-color: var(--primary-color);
  box-shadow: 0 0 0 2px var(--primary-soft);
}

.input--sm {
  height: 26px;
}

.input.is-disabled {
  opacity: 0.6;
}

.input__control {
  width: 100%;
  min-width: 0;
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  background: transparent;
  user-select: text;
}

.input__control::placeholder {
  color: var(--text-tertiary);
}

.input__prefix,
.input__suffix {
  display: inline-flex;
  flex: none;
  align-items: center;
  color: var(--text-tertiary);
}
</style>
