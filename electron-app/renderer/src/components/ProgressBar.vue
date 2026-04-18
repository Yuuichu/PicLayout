<template>
  <div class="progress-area" v-if="store.processing || store.outputFiles.length > 0 || store.errorMessage">

    <!-- 处理中 -->
    <div v-if="store.processing" class="card progress-card">
      <div class="status-text">{{ store.statusMessage || '正在处理...' }}</div>
      <div class="progress-track">
        <div class="progress-fill" :style="{ width: store.progress + '%' }" />
      </div>
      <div class="progress-pct">{{ store.progress }}%</div>
      <button class="btn-danger cancel-btn" @click="cancel">取消</button>
    </div>

    <!-- 完成 -->
    <div v-else-if="store.outputFiles.length > 0" class="card result-card">
      <p class="result-title">✓ 拼贴完成</p>
      <ul class="output-list">
        <li v-for="f in store.outputFiles" :key="f" :title="f">
          {{ basename(f) }}
        </li>
      </ul>
    </div>

    <!-- 错误 -->
    <div v-else-if="store.errorMessage" class="card error-card">
      <p class="error-title">✗ 处理失败</p>
      <p class="error-msg">{{ store.errorMessage }}</p>
    </div>

  </div>
</template>

<script setup lang="ts">
import { useAppStore } from '../stores/appStore'

const store = useAppStore()

function basename(path: string): string {
  return path.replace(/\\/g, '/').split('/').pop() ?? path
}

async function cancel() {
  await window.electronAPI.cancelCollage()
  store.processing = false
  store.statusMessage = '已取消'
}
</script>

<style scoped>
.progress-area {
  margin-top: 8px;
}

.progress-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.status-text {
  font-size: 13px;
  color: var(--color-text-secondary);
}

.progress-track {
  height: 8px;
  background: var(--color-border);
  border-radius: 4px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--color-primary);
  border-radius: 4px;
  transition: width 0.3s ease;
}

.progress-pct {
  font-size: 12px;
  color: var(--color-text-secondary);
  text-align: right;
}

.cancel-btn {
  align-self: flex-end;
  padding: 4px 12px;
  font-size: 12px;
}

.result-card {
  background: #e8f5e9;
}

.result-title {
  font-weight: 700;
  color: var(--color-success);
  margin-bottom: 8px;
}

.output-list {
  list-style: none;
  font-size: 12px;
  color: var(--color-text-secondary);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.error-card {
  background: #ffebee;
}

.error-title {
  font-weight: 700;
  color: var(--color-danger);
  margin-bottom: 6px;
}

.error-msg {
  font-size: 12px;
  color: #b71c1c;
}
</style>
