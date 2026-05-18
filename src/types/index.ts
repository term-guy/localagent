export type Capability = 'chat' | 'vision' | 'audio'

export interface HfFile {
  filename: string
  size_bytes: number
  download_url: string
  quant_name: string
}

export interface DownloadRequest {
  backend: string
  filename?: string
  url?: string
  size_bytes?: number
}

export interface ModelInfo {
  id: string
  display_name: string
  provider: string
  repo: string
  capabilities: Capability[]
  filename: string
  description: string
  default_backend: string
  // llama.cpp
  llama_cpp_url?: string
  llama_cpp_size_mb?: number
  llama_cpp_quant?: string
  // Cactus
  cactus_url?: string
  cactus_size_mb?: number
}

export interface InstalledModel extends ModelInfo {
  file_size_bytes: number
  downloaded_at: string
  backend: string
}

export interface DownloadProgress {
  model_id: string
  bytes_downloaded: number
  total_bytes: number
  speed_bps: number
  percentage: number
  phase?: 'downloading' | 'extracting'
}

export interface ChatMessage {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  image_path?: string
  audio_path?: string
  timestamp: string
  stats?: InferenceStats
}

export interface SessionMeta {
  id: string
  title: string
  created_at: string
  model_id: string
  message_count: number
}

export interface InferenceStats {
  tokens_generated: number
  duration_ms: number
  tokens_per_second: number
}

export interface Toast {
  id: string
  message: string
  type: 'success' | 'error' | 'info'
  duration?: number
}
