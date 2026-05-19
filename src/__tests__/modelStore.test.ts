import { describe, it, expect, beforeEach, vi } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useModelStore } from '@/stores/modelStore'
import type { ModelInfo, InstalledModel } from '@/types'

const mockCatalog: ModelInfo[] = [
  {
    id: 'gemma-3-1b-it',
    display_name: 'Gemma-3-1B',
    provider: 'Google',
    repo: 'unsloth/gemma-3-1b-it-GGUF',
    capabilities: ['chat'],
    filename: 'gemma-3-1b-it-Q4_K_M.gguf',
    description: 'Ultra-lightweight 1B chat model',
    default_backend: 'llama_cpp',
    llama_cpp_url: 'https://huggingface.co/unsloth/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q4_K_M.gguf',
    llama_cpp_size_mb: 769,
    llama_cpp_quant: 'Q4_K_M',
    cactus_url: 'https://huggingface.co/Cactus-Compute/gemma-3-1b-it/resolve/main/weights/gemma-3-1b-it-int4.zip',
    cactus_size_mb: 653,
  },
  {
    id: 'Bonsai-8B',
    display_name: 'Bonsai-8B',
    provider: 'Prisma ML',
    repo: 'prism-ml/Bonsai-8B-gguf',
    capabilities: ['chat'],
    filename: 'bonsai.gguf',
    description: 'Compact chat model',
    default_backend: 'llama_cpp',
    llama_cpp_url: 'https://huggingface.co/prism-ml/Bonsai-8B-gguf/resolve/main/Bonsai-8B.gguf',
    llama_cpp_size_mb: 1160,
    llama_cpp_quant: undefined,
    cactus_url: undefined,
    cactus_size_mb: undefined,
  },
]

const mockInstalled: InstalledModel[] = [
  {
    id: 'gemma-3-1b-it',
    display_name: 'Gemma-3-1B',
    provider: 'Google',
    repo: 'unsloth/gemma-3-1b-it-GGUF',
    capabilities: ['chat'],
    filename: 'gemma-3-1b-it-Q4_K_M.gguf',
    description: 'Ultra-lightweight 1B chat model',
    default_backend: 'llama_cpp',
    llama_cpp_url: 'https://huggingface.co/unsloth/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q4_K_M.gguf',
    llama_cpp_size_mb: 769,
    llama_cpp_quant: 'Q4_K_M',
    cactus_url: 'https://huggingface.co/Cactus-Compute/gemma-3-1b-it/resolve/main/weights/gemma-3-1b-it-int4.zip',
    cactus_size_mb: 653,
    file_size_bytes: 806354944,
    downloaded_at: '2025-01-01T00:00:00Z',
    backend: 'llama_cpp',
  },
]

