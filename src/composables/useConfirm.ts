import { ref } from 'vue'

interface ConfirmOptions {
  title?: string
  message: string
  confirmLabel?: string
  cancelLabel?: string
  danger?: boolean
}

const visible = ref(false)
const options = ref<ConfirmOptions>({ message: '' })
let resolveFn: ((value: boolean) => void) | null = null

export function useConfirm() {
  function confirm(opts: ConfirmOptions): Promise<boolean> {
    options.value = opts
    visible.value = true
    return new Promise((resolve) => {
      resolveFn = resolve
    })
  }

  function onConfirm() {
    visible.value = false
    resolveFn?.(true)
    resolveFn = null
  }

  function onCancel() {
    visible.value = false
    resolveFn?.(false)
    resolveFn = null
  }

  return { visible, options, confirm, onConfirm, onCancel }
}
