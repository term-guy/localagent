<script setup lang="ts">
import { ref, watch } from 'vue'
import { useTools } from '@/composables/useTools'

const props = defineProps<{ sidebarOpen: boolean }>()

const { tools, toggleTool } = useTools()

const expanded = ref(false)

// Auto-collapse when sidebar collapses
watch(
  () => props.sidebarOpen,
  (open) => { if (!open) expanded.value = false },
)
</script>

<template>
  <div class="px-2 py-2 border-t border-zinc-800">
    <!-- Collapsed: just icon -->
    <button
      v-if="!sidebarOpen"
      class="flex items-center justify-center w-full rounded-lg p-2 text-zinc-400
             hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
      title="Tools"
      @click="expanded = !expanded"
    >
      🛠️
    </button>

    <!-- Expanded sidebar: header row -->
    <template v-else>
      <button
        class="flex items-center gap-2 w-full rounded-lg px-2 py-1.5 text-sm text-zinc-400
               hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
        @click="expanded = !expanded"
      >
        <span class="text-base shrink-0">🛠️</span>
        <span class="flex-1 text-left text-xs font-medium">Tools</span>
        <span class="text-zinc-600 text-[10px]">{{ expanded ? '▲' : '▼' }}</span>
      </button>

      <!-- Tool list -->
      <div v-if="expanded" class="mt-1 space-y-1">
        <div
          v-for="tool in tools"
          :key="tool.id"
          class="rounded-lg bg-zinc-800/60 px-2.5 py-2"
        >
          <!-- Tool row -->
          <div class="flex items-center gap-2">
            <span class="text-sm shrink-0">{{ tool.icon }}</span>
            <span class="flex-1 text-xs text-zinc-300">{{ tool.name }}</span>
            <!-- Toggle switch -->
            <button
              class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full
                     transition-colors duration-200 focus:outline-none"
              :class="tool.enabled ? 'bg-primary-600' : 'bg-zinc-700'"
              :title="tool.enabled ? 'Disable' : 'Enable'"
              @click="toggleTool(tool.id)"
            >
              <span
                class="inline-block h-4 w-4 rounded-full bg-white shadow transform
                       transition-transform duration-200 mt-0.5"
                :class="tool.enabled ? 'translate-x-4' : 'translate-x-0.5'"
              />
            </button>
          </div>

        </div>
      </div>
    </template>
  </div>
</template>
