<template>
  <div class="card">
    <p class="section-title">水印设置</p>

    <div class="form-row">
      <label>
        <input
          type="checkbox"
          v-model="s.watermarkEnabled"
          @change="save"
          style="margin-right: 6px"
        />
        启用水印
      </label>
    </div>

    <template v-if="s.watermarkEnabled">
      <div class="form-row">
        <label>水印图片</label>
        <button class="btn-secondary" style="padding: 4px 10px; font-size:12px" @click="selectWatermark">
          选择图片
        </button>
        <span class="hint file-name">{{ watermarkBasename || '未选择' }}</span>
      </div>

      <div class="form-row">
        <label>缩放比例</label>
        <input
          v-model.number="s.watermark.scale_percent"
          type="number" min="10" max="300" style="max-width:80px"
          @change="save"
        />
        <span class="hint">%</span>
      </div>

      <div class="form-row">
        <label>水平位置</label>
        <input
          v-model.number="s.watermark.position_x_percent"
          type="number" min="0" max="100" style="max-width:80px"
          @change="save"
        />
        <span class="hint">%（50% = 居中）</span>
      </div>

      <div class="form-row">
        <label>垂直位置</label>
        <input
          v-model.number="s.watermark.position_y_percent"
          type="number" min="0" max="100" style="max-width:80px"
          @change="save"
        />
        <span class="hint">%（95% = 底部）</span>
      </div>

      <p class="tip">建议使用 PNG 格式图片以获得最佳透明效果</p>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAppStore } from '../stores/appStore'

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
    save()
  }
}

function save() {
  store.saveSettings()
}
</script>

<style scoped>
.file-name {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tip {
  font-size: 11px;
  color: var(--color-text-secondary);
  margin-top: 4px;
}
</style>
