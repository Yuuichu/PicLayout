<template>
  <div v-if="hasActivity" class="progress-area" :class="`variant-${variant}`">
    <div v-if="store.processing" class="card progress-card">
      <div class="status-row">
        <div class="status-text">{{ store.statusMessage || '正在处理...' }}</div>
        <div class="time-text">总耗时 {{ formatMs(store.wallElapsedMs) }}</div>
      </div>
      <div class="progress-track">
        <div class="progress-fill" :style="{ width: store.progress + '%' }" />
      </div>
      <div class="progress-pct">{{ store.progress }}%</div>
      <div v-if="visibleTimings.length" class="stage-timings">
        <span v-for="item in visibleTimings" :key="item.stage">
          {{ item.label }} {{ formatMs(item.elapsed_ms) }}
          <template v-if="item.detailsLabel">（{{ item.detailsLabel }}）</template>
        </span>
      </div>
      <button class="btn-danger cancel-btn" @click="cancel">取消</button>
    </div>

    <div v-else-if="store.outputFiles.length > 0" class="card result-card">
      <p class="result-title">拼贴完成</p>
      <p class="result-summary">
        成功处理 {{ store.processedCount }} 张
        <span v-if="store.failedImages.length">，失败 {{ store.failedImages.length }} 张</span>
      </p>
      <p class="result-summary">
        Rust 核心耗时 {{ formatMs(store.elapsedMs) }}
        <span v-if="store.wallElapsedMs">，总耗时 {{ formatMs(store.wallElapsedMs) }}</span>
      </p>
      <div v-if="visibleTimings.length" class="stage-timings">
        <span v-for="item in visibleTimings" :key="item.stage">
          {{ item.label }} {{ formatMs(item.elapsed_ms) }}
          <template v-if="item.detailsLabel">（{{ item.detailsLabel }}）</template>
        </span>
      </div>
      <ul class="output-list">
        <li v-for="f in store.outputFiles" :key="f" :title="f">
          {{ basename(f) }}
        </li>
      </ul>
      <button class="btn-secondary open-dir-btn" @click="openOutputDir">打开输出目录</button>
      <p v-if="store.errorMessage" class="open-dir-error">{{ store.errorMessage }}</p>
      <div v-if="store.warnings.length" class="warning-list">
        <p v-for="warning in store.warnings" :key="warning">{{ warning }}</p>
      </div>
      <div v-if="store.failedImages.length" class="failed-list">
        <p class="failed-title">失败图片</p>
        <p v-for="item in store.failedImages" :key="item.path" :title="item.path">
          {{ basename(item.path) }}：{{ item.message }}
        </p>
      </div>
    </div>

    <div v-else-if="store.cancelledMessage" class="card cancelled-card">
      <p class="cancelled-title">已取消</p>
      <p class="cancelled-msg">{{ store.cancelledMessage }}</p>
      <ul v-if="store.partialOutputs.length" class="output-list">
        <li v-for="f in store.partialOutputs" :key="f" :title="f">
          {{ basename(f) }}
        </li>
      </ul>
    </div>

    <div v-else-if="store.errorMessage" class="card error-card">
      <p class="error-title">处理失败</p>
      <p class="error-msg">{{ store.errorMessage }}</p>
    </div>
  </div>

  <div v-else class="activity-empty">
    <p>暂无处理记录</p>
    <span>开始导出后，这里会显示阶段进度、耗时、输出文件和错误信息。</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAppStore } from '../stores/appStore'

const props = withDefaults(
  defineProps<{
    variant?: 'panel'
  }>(),
  {
    variant: 'panel',
  }
)

const store = useAppStore()

const stageLabels: Record<string, string> = {
  processing_images: '单图处理',
  creating_collage: '贴入最终画布',
  adding_border: '最终边框',
  adding_watermark: '水印',
  saving_output: 'JPEG 输出',
}

const detailLabels: Record<string, string> = {
  decode: '解码',
  color_orient: '颜色/方向',
  resize: '缩放',
}

