<script setup lang="ts">
import { ShieldAlert, ShieldCheck, Play, Square, Loader2 } from 'lucide-vue-next';

defineProps<{
  isRunning: boolean;
  isAdmin: boolean;
  isStarting: boolean;
}>();

const emit = defineEmits<{
  (e: 'toggleProxy'): void;
  (e: 'restartAdmin'): void;
}>();
</script>

<template>
  <header class="h-16 border-b border-slate-800 bg-slate-900/80 backdrop-blur px-6 flex items-center justify-between select-none shrink-0" data-tauri-drag-region>
    <!-- Logo & Title -->
    <div class="flex items-center space-x-3 pointer-events-none">
      <div class="w-9 h-9 rounded-xl bg-gradient-to-tr from-indigo-600 to-cyan-500 flex items-center justify-center shadow-lg shadow-indigo-500/20">
        <span class="text-white font-black text-lg tracking-wider">P</span>
      </div>
      <div>
        <div class="flex items-center space-x-2">
          <h1 class="text-base font-bold text-white tracking-wide">Pinroute</h1>
          <span class="text-[10px] uppercase font-semibold px-1.5 py-0.5 rounded bg-slate-800 text-slate-400 border border-slate-700">v0.1</span>
        </div>
        <p class="text-xs text-slate-400">将指定 Windows 程序钉在专属代理线路上</p>
      </div>
    </div>

    <!-- Center Status & Admin Warnings -->
    <div class="flex items-center space-x-4">
      <div v-if="!isAdmin" class="flex items-center space-x-2 px-3 py-1.5 rounded-lg bg-amber-500/10 border border-amber-500/20 text-amber-400 text-xs">
        <ShieldAlert class="w-4 h-4 text-amber-400 shrink-0" />
        <span>未以管理员运行（拦截驱动受限）</span>
        <button 
          @click="emit('restartAdmin')"
          class="ml-2 font-medium underline hover:text-amber-300 transition-colors cursor-pointer"
        >
          立即提权重启
        </button>
      </div>

      <div v-else class="flex items-center space-x-1.5 text-xs text-emerald-400 bg-emerald-500/10 px-2.5 py-1 rounded-md border border-emerald-500/20">
        <ShieldCheck class="w-3.5 h-3.5" />
        <span>管理员权限已就绪</span>
      </div>
    </div>

    <!-- Right Controls: Toggle Button -->
    <div class="flex items-center space-x-3">
      <div class="flex items-center space-x-2 mr-2">
        <span 
          class="w-2.5 h-2.5 rounded-full transition-colors duration-300"
          :class="isRunning ? 'bg-emerald-500 shadow-md shadow-emerald-500/50 animate-pulse' : 'bg-slate-600'"
        ></span>
        <span class="text-xs font-medium" :class="isRunning ? 'text-emerald-400' : 'text-slate-400'">
          {{ isRunning ? '代理服务运行中' : '代理服务已停止' }}
        </span>
      </div>

      <button
        @click="emit('toggleProxy')"
        :disabled="isStarting"
        class="flex items-center space-x-2 px-4 py-2 rounded-xl text-sm font-semibold transition-all duration-200 cursor-pointer shadow-lg disabled:opacity-50"
        :class="isRunning 
          ? 'bg-rose-500/10 text-rose-400 border border-rose-500/30 hover:bg-rose-500/20' 
          : 'bg-indigo-600 text-white hover:bg-indigo-500 shadow-indigo-600/30'"
      >
        <Loader2 v-if="isStarting" class="w-4 h-4 animate-spin" />
        <template v-else>
          <Square v-if="isRunning" class="w-4 h-4 fill-current" />
          <Play v-else class="w-4 h-4 fill-current" />
        </template>
        <span>{{ isRunning ? '停止代理' : '启动代理' }}</span>
      </button>
    </div>
  </header>
</template>
