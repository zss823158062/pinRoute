<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { AppConfig } from './types';
import { invoke } from '@tauri-apps/api/tauri';
import Navbar from './components/Navbar.vue';
import ProcessList from './components/ProcessList.vue';
import ProxySettings from './components/ProxySettings.vue';
import LogViewer from './components/LogViewer.vue';
import { ListFilter, Server, Activity, Settings2, CheckCircle2, Zap } from 'lucide-vue-next';

const currentTab = ref<'rules' | 'proxy' | 'logs' | 'settings'>('rules');

const isRunning = ref(false);
const isAdmin = ref(false);
const isStarting = ref(false);
const toast = ref<string | null>(null);

const config = ref<AppConfig>({
  proxy: {
    protocol: 'socks5',
    host: '127.0.0.1',
    port: 10808,
    username: null,
    password: null,
  },
  rules: [],
  settings: {
    bypass_lan: true,
    block_udp_443: true,
    auto_start: false,
    minimize_to_tray: true,
  },
});

function showToast(msg: string) {
  toast.value = msg;
  setTimeout(() => {
    toast.value = null;
  }, 3000);
}

async function loadData() {
  try {
    const loadedConfig = await invoke<AppConfig>('get_config');
    config.value = loadedConfig;

    const status = await invoke<{ is_running: boolean; is_admin: boolean }>('get_proxy_status');
    isRunning.value = status.is_running;
    isAdmin.value = status.is_admin;
  } catch (err) {
    console.error('Failed to load initial config/status:', err);
  }
}

async function saveAllConfig() {
  try {
    await invoke('save_config', { config: config.value });
    showToast('配置已保存');
  } catch (err: any) {
    alert('保存配置失败: ' + String(err));
  }
}

async function onToggleProxy() {
  if (isRunning.value) {
    try {
      await invoke('stop_proxy');
      isRunning.value = false;
      showToast('代理服务已停止');
    } catch (err: any) {
      alert('停止失败: ' + String(err));
    }
  } else {
    isStarting.value = true;
    try {
      await invoke('start_proxy');
      isRunning.value = true;
      showToast('代理服务已成功启动！');
    } catch (err: any) {
      alert('启动失败: ' + String(err));
    } finally {
      isStarting.value = false;
    }
  }
}

async function onRestartAdmin() {
  try {
    await invoke('trigger_restart_as_admin');
  } catch (err: any) {
    alert('以管理员重启失败: ' + String(err));
  }
}

onMounted(() => {
  loadData();
});
</script>

