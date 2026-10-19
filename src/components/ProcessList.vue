<script setup lang="ts">
import { ref } from 'vue';
import { AppRule, ProcessInfo, MatchType } from '../types';
import { open } from '@tauri-apps/api/dialog';
import { Plus, FolderOpen, Layers, Trash2, CheckCircle2, Circle, GitFork } from 'lucide-vue-next';
import ProcessPickerModal from './ProcessPickerModal.vue';

const props = defineProps<{
  rules: AppRule[];
}>();

const emit = defineEmits<{
  (e: 'update:rules', rules: AppRule[]): void;
  (e: 'save'): void;
}>();

const showPicker = ref(false);
const showManualModal = ref(false);

const manualForm = ref({
  name: '',
  path: '',
  match_type: 'exact' as MatchType,
  include_children: true,
});

// Select executable using native Windows file dialog (F-01)
async function onBrowseExe() {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Windows Executable (*.exe)', extensions: ['exe'] }],
    });

    if (selected && typeof selected === 'string') {
      const fileName = selected.split(/[\\/]/).pop() || 'Unknown App';
      const name = fileName.replace(/\.exe$/i, '');
      addRule({
        id: Date.now().toString(),
        name,
        path: selected,
        match_type: 'exact',
        include_children: true,
        enabled: true,
      });
    }
  } catch (err) {
    console.error('File dialog error:', err);
  }
}

function onProcessSelected(proc: ProcessInfo) {
  showPicker.value = false;
  const name = proc.name.replace(/\.exe$/i, '');
  addRule({
    id: Date.now().toString(),
    name,
    path: proc.path,
    match_type: 'exact',
    include_children: true,
    enabled: true,
  });
}

function onManualSubmit() {
  if (!manualForm.value.path.trim()) return;
  const name = manualForm.value.name.trim() || manualForm.value.path.split(/[\\/]/).pop() || 'Rule';
  addRule({
    id: Date.now().toString(),
    name,
    path: manualForm.value.path.trim(),
    match_type: manualForm.value.match_type,
    include_children: manualForm.value.include_children,
    enabled: true,
  });
  showManualModal.value = false;
  manualForm.value = {
    name: '',
    path: '',
    match_type: 'exact',
    include_children: true,
  };
}

function addRule(newRule: AppRule) {
  // Avoid duplicate by path
  const existing = props.rules.find((r) => r.path.toLowerCase() === newRule.path.toLowerCase());
  if (existing) {
    existing.enabled = true;
  } else {
    emit('update:rules', [...props.rules, newRule]);
  }
  emit('save');
}

function removeRule(id: string) {
  emit('update:rules', props.rules.filter((r) => r.id !== id));
  emit('save');
}

function toggleRule(rule: AppRule) {
  rule.enabled = !rule.enabled;
  emit('update:rules', [...props.rules]);
  emit('save');
}

function toggleChildren(rule: AppRule) {
  rule.include_children = !rule.include_children;
  emit('update:rules', [...props.rules]);
  emit('save');
}
</script>

