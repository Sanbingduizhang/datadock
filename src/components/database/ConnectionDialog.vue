<script setup lang="ts">
import { LoaderCircle } from 'lucide-vue-next';
import { computed, ref, watch } from 'vue';

import AppButton from '@/components/common/AppButton.vue';
import AppInput from '@/components/common/AppInput.vue';
import AppModal from '@/components/common/AppModal.vue';

import { useConnectionStore } from '@/stores/connection';

import type { DataSourceType } from '@/types/api';
import type {
  ConnectionInfo,
  ConnectionOptions,
  SslMode,
  TestConnectionResponse,
} from '@/types/connection';

/**
 * 新建 / 编辑 MySQL 连接。
 * 密码只在提交时传给后端，前端不持久化。
 */
const props = defineProps<{
  modelValue: boolean;
  mode: 'create' | 'edit';
  connection?: ConnectionInfo | null;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: boolean];
  saved: [connectionId: string];
}>();

const connectionStore = useConnectionStore();

const sourceType = ref<DataSourceType>('mysql');
const name = ref('');
const host = ref('127.0.0.1');
const port = ref('3306');
const username = ref('root');
const password = ref('');
const defaultDatabase = ref('');

/* ---------- SSL / SSH 隧道 ---------- */
const sslEnabled = ref(false);
const sslMode = ref<SslMode>('prefer');
const sshEnabled = ref(false);
const sshHost = ref('');
const sshPort = ref('22');
const sshUsername = ref('');
const sshAuthType = ref<'password' | 'privateKey'>('password');
const sshPrivateKeyPath = ref('');

const saving = ref(false);
const testing = ref(false);
const testResult = ref<TestConnectionResponse | null>(null);
const formError = ref('');

const engineLabel = computed(() =>
  sourceType.value === 'redis' ? 'Redis' : sourceType.value === 'postgres' ? 'PostgreSQL' : 'MySQL',
);

const title = computed(() =>
  props.mode === 'create' ? `新建 ${engineLabel.value} 连接` : `编辑 ${engineLabel.value} 连接`,
);

function handleSourceTypeChange(next: DataSourceType): void {
  sourceType.value = next;
  if (props.mode === 'edit') {
    return;
  }
  if (next === 'redis') {
    port.value = '6379';
    username.value = '';
    defaultDatabase.value = '0';
  } else if (next === 'postgres') {
    port.value = '5432';
    username.value = 'postgres';
    defaultDatabase.value = 'postgres';
  } else {
    port.value = '3306';
    username.value = 'root';
    defaultDatabase.value = '';
  }
}

const passwordHint = computed(() =>
  props.mode === 'edit' ? '留空表示不修改密码' : '密码只保存到系统凭据，不写入本地数据库',
);

function resetForm(): void {
  const target = props.connection;
  sourceType.value = target?.sourceType ?? 'mysql';
  name.value = target?.name ?? '';
  host.value = target?.host ?? '127.0.0.1';
  port.value = String(
    target?.port ?? (sourceType.value === 'redis' ? 6379 : sourceType.value === 'postgres' ? 5432 : 3306),
  );
  username.value =
    target?.username ?? (sourceType.value === 'redis' ? '' : sourceType.value === 'postgres' ? 'postgres' : 'root');
  password.value = '';
  defaultDatabase.value =
    target?.defaultDatabase ??
    (sourceType.value === 'redis' ? '0' : sourceType.value === 'postgres' ? 'postgres' : '');

  // 回填 SSL / SSH（编辑模式；新建走默认值）
  const opts = target?.options;
  sslEnabled.value = opts?.sslEnabled ?? false;
  sslMode.value = opts?.sslMode ?? 'prefer';
  sshEnabled.value = opts?.sshEnabled ?? false;
  sshHost.value = opts?.sshHost ?? '';
  sshPort.value = String(opts?.sshPort ?? 22);
  sshUsername.value = opts?.sshUsername ?? '';
  sshAuthType.value = opts?.sshAuthType ?? 'password';
  sshPrivateKeyPath.value = opts?.sshPrivateKeyPath ?? '';

  testResult.value = null;
  formError.value = '';
}

watch(
  () => props.modelValue,
  (visible) => {
    if (visible) {
      resetForm();
    }
  },
);

/** 根据表单当前状态构造 SSL / SSH 选项 */
function buildOptions(): ConnectionOptions {
  return {
    sslEnabled: sslEnabled.value,
    sslMode: sslEnabled.value ? sslMode.value : undefined,
    sshEnabled: sshEnabled.value,
    sshHost: sshEnabled.value ? sshHost.value.trim() || undefined : undefined,
    sshPort: sshEnabled.value ? Number.parseInt(sshPort.value, 10) || undefined : undefined,
    sshUsername: sshEnabled.value ? sshUsername.value.trim() || undefined : undefined,
    sshAuthType: sshEnabled.value ? sshAuthType.value : undefined,
    sshPrivateKeyPath:
      sshEnabled.value && sshAuthType.value === 'privateKey'
        ? sshPrivateKeyPath.value.trim() || undefined
        : undefined,
  };
}

