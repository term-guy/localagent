<script setup lang="ts">
import type { DownloadProgress } from '@/types'

defineProps<{ progress: DownloadProgress }>()

function formatBytes(bytes: number) {
  const gb = bytes / (1000 ** 3)
  return gb >= 1 ? `${gb.toFixed(2)} GB` : `${(bytes / (1000 ** 2)).toFixed(0)} MB`
}

function formatSpeed(bps: number) {
  const mbs = bps / (1000 * 1000)
  return `${mbs.toFixed(1)} MB/s`
}
</script>

<template>
  <div class="space-y-2">
    <div class="flex justify-between text-xs text-zinc-400">
      <span>
        <span v-if="progress.phase === 'extracting'" class="text-zinc-300">Extracting… </span>
        {{ progress.phase !== 'extracting' ? `${formatBytes(progress.bytes_downloaded)} / ${formatBytes(progress.total_bytes)}` : '' }}
      </span>
      <span class="flex items-center gap-3">
        <span v-if="progress.phase !== 'extracting'">{{ formatSpeed(progress.speed_bps) }}</span>
        <span class="font-medium text-zinc-200">{{ progress.percentage.toFixed(1) }}%</span>
      </span>
    </div>
    <div class="h-1.5 w-full rounded-full bg-zinc-700 overflow-hidden">
      <div
        class="h-full rounded-full bg-gradient-to-r from-primary-600 to-primary-400"
        :style="{ width: `${progress.percentage}%` }"
      />
    </div>
  </div>
</template>