<template>
  <div class="h-full flex flex-col p-6 space-y-4">
    <!-- Header Actions -->
    <div class="flex items-center justify-between pb-3 border-b border-slate-800">
      <div>
        <h2 class="text-sm font-semibold text-white">指定走代理的程序规则</h2>
        <p class="text-xs text-slate-400">列表中的程序（及其子进程）发出的 TCP 连接将被透明重定向到上游代理</p>
      </div>

      <div class="flex items-center space-x-2.5">
        <button
          @click="showManualModal = true"
          class="flex items-center space-x-1.5 px-3 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-300 border border-slate-700 transition-colors cursor-pointer"
        >
          <Plus class="w-3.5 h-3.5" />
          <span>自定义规则</span>
        </button>

        <button
          @click="showPicker = true"
          class="flex items-center space-x-1.5 px-3 py-1.5 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-medium text-slate-300 border border-slate-700 transition-colors cursor-pointer"
        >
          <Layers class="w-3.5 h-3.5 text-indigo-400" />
          <span>运行中进程</span>
        </button>

        <button
          @click="onBrowseExe"
          class="flex items-center space-x-1.5 px-3.5 py-1.5 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-xs font-semibold text-white shadow-md shadow-indigo-600/20 transition-all cursor-pointer"
        >
          <FolderOpen class="w-3.5 h-3.5" />
          <span>浏览添加 .exe</span>
        </button>
      </div>
    </div>

    <!-- Rules Table -->
    <div class="flex-1 overflow-y-auto rounded-2xl border border-slate-800/80 bg-slate-900/40">
      <div v-if="rules.length === 0" class="h-full flex flex-col items-center justify-center py-20 text-slate-500 space-y-3">
        <FolderOpen class="w-10 h-10 text-slate-600" />
        <div class="text-center">
          <p class="text-sm font-medium text-slate-400">尚未添加任何程序规则</p>
          <p class="text-xs text-slate-500 mt-1">点击右上角"浏览添加 .exe"或"运行中进程"将程序钉入代理</p>
        </div>
      </div>

      <table v-else class="w-full table-fixed text-left text-xs border-collapse">
        <thead>
          <tr class="border-b border-slate-800 bg-slate-950/40 text-slate-400 font-medium whitespace-nowrap">
            <th class="py-3 px-4 w-16 text-center">状态</th>
            <th class="py-3 px-4 w-40">程序名称</th>
            <th class="py-3 px-4">路径 / 匹配规则</th>
            <th class="py-3 px-4 w-28 text-center">模式</th>
            <th class="py-3 px-4 w-28 text-center" title="匹配该程序派生的所有子进程">包含子进程</th>
            <th class="py-3 px-4 w-16 text-center">操作</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-slate-800/50">
          <tr 
            v-for="rule in rules" 
            :key="rule.id" 
            class="hover:bg-slate-800/20 transition-colors"
            :class="{ 'opacity-50': !rule.enabled }"
          >
            <!-- Toggle enable -->
            <td class="py-3 px-4 text-center">
              <button 
                @click="toggleRule(rule)"
                class="cursor-pointer transition-colors"
                :title="rule.enabled ? '停用此规则' : '启用此规则'"
              >
                <CheckCircle2 v-if="rule.enabled" class="w-4 h-4 text-emerald-400" />
                <Circle v-else class="w-4 h-4 text-slate-600" />
              </button>
            </td>

            <!-- Name -->
            <td class="py-3 px-4 font-semibold text-slate-200 truncate" :title="rule.name">
              {{ rule.name }}
            </td>

            <!-- Path -->
            <td class="py-3 px-4 font-mono text-[11px] text-slate-400 select-all truncate" :title="rule.path">
              {{ rule.path }}
            </td>

            <!-- Match Type Badge -->
            <td class="py-3 px-4 text-center">
              <span
                class="inline-block px-2 py-0.5 rounded text-[10px] font-medium border whitespace-nowrap"
                :class="{
                  'bg-blue-500/10 text-blue-400 border-blue-500/20': rule.match_type === 'exact',
                  'bg-purple-500/10 text-purple-400 border-purple-500/20': rule.match_type === 'prefix',
                  'bg-amber-500/10 text-amber-400 border-amber-500/20': rule.match_type === 'wildcard',
                }"
              >
                {{ rule.match_type === 'exact' ? '精确路径' : rule.match_type === 'prefix' ? '目录前缀' : '通配符' }}
              </span>
            </td>

            <!-- Children toggle -->
            <td class="py-3 px-4 text-center">
              <button
                @click="toggleChildren(rule)"
                class="inline-flex items-center space-x-1 px-2 py-0.5 rounded text-[10px] font-medium border whitespace-nowrap transition-colors cursor-pointer"
                :class="rule.include_children 
                  ? 'bg-indigo-500/10 text-indigo-400 border-indigo-500/20' 
                  : 'bg-slate-800/40 text-slate-500 border-slate-700/50'"
              >
                <GitFork class="w-3 h-3" />
                <span>{{ rule.include_children ? '是' : '否' }}</span>
              </button>
            </td>

            <!-- Delete -->
            <td class="py-3 px-4 text-center">
              <button 
                @click="removeRule(rule.id)"
                class="p-1 rounded-lg text-slate-500 hover:text-rose-400 hover:bg-rose-500/10 transition-colors cursor-pointer"
                title="删除规则"
              >
                <Trash2 class="w-3.5 h-3.5" />
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Modal: Process Picker -->
    <ProcessPickerModal 
      v-if="showPicker" 
      @close="showPicker = false" 
      @select="onProcessSelected" 
    />

    <!-- Modal: Manual Custom Rule -->
    <div v-if="showManualModal" class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm">
      <div class="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-lg p-6 space-y-4 shadow-2xl">
        <h3 class="text-sm font-semibold text-white">添加自定义匹配规则</h3>
        <p class="text-xs text-slate-400">支持目录前缀或通配符规则，适应版本更新或复杂多进程架构应用。</p>

        <div class="space-y-3 pt-2">
          <div>
            <label class="block text-xs font-medium text-slate-300 mb-1">规则名称</label>
            <input 
              v-model="manualForm.name"
              type="text" 
              placeholder="如：Claude Desktop"
              class="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-slate-200 focus:outline-none focus:border-indigo-500"
            />
          </div>

          <div>
            <label class="block text-xs font-medium text-slate-300 mb-1">路径 / 表达式</label>
            <input 
              v-model="manualForm.path"
              type="text" 
              placeholder="如：C:\Users\*\AppData\Local\Programs\Claude\*"
              class="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-slate-200 font-mono focus:outline-none focus:border-indigo-500"
            />
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs font-medium text-slate-300 mb-1">匹配模式</label>
              <select 
                v-model="manualForm.match_type"
                class="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-slate-200 focus:outline-none focus:border-indigo-500"
              >
                <option value="exact">精确路径 (Exact)</option>
                <option value="prefix">目录前缀 (Prefix)</option>
                <option value="wildcard">通配符 (Wildcard * ?)</option>
              </select>
            </div>

            <div>
              <label class="block text-xs font-medium text-slate-300 mb-1">包含子进程 (进程树)</label>
              <select 
                v-model="manualForm.include_children"
                class="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-slate-200 focus:outline-none focus:border-indigo-500"
              >
                <option :value="true">是 (匹配派生子进程)</option>
                <option :value="false">否 (仅限目标自身)</option>
              </select>
            </div>
          </div>
        </div>

        <div class="flex items-center justify-end space-x-3 pt-3 border-t border-slate-800">
          <button 
            @click="showManualModal = false"
            class="px-4 py-2 rounded-xl text-xs font-medium text-slate-400 hover:text-white transition-colors cursor-pointer"
          >
            取消
          </button>
          <button 
            @click="onManualSubmit"
            class="px-4 py-2 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-xs font-semibold text-white shadow-md shadow-indigo-600/20 cursor-pointer"
          >
            确认添加
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
