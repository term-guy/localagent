<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useModelStore } from '@/stores/modelStore'
import { useChatStore } from '@/stores/chatStore'
import Toast from '@/components/Toast.vue'
import ConfirmDialog from '@/components/ConfirmDialog.vue'

const modelStore = useModelStore()
const chatStore = useChatStore()

onMounted(async () => {
  await modelStore.loadCatalog()
  await modelStore.setupListeners()
  await chatStore.setupListeners()
  await chatStore.loadSessions()
})

onUnmounted(() => {
  modelStore.cleanup()
  chatStore.cleanup()
})
</script>

<template>
  <div class="h-screen w-screen overflow-hidden bg-zinc-900">
    <RouterView />
    <Toast />
    <ConfirmDialog />
  </div>
</template>
