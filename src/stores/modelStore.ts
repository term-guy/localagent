import { defineStore } from 'pinia'
import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { ModelInfo, InstalledModel, DownloadProgress } from '@/types'

export const useModelStore = defineStore('model', () => {
  const catalog = ref<ModelInfo[]>([])
  const installed = ref<InstalledModel[]>([])
  const activeModelId = ref<string | null>(null)
  const activeModelBackend = ref<string | null>(null)
  const modelLoading = ref(false)
  const downloadProgress = ref<Record<string, DownloadProgress>>({})
  const unlisteners = ref<UnlistenFn[]>([])

  const installedIds = computed(() => new Set(installed.value.map((m) => m.id)))
  const activeModel = computed(() =>
    installed.value.find(
      (m) => m.id === activeModelId.value && m.backend === activeModelBackend.value,
    ) ?? null,
  )
  const availableModels = computed(() =>
    catalog.value.filter((m) => !installedIds.value.has(m.id)),
  )

  async function loadCatalog() {
    catalog.value = await invoke<ModelInfo[]>('list_catalog')
  }

  async function loadInstalled() {
    installed.value = await invoke<InstalledModel[]>('list_installed')
    if (!activeModelId.value && installed.value.length > 0) {
      activeModelId.value = installed.value[0].id
      activeModelBackend.value = installed.value[0].backend
    }
  }

  async function setupListeners() {
    const unlisten1 = await listen<DownloadProgress>('download-progress', (e) => {
      downloadProgress.value[e.payload.model_id] = e.payload
    })
    const unlisten2 = await listen<{ model_id: string }>('download-complete', async (e) => {
      delete downloadProgress.value[e.payload.model_id]
      await loadInstalled()
    })
    const unlisten3 = await listen<{ model_id: string; error: string }>('download-error', (e) => {
      delete downloadProgress.value[e.payload.model_id]
    })
    unlisteners.value.push(unlisten1, unlisten2, unlisten3)
  }

  async function downloadModel(
    modelId: string,
    backend?: string,
    filename?: string,
    url?: string,
    sizeBytes?: number,
  ) {
    await invoke('download_model', { modelId, backend, filename, url, sizeBytes })
  }

  async function downloadHfModel(
    repo: string,
    filename: string,
    url: string,
    sizeBytes: number,
    backend: string,
  ) {
    await invoke('download_hf_model', { repo, filename, url, sizeBytes, backend })
  }

  async function cancelDownload(modelId: string) {
    await invoke('cancel_download', { modelId })
    delete downloadProgress.value[modelId]
  }

  async function removeModel(modelId: string, backend: string) {
    await invoke('remove_model', { modelId, backend })
    await loadInstalled()
    if (activeModelId.value === modelId && activeModelBackend.value === backend) {
      activeModelId.value = installed.value[0]?.id ?? null
      activeModelBackend.value = installed.value[0]?.backend ?? null
    }
  }

  function setActiveModel(modelId: string, backend: string) {
    activeModelId.value = modelId
    activeModelBackend.value = backend
  }

  watch(
    () => `${activeModelId.value}:${activeModelBackend.value}`,
    async (_newKey) => {
      const id = activeModelId.value
      const backend = activeModelBackend.value
      if (!id || !backend) return
      const model = installed.value.find((m) => m.id === id && m.backend === backend)
      if (!model) return

      modelLoading.value = true
      try {
        await invoke('load_model', { modelId: id, backend })
      } catch (e) {
        console.error(`Failed to preload model ${id} (${backend}):`, e)
      } finally {
        if (activeModelId.value === id && activeModelBackend.value === backend) {
          modelLoading.value = false
        }
      }
    },
  )

  function cleanup() {
    unlisteners.value.forEach((fn) => fn())
    unlisteners.value = []
  }

  return {
    catalog,
    installed,
    activeModelId,
    activeModelBackend,
    modelLoading,
    downloadProgress,
    activeModel,
    availableModels,
    installedIds,
    loadCatalog,
    loadInstalled,
    setupListeners,
    downloadModel,
    downloadHfModel,
    cancelDownload,
    removeModel,
    setActiveModel,
    cleanup,
  }
})