<template>
  <div class="h-screen w-screen flex flex-col bg-slate-950 select-none overflow-hidden">
    <!-- Navbar -->
    <Navbar 
      :is-running="isRunning" 
      :is-admin="isAdmin" 
      :is-starting="isStarting"
      @toggle-proxy="onToggleProxy"
      @restart-admin="onRestartAdmin"
    />

    <!-- Main Content Area -->
    <div class="flex-1 flex overflow-hidden">
      <!-- Sidebar / Navigation Tabs -->
      <aside class="w-48 bg-slate-900/40 border-r border-slate-800 flex flex-col p-3 space-y-1 shrink-0">
        <button
          @click="currentTab = 'rules'"
          class="flex items-center space-x-2.5 px-3 py-2.5 rounded-xl text-xs font-medium transition-all cursor-pointer"
          :class="currentTab === 'rules' 
            ? 'bg-indigo-600/15 text-indigo-400 border border-indigo-500/30 font-semibold' 
            : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/40'"
        >
          <ListFilter class="w-4 h-4" />
          <span>程序规则 ({{ config.rules.length }})</span>
        </button>

        <button
          @click="currentTab = 'proxy'"
          class="flex items-center space-x-2.5 px-3 py-2.5 rounded-xl text-xs font-medium transition-all cursor-pointer"
          :class="currentTab === 'proxy' 
            ? 'bg-indigo-600/15 text-indigo-400 border border-indigo-500/30 font-semibold' 
            : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/40'"
        >
          <Server class="w-4 h-4" />
          <span>上游代理节点</span>
        </button>

        <button
          @click="currentTab = 'logs'"
          class="flex items-center space-x-2.5 px-3 py-2.5 rounded-xl text-xs font-medium transition-all cursor-pointer"
          :class="currentTab === 'logs' 
            ? 'bg-indigo-600/15 text-indigo-400 border border-indigo-500/30 font-semibold' 
            : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/40'"
        >
          <Activity class="w-4 h-4" />
          <span>实时日志</span>
        </button>

        <button
          @click="currentTab = 'settings'"
          class="flex items-center space-x-2.5 px-3 py-2.5 rounded-xl text-xs font-medium transition-all cursor-pointer"
          :class="currentTab === 'settings' 
            ? 'bg-indigo-600/15 text-indigo-400 border border-indigo-500/30 font-semibold' 
            : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/40'"
        >
          <Settings2 class="w-4 h-4" />
          <span>高级设置</span>
        </button>

        <div class="mt-auto p-3 rounded-xl bg-slate-950/60 border border-slate-800/60 text-[11px] text-slate-400 space-y-1">
          <div class="flex items-center space-x-1 text-slate-300 font-semibold">
            <Zap class="w-3.5 h-3.5 text-indigo-400" />
            <span>智能分流已就绪</span>
          </div>
          <p class="text-[10px] text-slate-400 leading-relaxed">
            仅命中规则的程序经由上游代理转发，未命中程序原样直连。
          </p>
        </div>
      </aside>

      <!-- Content Views -->
      <main class="flex-1 overflow-hidden relative">
        <ProcessList 
          v-if="currentTab === 'rules'"
          :rules="config.rules"
          @update:rules="config.rules = $event"
          @save="saveAllConfig"
        />

        <ProxySettings 
          v-else-if="currentTab === 'proxy'"
          v-model="config.proxy"
          @save="saveAllConfig"
        />

        <LogViewer 
          v-else-if="currentTab === 'logs'"
        />

        <!-- Advanced Settings Tab -->
        <div v-else-if="currentTab === 'settings'" class="max-w-2xl mx-auto py-8 px-4 space-y-6">
          <div class="bg-slate-900/60 border border-slate-800/80 rounded-2xl p-6 backdrop-blur space-y-5">
            <h3 class="text-sm font-semibold text-white pb-3 border-b border-slate-800">高级流量策略设置</h3>

            <div class="space-y-4">
              <!-- Bypass LAN -->
              <div class="flex items-start justify-between">
                <div>
                  <h4 class="text-xs font-semibold text-slate-200">局域网与本地回环默认直连 (F-23)</h4>
                  <p class="text-[11px] text-slate-400 mt-0.5">
                    自动绕过 127.0.0.1、10.0.0.0/8、172.16.0.0/12、192.168.0.0/16 地址段
                  </p>
                </div>
                <input 
                  v-model="config.settings.bypass_lan"
                  type="checkbox"
                  @change="saveAllConfig"
                  class="rounded bg-slate-900 border-slate-700 text-indigo-600 focus:ring-0 mt-0.5 cursor-pointer"
                />
              </div>

              <!-- Block UDP 443 -->
              <div class="flex items-start justify-between pt-3 border-t border-slate-800/60">
                <div>
                  <h4 class="text-xs font-semibold text-slate-200">阻断命中程序的 UDP 443 (QUIC 对策, F-27)</h4>
                  <p class="text-[11px] text-slate-400 mt-0.5">
                    丢弃目标程序的 QUIC 包，促使 Chromium/Electron 系应用自动回退至 TCP 代理
                  </p>
                </div>
                <input 
                  v-model="config.settings.block_udp_443"
                  type="checkbox"
                  @change="saveAllConfig"
                  class="rounded bg-slate-900 border-slate-700 text-indigo-600 focus:ring-0 mt-0.5 cursor-pointer"
                />
              </div>
            </div>
          </div>
        </div>
      </main>
    </div>

    <!-- Toast Notification -->
    <transition
      enter-active-class="transform ease-out duration-200 transition"
      enter-from-class="translate-y-2 opacity-0"
      enter-to-class="translate-y-0 opacity-100"
      leave-active-class="transition ease-in duration-150"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div 
        v-if="toast" 
        class="fixed bottom-6 right-6 z-50 flex items-center space-x-2 px-4 py-2.5 rounded-xl bg-indigo-600 text-white text-xs font-medium shadow-xl shadow-indigo-600/30"
      >
        <CheckCircle2 class="w-4 h-4 shrink-0" />
        <span>{{ toast }}</span>
      </div>
    </transition>
  </div>
</template>
