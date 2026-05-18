<script setup lang="ts">
import { ref, computed } from 'vue'
import type { ModelInfo, DownloadProgress, DownloadRequest } from '@/types'
import QuantPickerPanel from './QuantPickerPanel.vue'
import DownloadProgressBar from './DownloadProgressBar.vue'

const props = defineProps<{
  model: ModelInfo
  progress?: DownloadProgress
  downloading?: boolean
  disabled?: boolean
}>()

const emit = defineEmits<{
  download: [req: DownloadRequest]
  cancel: []
}>()

const showPicker = ref(false)

const availableBackends = computed(() => {
  const backends: { value: string; label: string }[] = []
  if (props.model.llama_cpp_url || props.model.repo) backends.push({ value: 'llama_cpp', label: 'llama.cpp' })
  if (props.model.cactus_url) backends.push({ value: 'cactus', label: 'Cactus' })
  return backends
})

const selectedBackend = ref(props.model.default_backend)

// llama.cpp + HF repo → quant picker is available
const useQuantPicker = computed(
  () => selectedBackend.value === 'llama_cpp' && !!props.model.repo,
)

// llama.cpp + direct URL → catalog has a default quant we can download directly
const hasDefaultQuant = computed(
  () => useQuantPicker.value && !!props.model.llama_cpp_url,
)

function handleDownloadClick() {
  if (hasDefaultQuant.value) {
    emit('download', {
      backend: selectedBackend.value,
      filename: props.model.filename,
      url: props.model.llama_cpp_url,
      size_bytes: props.model.llama_cpp_size_mb
        ? props.model.llama_cpp_size_mb * 1024 * 1024
        : undefined,
    })
  } else if (useQuantPicker.value) {
    showPicker.value = !showPicker.value
  } else {
    emit('download', { backend: selectedBackend.value })
  }
}

function togglePicker() {
  showPicker.value = !showPicker.value
}

function handleQuantSelected(req: DownloadRequest) {
  showPicker.value = false
  emit('download', req)
}

const capBadge = (cap: string) => {
  if (cap === 'chat') return { cls: 'badge-chat', icon: '💬', label: 'Chat' }
  if (cap === 'vision') return { cls: 'badge-vision', icon: '🖼️', label: 'Vision' }
  if (cap === 'audio') return { cls: 'badge-audio', icon: '🎙️', label: 'Audio' }
  return { cls: 'badge', icon: '', label: cap }
}

const sizeLabel = computed(() => {
  const mb = selectedBackend.value === 'cactus'
    ? props.model.cactus_size_mb
    : props.model.llama_cpp_size_mb
  if (!mb) return null
  const size = mb >= 1000 ? `~${(mb / 1000).toFixed(1)} GB` : `~${mb} MB`
  const quant = selectedBackend.value === 'llama_cpp' ? props.model.llama_cpp_quant : null
  return quant ? `${size} · ${quant}` : size
})


</script>

<template>
  <div
    class="card p-5 transition-all duration-200"
    :class="{ 'opacity-50': disabled && !downloading }"
  >
    <div class="flex items-start justify-between gap-4">
      <div class="flex-1 min-w-0">
        <div class="flex items-center gap-2 flex-wrap mb-1">
          <h3 class="font-semibold text-zinc-100 text-sm">{{ model.display_name }}</h3>
          <span class="text-xs text-zinc-500">by {{ model.provider }}</span>
        </div>
        <p class="text-xs text-zinc-400 mb-3">{{ model.description }}</p>
        <div class="flex flex-wrap gap-1.5 mb-3">
          <span
            v-for="cap in model.capabilities"
            :key="cap"
            :class="capBadge(cap).cls"
          >
            {{ capBadge(cap).icon }} {{ capBadge(cap).label }}
          </span>
        </div>
        <span v-if="sizeLabel" class="text-xs text-zinc-500">{{ sizeLabel }}</span>
        <span v-else-if="useQuantPicker" class="text-xs text-zinc-500">multiple quants available</span>
      </div>

      <div class="shrink-0 flex flex-col items-end gap-2">
        <!-- Backend selector -->
        <div v-if="availableBackends.length > 0 && !downloading" class="flex rounded-md overflow-hidden border border-zinc-700 text-xs">
          <template v-if="availableBackends.length > 1">
            <button
              v-for="b in availableBackends"
              :key="b.value"
              class="px-2.5 py-1 transition-colors"
              :class="selectedBackend === b.value
                ? 'bg-zinc-600 text-zinc-100'
                : 'bg-zinc-800 text-zinc-400 hover:text-zinc-200'"
              @click="selectedBackend = b.value; showPicker = false"
            >
              {{ b.label }}
            </button>
          </template>
          <span v-else class="px-2.5 py-1 bg-zinc-800 text-zinc-400">
            {{ availableBackends[0].label }}
          </span>
        </div>

        <template v-if="!downloading">
          <button
            class="btn-primary text-xs"
            :disabled="disabled"
            @click="handleDownloadClick"
          >
            {{ hasDefaultQuant
              ? model.llama_cpp_quant ? `Download ${model.llama_cpp_quant}` : 'Download'
              : useQuantPicker ? (showPicker ? 'Hide Quants' : 'Choose Quant…')
              : 'Download' }}
          </button>
          <button
            v-if="hasDefaultQuant"
            class="btn-ghost text-xs"
            :disabled="disabled"
            @click="togglePicker"
          >
            {{ showPicker ? 'Hide Quants' : 'Choose Quant…' }}
          </button>
        </template>
        <button
          v-else
          class="btn-secondary text-xs"
          @click="emit('cancel')"
        >
          Cancel
        </button>
      </div>
    </div>

    <!-- Quant picker (inline, below card content) -->
    <QuantPickerPanel
      v-if="showPicker && !downloading"
      :repo="model.repo"
      :backend="selectedBackend"
      :recommended-filename="model.filename || undefined"
      @select="handleQuantSelected"
      @close="showPicker = false"
    />

    <!-- Download / extraction progress -->
    <DownloadProgressBar v-if="downloading && progress" :progress="progress" class="mt-4" />
  </div>
</template>
