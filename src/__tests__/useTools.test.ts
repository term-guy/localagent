import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'

// localStorage mock is provided by happy-dom; reset state between tests
// by clearing the storage key and resetting module-level enabledMap.
const STORAGE_KEY = 'localagent:tools:enabled'

// Re-import per test group so module state doesn't bleed across suites.
// We use vi.resetModules() in beforeEach to reset the singleton enabledMap.
async function freshImport() {
  vi.resetModules()
  return import('@/composables/useTools')
}

describe('parseToolCall', () => {
  let parseToolCall: typeof import('@/composables/useTools').parseToolCall

  beforeEach(async () => {
    ;({ parseToolCall } = await freshImport())
  })

  describe('<tool_call> XML format', () => {
    it('parses a valid <tool_call> block', () => {
      const content = `<tool_call>{"name":"get_weather","arguments":{"location":"Paris"}}</tool_call>`
      const result = parseToolCall(content)
      expect(result).toEqual({ name: 'get_weather', arguments: { location: 'Paris' } })
    })

    it('handles whitespace inside the XML tags', () => {
      const content = `<tool_call>
  { "name": "fetch_page", "arguments": { "url": "https://example.com" } }
</tool_call>`
      const result = parseToolCall(content)
      expect(result).toEqual({ name: 'fetch_page', arguments: { url: 'https://example.com' } })
    })

    it('is case-insensitive for the tag name', () => {
      const content = `<TOOL_CALL>{"name":"get_weather","arguments":{"location":"Tokyo"}}</TOOL_CALL>`
      const result = parseToolCall(content)
      expect(result).toEqual({ name: 'get_weather', arguments: { location: 'Tokyo' } })
    })

    it('returns null when the JSON inside is invalid', () => {
      const result = parseToolCall('<tool_call>not json</tool_call>')
      expect(result).toBeNull()
    })

    it('returns null when JSON is missing "name" field', () => {
      const result = parseToolCall('<tool_call>{"arguments":{"location":"Paris"}}</tool_call>')
      expect(result).toBeNull()
    })

    it('returns null when JSON is missing "arguments" field', () => {
      const result = parseToolCall('<tool_call>{"name":"get_weather"}</tool_call>')
      expect(result).toBeNull()
    })
  })

  describe('markdown code block format', () => {
    it('parses a ```json code block', () => {
      const content = '```json\n{"name":"get_weather","arguments":{"location":"London"}}\n```'
      const result = parseToolCall(content)
      expect(result).toEqual({ name: 'get_weather', arguments: { location: 'London' } })
    })

    it('parses a plain ``` code block without language tag', () => {
      const content = '```\n{"name":"fetch_page","arguments":{"url":"https://example.com"}}\n```'
      const result = parseToolCall(content)
      expect(result).toEqual({ name: 'fetch_page', arguments: { url: 'https://example.com' } })
    })

    it('returns null when markdown block contains non-tool JSON', () => {
      const content = '```json\n{"foo":"bar"}\n```'
      const result = parseToolCall(content)
      expect(result).toBeNull()
    })
  })

  describe('bare JSON fallback', () => {
    it('parses a bare JSON object anywhere in content', () => {
      const content = 'Here is the call: {"name":"get_weather","arguments":{"location":"Berlin"}} done.'
      const result = parseToolCall(content)
      expect(result).toEqual({ name: 'get_weather', arguments: { location: 'Berlin' } })
    })

    it('handles nested braces in arguments correctly', () => {
      const content = '{"name":"fetch_page","arguments":{"url":"https://example.com/path?a={b}"}}'
      const result = parseToolCall(content)
      expect(result).not.toBeNull()
      expect(result?.name).toBe('fetch_page')
    })

    it('returns null when no valid tool JSON is found', () => {
      expect(parseToolCall('plain text with no JSON')).toBeNull()
      expect(parseToolCall('')).toBeNull()
    })

    it('returns null when JSON has wrong shape', () => {
      const result = parseToolCall('{"key":"value"}')
      expect(result).toBeNull()
    })
  })

  it('prefers <tool_call> format over markdown and bare JSON', () => {
    const content = '```json\n{"name":"fetch_page","arguments":{"url":"wrong"}}\n```\n<tool_call>{"name":"get_weather","arguments":{"location":"Rome"}}</tool_call>'
    const result = parseToolCall(content)
    expect(result?.name).toBe('get_weather')
    expect(result?.arguments.location).toBe('Rome')
  })
})

describe('getToolSystemPrompt', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  afterEach(() => {
    localStorage.clear()
  })

  it('returns null when no tools are enabled', async () => {
    const { getToolSystemPrompt } = await freshImport()
    expect(getToolSystemPrompt()).toBeNull()
  })

  it('returns a non-null prompt when at least one tool is enabled', async () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ weather: true }))
    const { getToolSystemPrompt } = await freshImport()
    const prompt = getToolSystemPrompt()
    expect(prompt).not.toBeNull()
    expect(prompt).toContain('get_weather')
    expect(prompt).toContain('<tool_call>')
  })

  it('includes only enabled tools in the prompt', async () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ weather: true, browser: false }))
    const { getToolSystemPrompt } = await freshImport()
    const prompt = getToolSystemPrompt()!
    expect(prompt).toContain('get_weather')
    expect(prompt).not.toContain('fetch_page')
  })

  it('includes all enabled tools when both are on', async () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ weather: true, browser: true }))
    const { getToolSystemPrompt } = await freshImport()
    const prompt = getToolSystemPrompt()!
    expect(prompt).toContain('get_weather')
    expect(prompt).toContain('fetch_page')
  })
})

