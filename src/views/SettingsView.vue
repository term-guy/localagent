<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useModelStore } from '@/stores/modelStore'
import { useToast } from '@/composables/useToast'
import { useConfirm } from '@/composables/useConfirm'
import ModelCard from '@/components/ModelCard.vue'
import DownloadProgressBar from '@/components/DownloadProgressBar.vue'
import HfRepoImporter from '@/components/HfRepoImporter.vue'
import type { ModelInfo, InstalledModel, DownloadRequest } from '@/types'



const router = useRouter()
const modelStore = useModelStore()
const { show } = useToast()
const { confirm } = useConfirm()

const modelsDir = ref('')
const totalDiskUsage = computed(() => {
  const bytes = modelStore.installed.reduce((sum, m) => sum + m.file_size_bytes, 0)
  const gb = bytes / (1000 ** 3)
  return gb >= 1 ? `${gb.toFixed(2)} GB` : `${(bytes / (1000 ** 2)).toFixed(0)} MB`
})

onMounted(async () => {
  await modelStore.loadCatalog()
  await modelStore.loadInstalled()
  modelsDir.value = await invoke<string>('get_models_dir')
})

async function removeModel(model: InstalledModel) {
  const ok = await confirm({
    title: 'Remove Model',
    message: 'Remove this model from disk? You can re-download it later.',
    confirmLabel: 'Remove',
    danger: true,
  })
  if (!ok) return
  try {
    await modelStore.removeModel(model.id, model.backend)
    show('Model removed', 'success')
  } catch (e) {
    show(`Failed to remove: ${e}`, 'error')
  }
}

async function startDownload(model: ModelInfo, req: DownloadRequest) {
  try {
    await modelStore.downloadModel(model.id, req.backend, req.filename, req.url, req.size_bytes)
  } catch (e) {
    show(`Download failed: ${e}`, 'error')
  }
}

async function revealInFinder() {
  try {
    await invoke('reveal_models_dir')
  } catch (e) {
    show(`Could not open folder: ${e}`, 'error')
  }
}

function formatBytes(bytes: number) {
  const gb = bytes / (1000 ** 3)
  return gb >= 1 ? `${gb.toFixed(2)} GB` : `${(bytes / (1000 ** 2)).toFixed(0)} MB`
}

const installedGroups = computed(() => {
  const groups = new Map<string, InstalledModel[]>()
  for (const m of modelStore.installed) {
    if (!groups.has(m.id)) groups.set(m.id, [])
    groups.get(m.id)!.push(m)
  }
  return [...groups.values()]
})

function missingBackends(group: InstalledModel[]): { value: string; label: string }[] {
  const has = new Set(group.map((m) => m.backend))
  const ref = group[0]
  const missing: { value: string; label: string }[] = []
  if (!has.has('llama_cpp') && (ref.llama_cpp_url || ref.repo))
    missing.push({ value: 'llama_cpp', label: 'llama.cpp' })
  if (!has.has('cactus') && ref.cactus_url)
    missing.push({ value: 'cactus', label: 'Cactus' })
  return missing
}

async function downloadForBackend(model: InstalledModel, backend: string, label: string) {
  const ok = await confirm({
    title: 'Download Additional Backend',
    message: `Download "${model.display_name}" for ${label}? Both versions will be kept.`,
    confirmLabel: 'Download',
    danger: false,
  })
  if (!ok) return
  try {
    await modelStore.downloadModel(model.id, backend)
    show(`Downloading ${model.display_name} (${label})…`, 'info')
  } catch (e) {
    show(`Failed: ${e}`, 'error')
  }
}

function backendLabel(backend: string) {
  return backend === 'llama_cpp' ? 'llama.cpp' : 'Cactus'
}

function capBadge(cap: string) {
  if (cap === 'chat') return { cls: 'badge-chat', icon: '💬', label: 'Chat' }
  if (cap === 'vision') return { cls: 'badge-vision', icon: '🖼️', label: 'Vision' }
  if (cap === 'audio') return { cls: 'badge-audio', icon: '🎙️', label: 'Audio' }
  return { cls: 'badge', icon: '', label: cap }
}
</script>

