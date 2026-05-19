import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { v4 as uuidv4 } from 'uuid'
import type { ChatMessage, SessionMeta, InferenceStats } from '@/types'
import { useModelStore } from '@/stores/modelStore'
import { parseToolCall, executeTool } from '@/composables/useTools'

const MAX_TOOL_LOOPS = 5

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

  // Tracks the tool context for the current turn so the loop can re-use it
  const currentToolContext = ref<string | null>(null)
  const toolLoopCount = ref(0)
  const toolExecuting = ref(false)

  const activeSession = computed(() =>
    sessions.value.find((s) => s.id === activeSessionId.value) ?? null,
  )

  async function setupListeners() {
    const unlisten1 = await listen<{ session_id: string; token: string }>('token', (e) => {
      if (e.payload.session_id !== activeSessionId.value) return
      streamingContent.value += e.payload.token

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

        // Tool-call loop: if the model emitted a tool call and we haven't hit the cap, execute it
        if (last && last.role === 'assistant' && toolLoopCount.value < MAX_TOOL_LOOPS) {
          const call = parseToolCall(last.content)
          if (call) {
            toolLoopCount.value++
            toolExecuting.value = true
            let toolResult: string
            try {
              toolResult = await executeTool(call.name, call.arguments)
            } catch (err) {
              toolResult = `Tool error: ${err}`
            }
            toolExecuting.value = false

            const toolMsg: ChatMessage = {
              id: uuidv4(),
              role: 'user',
              content: `[Tool result: ${call.name}]\n${toolResult}`,
              timestamp: new Date().toISOString(),
            }
            const nextAssistant: ChatMessage = {
              id: uuidv4(),
              role: 'assistant',
              content: '',
              timestamp: new Date().toISOString(),
            }
            messages.value.push(toolMsg, nextAssistant)
            isStreaming.value = true
            streamingContent.value = ''

            await invoke('send_message', {
              sessionId: activeSessionId.value,
              messages: messages.value.filter((m) => m.id !== nextAssistant.id),
              imagePath: null,
              audioPath: null,
              toolContext: currentToolContext.value,
            })
            return
          }
        }

        toolLoopCount.value = 0
        toolExecuting.value = false
        await persistSession()
      },
    )

    const unlisten3 = await listen<{ session_id: string; error: string }>(
      'inference-error',
      (e) => {
        if (e.payload.session_id !== activeSessionId.value) return
        isStreaming.value = false
        streamingContent.value = ''
        toolLoopCount.value = 0
        toolExecuting.value = false
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

  async function sendMessage(content: string, toolContext?: string | null) {
    if (!content.trim() || isStreaming.value) return

    const modelStore = useModelStore()
    if (!modelStore.activeModelId) throw new Error('No model selected')

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
    currentToolContext.value = toolContext ?? null
    toolLoopCount.value = 0

    const { image, audio } = pendingAttachments.value
    pendingAttachments.value = {}

    await invoke('load_model', { modelId: modelStore.activeModelId })

    await invoke('send_message', {
      sessionId: activeSessionId.value,
      messages: messages.value.filter((m) => m.role !== 'assistant' || m.id !== assistantMsg.id),
      imagePath: image ?? null,
      audioPath: audio ?? null,
      toolContext: toolContext ?? null,
    })
  }

  async function cancelStreaming() {
    await invoke('cancel_inference')
    isStreaming.value = false
    streamingContent.value = ''
    toolLoopCount.value = 0
    toolExecuting.value = false
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
    toolExecuting,
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
