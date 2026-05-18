import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { useToast } from '@/composables/useToast'

// Mock uuid so we get predictable IDs
vi.mock('uuid', () => ({
  v4: vi.fn(() => 'toast-0000-0000'),
}))

describe('useToast', () => {
  beforeEach(() => {
    const { toasts } = useToast()
    toasts.value = []
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('adds a toast', () => {
    const { show, toasts } = useToast()
    show('Test message', 'info')

    expect(toasts.value).toHaveLength(1)
    expect(toasts.value[0].message).toBe('Test message')
    expect(toasts.value[0].type).toBe('info')
    expect(toasts.value[0].id).toBeTruthy()
  })

  it('dismisses a toast', () => {
    const { show, dismiss, toasts } = useToast()
    show('Message', 'info')
    expect(toasts.value).toHaveLength(1)

    dismiss(toasts.value[0].id)
    expect(toasts.value).toHaveLength(0)
  })

  it('auto-dismisses after duration', () => {
    const { show, toasts } = useToast()
    show('Auto dismiss', 'info', 1000)

    expect(toasts.value).toHaveLength(1)

    vi.advanceTimersByTime(999)
    expect(toasts.value).toHaveLength(1)

    vi.advanceTimersByTime(1)
    expect(toasts.value).toHaveLength(0)
  })

  it('supports multiple toasts', () => {
    const { show, toasts } = useToast()
    show('First', 'info')
    show('Second', 'success')
    show('Third', 'error')

    expect(toasts.value).toHaveLength(3)
  })

  it('default type is info', () => {
    const { show, toasts } = useToast()
    show('Default type')

    expect(toasts.value[0].type).toBe('info')
  })

  it('default duration is 3500ms', () => {
    const { show, toasts } = useToast()
    show('Default duration')

    expect(toasts.value[0].duration).toBe(3500)
  })
})