describe('toggleTool', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  afterEach(() => {
    localStorage.clear()
  })

  it('enables a disabled tool', async () => {
    const { useTools } = await freshImport()
    const { tools, toggleTool } = useTools()

    const before = tools.value.find((t) => t.id === 'weather')!
    expect(before.enabled).toBe(false)

    toggleTool('weather')
    const after = tools.value.find((t) => t.id === 'weather')!
    expect(after.enabled).toBe(true)
  })

  it('disables an enabled tool', async () => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ weather: true }))
    const { useTools } = await freshImport()
    const { tools, toggleTool } = useTools()

    toggleTool('weather')
    expect(tools.value.find((t) => t.id === 'weather')!.enabled).toBe(false)
  })

  it('persists state to localStorage', async () => {
    const { useTools } = await freshImport()
    const { toggleTool } = useTools()
    toggleTool('browser')
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}')
    expect(stored.browser).toBe(true)
  })

  it('toggles independently per tool', async () => {
    const { useTools } = await freshImport()
    const { tools, toggleTool } = useTools()

    toggleTool('weather')
    toggleTool('browser')
    toggleTool('weather') // back off

    expect(tools.value.find((t) => t.id === 'weather')!.enabled).toBe(false)
    expect(tools.value.find((t) => t.id === 'browser')!.enabled).toBe(true)
  })
})

describe('executeTool', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('throws for unknown tool names', async () => {
    const { executeTool } = await freshImport()
    await expect(executeTool('nonexistent', {})).rejects.toThrow('Unknown tool: nonexistent')
  })

  it('fetch_page: throws when url is missing', async () => {
    const { executeTool } = await freshImport()
    await expect(executeTool('fetch_page', {})).rejects.toThrow('fetch_page requires a url argument')
  })

  it('fetch_page: invokes fetch_url and returns title + text on success', async () => {
    vi.mocked(invoke).mockResolvedValue({
      url: 'https://example.com',
      title: 'Example Domain',
      text: 'This domain is for use in illustrative examples.',
      error: undefined,
    })

    const { executeTool } = await freshImport()
    const result = await executeTool('fetch_page', { url: 'https://example.com' })

    expect(invoke).toHaveBeenCalledWith('fetch_url', { url: 'https://example.com' })
    expect(result).toContain('Example Domain')
    expect(result).toContain('This domain is for use in illustrative examples.')
  })

  it('fetch_page: returns error message when backend reports an error', async () => {
    vi.mocked(invoke).mockResolvedValue({
      url: 'https://bad.example',
      title: '',
      text: '',
      error: 'HTTP 404',
    })

    const { executeTool } = await freshImport()
    const result = await executeTool('fetch_page', { url: 'https://bad.example' })

    expect(result).toContain('Error fetching https://bad.example')
    expect(result).toContain('HTTP 404')
  })

  it('fetch_page: omits Page/URL header when title is empty', async () => {
    vi.mocked(invoke).mockResolvedValue({
      url: 'https://example.com/plain',
      title: '',
      text: 'plain content',
      error: undefined,
    })

    const { executeTool } = await freshImport()
    const result = await executeTool('fetch_page', { url: 'https://example.com/plain' })

    expect(result).not.toContain('Page:')
    expect(result).toContain('URL: https://example.com/plain')
    expect(result).toContain('plain content')
  })

  describe('get_weather', () => {
    it('throws when location is missing', async () => {
      const { executeTool } = await freshImport()
      await expect(executeTool('get_weather', {})).rejects.toThrow('get_weather requires a location argument')
    })

    it('calls Open-Meteo geocoding and weather APIs and formats result', async () => {
      const mockFetch = vi.fn()
        .mockResolvedValueOnce({
          json: () => Promise.resolve({
            results: [{ latitude: 48.85, longitude: 2.35, name: 'Paris', country: 'France' }],
          }),
        })
        .mockResolvedValueOnce({
          json: () => Promise.resolve({
            current_weather: { weathercode: 1, temperature: 18.5, windspeed: 12 },
          }),
        })
      vi.stubGlobal('fetch', mockFetch)

      const { executeTool } = await freshImport()
      const result = await executeTool('get_weather', { location: 'Paris' })

      expect(result).toContain('Paris')
      expect(result).toContain('France')
      expect(result).toContain('18.5°C')
      expect(result).toContain('12 km/h')
      // WMO code 1 → "Mainly clear"
      expect(result).toContain('Mainly clear')

      vi.unstubAllGlobals()
    })

    it('throws when location is not found', async () => {
      const mockFetch = vi.fn().mockResolvedValue({
        json: () => Promise.resolve({ results: [] }),
      })
      vi.stubGlobal('fetch', mockFetch)

      const { executeTool } = await freshImport()
      await expect(executeTool('get_weather', { location: 'Atlantis' })).rejects.toThrow('Location not found: Atlantis')

      vi.unstubAllGlobals()
    })
  })
})

describe('useTools', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  afterEach(() => {
    localStorage.clear()
  })

  it('exposes tools, toggleTool, getToolSystemPrompt, executeTool', async () => {
    const { useTools } = await freshImport()
    const result = useTools()

    expect(result).toHaveProperty('tools')
    expect(result).toHaveProperty('toggleTool')
    expect(result).toHaveProperty('getToolSystemPrompt')
    expect(result).toHaveProperty('executeTool')
  })

  it('tools list contains weather and browser entries', async () => {
    const { useTools } = await freshImport()
    const { tools } = useTools()

    const ids = tools.value.map((t) => t.id)
    expect(ids).toContain('weather')
    expect(ids).toContain('browser')
  })

  it('all tools start disabled when localStorage is empty', async () => {
    const { useTools } = await freshImport()
    const { tools } = useTools()

    expect(tools.value.every((t) => !t.enabled)).toBe(true)
  })
})
