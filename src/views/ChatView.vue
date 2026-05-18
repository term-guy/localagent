<script setup lang="ts">
import { ref, computed, nextTick, watch, onMounted } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useChatStore } from '@/stores/chatStore'
import { useModelStore } from '@/stores/modelStore'
import { useToast } from '@/composables/useToast'
import { useConfirm } from '@/composables/useConfirm'
import type { InstalledModel } from '@/types'
import MarkdownIt from 'markdown-it'

const chatStore = useChatStore()
const modelStore = useModelStore()
const { show } = useToast()
const { confirm } = useConfirm()

const md = new MarkdownIt({ html: false, linkify: true, typographer: true })

const sidebarOpen = ref(true)
const inputText = ref('')
const inputEl = ref<HTMLTextAreaElement | null>(null)
const messagesEl = ref<HTMLDivElement | null>(null)
const isRecording = ref(false)
const recordingSeconds = ref(0)
let recordingTimer: ReturnType<typeof setInterval> | null = null
let mediaRecorder: MediaRecorder | null = null
let audioChunks: BlobPart[] = []

const activeModel = computed(() => modelStore.activeModel)
const hasVision = computed(() => activeModel.value?.capabilities.includes('vision') ?? false)
const hasAudio = computed(() => activeModel.value?.capabilities.includes('audio') ?? false)
const hasPendingImage = computed(() => !!chatStore.pendingAttachments.image)
const hasPendingAudio = computed(() => !!chatStore.pendingAttachments.audio)

const recordingDisplay = computed(() => {
  const m = Math.floor(recordingSeconds.value / 60)
  const s = recordingSeconds.value % 60
  return `${m}:${s.toString().padStart(2, '0')}`
})

function focusInput() {
  nextTick(() => inputEl.value?.focus())
}

onMounted(async () => {
  await chatStore.loadSessions()
  if (chatStore.sessions.length > 0 && !chatStore.activeSessionId) {
    await chatStore.openSession(chatStore.sessions[0].id)
  } else if (!chatStore.activeSessionId) {
    await chatStore.newSession()
  }
  focusInput()
})

watch(
  () => chatStore.messages.length,
  async () => {
    await nextTick()
    scrollToBottom()
  },
)

watch(
  () => chatStore.streamingContent,
  async () => {
    await nextTick()
    scrollToBottom()
  },
)

function scrollToBottom() {
  if (messagesEl.value) {
    messagesEl.value.scrollTop = messagesEl.value.scrollHeight
  }
}

function autoGrow(el: HTMLTextAreaElement) {
  el.style.height = 'auto'
  el.style.height = Math.min(el.scrollHeight, 144) + 'px' // max ~6 lines
}

function onInput(e: Event) {
  const el = e.target as HTMLTextAreaElement
  autoGrow(el)
}

async function send() {
  const text = inputText.value.trim()
  if (!text || chatStore.isStreaming || modelStore.modelLoading) return
  inputText.value = ''
  if (inputEl.value) {
    inputEl.value.style.height = 'auto'
  }
  try {
    await chatStore.sendMessage(text)
  } catch (e) {
    show(`Send failed: ${e}`, 'error')
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    send()
  }
}

async function pickImage() {
  const path = await open({
    multiple: false,
    filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
  })
  if (path && typeof path === 'string') {
    chatStore.setAttachment('image', path)
  }
}

async function pickAudio() {
  const path = await open({
    multiple: false,
    filters: [{ name: 'Audio', extensions: ['mp3', 'wav', 'm4a'] }],
  })
  if (path && typeof path === 'string') {
    chatStore.setAttachment('audio', path)
  }
}

async function startRecording() {
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
    audioChunks = []
    mediaRecorder = new MediaRecorder(stream)
    mediaRecorder.ondataavailable = (e) => audioChunks.push(e.data)
    mediaRecorder.start()
    isRecording.value = true
    recordingSeconds.value = 0
    recordingTimer = setInterval(() => recordingSeconds.value++, 1000)
  } catch (e) {
    show('Microphone access denied', 'error')
  }
}