function buildTarget() {
  const portNumber = Number.parseInt(port.value, 10);
  return {
    sourceType: sourceType.value,
    host: host.value.trim(),
    port: Number.isNaN(portNumber) ? 0 : portNumber,
    username: username.value.trim(),
    defaultDatabase: defaultDatabase.value.trim() || undefined,
    options: buildOptions(),
  };
}

function validate(): string | null {
  if (!name.value.trim()) {
    return '请填写连接名称';
  }
  if (!host.value.trim()) {
    return '请填写主机地址';
  }
  const portNumber = Number.parseInt(port.value, 10);
  if (Number.isNaN(portNumber) || portNumber < 1 || portNumber > 65535) {
    return '端口需要在 1~65535 之间';
  }
  // Redis 允许匿名/默认用户
  if (!['redis', 'postgres'].includes(sourceType.value) && !username.value.trim()) {
    return '请填写用户名';
  }

  // SSH 隧道：启用时主机、用户名为必填，端口合法
  if (sshEnabled.value) {
    if (!sshHost.value.trim()) {
      return '已启用 SSH 隧道，请填写 SSH 主机';
    }
    if (!sshUsername.value.trim()) {
      return '已启用 SSH 隧道，请填写 SSH 用户名';
    }
    const sshPortNumber = Number.parseInt(sshPort.value, 10);
    if (Number.isNaN(sshPortNumber) || sshPortNumber < 1 || sshPortNumber > 65535) {
      return 'SSH 端口需要在 1~65535 之间';
    }
    if (sshAuthType.value === 'privateKey' && !sshPrivateKeyPath.value.trim()) {
      return '私钥认证需要填写私钥文件路径';
    }
  }

  // 允许空密码：部分测试/本地实例使用空密码
  return null;
}

async function handleTest(): Promise<void> {
  const validation = validate();
  if (validation) {
    formError.value = validation;
    return;
  }

  formError.value = '';
  testing.value = true;

  try {
    testResult.value = await connectionStore.test({
      ...buildTarget(),
      password: password.value,
    });
  } finally {
    testing.value = false;
  }
}

