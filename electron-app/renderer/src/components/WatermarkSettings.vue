<template>
  <div class="watermark-settings" :class="{ card: !embedded, embedded }">
    <div class="watermark-header">
      <p class="section-title">水印设置</p>
      <label class="toggle-label">
        <input
          v-model="s.watermarkEnabled"
          type="checkbox"
          @change="save"
        />
        启用水印
      </label>
    </div>

    <template v-if="s.watermarkEnabled">
      <div class="watermark-grid">
        <div class="form-row compact-row">
          <label>水印图片</label>
          <button class="btn-secondary watermark-btn" :disabled="store.processing" @click="selectWatermark">
            选择图片
          </button>
          <span class="hint file-name" :title="s.watermark.path">
            {{ watermarkBasename || '未选择' }}
          </span>
        </div>

        <div class="form-row compact-row">
          <label>缩放</label>
          <input
            v-model.number="s.watermark.scale_percent"
            class="range-input"
            type="range"
            min="10"
            max="300"
            @input="save"
          />
          <input
            v-model.number="s.watermark.scale_percent"
            type="number"
            min="10"
            max="300"
            class="number-input"
            @change="save"
          />
          <span class="hint">%</span>
        </div>

        <div class="form-row compact-row">
          <label>水平位置</label>
          <input
            v-model.number="s.watermark.position_x_percent"
            class="range-input"
            type="range"
            min="0"
            max="100"
            @input="save"
          />
          <input
            v-model.number="s.watermark.position_x_percent"
            type="number"
            min="0"
            max="100"
            class="number-input"
            @change="save"
          />
          <span class="hint">%</span>
        </div>

        <div class="form-row compact-row">
          <label>垂直位置</label>
          <input
            v-model.number="s.watermark.position_y_percent"
            class="range-input"
            type="range"
            min="0"
            max="100"
            @input="save"
          />
          <input
            v-model.number="s.watermark.position_y_percent"
            type="number"
            min="0"
            max="100"
            class="number-input"
            @change="save"
          />
          <span class="hint">%</span>
        </div>
      </div>

      <p class="tip">水印会叠加在下方拼贴预览上；导出时会使用原始水印图片，建议使用带透明通道的 PNG。</p>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAppStore } from '../stores/appStore'

defineProps<{
  embedded?: boolean
}>()

const store = useAppStore()
const s = store.settings

const watermarkBasename = computed(() => {
  if (!s.watermark.path) return ''
  return s.watermark.path.replace(/\\/g, '/').split('/').pop() ?? s.watermark.path
})

async function selectWatermark() {
  const path = await window.electronAPI.openWatermark()
  if (path) {
    s.watermark.path = path
    await store.ensureThumbnail(path)
    await store.ensureImageSize(path)
    save()
  }
}

function save() {
  store.saveSettings()
}
</script>

<style scoped>
.watermark-settings {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.watermark-settings.embedded {
  padding: 0;
  border: 0;
  background: transparent;
}

.watermark-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.watermark-header .section-title {
  margin: 0;
  border: 0;
  padding: 0;
}

.toggle-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--color-text);
}

.watermark-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 10px;
}

.compact-row {
  align-items: stretch;
  flex-direction: column;
  gap: 6px;
  margin: 0;
}

.compact-row label {
  min-width: 0;
  color: var(--color-text-muted);
  font-size: 11px;
  font-weight: 700;
}

.watermark-btn {
  width: 100%;
  font-size: 12px;
}

.file-name {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.range-input {
  width: 100%;
}

.number-input {
  max-width: 86px;
}

.tip {
  font-size: 11px;
  color: var(--color-text-subtle);
  margin-top: 0;
}

@media (max-width: 760px) {
  .watermark-header {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
