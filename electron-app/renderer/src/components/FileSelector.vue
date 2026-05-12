<template>
  <div class="card file-selector">
    <p class="section-title">图片选择</p>

    <div class="form-row">
      <label>输出文件名前缀</label>
      <input
        v-model="store.settings.prefix"
        type="text"
        placeholder="output"
        style="max-width: 160px"
        @change="store.saveSettings()"
      />
    </div>

    <div class="file-actions">
      <button class="btn-success" :disabled="processing" @click="selectImages">
        选择图片
      </button>
      <button
        v-if="store.selectedFiles.length > 0"
        class="btn-secondary"
        :disabled="processing"
        @click="clearImages"
      >
        清空
      </button>
      <span class="file-count">
        {{ fileCountText }}
      </span>
    </div>

    <div v-if="store.selectedFiles.length > 0" class="file-list">
      <div
        v-for="(f, i) in store.selectedFiles"
        :key="i"
        class="file-row"
        :title="f"
      >
        <span class="file-index">{{ i + 1 }}</span>
        <span class="file-name">{{ basename(f) }}</span>
        <button
          class="remove-btn"
          :disabled="processing"
          @click="removeImage(i)"
        >
          移除
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAppStore } from '../stores/appStore'

const store = useAppStore()

const processing = computed(() => store.processing)

const fileCountText = computed(() => {
  const n = store.selectedFiles.length
  if (n === 0) return '未选择图片'
  return `已选择 ${n} 张图片`
})

function basename(path: string): string {
  return path.replace(/\\/g, '/').split('/').pop() ?? path
}

async function selectImages() {
  const files = await window.electronAPI.openImages()
  if (!files.length) return

  const max = store.settings.maxImages
  if (files.length > max) {
    alert(`选择的图片数量不能超过 ${max} 张，当前选择了 ${files.length} 张。`)
    return
  }

  store.setSelectedFiles(files)
}

function removeImage(index: number) {
  store.setSelectedFiles(store.selectedFiles.filter((_, i) => i !== index))
}

function clearImages() {
  store.setSelectedFiles([])
}
</script>

<style scoped>
.file-selector {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.file-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.file-count {
  font-size: 13px;
  color: var(--color-primary);
  font-weight: 500;
}

.file-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 80px;
  overflow-y: auto;
}

.file-row {
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  background: var(--color-bg);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  padding: 4px 6px;
  font-size: 12px;
  color: var(--color-text-secondary);
}

.file-index {
  color: var(--color-primary);
  font-weight: 600;
}

.file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.remove-btn {
  padding: 2px 8px;
  font-size: 12px;
  border-radius: 4px;
  background: transparent;
  color: var(--color-danger);
  border: 1px solid var(--color-danger);
}
</style>
