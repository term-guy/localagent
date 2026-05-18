import { describe, it, expect, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import { useConfirm } from '@/composables/useConfirm'

describe('ConfirmDialog.vue', () => {
  beforeEach(() => {
    // Reset confirm state between tests
    const confirm = useConfirm()
    confirm.visible.value = false
    confirm.options.value = { message: '' }
  })

  it('does not render when not visible', () => {
    const wrapper = mount(ConfirmDialog, { attachTo: document.body })
    expect(document.body.querySelector('[class*="inset-0"]')).toBeNull()
    wrapper.unmount()
  })

  it('renders with title and message when visible', () => {
    const confirm = useConfirm()
    confirm.visible.value = true
    confirm.options.value = {
      title: 'Delete model?',
      message: 'Are you sure you want to delete this model?',
    }

    const wrapper = mount(ConfirmDialog, { attachTo: document.body })
    expect(document.body.textContent).toContain('Delete model?')
    expect(document.body.textContent).toContain('Are you sure you want to delete this model?')
    wrapper.unmount()
  })

  it('renders default button labels', () => {
    const confirm = useConfirm()
    confirm.visible.value = true
    confirm.options.value = {
      title: 'Confirm',
      message: 'Proceed?',
    }

    const wrapper = mount(ConfirmDialog, { attachTo: document.body })
    const buttons = document.body.querySelectorAll('button')
    expect(buttons).toHaveLength(2)
    expect(buttons[0].textContent).toBe('Cancel')
    expect(buttons[1].textContent).toBe('Confirm')
    wrapper.unmount()
  })

  it('renders custom button labels', () => {
    const confirm = useConfirm()
    confirm.visible.value = true
    confirm.options.value = {
      title: 'Confirm',
      message: 'Proceed?',
      confirmLabel: 'Yes, delete',
      cancelLabel: 'No, keep',
    }

    const wrapper = mount(ConfirmDialog, { attachTo: document.body })
    const buttons = document.body.querySelectorAll('button')
    expect(buttons[0].textContent).toBe('No, keep')
    expect(buttons[1].textContent).toBe('Yes, delete')
    wrapper.unmount()
  })

  it('confirm button closes dialog (onConfirm resets visible)', async () => {
    const confirm = useConfirm()
    confirm.visible.value = true
    confirm.options.value = {
      title: 'Confirm',
      message: 'Proceed?',
    }

    const wrapper = mount(ConfirmDialog, { attachTo: document.body })
    expect(document.body.querySelector('[class*="inset-0"]')).not.toBeNull()

    const buttons = document.body.querySelectorAll('button')
    await (buttons[1] as HTMLButtonElement).click()

    expect(confirm.visible.value).toBe(false)
    wrapper.unmount()
  })

  it('cancel button closes dialog (onCancel resets visible)', async () => {
    const confirm = useConfirm()
    confirm.visible.value = true
    confirm.options.value = {
      title: 'Confirm',
      message: 'Proceed?',
    }

    const wrapper = mount(ConfirmDialog, { attachTo: document.body })
    const buttons = document.body.querySelectorAll('button')
    await (buttons[0] as HTMLButtonElement).click()

    expect(confirm.visible.value).toBe(false)
    wrapper.unmount()
  })
})

describe('useConfirm promise', () => {
  beforeEach(() => {
    const { visible, options } = useConfirm()
    visible.value = false
    options.value = { message: '' }
  })

  it('confirm() resolves true when confirmed', async () => {
    const { confirm, onConfirm } = useConfirm()
    const result = confirm({ title: 'Test', message: 'Proceed?' })
    onConfirm()
    await expect(result).resolves.toBe(true)
  })

  it('confirm() resolves false when cancelled', async () => {
    const { confirm, onCancel } = useConfirm()
    const result = confirm({ title: 'Test', message: 'Proceed?' })
    onCancel()
    await expect(result).resolves.toBe(false)
  })
})
