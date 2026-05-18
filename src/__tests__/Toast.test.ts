import { describe, it, expect, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import Toast from '@/components/Toast.vue'
import { useToast } from '@/composables/useToast'

describe('Toast.vue', () => {
  beforeEach(() => {
    // Clear toasts between tests
    const { toasts } = useToast()
    toasts.value = []
  })

  it('renders nothing when there are no toasts', () => {
    const wrapper = mount(Toast, { attachTo: document.body })
    expect(document.body.querySelector('[class*="fixed"]')).not.toBeNull()
    // No toast items rendered
    expect(document.body.textContent).toBe('')
    wrapper.unmount()
  })

  it('renders info toasts', async () => {
    const { show } = useToast()
    show('Hello world', 'info')

    const wrapper = mount(Toast, { attachTo: document.body })
    expect(document.body.textContent).toContain('Hello world')
    expect(document.body.textContent).toContain('ℹ')
    wrapper.unmount()
  })

  it('renders success toasts', () => {
    const { show } = useToast()
    show('Download complete', 'success')

    const wrapper = mount(Toast, { attachTo: document.body })
    expect(document.body.textContent).toContain('Download complete')
    expect(document.body.textContent).toContain('✓')
    wrapper.unmount()
  })

  it('renders error toasts', () => {
    const { show } = useToast()
    show('Something failed', 'error')

    const wrapper = mount(Toast, { attachTo: document.body })
    expect(document.body.textContent).toContain('Something failed')
    expect(document.body.textContent).toContain('✗')
    wrapper.unmount()
  })

  it('renders multiple toasts', () => {
    const { show } = useToast()
    show('First', 'info')
    show('Second', 'success')

    const wrapper = mount(Toast, { attachTo: document.body })
    expect(document.body.textContent).toContain('First')
    expect(document.body.textContent).toContain('Second')
    wrapper.unmount()
  })

  it('dismiss removes toast from state via composable', async () => {
    const { show, toasts, dismiss } = useToast()
    show('Dismiss me', 'info')
    expect(toasts.value).toHaveLength(1)

    // Test the composable directly
    dismiss(toasts.value[0].id)
    expect(toasts.value).toHaveLength(0)
  })

  it('dismiss button click triggers dismiss', async () => {
    const { show, toasts } = useToast()
    show('Dismiss me', 'info')
    const wrapper = mount(Toast, { attachTo: document.body })
    expect(document.body.textContent).toContain('Dismiss me')

    // Button is teleported to body — query via document
    const dismissBtn = document.body.querySelector('button')!
    dismissBtn.dispatchEvent(new MouseEvent('click', { bubbles: true }))

    // After click, toast should be removed from the reactive array
    expect(toasts.value).toHaveLength(0)
    wrapper.unmount()
  })
})
