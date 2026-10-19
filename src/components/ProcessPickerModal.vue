<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { ProcessInfo } from '../types';
import { invoke } from '@tauri-apps/api/tauri';
import { X, Search, RefreshCw, Layers } from 'lucide-vue-next';

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'select', process: ProcessInfo): void;
}>();

const loading = ref(false);
const processes = ref<ProcessInfo[]>([]);
const searchQuery = ref('');

async function loadProcesses() {
  loading.value = true;
  try {
    processes.value = await invoke<ProcessInfo[]>('list_running_processes');
  } catch (err) {
    console.error('Failed to list processes:', err);
  } finally {
    loading.value = false;
  }
}

const filteredProcesses = computed(() => {
  const query = searchQuery.value.toLowerCase().trim();
  if (!query) return processes.value;
  return processes.value.filter(
    (p) =>
      p.name.toLowerCase().includes(query) ||
      p.path.toLowerCase().includes(query) ||
      p.pid.toString().includes(query)
  );
});

onMounted(() => {
  loadProcesses();
});
</script>

<template>
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm">
    <div class="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-2xl max-h-[80vh] flex flex-col shadow-2xl overflow-hidden">
      <!-- Header -->
      <div class="px-6 py-4 border-b border-slate-800 flex items-center justify-between">
        <div class="flex items-center space-x-2.5">
          <Layers class="w-5 h-5 text-indigo-400" />
          <h3 class="text-sm font-semibold text-white">从正在运行的进程中选择</h3>
        </div>
        <button 
          @click="emit('close')"
          class="p-1 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors cursor-pointer"
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Search & Refresh bar -->
      <div class="p-4 border-b border-slate-800/80 bg-slate-950/40 flex items-center space-x-3">
        <div class="relative flex-1">
          <Search class="w-4 h-4 text-slate-500 absolute left-3 top-2.5" />
          <input 
            v-model="searchQuery"
            type="text"
            placeholder="搜索进程名、路径或 PID..."
            class="w-full bg-slate-900 border border-slate-800 rounded-xl pl-9 pr-4 py-2 text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition-colors"
          />
        </div>
        <button 
          @click="loadProcesses"
          :disabled="loading"
          class="p-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700 transition-colors cursor-pointer disabled:opacity-50"
          title="刷新列表"
        >
          <RefreshCw class="w-4 h-4" :class="{ 'animate-spin': loading }" />
        </button>
      </div>

      <!-- Process List -->
      <div class="flex-1 overflow-y-auto p-4 space-y-1.5">
        <div v-if="filteredProcesses.length === 0" class="text-center py-12 text-slate-500 text-xs">
          {{ loading ? '正在扫描系统进程...' : '未找到匹配的运行中进程' }}
        </div>

        <div
          v-for="proc in filteredProcesses"
          :key="proc.path"
          @click="emit('select', proc)"
          class="p-3 rounded-xl bg-slate-950/60 hover:bg-indigo-600/10 border border-slate-800/80 hover:border-indigo-500/40 flex items-center justify-between cursor-pointer group transition-all"
        >
          <div class="min-w-0 pr-4">
            <div class="flex items-center space-x-2">
              <span class="text-xs font-semibold text-slate-200 group-hover:text-indigo-400 transition-colors">
                {{ proc.name }}
              </span>
              <span class="text-[10px] px-1.5 py-0.2 rounded bg-slate-800 text-slate-400 font-mono">
                PID: {{ proc.pid }}
              </span>
            </div>
            <p class="text-[11px] text-slate-400 truncate mt-0.5 font-mono select-all" :title="proc.path">
              {{ proc.path }}
            </p>
          </div>

          <button class="px-2.5 py-1 rounded-lg bg-indigo-600 text-white text-[11px] font-medium opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
            添加
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
