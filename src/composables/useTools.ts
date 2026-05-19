import { ref, computed, readonly } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { BrowseResult } from '@/types'

export interface ToolDef {
  id: string
  name: string
  description: string
  icon: string
}

const TOOL_DEFS: ToolDef[] = [
  {
    id: 'weather',
    name: 'Weather',
    description: 'Get current weather for any location',
    icon: '🌤️',
  },
  {
    id: 'browser',
    name: 'Web Browser',
    description: 'Lets the LLM fetch and read web pages on demand',
    icon: '🌐',
  },
]

const TOOL_SCHEMAS = [
  {
    id: 'weather',
    schema: {
      name: 'get_weather',
      description:
        'Get current weather conditions for a city or location. Use this when the user asks about weather.',
      parameters: {
        type: 'object',
        properties: {
          location: {
            type: 'string',
            description: 'City name or location, e.g. "Paris" or "Tokyo"',
          },
        },
        required: ['location'],
      },
    },
  },
  {
    id: 'browser',
    schema: {
      name: 'fetch_page',
      description:
        'Fetch and read the text content of a web page. Use this to look up current information, documentation, or any URL the user mentions.',
      parameters: {
        type: 'object',
        properties: {
          url: {
            type: 'string',
            description: 'The full URL to fetch, including https://',
          },
        },
        required: ['url'],
      },
    },
  },
]

const STORAGE_KEY = 'localagent:tools:enabled'

function loadState(): Record<string, boolean> {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}')
  } catch {
    return {}
  }
}

const enabledMap = ref<Record<string, boolean>>(loadState())

const tools = computed(() =>
  TOOL_DEFS.map((t) => ({ ...t, enabled: enabledMap.value[t.id] ?? false })),
)

function toggleTool(id: string) {
  enabledMap.value = { ...enabledMap.value, [id]: !(enabledMap.value[id] ?? false) }
  localStorage.setItem(STORAGE_KEY, JSON.stringify(enabledMap.value))
}

function tryParseAsToolCall(
  text: string,
): { name: string; arguments: Record<string, string> } | null {
  try {
    const p = JSON.parse(text.trim())
    if (typeof p.name === 'string' && p.arguments && typeof p.arguments === 'object') {
      return { name: p.name, arguments: p.arguments as Record<string, string> }
    }
  } catch {
    /* not valid JSON or wrong shape */
  }
  return null
}

export function parseToolCall(
  content: string,
): { name: string; arguments: Record<string, string> } | null {
  // 1. <tool_call>...</tool_call> — the ideal format
  const xmlMatch = content.match(/<tool_call>\s*([\s\S]*?)\s*<\/tool_call>/i)
  if (xmlMatch) return tryParseAsToolCall(xmlMatch[1])

  // 2. Markdown ```json ... ``` — some models wrap in a code block
  const mdMatch = content.match(/```(?:json)?\s*\n?([\s\S]*?)\n?\s*```/)
  if (mdMatch) {
    const r = tryParseAsToolCall(mdMatch[1])
    if (r) return r
  }

  // 3. Bare JSON object — brace-counting to handle nested args correctly
  const start = content.indexOf('{')
  if (start !== -1) {
    let depth = 0
    let end = -1
    for (let i = start; i < content.length; i++) {
      if (content[i] === '{') depth++
      else if (content[i] === '}') {
        depth--
        if (depth === 0) {
          end = i
          break
        }
      }
    }
    if (end !== -1) {
      const r = tryParseAsToolCall(content.slice(start, end + 1))
      if (r) return r
    }
  }

  return null
}

export function getToolSystemPrompt(): string | null {
  const enabledSchemas = TOOL_SCHEMAS.filter((t) => enabledMap.value[t.id] ?? false).map(
    (t) => t.schema,
  )
  if (enabledSchemas.length === 0) return null

  return `You have access to the following tools. Use them when needed to answer questions accurately.

To call a tool, output ONLY this block — nothing after the closing tag:
<tool_call>
{"name": "tool_name", "arguments": {"param": "value"}}
</tool_call>

The system will execute the tool and return the result. Write your final response only after receiving the result.

Available tools:
${JSON.stringify(enabledSchemas, null, 2)}`
}

// WMO weather interpretation codes
const WMO: Record<number, string> = {
  0: 'Clear sky',
  1: 'Mainly clear',
  2: 'Partly cloudy',
  3: 'Overcast',
  45: 'Foggy',
  48: 'Icy fog',
  51: 'Light drizzle',
  53: 'Drizzle',
  55: 'Dense drizzle',
  61: 'Light rain',
  63: 'Moderate rain',
  65: 'Heavy rain',
  71: 'Light snow',
  73: 'Moderate snow',
  75: 'Heavy snow',
  80: 'Rain showers',
  81: 'Showers',
  82: 'Violent showers',
  95: 'Thunderstorm',
  99: 'Thunderstorm with hail',
}

async function fetchWeatherByLocation(location: string): Promise<string> {
  const geoUrl = `https://geocoding-api.open-meteo.com/v1/search?name=${encodeURIComponent(location)}&count=1`
  const geoRes = await fetch(geoUrl).then((r) => r.json())

  if (!geoRes.results?.length) throw new Error(`Location not found: ${location}`)

  const { latitude, longitude, name, country } = geoRes.results[0]
  const weatherUrl = `https://api.open-meteo.com/v1/forecast?latitude=${latitude}&longitude=${longitude}&current_weather=true`
  const w = await fetch(weatherUrl).then((r) => r.json())
  const cw = w.current_weather

  return `${name}, ${country}: ${WMO[cw.weathercode as number] ?? 'Unknown'}, ${cw.temperature}°C, wind ${cw.windspeed} km/h`
}

export async function executeTool(
  name: string,
  args: Record<string, string>,
): Promise<string> {
  if (name === 'get_weather') {
    if (!args.location) throw new Error('get_weather requires a location argument')
    return await fetchWeatherByLocation(args.location)
  }

  if (name === 'fetch_page') {
    if (!args.url) throw new Error('fetch_page requires a url argument')
    const result = await invoke<BrowseResult>('fetch_url', { url: args.url })
    if (result.error) return `Error fetching ${args.url}: ${result.error}`
    const header = result.title
      ? `Page: ${result.title}\nURL: ${args.url}\n\n`
      : `URL: ${args.url}\n\n`
    return header + result.text
  }

  throw new Error(`Unknown tool: ${name}`)
}

export function useTools() {
  return {
    tools: readonly(tools),
    toggleTool,
    getToolSystemPrompt,
    executeTool,
  }
}
