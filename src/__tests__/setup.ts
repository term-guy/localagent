import { vi } from 'vitest'

// Mock Tauri IPC
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

// Mock Tauri events
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
  emit: vi.fn(),
}))

// Mock UUID
vi.mock('uuid', () => ({
  v4: vi.fn(() => '00000000-0000-0000-0000-000000000000'),
}))
