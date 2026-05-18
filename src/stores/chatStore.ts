import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { v4 as uuidv4 } from 'uuid'
import type { ChatMessage, SessionMeta, InferenceStats } from '@/types'
import { useModelStore } from '@/stores/modelStore'

interface PendingAttachments {
  image?: string
  audio?: string
}

export const useChatStore = defineStore('chat', () => {
  const sessions = ref<SessionMeta[]>([])
  const activeSessionId = ref<string | null>(null)
  const messages = ref<ChatMessage[]>([])
  const isStreaming = ref(false)
  const pendingAttachments = ref<PendingAttachments>({})
  const streamingContent = ref('')
  const unlisteners = ref<UnlistenFn[]>([])

  const activeSession = computed(() =>
    sessions.value.find((s) => s.id === activeSessionId.value) ?? null,
  )

  async function setupListeners() {
    const unlisten1 = await listen<{ session_id: string; token: string }>('token', (e) => {
      if (e.payload.session_id !== activeSessionId.value) return
      streamingContent.value += e.payload.token

      // Update the last assistant message in place
      const last = messages.value[messages.value.length - 1]
      if (last && last.role === 'assistant') {
        last.content = streamingContent.value
      }
    })

    const unlisten2 = await listen<{ session_id: string; stats: InferenceStats }>(
      'inference-complete',
      async (e) => {
        if (e.payload.session_id !== activeSessionId.value) return
        isStreaming.value = false
        streamingContent.value = ''
        const last = messages.value[messages.value.length - 1]
        if (last && last.role === 'assistant') {
          last.stats = e.payload.stats
        }
        await persistSession()
      },
    )

    const unlisten3 = await listen<{ session_id: string; error: string }>(
      'inference-error',
      (e) => {
        if (e.payload.session_id !== activeSessionId.value) return
        isStreaming.value = false
        streamingContent.value = ''
        // Replace streaming placeholder with error message
        const last = messages.value[messages.value.length - 1]
        if (last && last.role === 'assistant') {
          last.content = `⚠️ Error: ${e.payload.error}`
        }
      },
    )

    unlisteners.value.push(unlisten1, unlisten2, unlisten3)
  }

  async function loadSessions() {
    sessions.value = await invoke<SessionMeta[]>('list_sessions')
  }

  async function openSession(sessionId: string) {
    activeSessionId.value = sessionId
    messages.value = await invoke<ChatMessage[]>('get_session', { sessionId })
    streamingContent.value = ''
    isStreaming.value = false
    pendingAttachments.value = {}
  }

  async function newSession() {
    const id = uuidv4()
    activeSessionId.value = id
    messages.value = []
    streamingContent.value = ''
    isStreaming.value = false
    pendingAttachments.value = {}
  }

  function deriveTitle(content: string): string {
    const text = content.trim()
    if (text.length <= 50) return text
    const truncated = text.slice(0, 50)
    const lastSpace = truncated.lastIndexOf(' ')
    return (lastSpace > 20 ? truncated.slice(0, lastSpace) : truncated) + '…'
  }

  async function sendMessage(content: string) {
    if (!content.trim() || isStreaming.value) return

    const modelStore = useModelStore()
    if (!modelStore.activeModelId) throw new Error('No model selected')

    // Ensure session exists
    if (!activeSessionId.value) {
      await newSession()
    }

    const now = new Date().toISOString()
    const isFirstMessage = messages.value.length === 0

    const userMsg: ChatMessage = {
      id: uuidv4(),
      role: 'user',
      content: content.trim(),
      image_path: pendingAttachments.value.image,
      audio_path: pendingAttachments.value.audio,
      timestamp: now,
    }

    const assistantMsg: ChatMessage = {
      id: uuidv4(),
      role: 'assistant',
      content: '',
      timestamp: now,
    }

    messages.value.push(userMsg, assistantMsg)

    // Show session in sidebar immediately on first message
    if (isFirstMessage && !sessions.value.find((s) => s.id === activeSessionId.value)) {
      sessions.value.unshift({
        id: activeSessionId.value!,
        title: deriveTitle(content),
        created_at: now,
        model_id: modelStore.activeModelId ?? '',
        message_count: 0,
      })
    }

    isStreaming.value = true
    streamingContent.value = ''

    const { image, audio } = pendingAttachments.value
    pendingAttachments.value = {}

    // Ensure model is loaded
    await invoke('load_model', { modelId: modelStore.activeModelId })

    await invoke('send_message', {
      sessionId: activeSessionId.value,
      messages: messages.value.filter((m) => m.role !== 'assistant' || m.id !== assistantMsg.id),
      imagePath: image ?? null,
      audioPath: audio ?? null,
    })
  }

  async function cancelStreaming() {
    await invoke('cancel_inference')
    isStreaming.value = false
    streamingContent.value = ''
  }

  async function persistSession() {
    if (!activeSessionId.value) return
    const modelStore = useModelStore()
    await invoke('save_session', {
      sessionId: activeSessionId.value,
      messages: messages.value,
      modelId: modelStore.activeModelId ?? '',
    })
    await loadSessions()
  }

  async function deleteSession(sessionId: string) {
    await invoke('delete_session', { sessionId })
    if (activeSessionId.value === sessionId) {
      await newSession()
    }
    await loadSessions()
  }

  async function clearMessages() {
    if (!activeSessionId.value) return
    await deleteSession(activeSessionId.value)
    await newSession()
  }

  function setAttachment(type: 'image' | 'audio', path: string) {
    pendingAttachments.value[type] = path
  }

  function clearAttachment(type: 'image' | 'audio') {
    delete pendingAttachments.value[type]
  }

  function cleanup() {
    unlisteners.value.forEach((fn) => fn())
    unlisteners.value = []
  }

  return {
    sessions,
    activeSessionId,
    messages,
    isStreaming,
    pendingAttachments,
    streamingContent,
    activeSession,
    setupListeners,
    loadSessions,
    openSession,
    newSession,
    sendMessage,
    cancelStreaming,
    deleteSession,
    clearMessages,
    setAttachment,
    clearAttachment,
    cleanup,
  }
})
