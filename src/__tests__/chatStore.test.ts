import { describe, it, expect, beforeEach, vi } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useChatStore } from '@/stores/chatStore'
import { useModelStore } from '@/stores/modelStore'
import type { SessionMeta, ChatMessage } from '@/types'

const mockSessions: SessionMeta[] = [
  { id: 'session-1', title: 'Hello world', created_at: '2025-01-01T00:00:00Z', model_id: 'gemma-3-1b-it', message_count: 2 },
  { id: 'session-2', title: 'Second chat', created_at: '2025-01-02T00:00:00Z', model_id: 'gemma-3-1b-it', message_count: 4 },
]

const mockMessages: ChatMessage[] = [
  { id: 'msg-1', role: 'user', content: 'Hello', timestamp: '2025-01-01T00:00:00Z' },
  { id: 'msg-2', role: 'assistant', content: 'Hi there!', timestamp: '2025-01-01T00:00:01Z' },
]

describe('chatStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  describe('loadSessions', () => {
    it('loads sessions from backend', async () => {
      vi.mocked(invoke).mockResolvedValue(mockSessions)

      const store = useChatStore()
      await store.loadSessions()

      expect(invoke).toHaveBeenCalledWith('list_sessions')
      expect(store.sessions).toEqual(mockSessions)
    })

    it('handles empty session list', async () => {
      vi.mocked(invoke).mockResolvedValue([])

      const store = useChatStore()
      await store.loadSessions()

      expect(store.sessions).toEqual([])
    })
  })

  describe('openSession', () => {
    it('loads messages for a given session', async () => {
      vi.mocked(invoke).mockResolvedValue(mockMessages)

      const store = useChatStore()
      await store.openSession('session-1')

      expect(invoke).toHaveBeenCalledWith('get_session', { sessionId: 'session-1' })
      expect(store.activeSessionId).toBe('session-1')
      expect(store.messages).toEqual(mockMessages)
      expect(store.isStreaming).toBe(false)
      expect(store.streamingContent).toBe('')
    })

    it('clears pending attachments when opening a session', async () => {
      vi.mocked(invoke).mockResolvedValue(mockMessages)

      const store = useChatStore()
      store.setAttachment('image', '/old.png')
      await store.openSession('session-1')

      expect(store.pendingAttachments).toEqual({})
    })
  })

  describe('newSession', () => {
    it('creates a new empty session state', () => {
      const store = useChatStore()
      store.newSession()

      expect(store.activeSessionId).toBeTruthy()
      expect(store.messages).toEqual([])
      expect(store.streamingContent).toBe('')
      expect(store.isStreaming).toBe(false)
      expect(store.pendingAttachments).toEqual({})
    })
  })

  describe('activeSession', () => {
    it('returns the currently active session meta', async () => {
      vi.mocked(invoke).mockResolvedValue(mockSessions)

      const store = useChatStore()
      await store.loadSessions()
      expect(store.activeSession).toBeNull()

      store.activeSessionId = 'session-1'
      expect(store.activeSession?.title).toBe('Hello world')
    })
  })

  // deriveTitle is a private store function — tested indirectly via sendMessage's sidebar title

  describe('sendMessage', () => {
    it('throws if no model is selected', async () => {
      const store = useChatStore()
      const modelStore = useModelStore()
      modelStore.activeModelId = null

      await expect(store.sendMessage('test')).rejects.toThrow('No model selected')
      expect(invoke).not.toHaveBeenCalled()
    })

    it('sends a message and creates an assistant placeholder', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined)
      const modelStore = useModelStore()
      modelStore.activeModelId = 'gemma-3-1b-it'

      const store = useChatStore()
      await store.sendMessage('Hello world')

      expect(store.messages).toHaveLength(2)
      expect(store.messages[0].role).toBe('user')
      expect(store.messages[0].content).toBe('Hello world')
      expect(store.messages[1].role).toBe('assistant')
      expect(store.messages[1].content).toBe('')
      expect(store.isStreaming).toBe(true)

      // Should have created a session and preloaded the model
      expect(store.activeSessionId).toBeTruthy()
      expect(invoke).toHaveBeenCalledWith('load_model', { modelId: 'gemma-3-1b-it' })
      expect(invoke).toHaveBeenCalledWith('send_message', expect.objectContaining({
        sessionId: store.activeSessionId,
      }))
    })

    it('adds session to sidebar on first message', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined)
      const modelStore = useModelStore()
      modelStore.activeModelId = 'gemma-3-1b-it'

      const store = useChatStore()
      await store.sendMessage('First message')

      expect(store.sessions).toHaveLength(1)
      expect(store.sessions[0].title).toBe('First message')
    })

    it('does not send empty messages', async () => {
      const store = useChatStore()
      const modelStore = useModelStore()
      modelStore.activeModelId = 'gemma-3-1b-it'

      await store.sendMessage('')
      expect(invoke).not.toHaveBeenCalledWith('send_message', expect.anything())
    })

    it('does not send whitespace-only messages', async () => {
      const store = useChatStore()
      const modelStore = useModelStore()
      modelStore.activeModelId = 'gemma-3-1b-it'

      await store.sendMessage('   ')
      expect(invoke).not.toHaveBeenCalledWith('send_message', expect.anything())
    })

    it('does not send while streaming', async () => {
      const store = useChatStore()
      store.isStreaming = true
      const modelStore = useModelStore()
      modelStore.activeModelId = 'gemma-3-1b-it'

      await store.sendMessage('test')
      expect(invoke).not.toHaveBeenCalledWith('send_message', expect.anything())
    })

    it('truncates long first message as session title', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined)
      const modelStore = useModelStore()
      modelStore.activeModelId = 'gemma-3-1b-it'

      const store = useChatStore()
      const longMsg = 'This is a very long message that is definitely longer than fifty characters'
      await store.sendMessage(longMsg)

      expect(store.sessions[0].title).toContain('…')
      expect(store.sessions[0].title.replace('…', '').length).toBeLessThanOrEqual(50)
    })

    it('passes attachment paths to send_message and clears them', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined)
      const modelStore = useModelStore()
      modelStore.activeModelId = 'gemma-3-1b-it'

      const store = useChatStore()
      // Set session id first: newSession() resets pendingAttachments, so attachments
      // must be set after a session already exists.
      store.activeSessionId = 'existing-session'
      store.setAttachment('image', '/path/img.png')
      store.setAttachment('audio', '/path/audio.wav')
      await store.sendMessage('With attachments')

      expect(invoke).toHaveBeenCalledWith('send_message', expect.objectContaining({
        imagePath: '/path/img.png',
        audioPath: '/path/audio.wav',
      }))
      expect(store.pendingAttachments).toEqual({})
    })

    it('does not add session to sidebar on subsequent messages', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined)
      const modelStore = useModelStore()
      modelStore.activeModelId = 'gemma-3-1b-it'

      const store = useChatStore()
      store.activeSessionId = 'existing-session'
      store.sessions = [{ id: 'existing-session', title: 'Previous', created_at: '', model_id: '', message_count: 2 }]
      store.messages.push(
        { id: 'msg-1', role: 'user', content: 'Previous', timestamp: '' },
        { id: 'msg-2', role: 'assistant', content: 'Response', timestamp: '' },
      )

      await store.sendMessage('Second message')
      expect(store.sessions).toHaveLength(1)
    })
  })

  describe('cancelStreaming', () => {
    it('cancels inference and resets streaming state', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined)

      const store = useChatStore()
      store.isStreaming = true
      store.streamingContent = 'partial response'

      await store.cancelStreaming()

      expect(invoke).toHaveBeenCalledWith('cancel_inference')
      expect(store.isStreaming).toBe(false)
      expect(store.streamingContent).toBe('')
    })
  })

  describe('deleteSession', () => {
    it('deletes a session and switches to new one if active', async () => {
      vi.mocked(invoke).mockResolvedValueOnce(undefined) // delete_session
      vi.mocked(invoke).mockResolvedValueOnce([]) // list_sessions after delete

      const store = useChatStore()
      store.sessions = mockSessions
      store.activeSessionId = 'session-1'

      await store.deleteSession('session-1')

      expect(invoke).toHaveBeenCalledWith('delete_session', { sessionId: 'session-1' })
      // Should have created a new session to replace the deleted one
      expect(store.activeSessionId).toBeTruthy()
      expect(store.activeSessionId).not.toBe('session-1')
    })

    it('does not start a new session when deleting a non-active session', async () => {
      vi.mocked(invoke).mockResolvedValueOnce(undefined) // delete_session
      vi.mocked(invoke).mockResolvedValueOnce([mockSessions[0]]) // list_sessions

      const store = useChatStore()
      store.sessions = mockSessions
      store.activeSessionId = 'session-1'

      await store.deleteSession('session-2')

      expect(store.activeSessionId).toBe('session-1')
      expect(invoke).toHaveBeenCalledWith('delete_session', { sessionId: 'session-2' })
    })
  })

  describe('clearMessages', () => {
    it('deletes current session and starts a new one', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined)

      const store = useChatStore()
      store.activeSessionId = 'session-1'

      await store.clearMessages()

      expect(invoke).toHaveBeenCalledWith('delete_session', { sessionId: 'session-1' })
    })

    it('does nothing when there is no active session', async () => {
      const store = useChatStore()
      store.activeSessionId = null

      await store.clearMessages()
      expect(invoke).not.toHaveBeenCalled()
    })
  })

  describe('attachments', () => {
    it('setAttachment stores attachment path', () => {
      const store = useChatStore()

      store.setAttachment('image', '/path/to/image.png')
      expect(store.pendingAttachments.image).toBe('/path/to/image.png')

      store.setAttachment('audio', '/path/to/audio.wav')
      expect(store.pendingAttachments.audio).toBe('/path/to/audio.wav')
    })

    it('clearAttachment removes attachment path', () => {
      const store = useChatStore()
      store.setAttachment('image', '/path/to/image.png')
      store.clearAttachment('image')

      expect(store.pendingAttachments.image).toBeUndefined()
    })
  })

  describe('setupListeners', () => {
    it('registers token, inference-complete, inference-error listeners', async () => {
      vi.mocked(listen).mockResolvedValue(vi.fn())

      const store = useChatStore()
      await store.setupListeners()

      expect(listen).toHaveBeenCalledWith('token', expect.any(Function))
      expect(listen).toHaveBeenCalledWith('inference-complete', expect.any(Function))
      expect(listen).toHaveBeenCalledWith('inference-error', expect.any(Function))
    })

    it('tokens are appended to streaming content during active session', async () => {
      let tokenHandler: (e: { payload: { session_id: string; token: string } }) => void = () => {}
      vi.mocked(listen).mockImplementation((event: string, handler: any) => {
        if (event === 'token') tokenHandler = handler
        return Promise.resolve(vi.fn())
      })

      const store = useChatStore()
      await store.setupListeners()

      store.messages.push({ id: 'msg-1', role: 'assistant', content: '', timestamp: '' })
      store.activeSessionId = 'session-1'

      tokenHandler({ payload: { session_id: 'session-1', token: 'Hello' } })
      expect(store.streamingContent).toBe('Hello')
      expect(store.messages[store.messages.length - 1].content).toBe('Hello')

      tokenHandler({ payload: { session_id: 'session-1', token: ' world' } })
      expect(store.streamingContent).toBe('Hello world')
    })

    it('ignores tokens for non-active sessions', async () => {
      let tokenHandler: (e: { payload: { session_id: string; token: string } }) => void = () => {}
      vi.mocked(listen).mockImplementation((event: string, handler: any) => {
        if (event === 'token') tokenHandler = handler
        return Promise.resolve(vi.fn())
      })

      const store = useChatStore()
      await store.setupListeners()

      store.activeSessionId = 'session-1'
      tokenHandler({ payload: { session_id: 'session-2', token: 'Hello' } })
      expect(store.streamingContent).toBe('')
    })

    it('inference-complete resets streaming and sets stats', async () => {
      let completeHandler: (e: { payload: { session_id: string; stats: any } }) => void = () => {}
      vi.mocked(listen).mockImplementation((event: string, handler: any) => {
        if (event === 'inference-complete') completeHandler = handler
        return Promise.resolve(vi.fn())
      })

      const store = useChatStore()
      await store.setupListeners()

      store.activeSessionId = 'session-1'
      store.messages.push({ id: 'msg-1', role: 'assistant', content: 'Hello', timestamp: '' })
      store.isStreaming = true
      store.streamingContent = 'Hello'

      completeHandler({
        payload: {
          session_id: 'session-1',
          stats: { tokens_generated: 5, duration_ms: 100, tokens_per_second: 50 },
        },
      })

      expect(store.isStreaming).toBe(false)
      expect(store.streamingContent).toBe('')
      expect(store.messages[0].stats).toEqual({ tokens_generated: 5, duration_ms: 100, tokens_per_second: 50 })
    })

    it('inference-complete is ignored for a different session', async () => {
      let completeHandler: (e: { payload: { session_id: string; stats: any } }) => void = () => {}
      vi.mocked(listen).mockImplementation((event: string, handler: any) => {
        if (event === 'inference-complete') completeHandler = handler
        return Promise.resolve(vi.fn())
      })

      const store = useChatStore()
      await store.setupListeners()

      store.activeSessionId = 'session-1'
      store.isStreaming = true
      store.streamingContent = 'partial'

      completeHandler({ payload: { session_id: 'other-session', stats: {} } })

      expect(store.isStreaming).toBe(true)
      expect(store.streamingContent).toBe('partial')
    })

    it('inference-error replaces streaming placeholder with error', async () => {
      let errorHandler: (e: { payload: { session_id: string; error: string } }) => void = () => {}
      vi.mocked(listen).mockImplementation((event: string, handler: any) => {
        if (event === 'inference-error') errorHandler = handler
        return Promise.resolve(vi.fn())
      })

      const store = useChatStore()
      await store.setupListeners()

      store.activeSessionId = 'session-1'
      store.messages.push({ id: 'msg-1', role: 'assistant', content: '', timestamp: '' })
      store.isStreaming = true
      store.streamingContent = 'partial'

      errorHandler({ payload: { session_id: 'session-1', error: 'Out of memory' } })

      expect(store.isStreaming).toBe(false)
      expect(store.streamingContent).toBe('')
      expect(store.messages[0].content).toContain('Error')
      expect(store.messages[0].content).toContain('Out of memory')
    })

    it('inference-error is ignored for a different session', async () => {
      let errorHandler: (e: { payload: { session_id: string; error: string } }) => void = () => {}
      vi.mocked(listen).mockImplementation((event: string, handler: any) => {
        if (event === 'inference-error') errorHandler = handler
        return Promise.resolve(vi.fn())
      })

      const store = useChatStore()
      await store.setupListeners()

      store.activeSessionId = 'session-1'
      store.isStreaming = true
      store.messages.push({ id: 'msg-1', role: 'assistant', content: 'partial', timestamp: '' })

      errorHandler({ payload: { session_id: 'other-session', error: 'oops' } })

      expect(store.isStreaming).toBe(true)
      expect(store.messages[0].content).toBe('partial')
    })
  })

  describe('cleanup', () => {
    it('unregisters all event listeners', async () => {
      const unlistenFn = vi.fn()
      vi.mocked(listen).mockResolvedValue(unlistenFn)

      const store = useChatStore()
      await store.setupListeners()
      store.cleanup()

      expect(unlistenFn).toHaveBeenCalledTimes(3)
    })
  })
})
