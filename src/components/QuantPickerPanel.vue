<script setup lang="ts">
import { ref, computed, onMounted, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { HfFile, DownloadRequest } from '@/types'

const props = defineProps<{
  repo: string
  backend: string
  recommendedFilename?: string
}>()

const emit = defineEmits<{
  select: [req: DownloadRequest]
  close: []
}>()

const files = ref<HfFile[]>([])
const loading = ref(true)
const error = ref<string | null>(null)
const selected = ref<HfFile | null>(null)
const rootEl = ref<HTMLElement | null>(null)

onMounted(async () => {
  try {
    files.value = await invoke<HfFile[]>('fetch_hf_quants', { repo: props.repo })
    if (props.recommendedFilename) {
      selected.value = files.value.find(f => f.filename === props.recommendedFilename) ?? null
    }
  } catch (e) {
    error.value = String(e)
  } finally {
    loading.value = false
    nextTick(() => rootEl.value?.scrollIntoView({ behavior: 'smooth', block: 'end' }))
  }
})

function getBits(quant: string): number {
  if (/IQ1/.test(quant)) return 1
  if (/IQ2|Q2/.test(quant)) return 2
  if (/IQ3|Q3/.test(quant)) return 3
  if (/IQ4|Q4/.test(quant)) return 4
  if (/Q5/.test(quant)) return 5
  if (/Q6/.test(quant)) return 6
  if (/Q8/.test(quant)) return 8
  if (/BF16|F16/.test(quant)) return 16
  return 99
}

function bitLabel(bits: number): string {
  if (bits === 99) return 'Other'
  return `${bits}-bit`
}

const grouped = computed(() => {
  const map = new Map<number, HfFile[]>()
  for (const f of files.value) {
    const bits = getBits(f.quant_name)
    if (!map.has(bits)) map.set(bits, [])
    map.get(bits)!.push(f)
  }
  return [...map.entries()].sort((a, b) => a[0] - b[0])
})

function formatSize(bytes: number): string {
  if (bytes === 0) return '?'
  const gb = bytes / 1000 ** 3
  return gb >= 1 ? `${gb.toFixed(2)} GB` : `${(bytes / 1000 ** 2).toFixed(0)} MB`
}

function confirm() {
  if (!selected.value) return
  emit('select', {
    backend: props.backend,
    filename: selected.value.filename,
    url: selected.value.download_url,
    size_bytes: selected.value.size_bytes,
  })
}
</script>

<template>
  <div ref="rootEl" class="mt-4 border-t border-zinc-700/60 pt-4">
    <!-- Loading -->
    <div v-if="loading" class="flex items-center gap-2 text-xs text-zinc-400 py-2">
      <span class="animate-spin inline-block w-3 h-3 border border-zinc-500 border-t-transparent rounded-full" />
      Fetching available quants…
    </div>

    <!-- Error -->
    <div v-else-if="error" class="text-xs text-red-400 py-2">
      {{ error }}
      <button class="ml-2 underline opacity-70 hover:opacity-100" @click="emit('close')">dismiss</button>
    </div>

    <!-- Quant list -->
    <template v-else-if="files.length === 0">
      <p class="text-xs text-zinc-500 py-2">
        No compatible quantizations found for this repo.
      </p>
      <div class="flex justify-end mt-2">
        <button class="btn-ghost text-xs" @click="emit('close')">Dismiss</button>
      </div>
    </template>

    <template v-else>
      <p class="text-xs text-zinc-400 mb-3">Select a quantization to download:</p>
      <div class="space-y-3 max-h-72 overflow-y-auto pr-1">
        <div v-for="[bits, group] in grouped" :key="bits">
          <p class="text-[10px] font-semibold uppercase tracking-widest text-zinc-500 mb-1.5">
            {{ bitLabel(bits) }}
          </p>
          <div class="space-y-1">
            <button
              v-for="f in group"
              :key="f.filename"
              class="w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs transition-colors"
              :class="selected?.filename === f.filename
                ? 'bg-primary-600/20 border border-primary-500/50 text-zinc-100'
                : 'bg-zinc-800/60 border border-transparent text-zinc-300 hover:border-zinc-600 hover:text-zinc-100'"
              @click="selected = f"
            >
              <span class="flex items-center gap-2">
                <span class="font-mono font-medium">{{ f.quant_name }}</span>
                <span
                  v-if="f.filename === recommendedFilename"
                  class="text-[10px] font-medium text-primary-400 bg-primary-900/30 px-1.5 py-0.5 rounded"
                >Recommended</span>
              </span>
              <span class="text-zinc-400 tabular-nums ml-4">{{ formatSize(f.size_bytes) }}</span>
            </button>
          </div>
        </div>
      </div>

      <div class="flex items-center justify-end gap-2 mt-4">
        <button class="btn-ghost text-xs" @click="emit('close')">Cancel</button>
        <button
          class="btn-primary text-xs"
          :disabled="!selected"
          @click="confirm"
        >
          Download {{ selected ? selected.quant_name : '' }}
        </button>
      </div>
    </template>
  </div>
</template>