describe('modelStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
    vi.clearAllMocks()
  })

  describe('loadCatalog', () => {
    it('fetches catalog from backend and stores it', async () => {
      vi.mocked(invoke).mockResolvedValue(mockCatalog)

      const store = useModelStore()
      expect(store.catalog).toEqual([])

      await store.loadCatalog()

      expect(invoke).toHaveBeenCalledWith('list_catalog')
      expect(store.catalog).toEqual(mockCatalog)
    })

    it('handles empty catalog', async () => {
      vi.mocked(invoke).mockResolvedValue([])

      const store = useModelStore()
      await store.loadCatalog()

      expect(store.catalog).toEqual([])
    })
  })

  describe('loadInstalled', () => {
    it('fetches installed models and selects first by default', async () => {
      vi.mocked(invoke).mockResolvedValue(mockInstalled)

      const store = useModelStore()
      await store.loadInstalled()

      expect(invoke).toHaveBeenCalledWith('list_installed')
      expect(store.installed).toEqual(mockInstalled)
      expect(store.activeModelId).toBe('gemma-3-1b-it')
      expect(store.activeModelBackend).toBe('llama_cpp')
    })

    it('stays unloaded when localStorage has none sentinel', async () => {
      localStorage.setItem('activeModelId', 'none')
      localStorage.setItem('activeModelBackend', 'none')
      vi.mocked(invoke).mockResolvedValue(mockInstalled)

      const store = useModelStore()
      await store.loadInstalled()

      expect(store.installed).toEqual(mockInstalled)
      expect(store.activeModelId).toBeNull()
      expect(store.activeModelBackend).toBeNull()
    })

    it('handles no installed models', async () => {
      vi.mocked(invoke).mockResolvedValue([])

      const store = useModelStore()
      await store.loadInstalled()

      expect(store.installed).toEqual([])
      expect(store.activeModelId).toBeNull()
    })
  })

  describe('computed properties', () => {
    it('installedIds returns set of installed model ids', async () => {
      vi.mocked(invoke).mockResolvedValue(mockInstalled)

      const store = useModelStore()
      await store.loadInstalled()

      expect(store.installedIds).toEqual(new Set(['gemma-3-1b-it']))
    })

    it('activeModel returns matching installed model', async () => {
      vi.mocked(invoke).mockResolvedValue(mockInstalled)

      const store = useModelStore()
      await store.loadInstalled()

      expect(store.activeModel).toEqual(mockInstalled[0])
    })

    it('activeModel returns null when no model matches', () => {
      const store = useModelStore()
      store.activeModelId = 'nonexistent'
      expect(store.activeModel).toBeNull()
    })

    it('activeModel returns null when backend does not match', async () => {
      vi.mocked(invoke).mockResolvedValue(mockInstalled)

      const store = useModelStore()
      await store.loadInstalled() // sets activeModelId = 'gemma-3-1b-it', backend = 'llama_cpp'

      store.activeModelBackend = 'cactus' // id matches but backend differs
      expect(store.activeModel).toBeNull()
    })

    it('availableModels filters out installed models from catalog', async () => {
      vi.mocked(invoke).mockResolvedValue(mockCatalog)
      const store = useModelStore()
      await store.loadCatalog()

      vi.mocked(invoke).mockResolvedValue(mockInstalled)
      await store.loadInstalled()

      expect(store.availableModels).toHaveLength(1)
      expect(store.availableModels[0].id).toBe('Bonsai-8B')
    })
  })

  describe('downloadHfModel', () => {
    it('invokes download_hf_model with correct params', async () => {
      const store = useModelStore()
      await store.downloadHfModel('owner/repo', 'model.gguf', 'https://example.com/model.gguf', 1_000_000, 'llama_cpp')

      expect(invoke).toHaveBeenCalledWith('download_hf_model', {
        repo: 'owner/repo',
        filename: 'model.gguf',
        url: 'https://example.com/model.gguf',
        sizeBytes: 1_000_000,
        backend: 'llama_cpp',
      })
    })
  })

  describe('downloadModel', () => {
    it('invokes download_model with correct params', async () => {
      const store = useModelStore()
      await store.downloadModel('gemma-3-1b-it', 'llama_cpp')

      expect(invoke).toHaveBeenCalledWith('download_model', {
        modelId: 'gemma-3-1b-it',
        backend: 'llama_cpp',
        filename: undefined,
        url: undefined,
        sizeBytes: undefined,
      })
    })

    it('supports quant-specific downloads', async () => {
      const store = useModelStore()
      await store.downloadModel('gemma-3-1b-it', 'llama_cpp', 'gguf', 'https://example.com/model.gguf', 500_000_000)

      expect(invoke).toHaveBeenCalledWith('download_model', {
        modelId: 'gemma-3-1b-it',
        backend: 'llama_cpp',
        filename: 'gguf',
        url: 'https://example.com/model.gguf',
        sizeBytes: 500_000_000,
      })
    })
  })

  describe('cancelDownload', () => {
    it('invokes cancel_download and clears progress', async () => {
      const store = useModelStore()
      store.downloadProgress['gemma-3-1b-it'] = {
        model_id: 'gemma-3-1b-it',
        bytes_downloaded: 100,
        total_bytes: 1000,
        speed_bps: 50000,
        percentage: 10,
      }

      await store.cancelDownload('gemma-3-1b-it')

      expect(invoke).toHaveBeenCalledWith('cancel_download', { modelId: 'gemma-3-1b-it' })
      expect(store.downloadProgress['gemma-3-1b-it']).toBeUndefined()
    })
  })

  describe('removeModel', () => {
    it('invokes remove_model, reloads installed, clears active if needed', async () => {
      const store = useModelStore()
      // Setting active model triggers a reactive watch that calls load_model.
      // Clear mock calls afterwards so we can set up precise returns.
      store.activeModelId = 'gemma-3-1b-it'
      store.activeModelBackend = 'llama_cpp'
      vi.mocked(invoke).mockClear()

      vi.mocked(invoke).mockResolvedValueOnce(undefined) // remove_model
      vi.mocked(invoke).mockResolvedValueOnce([]) // list_installed from loadInstalled

      await store.removeModel('gemma-3-1b-it', 'llama_cpp')

      expect(invoke).toHaveBeenCalledWith('remove_model', {
        modelId: 'gemma-3-1b-it',
        backend: 'llama_cpp',
      })
      expect(store.activeModelId).toBeNull()
    })

    it('switches to another model when removing active one', async () => {
      const otherModel: InstalledModel = { ...mockInstalled[0], id: 'Bonsai-8B' }

      const store = useModelStore()
      store.activeModelId = 'gemma-3-1b-it'
      store.activeModelBackend = 'llama_cpp'
      vi.mocked(invoke).mockClear()

      vi.mocked(invoke).mockResolvedValueOnce(undefined) // remove_model
      vi.mocked(invoke).mockResolvedValueOnce([otherModel]) // list_installed

      await store.removeModel('gemma-3-1b-it', 'llama_cpp')

      expect(store.activeModelId).toBe('Bonsai-8B')
    })

    it('does not change active model when removing a different model', async () => {
      const store = useModelStore()
      store.activeModelId = 'gemma-3-1b-it'
      store.activeModelBackend = 'llama_cpp'
      vi.mocked(invoke).mockClear()

      vi.mocked(invoke).mockResolvedValueOnce(undefined) // remove_model
      vi.mocked(invoke).mockResolvedValueOnce(mockInstalled) // list_installed

      await store.removeModel('Bonsai-8B', 'llama_cpp')

      expect(store.activeModelId).toBe('gemma-3-1b-it')
      expect(store.activeModelBackend).toBe('llama_cpp')
    })
  })

  describe('setupListeners', () => {
    it('registers download-progress, download-complete, download-error listeners', async () => {
      vi.mocked(listen).mockResolvedValue(vi.fn())

      const store = useModelStore()
      await store.setupListeners()

      expect(listen).toHaveBeenCalledWith('download-progress', expect.any(Function))
      expect(listen).toHaveBeenCalledWith('download-complete', expect.any(Function))
      expect(listen).toHaveBeenCalledWith('download-error', expect.any(Function))
    })

    it('download-progress event updates downloadProgress record', async () => {
      const handlers: Record<string, (e: any) => void> = {}
      vi.mocked(listen).mockImplementation((event: string, handler: any) => {
        handlers[event] = handler
        return Promise.resolve(vi.fn())
      })

      const store = useModelStore()
      await store.setupListeners()

      const progress = {
        model_id: 'gemma-3-1b-it',
        bytes_downloaded: 500,
        total_bytes: 1000,
        speed_bps: 50000,
        percentage: 50,
      }
      handlers['download-progress']({ payload: progress })

      expect(store.downloadProgress['gemma-3-1b-it']).toEqual(progress)
    })

    it('download-complete event removes progress and reloads installed', async () => {
      const handlers: Record<string, (e: any) => any> = {}
      vi.mocked(listen).mockImplementation((event: string, handler: any) => {
        handlers[event] = handler
        return Promise.resolve(vi.fn())
      })
      vi.mocked(invoke).mockResolvedValue(mockInstalled)

      const store = useModelStore()
      store.downloadProgress['gemma-3-1b-it'] = {
        model_id: 'gemma-3-1b-it',
        bytes_downloaded: 1000,
        total_bytes: 1000,
        speed_bps: 0,
        percentage: 100,
      }
      await store.setupListeners()

      await handlers['download-complete']({ payload: { model_id: 'gemma-3-1b-it' } })

      expect(store.downloadProgress['gemma-3-1b-it']).toBeUndefined()
      expect(invoke).toHaveBeenCalledWith('list_installed')
    })

    it('download-error event removes progress', async () => {
      const handlers: Record<string, (e: any) => void> = {}
      vi.mocked(listen).mockImplementation((event: string, handler: any) => {
        handlers[event] = handler
        return Promise.resolve(vi.fn())
      })

      const store = useModelStore()
      store.downloadProgress['gemma-3-1b-it'] = {
        model_id: 'gemma-3-1b-it',
        bytes_downloaded: 500,
        total_bytes: 1000,
        speed_bps: 50000,
        percentage: 50,
      }
      await store.setupListeners()

      handlers['download-error']({ payload: { model_id: 'gemma-3-1b-it', error: 'Network error' } })

      expect(store.downloadProgress['gemma-3-1b-it']).toBeUndefined()
    })
  })

  describe('cleanup', () => {
    it('unregisters all event listeners', async () => {
      const unlistenFn = vi.fn()
      vi.mocked(listen).mockResolvedValue(unlistenFn)

      const store = useModelStore()
      await store.setupListeners()
      store.cleanup()

      expect(unlistenFn).toHaveBeenCalledTimes(5)
    })
  })

  describe('setActiveModel', () => {
    it('sets the active model id and backend', () => {
      const store = useModelStore()
      store.setActiveModel('Bonsai-8B', 'llama_cpp')

      expect(store.activeModelId).toBe('Bonsai-8B')
      expect(store.activeModelBackend).toBe('llama_cpp')
    })
  })
})