async function handleSave(): Promise<void> {
  const validation = validate();
  if (validation) {
    formError.value = validation;
    return;
  }

  formError.value = '';
  saving.value = true;

  try {
    if (props.mode === 'create') {
      const connectionId = await connectionStore.create({
        ...buildTarget(),
        name: name.value.trim(),
        password: password.value,
      });
      if (connectionId) {
        emit('saved', connectionId);
        emit('update:modelValue', false);
      }
      return;
    }

    const target = props.connection;
    if (!target) {
      formError.value = '缺少待编辑的连接信息';
      return;
    }

    const updated = await connectionStore.update({
      ...buildTarget(),
      connectionId: target.id,
      name: name.value.trim(),
      newPassword: password.value.length > 0 ? password.value : undefined,
    });
    if (updated) {
      emit('saved', target.id);
      emit('update:modelValue', false);
    }
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <AppModal
    :model-value="modelValue"
    :title="title"
    :width="480"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <div class="form">
      <label class="field">
        <span class="field__label">类型</span>
        <select
          class="field__select"
          :value="sourceType"
          :disabled="mode === 'edit'"
          @change="
            handleSourceTypeChange(
              ($event.target as HTMLSelectElement).value as DataSourceType,
            )
          "
        >
          <option value="mysql">MySQL</option>
          <option value="postgres">PostgreSQL</option>
          <option value="redis">Redis（命令控制台）</option>
        </select>
      </label>

      <label class="field">
        <span class="field__label">连接名称</span>
        <AppInput v-model="name" placeholder="例如：本地开发库" />
      </label>

      <div class="field-row">
        <label class="field field--grow">
          <span class="field__label">主机</span>
          <AppInput v-model="host" placeholder="127.0.0.1" />
        </label>
        <label class="field field--port">
          <span class="field__label">端口</span>
          <AppInput v-model="port" :placeholder="sourceType === 'redis' ? '6379' : sourceType === 'postgres' ? '5432' : '3306'" />
        </label>
      </div>

      <div class="field-row">
        <label class="field field--grow">
          <span class="field__label">用户名</span>
          <AppInput
            v-model="username"
            :placeholder="sourceType === 'redis' ? '可留空' : sourceType === 'postgres' ? 'postgres' : 'root'"
          />
        </label>
        <label class="field field--grow">
          <span class="field__label">
            {{ sourceType === 'redis' ? '数据库索引' : sourceType === 'postgres' ? '默认数据库（PG 库名）' : '默认数据库' }}
          </span>
          <AppInput
            v-model="defaultDatabase"
            :placeholder="sourceType === 'redis' ? '0 - 15，默认 0' : sourceType === 'postgres' ? 'postgres' : '可选'"
          />
        </label>
      </div>

      <label class="field">
        <span class="field__label">密码</span>
        <AppInput v-model="password" type="password" :placeholder="passwordHint" />
      </label>

      <!-- SSL 加密 -->
      <div class="section">
        <label class="toggle">
          <input v-model="sslEnabled" type="checkbox" />
          <span class="field__label">SSL 加密连接</span>
        </label>
        <template v-if="sslEnabled">
          <label class="field">
            <span class="field__label">加密模式</span>
            <select v-model="sslMode" class="field__select">
              <option value="disabled">不加密（disabled）</option>
              <option value="prefer">尽量加密（prefer）</option>
              <option value="require">强制加密（require）</option>
              <option value="verify-ca">校验证书（verify-ca）</option>
              <option value="verify-full">校验证书与主机名（verify-full）</option>
            </select>
          </label>
        </template>
      </div>

      <!-- SSH 隧道 -->
      <div class="section">
        <label class="toggle">
          <input v-model="sshEnabled" type="checkbox" />
          <span class="field__label">SSH 隧道</span>
        </label>
        <template v-if="sshEnabled">
          <div class="field-row">
            <label class="field field--grow">
              <span class="field__label">SSH 主机</span>
              <AppInput v-model="sshHost" placeholder="跳板机地址" />
            </label>
            <label class="field field--port">
              <span class="field__label">SSH 端口</span>
              <AppInput v-model="sshPort" placeholder="22" />
            </label>
          </div>
          <div class="field-row">
            <label class="field field--grow">
              <span class="field__label">SSH 用户名</span>
              <AppInput v-model="sshUsername" placeholder="跳板机用户名" />
            </label>
            <label class="field field--grow">
              <span class="field__label">认证方式</span>
              <select v-model="sshAuthType" class="field__select">
                <option value="password">密码</option>
                <option value="privateKey">私钥</option>
              </select>
            </label>
          </div>
          <label v-if="sshAuthType === 'privateKey'" class="field">
            <span class="field__label">私钥文件路径</span>
            <AppInput v-model="sshPrivateKeyPath" placeholder="例如 ~/.ssh/id_rsa" />
          </label>
          <p v-else class="hint">SSH 密码与数据库密码共用同一个密码框。</p>
        </template>
      </div>

      <p v-if="formError" class="alert alert--error">{{ formError }}</p>

      <p
        v-else-if="testResult"
        class="alert"
        :class="testResult.success ? 'alert--ok' : 'alert--error'"
      >
        <template v-if="testResult.success">
          连接成功 · {{ engineLabel }} {{ testResult.serverVersion }} ·
          {{ testResult.elapsedMs }}ms
        </template>
        <template v-else>连接失败：{{ testResult.message }}</template>
      </p>

      <p class="hint">
        密码保存在系统凭据服务（macOS Keychain / Windows Credential Manager），不会写入本地数据库或日志。
      </p>
    </div>

    <template #footer>
      <AppButton :disabled="testing || saving" @click="handleTest">
        <LoaderCircle v-if="testing" :size="14" class="spin" />
        {{ testing ? '测试中…' : '测试连接' }}
      </AppButton>
      <span class="footer-spacer"></span>
      <AppButton :disabled="saving" @click="emit('update:modelValue', false)">取消</AppButton>
      <AppButton variant="primary" :loading="saving" @click="handleSave">保存</AppButton>
    </template>
  </AppModal>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.field {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.field__label {
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.field-row {
  display: flex;
  gap: var(--space-2);
}

.section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
}

.toggle {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  cursor: pointer;
}

.toggle input {
  accent-color: var(--primary-color);
  margin: 0;
}

.field--grow {
  flex: 1;
  min-width: 0;
}

.field__select {
  height: 30px;
  padding: 0 6px;
  color: var(--text-primary);
  font-size: var(--font-size-sm);
  background-color: var(--surface-subtle);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-sm);
  outline: none;
}

.field--port {
  width: 96px;
}

.alert {
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-xs);
  border-radius: var(--radius-md);
}

.alert--ok {
  color: var(--success-color);
  background-color: var(--success-soft);
}

.alert--error {
  color: var(--danger-color);
  background-color: var(--danger-soft);
}

.hint {
  color: var(--text-tertiary);
  font-size: 11px;
  line-height: 1.6;
}

.footer-spacer {
  flex: 1;
}

.spin {
  animation: spin 900ms linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
