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
      <span class="file-count">
        {{ fileCountText }}
      </span>
    </div>

    <div v-if="store.selectedFiles.length > 0" class="file-list-preview">
      <span
        v-for="(f, i) in previewFiles"
        :key="i"
        class="file-chip"
        :title="f"
      >{{ basename(f) }}</span>
      <span v-if="store.selectedFiles.length > MAX_PREVIEW" class="file-chip more">
        +{{ store.selectedFiles.length - MAX_PREVIEW }} 张
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAppStore } from '../stores/appStore'

const store = useAppStore()
const MAX_PREVIEW = 8

const processing = computed(() => store.processing)

const fileCountText = computed(() => {
  const n = store.selectedFiles.length
  if (n === 0) return '未选择图片'
  return `已选择 ${n} 张图片`
})

const previewFiles = computed(() => store.selectedFiles.slice(0, MAX_PREVIEW))

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

.file-list-preview {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  max-height: 80px;
  overflow-y: auto;
}

.file-chip {
  background: var(--color-bg);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  padding: 2px 8px;
  font-size: 12px;
  color: var(--color-text-secondary);
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-chip.more {
  background: var(--color-primary);
  color: white;
  border-color: var(--color-primary);
}
</style>