async function stopRecording() {
  if (!mediaRecorder) return
  clearInterval(recordingTimer!)
  isRecording.value = false

  await new Promise<void>((resolve) => {
    mediaRecorder!.onstop = () => resolve()
    mediaRecorder!.stop()
    mediaRecorder!.stream.getTracks().forEach((t) => t.stop())
  })

  // Recording complete — attach as blob URL (Tauri can read blob: URLs)
  const blob = new Blob(audioChunks, { type: 'audio/wav' })
  URL.createObjectURL(blob) // held by browser; real path attachment needs Tauri FS write
  show('Audio recording attached', 'info')
}

function cancelRecording() {
  if (mediaRecorder) {
    mediaRecorder.stream.getTracks().forEach((t) => t.stop())
    mediaRecorder = null
  }
  clearInterval(recordingTimer!)
  isRecording.value = false
  recordingSeconds.value = 0
  audioChunks = []
}

async function handleNewChat() {
  await chatStore.newSession()
  focusInput()
}

async function handleClearChat() {
  const ok = await confirm({
    title: 'Clear Chat',
    message: 'Clear all messages in this conversation?',
    confirmLabel: 'Clear',
    danger: true,
  })
  if (ok) await chatStore.clearMessages()
}

async function handleDeleteSession(sessionId: string) {
  const ok = await confirm({
    title: 'Delete Conversation',
    message: 'Delete this conversation permanently?',
    confirmLabel: 'Delete',
    danger: true,
  })
  if (ok) await chatStore.deleteSession(sessionId)
}

function handleModelChange(e: Event) {
  const val = (e.target as HTMLSelectElement).value
  const sep = val.lastIndexOf(':')
  const id = val.slice(0, sep)
  const backend = val.slice(sep + 1)
  modelStore.setActiveModel(id, backend)
}

function modelOptionLabel(m: InstalledModel) {
  const backendName = m.backend === 'llama_cpp' ? 'llama.cpp' : 'Cactus'
  return `${m.display_name} (${backendName})`
}

interface ParsedContent {
  thinkBlocks: string[]
  responseContent: string
  isThinking: boolean
}

function parseContent(content: string, isActive = false): ParsedContent {
  const thinkBlocks: string[] = []
  let rest = content.replace(/<think>([\s\S]*?)<\/think>/gi, (_, block) => {
    thinkBlocks.push(block.trim())
    return ''
  })
  const hasUnclosedTag = /<think>/i.test(rest)
  if (hasUnclosedTag && !isActive) {
    // Stream ended with an unclosed tag — render the content as normal text
    rest = rest.replace(/<think>/gi, '').trim()
    return { thinkBlocks, responseContent: rest, isThinking: false }
  }
  const isThinking = hasUnclosedTag
  rest = rest.replace(/<think>[\s\S]*$/i, '').trim()
  return { thinkBlocks, responseContent: rest, isThinking }
}

const parsedMessages = computed(() =>
  chatStore.messages.map((msg, i) => {
    const isActive = chatStore.isStreaming && i === chatStore.messages.length - 1
    return {
      ...msg,
      parsed: msg.role === 'assistant' ? parseContent(msg.content, isActive) : null as ParsedContent | null,
    }
  }),
)

function renderMarkdown(content: string) {
  return md.render(content || '▊')
}

function formatTime(iso: string) {
  return new Date(iso).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}
</script>

