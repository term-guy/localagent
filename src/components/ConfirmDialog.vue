<script setup lang="ts">
import { useConfirm } from '@/composables/useConfirm'

const { visible, options, onConfirm, onCancel } = useConfirm()
</script>

<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition-all duration-200 ease-out"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition-all duration-150 ease-in"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="visible"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
        @click.self="onCancel"
      >
        <div
          class="card w-full max-w-sm mx-4 p-6 shadow-2xl"
          @click.stop
        >
          <h3 class="text-base font-semibold text-zinc-100 mb-2">
            {{ options.title ?? 'Confirm' }}
          </h3>
          <p class="text-sm text-zinc-400 mb-6">{{ options.message }}</p>
          <div class="flex justify-end gap-3">
            <button class="btn-secondary" @click="onCancel">
              {{ options.cancelLabel ?? 'Cancel' }}
            </button>
            <button
              :class="options.danger ? 'btn-danger' : 'btn-primary'"
              @click="onConfirm"
            >
              {{ options.confirmLabel ?? 'Confirm' }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
