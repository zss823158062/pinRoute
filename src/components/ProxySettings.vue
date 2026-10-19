<script setup lang="ts">
import { ref, reactive } from 'vue';
import { ProxyConfig } from '../types';
import { Network, Server, CheckCircle2, AlertCircle, Loader2 } from 'lucide-vue-next';
import { invoke } from '@tauri-apps/api/tauri';

const props = defineProps<{
  modelValue: ProxyConfig;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: ProxyConfig): void;
  (e: 'save'): void;
}>();

const form = reactive<ProxyConfig>({ ...props.modelValue });

const testing = ref(false);
const testResult = ref<{ success: boolean; message: string } | null>(null);

async function onTest() {
  testing.value = true;
  testResult.value = null;
  try {
    const res = await invoke<string>('test_proxy_connection', { proxy: form });
    testResult.value = { success: true, message: res };
  } catch (err: any) {
    testResult.value = { success: false, message: String(err) };
  } finally {
    testing.value = false;
  }
}

function onSave() {
  emit('update:modelValue', { ...form });
  emit('save');
}
</script>

<template>
  <div class="max-w-3xl mx-auto py-6 px-4">
    <div class="bg-slate-900/60 border border-slate-800/80 rounded-2xl p-6 backdrop-blur">
      <div class="flex items-center space-x-3 mb-6 pb-4 border-b border-slate-800">
        <div class="p-2.5 rounded-xl bg-indigo-500/10 text-indigo-400 border border-indigo-500/20">
          <Server class="w-5 h-5" />
        </div>
        <div>
          <h2 class="text-base font-semibold text-white">上游代理节点设置</h2>
          <p class="text-xs text-slate-400">所有被选中的程序流量都将通过此上游代理发出</p>
        </div>
      </div>

      <div class="grid grid-cols-1 md:grid-cols-3 gap-5">
        <!-- Protocol -->
        <div>
          <label class="block text-xs font-medium text-slate-300 mb-1.5">代理协议</label>
          <div class="relative">
            <select 
              v-model="form.protocol"
              class="w-full bg-slate-950/80 border border-slate-700/80 rounded-xl px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-indigo-500 transition-colors"
            >
              <option value="socks5">SOCKS5 代理 (如 7898)</option>
              <option value="http">HTTP / HTTPS 代理 (如 7899)</option>
            </select>
          </div>
        </div>

        <!-- Host -->
        <div>
          <label class="block text-xs font-medium text-slate-300 mb-1.5">服务器地址 (Host / IP)</label>
          <input 
            v-model="form.host"
            type="text" 
            placeholder="127.0.0.1"
            class="w-full bg-slate-950/80 border border-slate-700/80 rounded-xl px-3 py-2 text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition-colors"
          />
        </div>

        <!-- Port -->
        <div>
          <label class="block text-xs font-medium text-slate-300 mb-1.5">端口号 (Port)</label>
          <input 
            v-model.number="form.port"
            type="number" 
            placeholder="10808"
            class="w-full bg-slate-950/80 border border-slate-700/80 rounded-xl px-3 py-2 text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition-colors"
          />
        </div>
      </div>

      <!-- Authentication -->
      <div class="mt-5 pt-5 border-t border-slate-800/60 grid grid-cols-1 md:grid-cols-2 gap-5">
        <div>
          <label class="block text-xs font-medium text-slate-300 mb-1.5">
            用户名 <span class="text-slate-500 font-normal">(可选)</span>
          </label>
          <input 
            v-model="form.username"
            type="text" 
            placeholder="留空即为免密认证"
            class="w-full bg-slate-950/80 border border-slate-700/80 rounded-xl px-3 py-2 text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition-colors"
          />
        </div>

        <div>
          <label class="block text-xs font-medium text-slate-300 mb-1.5">
            密码 <span class="text-slate-500 font-normal">(可选)</span>
          </label>
          <input 
            v-model="form.password"
            type="password" 
            placeholder="••••••••"
            class="w-full bg-slate-950/80 border border-slate-700/80 rounded-xl px-3 py-2 text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition-colors"
          />
        </div>
      </div>

      <!-- Test & Save actions -->
      <div class="mt-6 flex flex-wrap items-center justify-between gap-4 pt-4 border-t border-slate-800">
        <div class="flex items-center space-x-3">
          <button
            @click="onTest"
            :disabled="testing"
            class="flex items-center space-x-1.5 px-3.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-200 border border-slate-700 transition-colors cursor-pointer disabled:opacity-50"
          >
            <Loader2 v-if="testing" class="w-3.5 h-3.5 animate-spin text-slate-400" />
            <Network v-else class="w-3.5 h-3.5 text-slate-400" />
            <span>{{ testing ? '测试连通性中...' : '测试代理连接' }}</span>
          </button>

          <!-- Result Indicator -->
          <div v-if="testResult" class="flex items-center space-x-1.5 text-xs">
            <CheckCircle2 v-if="testResult.success" class="w-4 h-4 text-emerald-400 shrink-0" />
            <AlertCircle v-else class="w-4 h-4 text-rose-400 shrink-0" />
            <span :class="testResult.success ? 'text-emerald-400' : 'text-rose-400'">
              {{ testResult.message }}
            </span>
          </div>
        </div>

        <button
          @click="onSave"
          class="px-5 py-2 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold shadow-md shadow-indigo-600/20 transition-all cursor-pointer"
        >
          保存设置
        </button>
      </div>
    </div>
  </div>
</template>
