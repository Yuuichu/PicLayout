<template>
  <div class="settings-panel">

    <div class="card">
      <p class="section-title">基础参数</p>

      <div class="form-row">
        <label>最大图片数量</label>
        <input v-model.number="s.maxImages" type="number" min="1" max="500" style="max-width:90px" @change="save" />
        <span class="hint">默认建议 30 张以内</span>
      </div>

      <div class="form-row">
        <label>重采样大小</label>
        <input v-model.number="s.resampleSize" type="number" min="500" max="20000" style="max-width:90px" @change="save" />
        <span class="hint">像素（长边）</span>
      </div>

      <div class="form-row">
        <label>单图边框大小</label>
        <input v-model.number="s.borderSize" type="number" min="500" max="20000" style="max-width:90px" @change="save" />
        <span class="hint">像素（正方形边长）</span>
      </div>

      <div class="form-row">
        <label>最终图像大小</label>
        <input v-model.number="s.finalSize" type="number" min="1000" max="30000" style="max-width:90px" @change="save" />
        <span class="hint">像素（长边）</span>
      </div>

      <div class="form-row">
        <label>图像 DPI</label>
        <select v-model.number="s.dpi" style="max-width:100px" @change="save">
          <option :value="72">72</option>
          <option :value="150">150</option>
          <option :value="300">300</option>
          <option :value="600">600</option>
        </select>
        <span class="hint">每英寸点数</span>
      </div>

      <div class="form-row">
        <label>背景颜色</label>
        <div class="color-selector">
          <div
            v-for="opt in colorOptions"
            :key="opt.value"
            class="color-swatch"
            :class="{ active: s.backgroundColor === opt.value }"
            :style="{ background: opt.hex }"
            :title="opt.label"
            @click="selectColor(opt.value)"
          />
        </div>
        <span class="hint">{{ currentColorLabel }}</span>
      </div>

      <p class="memory-tip">高分辨率参数和大量图片会显著增加内存占用；如处理失败，请降低图片数量或尺寸参数。</p>
    </div>

    <div class="card">
      <p class="section-title">输出质量</p>

      <div class="form-row">
        <label>JPEG 质量</label>
        <input v-model.number="s.jpegQuality" type="number" min="1" max="100" style="max-width:90px" @change="save" />
        <span class="hint">默认 95</span>
      </div>

      <div class="form-row">
        <label>
          <input type="checkbox" v-model="s.autoOrient" style="margin-right: 6px" @change="save" />
          EXIF 自动旋正
        </label>
      </div>
    </div>

    <div class="card">
      <p class="section-title">色彩管理</p>

      <div class="form-row">
        <label>
          <input type="checkbox" v-model="s.colorManagementEnabled" style="margin-right: 6px" @change="save" />
          启用 ICC 转换
        </label>
      </div>

      <template v-if="s.colorManagementEnabled">
        <div class="form-row">
          <label>目标 Profile</label>
          <select v-model="s.targetProfileMode" style="max-width:120px" @change="save">
            <option value="srgb">sRGB</option>
            <option value="custom">自定义 ICC</option>
          </select>
          <button
            v-if="s.targetProfileMode === 'custom'"
            class="btn-secondary icc-btn"
            @click="selectIccProfile"
          >
            选择 ICC
          </button>
          <span v-if="s.targetProfileMode === 'custom'" class="hint icc-name" :title="s.targetProfilePath">
            {{ iccBasename || '未选择' }}
          </span>
        </div>

        <div class="form-row">
          <label>渲染意图</label>
          <select v-model="s.renderingIntent" style="max-width:170px" @change="save">
            <option value="perceptual">Perceptual</option>
            <option value="relative_colorimetric">Relative Colorimetric</option>
          </select>
        </div>
      </template>
    </div>

  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAppStore } from '../stores/appStore'
import { BACKGROUND_COLOR_OPTIONS, type BackgroundColor } from '../types/protocol'

const store = useAppStore()
const s = store.settings
const colorOptions = BACKGROUND_COLOR_OPTIONS

const currentColorLabel = computed(
  () => colorOptions.find((o) => o.value === s.backgroundColor)?.label ?? ''
)

const iccBasename = computed(() => {
  if (!s.targetProfilePath) return ''
  return s.targetProfilePath.replace(/\\/g, '/').split('/').pop() ?? s.targetProfilePath
})

function selectColor(val: BackgroundColor) {
  s.backgroundColor = val
  save()
}

function save() {
  store.saveSettings()
}

async function selectIccProfile() {
  const path = await window.electronAPI.openIccProfile()
  if (!path) return
  s.targetProfilePath = path
  save()
}
</script>

<style scoped>
.settings-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.color-selector {
  display: flex;
  gap: 6px;
}

.color-swatch {
  width: 22px;
  height: 22px;
  border-radius: 4px;
  border: 2px solid var(--color-border);
  cursor: pointer;
  transition: transform 0.1s, border-color 0.1s;
}

.color-swatch.active {
  border-color: var(--color-primary);
  transform: scale(1.2);
}

.color-swatch:hover:not(.active) {
  border-color: #999;
}

.memory-tip {
  font-size: 11px;
  color: var(--color-text-secondary);
  line-height: 1.5;
}

.icc-btn {
  width: auto;
  padding: 5px 10px;
  font-size: 12px;
}

.icc-name {
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