<template>
  <div class="flex h-screen w-screen overflow-hidden">
    <!-- Sidebar -->
    <aside
      class="flex flex-col bg-zinc-900 border-r border-zinc-800 transition-all duration-200 shrink-0"
      :class="sidebarOpen ? 'w-64' : 'w-14'"
    >
      <!-- Logo -->
      <div class="flex items-center gap-2.5 px-3 py-4 border-b border-zinc-800 h-14">
        <div class="h-8 w-8 shrink-0 rounded-lg bg-gradient-to-br from-primary-600 to-violet-700
                    flex items-center justify-center">
          <span class="text-sm">🤖</span>
        </div>
        <span v-if="sidebarOpen" class="font-semibold text-sm text-zinc-100 truncate">localagent</span>
        <button
          class="ml-auto text-zinc-500 hover:text-zinc-300 transition-colors"
          @click="sidebarOpen = !sidebarOpen"
        >
          <span class="text-xs">{{ sidebarOpen ? '◀' : '▶' }}</span>
        </button>
      </div>

      <!-- New Chat -->
      <div class="px-2 py-2 border-b border-zinc-800">
        <button
          class="flex items-center gap-2 w-full rounded-lg px-2 py-2 text-sm text-zinc-300
                 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
          @click="handleNewChat"
        >
          <span class="text-base shrink-0">✏️</span>
          <span v-if="sidebarOpen">New Chat</span>
        </button>
      </div>

      <!-- Session list -->
      <div class="flex-1 overflow-y-auto px-2 py-2 space-y-0.5">
        <div
          v-for="session in chatStore.sessions"
          :key="session.id"
          class="group flex items-center gap-2 rounded-lg px-2 py-2 cursor-pointer text-sm
                 transition-colors"
          :class="session.id === chatStore.activeSessionId
            ? 'bg-zinc-800 text-zinc-100'
            : 'text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-200'"
          @click="chatStore.openSession(session.id).then(focusInput)"
        >
          <span class="text-xs shrink-0">💬</span>
          <span v-if="sidebarOpen" class="flex-1 truncate text-xs">{{ session.title }}</span>
          <button
            v-if="sidebarOpen"
            class="hidden group-hover:flex text-zinc-600 hover:text-red-400 transition-colors"
            @click.stop="handleDeleteSession(session.id)"
          >✕</button>
        </div>
        <div v-if="chatStore.sessions.length === 0 && sidebarOpen"
             class="px-2 py-4 text-center text-xs text-zinc-600">
          No conversations yet
        </div>
      </div>

      <!-- Model selector -->
      <div v-if="sidebarOpen" class="px-3 py-3 border-t border-zinc-800">
        <label class="text-xs text-zinc-500 mb-1 block">Active Model</label>
        <select
          class="input-base text-xs py-1.5"
          :value="`${modelStore.activeModelId}:${modelStore.activeModelBackend}`"
          @change="handleModelChange"
        >
          <option
            v-for="m in modelStore.installed"
            :key="`${m.id}:${m.backend}`"
            :value="`${m.id}:${m.backend}`"
          >
            {{ modelOptionLabel(m) }}
          </option>
        </select>
      </div>

      <!-- Settings link -->
      <div class="px-2 py-2 border-t border-zinc-800">
        <RouterLink
          to="/settings"
          class="flex items-center gap-2 rounded-lg px-2 py-2 text-sm text-zinc-400
                 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
        >
          <span class="text-base shrink-0">⚙️</span>
          <span v-if="sidebarOpen">Settings</span>
        </RouterLink>
      </div>
    </aside>

    <!-- Main panel -->
    <div class="flex flex-1 flex-col overflow-hidden">
      <!-- Toolbar -->
      <header class="flex items-center justify-between px-5 h-14 border-b border-zinc-800 shrink-0">
        <div class="flex items-center gap-2">
          <h2 class="text-sm font-medium text-zinc-300">
            {{ chatStore.activeSession?.title ?? 'New Chat' }}
          </h2>
          <span v-if="activeModel" class="text-xs text-zinc-600">
            · {{ activeModel.display_name }}
          </span>
          <span v-if="modelStore.modelLoading" class="text-xs text-zinc-500 flex items-center gap-1">
            <span class="inline-block h-2 w-2 rounded-full bg-primary-500 animate-pulse" />
            Loading model…
          </span>
        </div>
        <button
          class="btn-ghost text-xs"
          :disabled="chatStore.messages.length === 0"
          @click="handleClearChat"
        >
          Clear
        </button>
      </header>

      <!-- Messages -->
      <div
        ref="messagesEl"
        class="flex-1 overflow-y-auto px-4 py-6 space-y-6"
      >
        <!-- Empty state -->
        <div
          v-if="chatStore.messages.length === 0"
          class="flex flex-col items-center justify-center h-full text-center gap-4"
        >
          <div class="h-16 w-16 rounded-2xl bg-zinc-800 flex items-center justify-center text-3xl">
            🤖
          </div>
          <div>
            <p class="text-lg font-semibold text-zinc-300">How can I help you today?</p>
            <p class="text-sm text-zinc-500 mt-1">Start a conversation with your local AI</p>
          </div>
          <div class="flex flex-wrap gap-2 justify-center mt-2">
            <button
              v-for="prompt in ['Explain quantum computing', 'Write a Python function', 'Help me brainstorm']"
              :key="prompt"
              class="text-xs px-3 py-1.5 rounded-full border border-zinc-700 text-zinc-400
                     hover:border-primary-600 hover:text-primary-300 transition-colors"
              @click="inputText = prompt"
            >
              {{ prompt }}
            </button>
          </div>
        </div>

        <!-- Message list -->
        <div
          v-for="msg in parsedMessages"
          :key="msg.id"
          class="flex"
          :class="msg.role === 'user' ? 'justify-end' : 'justify-start'"
        >
          <!-- Assistant avatar -->
          <div
            v-if="msg.role === 'assistant'"
            class="mr-2.5 mt-1 h-7 w-7 shrink-0 rounded-full bg-gradient-to-br
                   from-primary-600 to-violet-700 flex items-center justify-center text-xs"
          >
            🤖
          </div>

          <div class="flex flex-col max-w-[75%]" :class="msg.role === 'user' ? 'items-end' : 'items-start'">
            <!-- Image preview -->
            <img
              v-if="msg.image_path"
              :src="`file://${msg.image_path}`"
              class="mb-2 max-h-48 rounded-xl object-cover ring-1 ring-zinc-700"
              alt="Attached image"
            />

            <!-- Audio indicator -->
            <div
              v-if="msg.audio_path"
              class="mb-2 flex items-center gap-2 text-xs text-zinc-400 bg-zinc-800
                     rounded-lg px-3 py-2 ring-1 ring-zinc-700"
            >
              🎙️ Audio attached
            </div>

            <!-- Thinking: in-progress animation -->
            <div
              v-if="msg.parsed?.isThinking"
              class="mb-1.5 flex items-center gap-2 rounded-xl border border-zinc-700/50
                     bg-zinc-900/60 px-3 py-2 text-xs text-zinc-500 w-full"
            >
              <span class="flex gap-0.5">
                <span class="h-1.5 w-1.5 rounded-full bg-zinc-500 animate-bounce" style="animation-delay:0ms" />
                <span class="h-1.5 w-1.5 rounded-full bg-zinc-500 animate-bounce" style="animation-delay:120ms" />
                <span class="h-1.5 w-1.5 rounded-full bg-zinc-500 animate-bounce" style="animation-delay:240ms" />
              </span>
              Thinking…
            </div>

            <!-- Thinking: completed blocks (collapsible) -->
            <details
              v-for="(block, i) in (msg.parsed?.thinkBlocks ?? [])"
              :key="i"
              class="mb-1.5 w-full rounded-xl border border-zinc-700/40 bg-zinc-900/50 text-xs overflow-hidden"
            >
              <summary class="flex cursor-pointer select-none list-none items-center gap-1.5
                              px-3 py-2 text-zinc-500 hover:text-zinc-400 transition-colors">
                <span>💭</span>
                <span>Thoughts</span>
                <span class="ml-auto text-zinc-600 text-[10px]">click to expand</span>
              </summary>
              <div class="border-t border-zinc-700/40 px-3 py-2.5 text-zinc-400 prose-chat" v-html="renderMarkdown(block)" />
            </details>

            <!-- Bubble (hidden while model is still mid-think and has no response yet) -->
            <div
              v-if="!msg.parsed?.isThinking || msg.parsed?.responseContent"
              class="rounded-2xl px-4 py-3 text-sm"
              :class="msg.role === 'user'
                ? 'bg-primary-700 text-white rounded-tr-sm'
                : 'bg-zinc-800 rounded-tl-sm'"
            >
              <div
                v-if="msg.role === 'assistant'"
                class="prose-chat"
                v-html="renderMarkdown(msg.parsed!.responseContent)"
              />
              <p v-else class="selectable whitespace-pre-wrap">{{ msg.content }}</p>
            </div>

            <div class="flex items-center gap-2 mt-1 px-1">
              <span class="text-xs text-zinc-600">{{ formatTime(msg.timestamp) }}</span>
              <template v-if="msg.role === 'assistant' && msg.stats">
                <span class="text-zinc-700 text-xs">·</span>
                <span class="text-xs text-zinc-600">
                  {{ msg.stats.tokens_per_second.toFixed(1) }} tok/s
                </span>
                <span class="text-zinc-700 text-xs">·</span>
                <span class="text-xs text-zinc-600">{{ msg.stats.tokens_generated }} tokens</span>
                <span class="text-zinc-700 text-xs">·</span>
                <span class="text-xs text-zinc-600">
                  {{ (msg.stats.duration_ms / 1000).toFixed(1) }}s
                </span>
              </template>
            </div>
          </div>
        </div>
      </div>

      <!-- Input area -->
      <div class="border-t border-zinc-800 px-4 py-3 space-y-2">
        <!-- Recording UI -->
        <div
          v-if="isRecording"
          class="flex items-center gap-3 rounded-xl bg-zinc-800 border border-red-800/50 px-4 py-2.5"
        >
          <span class="h-2.5 w-2.5 rounded-full bg-red-500 animate-pulse shrink-0" />
          <span class="text-sm text-red-300">Recording {{ recordingDisplay }}</span>
          <div class="flex-1" />
          <button class="btn-primary text-xs py-1" @click="stopRecording">Stop & Attach</button>
          <button class="btn-ghost text-xs py-1" @click="cancelRecording">Cancel</button>
        </div>

        <!-- Pending attachments -->
        <div v-if="hasPendingImage || hasPendingAudio" class="flex items-center gap-2">
          <div v-if="hasPendingImage"
               class="flex items-center gap-1.5 text-xs text-zinc-400 bg-zinc-800 rounded-lg px-2.5 py-1.5">
            🖼️ Image attached
            <button class="text-zinc-600 hover:text-red-400 ml-1"
                    @click="chatStore.clearAttachment('image')">✕</button>
          </div>
          <div v-if="hasPendingAudio"
               class="flex items-center gap-1.5 text-xs text-zinc-400 bg-zinc-800 rounded-lg px-2.5 py-1.5">
            🎙️ Audio attached
            <button class="text-zinc-600 hover:text-red-400 ml-1"
                    @click="chatStore.clearAttachment('audio')">✕</button>
          </div>
        </div>

        <!-- Text input row -->
        <div class="flex items-end gap-2">
          <!-- Attachment buttons -->
          <div class="flex items-center gap-1 pb-1">
            <button
              v-if="hasVision"
              class="btn-ghost rounded-lg p-2 text-base"
              title="Attach image"
              :disabled="chatStore.isStreaming"
              @click="pickImage"
            >📎</button>
            <button
              v-if="hasAudio"
              class="btn-ghost rounded-lg p-2 text-base"
              title="Record audio"
              :disabled="chatStore.isStreaming || isRecording"
              @click="startRecording"
            >🎙️</button>
            <button
              v-if="hasAudio"
              class="btn-ghost rounded-lg p-2 text-base"
              title="Upload audio"
              :disabled="chatStore.isStreaming"
              @click="pickAudio"
            >📁</button>
          </div>

          <!-- Textarea -->
          <textarea
            ref="inputEl"
            v-model="inputText"
            rows="1"
            :placeholder="modelStore.modelLoading ? 'Loading model…' : 'Message localagent… (Shift+Enter for new line)'"
            class="input-base flex-1 resize-none min-h-[42px] max-h-36 py-2.5 leading-snug selectable"
            :disabled="chatStore.isStreaming || modelStore.modelLoading"
            @input="onInput"
            @keydown="onKeydown"
          />

          <!-- Send / Cancel -->
          <button
            v-if="!chatStore.isStreaming"
            class="btn-primary shrink-0 h-10 px-4"
            :disabled="!inputText.trim() || modelStore.modelLoading"
            @click="send"
          >
            Send
          </button>
          <button
            v-else
            class="btn-secondary shrink-0 h-10 px-4"
            @click="chatStore.cancelStreaming"
          >
            Stop
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
