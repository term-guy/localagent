<script setup lang="ts">
import { useToast } from '@/composables/useToast'

const { toasts, dismiss } = useToast()
</script>

<template>
  <Teleport to="body">
    <div class="fixed bottom-6 right-6 z-50 flex flex-col gap-2 pointer-events-none">
      <TransitionGroup
        enter-active-class="transition-all duration-300 ease-out"
        enter-from-class="opacity-0 translate-y-2 scale-95"
        enter-to-class="opacity-100 translate-y-0 scale-100"
        leave-active-class="transition-all duration-200 ease-in"
        leave-from-class="opacity-100"
        leave-to-class="opacity-0 scale-95"
      >
        <div
          v-for="toast in toasts"
          :key="toast.id"
          class="pointer-events-auto flex items-center gap-3 rounded-xl px-4 py-3 text-sm
                 shadow-xl min-w-[240px] max-w-sm"
          :class="{
            'bg-zinc-800 border border-zinc-700 text-zinc-100': toast.type === 'info',
            'bg-green-900/80 border border-green-700 text-green-100': toast.type === 'success',
            'bg-red-900/80 border border-red-700 text-red-100': toast.type === 'error',
          }"
        >
          <span v-if="toast.type === 'success'" class="text-green-400">✓</span>
          <span v-else-if="toast.type === 'error'" class="text-red-400">✗</span>
          <span v-else class="text-zinc-400">ℹ</span>
          <span class="flex-1">{{ toast.message }}</span>
          <button
            class="text-current opacity-50 hover:opacity-100 transition-opacity"
            @click="dismiss(toast.id)"
          >✕</button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>