<template>
  <div class="h-screen flex flex-col bg-zinc-900 overflow-hidden">
    <!-- Header -->
    <header class="flex items-center gap-4 px-6 h-14 border-b border-zinc-800 shrink-0">
      <button class="btn-ghost text-sm gap-1.5" @click="router.back()">
        ← Back
      </button>
      <h1 class="text-base font-semibold text-zinc-100">Settings</h1>
    </header>

    <div class="flex-1 overflow-y-auto px-6 py-8 max-w-3xl mx-auto w-full space-y-10">

      <!-- Installed models -->
      <section>
        <h2 class="text-sm font-semibold text-zinc-300 mb-4 uppercase tracking-wider">
          Installed Models
        </h2>

        <div v-if="modelStore.installed.length === 0" class="text-sm text-zinc-500">
          No models installed.
        </div>

        <div class="space-y-3">
          <div
            v-for="group in installedGroups"
            :key="group[0].id"
            class="card p-4"
          >
            <!-- Model header -->
            <div class="flex items-start justify-between gap-4 mb-3">
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2 flex-wrap mb-1">
                  <span class="text-sm font-medium text-zinc-100">{{ group[0].display_name }}</span>
                  <span class="text-xs text-zinc-500">· {{ group[0].provider }}</span>
                </div>
                <div class="flex flex-wrap gap-1.5">
                  <span
                    v-for="cap in group[0].capabilities"
                    :key="cap"
                    :class="capBadge(cap).cls"
                  >
                    {{ capBadge(cap).icon }} {{ capBadge(cap).label }}
                  </span>
                </div>
              </div>

              <!-- Download missing backend / Cancel -->
              <div class="flex items-center gap-2 shrink-0">
                <template v-if="modelStore.downloadProgress[group[0].id]">
                  <button class="btn-secondary text-xs" @click="modelStore.cancelDownload(group[0].id)">
                    Cancel
                  </button>
                </template>
                <template v-else>
                  <button
                    v-for="alt in missingBackends(group)"
                    :key="alt.value"
                    class="btn-secondary text-xs"
                    :disabled="Object.keys(modelStore.downloadProgress).length > 0"
                    @click="downloadForBackend(group[0], alt.value, alt.label)"
                  >
                    Download ({{ alt.label }})
                  </button>
                </template>
              </div>
            </div>

            <!-- One row per installed backend -->
            <div class="space-y-2">
              <div
                v-for="model in group"
                :key="model.backend"
                class="flex items-center justify-between"
              >
                <div class="flex items-center gap-2 flex-wrap">
                  <span class="text-xs text-zinc-600 bg-zinc-800 rounded px-1.5 py-0.5">
                    {{ backendLabel(model.backend) }}
                  </span>
                  <span class="text-xs text-zinc-500">{{ formatBytes(model.file_size_bytes) }}</span>
                  <span
                    v-if="model.id === modelStore.activeModelId && model.backend === modelStore.activeModelBackend"
                    class="text-xs font-medium text-green-400 bg-green-400/10 rounded-full px-2 py-0.5"
                  >
                    Active
                  </span>
                </div>
                <button class="btn-danger text-xs" @click="removeModel(model)">
                  Remove
                </button>
              </div>
            </div>

            <DownloadProgressBar
              v-if="modelStore.downloadProgress[group[0].id]"
              :progress="modelStore.downloadProgress[group[0].id]"
              class="mt-3"
            />
          </div>
        </div>
      </section>

      <!-- Available models -->
      <section v-if="modelStore.availableModels.length > 0">
        <h2 class="text-sm font-semibold text-zinc-300 mb-4 uppercase tracking-wider">
          Available Models
        </h2>
        <div class="space-y-3">
          <ModelCard
            v-for="model in modelStore.availableModels"
            :key="model.id"
            :model="model"
            :progress="modelStore.downloadProgress[model.id]"
            :downloading="!!modelStore.downloadProgress[model.id]"
            :disabled="Object.keys(modelStore.downloadProgress).length > 0
                       && !modelStore.downloadProgress[model.id]"
            @download="(req) => startDownload(model, req)"
            @cancel="modelStore.cancelDownload(model.id)"
          />
        </div>
      </section>

      <!-- Add from HuggingFace -->
      <section>
        <h2 class="text-sm font-semibold text-zinc-300 mb-4 uppercase tracking-wider">
          Add from HuggingFace
        </h2>
        <HfRepoImporter
          :disabled="Object.keys(modelStore.downloadProgress).length > 0"
        />
      </section>

      <!-- Storage -->
      <section>
        <h2 class="text-sm font-semibold text-zinc-300 mb-4 uppercase tracking-wider">
          Storage
        </h2>
        <div class="card p-4 space-y-3">
          <div class="flex items-center justify-between text-sm">
            <span class="text-zinc-400">Total disk usage</span>
            <span class="font-medium text-zinc-200">{{ totalDiskUsage }}</span>
          </div>
          <div class="flex items-start justify-between gap-4">
            <div class="flex-1 min-w-0">
              <p class="text-xs text-zinc-500 mb-0.5">Model files location</p>
              <p class="text-xs font-mono text-zinc-400 break-all">{{ modelsDir }}</p>
            </div>
            <button class="btn-secondary text-xs shrink-0" @click="revealInFinder">
              Reveal
            </button>
          </div>
        </div>
      </section>

    </div>
  </div>
</template>
