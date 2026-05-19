<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useModelStore } from '@/stores/modelStore'
import { useToast } from '@/composables/useToast'
import { listen } from '@tauri-apps/api/event'
import ModelCard from '@/components/ModelCard.vue'
import HfRepoImporter from '@/components/HfRepoImporter.vue'
import type { ModelInfo, DownloadRequest } from '@/types'

const router = useRouter()
const modelStore = useModelStore()
const { show } = useToast()

const downloading = computed(() => Object.keys(modelStore.downloadProgress))
const isAnyDownloading = computed(() => downloading.value.length > 0)

onMounted(async () => {
  await modelStore.loadCatalog()

  const unlisten = await listen<{ model_id: string }>('download-complete', async () => {
    await modelStore.loadInstalled()
    show('Model downloaded successfully!', 'success')
    router.push('/')
    unlisten()
  })
})

async function startDownload(model: ModelInfo, req: DownloadRequest) {
  try {
    await modelStore.downloadModel(model.id, req.backend, req.filename, req.url, req.size_bytes)
  } catch (e) {
    show(`Download failed: ${e}`, 'error')
  }
}

async function cancelDownload(modelId: string) {
  await modelStore.cancelDownload(modelId)
}
</script>

<template>
  <div class="h-full overflow-y-auto flex flex-col items-center px-4 py-12 bg-zinc-900">
    <!-- Logo / header -->
    <div class="mb-10 text-center">
      <div class="mb-4 flex justify-center">
        <div class="h-16 w-16 rounded-2xl bg-gradient-to-br from-primary-600 to-primary-800
                    flex items-center justify-center shadow-lg shadow-primary-900/40">
          <span class="text-3xl">🤖</span>
        </div>
      </div>
      <h1 class="text-3xl font-bold text-zinc-100 tracking-tight">localagent</h1>
      <p class="mt-2 text-zinc-400">Your private AI assistant — fully offline</p>
    </div>

    <div class="w-full max-w-xl">
      <p class="text-sm font-medium text-zinc-400 mb-4 text-center">
        Choose your first model to get started
      </p>

      <div class="space-y-3">
        <ModelCard
          v-for="model in modelStore.catalog"
          :key="model.id"
          :model="model"
          :progress="modelStore.downloadProgress[model.id]"
          :downloading="!!modelStore.downloadProgress[model.id]"
          :disabled="isAnyDownloading && !modelStore.downloadProgress[model.id]"
          @download="(req) => startDownload(model, req)"
          @cancel="cancelDownload(model.id)"
        />

        <div class="relative flex items-center py-1">
          <div class="flex-1 border-t border-zinc-700/60" />
          <span class="mx-3 text-xs text-zinc-600">or</span>
          <div class="flex-1 border-t border-zinc-700/60" />
        </div>

        <HfRepoImporter :disabled="isAnyDownloading" />
      </div>
    </div>
  </div>
</template>
