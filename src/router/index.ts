import { createRouter, createWebHistory } from 'vue-router'
import { useModelStore } from '@/stores/modelStore'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/setup',
      name: 'setup',
      component: () => import('@/views/SetupView.vue'),
    },
    {
      path: '/',
      name: 'chat',
      component: () => import('@/views/ChatView.vue'),
    },
    {
      path: '/settings',
      name: 'settings',
      component: () => import('@/views/SettingsView.vue'),
    },
  ],
})

router.beforeEach(async (to) => {
  const modelStore = useModelStore()
  await modelStore.loadInstalled()

  const hasModel = modelStore.installed.length > 0

  if (!hasModel && to.name !== 'setup') {
    return { name: 'setup' }
  }
  if (hasModel && to.name === 'setup') {
    return { name: 'chat' }
  }
})

export default router