const visibleTimings = computed(() =>
  store.stageTimings.map((item) => ({
    ...item,
    label: stageLabels[item.stage] ?? item.stage,
    detailsLabel: (item.details ?? [])
      .filter((detail) => detail.elapsed_ms > 0)
      .map((detail) => `${detailLabels[detail.name] ?? detail.name} ${formatMs(detail.elapsed_ms)}`)
      .join(' / '),
  }))
)

const variant = computed(() => props.variant)

const hasActivity = computed(
  () =>
    store.processing ||
    store.outputFiles.length > 0 ||
    !!store.cancelledMessage ||
    !!store.errorMessage
)

function basename(path: string): string {
  return path.replace(/\\/g, '/').split('/').pop() ?? path
}

function formatMs(ms: number): string {
  if (!ms) return '0.0s'
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}

async function cancel() {
  store.statusMessage = '正在取消...'
  await window.electronAPI.cancelCollage()
}

async function openOutputDir() {
  if (!store.settings.outputDir) return
  const error = await window.electronAPI.openPath(store.settings.outputDir)
  if (error) {
    store.errorMessage = error
    return
  }
  store.errorMessage = ''
}
</script>

<style scoped>
.progress-area {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.progress-card,
.result-card,
.cancelled-card,
.error-card {
  display: flex;
  flex-direction: column;
  gap: 9px;
  padding: 10px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-panel-raised);
}

.status-row {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 4px;
  align-items: flex-start;
}

.status-text {
  color: var(--color-text);
  font-size: 12px;
  font-weight: 650;
}

.time-text,
.progress-pct {
  color: var(--color-text-muted);
  font-size: 11px;
  text-align: right;
}

.progress-track {
  height: 4px;
  background: var(--color-border);
  border-radius: 999px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--color-accent);
  border-radius: 999px;
  transition: width 0.3s ease;
}

.progress-pct {
  align-self: flex-end;
}

.stage-timings {
  display: flex;
  flex-direction: column;
  gap: 6px 10px;
  font-size: 11px;
  color: var(--color-text-subtle);
}

.cancel-btn {
  align-self: flex-end;
  padding: 4px 10px;
  font-size: 12px;
}

.result-card {
  border-left: 2px solid var(--color-success);
}

.result-title {
  font-weight: 700;
  color: var(--color-success);
}

.result-summary {
  font-size: 12px;
  color: var(--color-text-muted);
}

.output-list {
  list-style: none;
  font-size: 12px;
  color: var(--color-text-muted);
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.output-list li {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.open-dir-btn {
  padding: 5px 12px;
  font-size: 12px;
}

.open-dir-error {
  margin-top: 6px;
  font-size: 12px;
  color: var(--color-danger);
}

.warning-list,
.failed-list {
  font-size: 12px;
  color: var(--color-text-muted);
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.warning-list {
  color: var(--color-warning);
}

.failed-title {
  font-weight: 700;
  color: var(--color-danger);
}

.cancelled-card {
  border-left: 2px solid var(--color-warning);
}

.cancelled-title {
  font-weight: 700;
  color: var(--color-warning);
  margin-bottom: 6px;
}

.cancelled-msg {
  font-size: 12px;
  color: var(--color-text-muted);
}

.error-card {
  border-left: 2px solid var(--color-danger);
}

.error-title {
  font-weight: 700;
  color: var(--color-danger);
  margin-bottom: 6px;
}

.error-msg {
  font-size: 12px;
  color: var(--color-text-muted);
  overflow-wrap: anywhere;
}

.activity-empty {
  display: flex;
  flex-direction: column;
  gap: 7px;
  padding: 12px;
  border: 1px dashed var(--color-border);
  border-radius: 4px;
  color: var(--color-text-subtle);
}

.activity-empty p {
  color: var(--color-text);
  font-size: 12px;
  font-weight: 700;
}

.activity-empty span {
  font-size: 11px;
  line-height: 1.5;
}
</style>
