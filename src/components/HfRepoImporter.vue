<script setup lang="ts">
import { ref, computed } from 'vue'
import { useModelStore } from '@/stores/modelStore'
import { useToast } from '@/composables/useToast'
import QuantPickerPanel from './QuantPickerPanel.vue'
import DownloadProgressBar from './DownloadProgressBar.vue'
import type { DownloadRequest } from '@/types'

const props = defineProps<{
  disabled?: boolean
}>()

const modelStore = useModelStore()
const { show } = useToast()

const repoInput = ref('')
const activeRepo = ref('')
const showPicker = ref(false)

function modelIdFromRepo(repo: string) {
  return repo.replaceAll('/', '--')
}

const modelId = computed(() => modelIdFromRepo(activeRepo.value))
const progress = computed(() => modelStore.downloadProgress[modelId.value] ?? null)
const isDownloading = computed(() => !!progress.value)

function browse() {
  const repo = repoInput.value.trim()
  if (!repo || isDownloading.value || props.disabled) return
  activeRepo.value = repo
  showPicker.value = true
}

async function startDownload(req: DownloadRequest) {
  if (!activeRepo.value || !req.filename || !req.url) return
  try {
    await modelStore.downloadHfModel(
      activeRepo.value,
      req.filename,
      req.url,
      req.size_bytes ?? 0,
      req.backend,
    )
    showPicker.value = false
  } catch (e) {
    show(`Download failed: ${e}`, 'error')
  }
}
</script>

<template>
  <div class="card p-4">
    <h3 class="text-sm font-medium text-zinc-300 mb-1">From HuggingFace</h3>
    <p class="text-xs text-zinc-500 mb-3">Enter a GGUF repo to browse available quantizations</p>

    <div class="flex gap-2">
      <input
        v-model="repoInput"
        placeholder="e.g. unsloth/gemma-4-E2B-it-GGUF"
        class="flex-1 bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-2 text-sm text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-primary-500 disabled:opacity-40"
        :disabled="isDownloading || showPicker || disabled"
        @keydown.enter="browse"
      />
      <button
        class="btn-primary text-sm"
        :disabled="!repoInput.trim() || isDownloading || showPicker || disabled"
        @click="browse"
      >
        Browse
      </button>
    </div>

    <!-- Download progress -->
    <template v-if="isDownloading && progress">
      <div class="mt-4 space-y-2">
        <div class="flex items-center justify-between text-xs text-zinc-400">
          <span class="truncate font-mono">{{ activeRepo }}</span>
          <button
            class="btn-ghost text-xs ml-2 shrink-0"
            @click="modelStore.cancelDownload(modelId)"
          >
            Cancel
          </button>
        </div>
        <DownloadProgressBar :progress="progress" />
      </div>
    </template>

    <!-- Quant picker -->
    <QuantPickerPanel
      v-else-if="showPicker && activeRepo"
      :repo="activeRepo"
      backend="llama_cpp"
      @select="startDownload"
      @close="showPicker = false"
    />
  </div>
</template>
