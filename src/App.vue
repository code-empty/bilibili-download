<template>
  <div class="page">
    <header class="page__header">
      <div class="brand">
        <span class="brand__logo">SD</span>
        <div>
          <h1 class="brand__title">SnapDown</h1>
          <p class="brand__subtitle">桌面视频下载器 · 支持 B站 / 抖音 / YouTube</p>
        </div>
      </div>
      <el-button size="large" type="primary" class="btn-glow" @click="openSettings = true">
        <el-icon><Setting /></el-icon>
        <span>参数设置</span>
      </el-button>
    </header>

    <main class="page__content">
      <el-card class="panel panel--hero" shadow="never">
        <template #header>
          <div class="panel__header">
            <div>
              <h2 class="panel__title">创建下载任务</h2>
              <p class="panel__subtitle">支持 Bilibili / Douyin / YouTube 单链接下载</p>
            </div>
            <el-button @click="refreshList">
              <el-icon><Refresh /></el-icon>
              刷新
            </el-button>
          </div>
        </template>

        <el-form :model="form" label-position="top" class="create-form">
          <div class="form-grid">
            <el-form-item label="视频链接">
              <el-input
                v-model="form.url"
                clearable
                placeholder="粘贴链接，例如 https://www.bilibili.com/video/BVxxx"
                :prefix-icon="Link"
                @keyup.enter="submitTask"
                size="large"
              />
            </el-form-item>

            <el-form-item label="站点">
              <el-select v-model="form.platform_hint" placeholder="自动识别" size="large">
                <el-option label="自动识别" value="auto" />
                <el-option label="Bilibili" value="bilibili" />
                <el-option label="Douyin" value="douyin" />
                <el-option label="YouTube" value="youtube" />
              </el-select>
            </el-form-item>

            <el-form-item label="清晰度" v-if="!isAudioOnly">
              <el-select v-model="form.quality" size="large">
                <el-option label="自动" value="" />
                <el-option label="1080P" value="1080" />
                <el-option label="720P" value="720" />
                <el-option label="480P" value="480" />
              </el-select>
            </el-form-item>

            <el-form-item label="视频编码" v-if="!isAudioOnly">
              <el-select v-model="form.vcodec" size="large">
                <el-option label="自动（默认）" value="auto" />
                <el-option label="H.264 / AVC（适配老播放机）" value="h264" />
                <el-option label="HEVC / H.265（高压缩率）" value="hevc" />
                <el-option label="AV1（超高压缩率）" value="av1" />
              </el-select>
            </el-form-item>

            <el-form-item label="输出格式">
              <el-select v-model="form.format" size="large">
                <el-option-group label="视频">
                  <el-option label="MP4（视频+音频）" value="mp4" />
                  <el-option label="MKV（视频+音频）" value="mkv" />
                  <el-option label="仅视频（无声音）" value="video-only" />
                </el-option-group>
                <el-option-group label="音频">
                  <el-option label="MP3" value="mp3" />
                  <el-option label="M4A（AAC）" value="m4a" />
                  <el-option label="FLAC（无损）" value="flac" />
                </el-option-group>
              </el-select>
              <p v-if="isAudioOnly" class="format-hint">需要安装 FFmpeg 才能提取音频</p>
            </el-form-item>
          </div>

          <div class="form-actions">
            <el-button type="primary" :loading="submitting" size="large" class="btn-glow" @click="submitTask">
              <el-icon><UploadFilled /></el-icon>
              开始下载
            </el-button>
            <el-button size="large" @click="openOutputDir">打开输出目录</el-button>
          </div>
        </el-form>

        <div class="stats">
          <div class="stat">
            <h3>默认目录</h3>
            <p>{{ settings.output_dir || '未设置' }}</p>
          </div>
          <div class="stat">
            <h3>Cookie 文件</h3>
            <p>{{ settings.cookie_path || '未设置' }}</p>
          </div>
          <div class="stat">
            <h3>重试次数</h3>
            <p>{{ settings.retry_count }} 次</p>
          </div>
          <div class="stat">
            <h3>运行中 / 完成</h3>
            <p>{{ runningCount }} / {{ completedCount }}</p>
          </div>
        </div>
      </el-card>

      <el-card class="panel" shadow="never">
        <template #header>
          <div class="panel__header">
            <div>
              <h2 class="panel__title">下载任务列表</h2>
              <p class="panel__subtitle">支持重试、取消、打开文件</p>
            </div>
            <el-space>
              <el-button size="small" type="danger" plain @click="clearFinished">清除已结束</el-button>
              <el-tag type="info">{{ tasks.length }} 条</el-tag>
            </el-space>
          </div>
        </template>

        <el-table :data="tasks" border stripe size="large" table-layout="fixed">
          <el-table-column prop="platform" label="站点" width="110" />
          <el-table-column prop="url" label="链接" min-width="280" show-overflow-tooltip />
          <el-table-column label="状态" width="130">
            <template #default="scope">
              <el-tag :type="statusTagType(scope.row.status)" effect="light">
                {{ statusText(scope.row.status) }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="进度" width="180">
            <template #default="scope">
              <el-progress
                :percentage="Math.round(scope.row.progress)"
                :status="progressStatus(scope.row.status)"
                :stroke-width="12"
                text-inside
              />
            </template>
          </el-table-column>
          <el-table-column label="速度" width="120" prop="speed" />
          <el-table-column label="编码" width="110">
            <template #default="scope">
              <el-tag v-if="scope.row.vcodec && scope.row.vcodec !== 'auto'" size="small" type="warning" effect="plain">
                {{ scope.row.vcodec.toUpperCase() }}
              </el-tag>
              <span v-else style="color: #8c8c8c; font-size: 12px;">自动</span>
            </template>
          </el-table-column>
          <el-table-column label="剩余" width="90" prop="eta">
            <template #default="scope">
              {{ scope.row.eta ? `${scope.row.eta}s` : '-' }}
            </template>
          </el-table-column>
          <el-table-column label="动作" width="310" fixed="right">
            <template #default="scope">
              <el-space>
                <el-button
                  size="small"
                  type="primary"
                  :disabled="scope.row.status === 'running' || scope.row.status === 'queued'"
                  @click="retry(scope.row.id)"
                >重试</el-button>
                <el-button
                  size="small"
                  :disabled="scope.row.status !== 'running'"
                  @click="cancel(scope.row.id)"
                >取消</el-button>
                <el-button
                  size="small"
                  :disabled="!scope.row.file_path"
                  @click="openFile(scope.row.id)"
                >打开文件</el-button>
                <el-button
                  size="small"
                  type="danger"
                  :disabled="scope.row.status === 'running'"
                  @click="removeTask(scope.row.id)"
                >删除</el-button>
              </el-space>
            </template>
          </el-table-column>
        </el-table>
      </el-card>

      <el-card class="panel" shadow="never">
        <template #header>
          <div class="panel__header">
            <h2 class="panel__title">下载日志</h2>
            <el-button size="small" plain @click="logs = []">清空</el-button>
          </div>
        </template>
        <div class="logs">
          <div v-for="(item, idx) in logs" :key="`${item.task_id}-${idx}`" class="log-line">
            <span class="log-level" :class="`level-${item.level}`">{{ item.level.toUpperCase() }}</span>
            <span class="log-task">{{ item.task_id }}</span>
            <span class="log-message">{{ item.message }}</span>
          </div>
          <p v-if="!logs.length" class="logs-empty">暂无日志</p>
        </div>
      </el-card>
    </main>

    <el-drawer v-model="openSettings" title="设置" size="380px">
      <div class="settings">
        <el-form label-position="top">
          <el-form-item label="默认输出目录">
            <div class="settings__row">
              <el-input v-model="settings.output_dir" placeholder="例如 C:\\Users\\Me\\Downloads\\视频" />
              <el-button @click="pickOutputDir">选择</el-button>
            </div>
          </el-form-item>
          <el-form-item label="登录 Cookie 文件（可选）">
            <div class="settings__row settings__row--split">
              <el-input v-model="settings.cookie_path" placeholder="例如 C:\\Users\\Me\\cookies.txt" />
              <el-button @click="pickCookieFile">选择</el-button>
              <el-button @click="clearCookieFile">清空</el-button>
            </div>
            <p class="setting-tip">仅用于你本人授权或登录态可访问内容，建议仅导入来自你浏览器的导出文件</p>
          </el-form-item>
          <el-form-item label="默认视频编码">
            <el-select v-model="settings.vcodec" style="width: 100%">
              <el-option label="自动（默认）" value="auto" />
              <el-option label="H.264 / AVC（适配老播放机）" value="h264" />
              <el-option label="HEVC / H.265（高压缩率）" value="hevc" />
              <el-option label="AV1（超高压缩率）" value="av1" />
            </el-select>
          </el-form-item>
          <el-form-item label="失败重试次数">
            <el-input-number v-model="settings.retry_count" :min="0" :max="10" />
          </el-form-item>
        </el-form>
        <el-space>
          <el-button type="primary" @click="saveSettings">保存</el-button>
          <el-button @click="openSettings = false">关闭</el-button>
        </el-space>
      </div>
    </el-drawer>

    <p class="footer">
      合规提醒：仅下载你有权访问或已授权的内容；不主动提供绕过版权或反爬/风控策略的工具。
    </p>
  </div>
</template>

<script lang="ts" setup>
import { computed, onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import { ElMessage } from 'element-plus';
import { Link, Refresh, Setting, UploadFilled } from '@element-plus/icons-vue';

interface TaskRecord {
  id: string;
  url: string;
  platform: string;
  quality: string;
  format: string;
  vcodec: string;
  status: 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';
  progress: number;
  speed?: string;
  eta?: number;
  file_path?: string;
  error?: string;
  output_dir: string;
  created_at: string;
  updated_at: string;
  retry_count: number;
}

interface Settings {
  output_dir: string;
  cookie_path: string;
  retry_count: number;
  vcodec: string;
}

interface ProgressEvent {
  task_id: string;
  percent: number;
  speed?: string;
  eta?: number;
  status: TaskRecord['status'];
  message: string;
}

interface LogLine {
  task_id: string;
  level: string;
  message: string;
}

interface ResultEvent {
  task_id: string;
  success: boolean;
  file_path?: string;
  duration_ms: number;
  error?: string;
  raw?: string;
}

const form = ref({
  url: '',
  platform_hint: 'auto',
  quality: '',
  format: 'mp4',
  vcodec: 'auto',
  output_dir: '',
  overwrite: false,
});

const tasks = ref<TaskRecord[]>([]);
const settings = ref<Settings>({ output_dir: '', cookie_path: '', retry_count: 2, vcodec: 'auto' });
const logs = ref<LogLine[]>([]);
const openSettings = ref(false);
const submitting = ref(false);

const AUDIO_FORMATS = ['mp3', 'm4a', 'flac'];
const isAudioOnly = computed(() => AUDIO_FORMATS.includes(form.value.format));

const runningCount = computed(() => tasks.value.filter((task) => task.status === 'running').length);
const completedCount = computed(() => tasks.value.filter((task) => task.status === 'completed').length);

const statusText = (status: TaskRecord['status']) =>
  ({
    queued: '排队中',
    running: '下载中',
    completed: '已完成',
    failed: '失败',
    cancelled: '已取消',
  })[status];

const statusTagType = (status: TaskRecord['status']) =>
  ({
    queued: 'info',
    running: 'primary',
    completed: 'success',
    failed: 'danger',
    cancelled: 'warning',
  })[status];

const progressStatus = (status: TaskRecord['status']) => {
  if (status === 'failed' || status === 'cancelled') return 'exception';
  if (status === 'completed') return 'success';
  return '';
};

function trimLog(message: string, max = 120) {
  if (!message) return '';
  return message.length > max ? `${message.slice(0, max)}...` : message;
}

function pushLog(item: LogLine) {
  logs.value.unshift({
    task_id: item.task_id || 'system',
    level: item.level || 'info',
    message: item.message,
  });
  if (logs.value.length > 360) logs.value.length = 360;
}

async function refreshList() {
  tasks.value = (await invoke<TaskRecord[]>('list_tasks')) || [];
}

async function refreshSettings() {
  settings.value =
    (await invoke<Settings>('get_settings')) || { output_dir: '', cookie_path: '', retry_count: 2, vcodec: 'auto' };
  if (settings.value.vcodec && form.value.vcodec === 'auto') {
    form.value.vcodec = settings.value.vcodec;
  }
}

async function submitTask() {
  if (!form.value.url.trim()) {
    ElMessage.warning('请输入要下载的链接');
    return;
  }
  submitting.value = true;
  try {
    await invoke<string>('create_task', {
      input: {
        url: form.value.url.trim(),
        platform_hint: form.value.platform_hint === 'auto' ? null : form.value.platform_hint,
        output_dir: form.value.output_dir.trim() || settings.value.output_dir || null,
        cookie_path: settings.value.cookie_path || null,
        quality: form.value.quality || null,
        format: form.value.format || 'mp4',
        vcodec: form.value.vcodec || settings.value.vcodec || 'auto',
        overwrite: form.value.overwrite,
      },
    });
    ElMessage.success('任务已提交');
    form.value.url = '';
    await refreshList();
  } catch (err: unknown) {
    ElMessage.error(`创建任务失败：${String(err)}`);
  } finally {
    submitting.value = false;
  }
}

async function retry(taskId: string) {
  const task = tasks.value.find((item) => item.id === taskId);
  if (!task) return;
  await invoke<string>('create_task', {
    input: {
      url: task.url,
      platform_hint: task.platform,
      output_dir: task.output_dir || settings.value.output_dir || null,
      cookie_path: settings.value.cookie_path || null,
      quality: task.quality || null,
      format: task.format || 'mp4',
      vcodec: task.vcodec || settings.value.vcodec || 'auto',
      overwrite: true,
    },
  });
  await refreshList();
}

async function cancel(taskId: string) {
  const ok = await invoke<boolean>('cancel_task', { taskId });
  if (!ok) {
    ElMessage.warning('该任务当前不可取消');
  }
}

async function removeTask(taskId: string) {
  await invoke<boolean>('remove_task', { taskId });
  await refreshList();
}

async function clearFinished() {
  const count = await invoke<number>('clear_finished_tasks');
  if (count > 0) {
    ElMessage.success(`已清除 ${count} 条已结束任务`);
  } else {
    ElMessage.info('没有可清除的任务');
  }
  await refreshList();
}

async function openFile(taskId: string) {
  const ok = await invoke<boolean>('open_file', { taskId });
  if (!ok) {
    ElMessage.info('文件不存在或尚未下载完成');
  }
}

async function openOutputDir() {
  const ok = await invoke<boolean>('reveal_output_dir');
  if (!ok) {
    ElMessage.warning('打开输出目录失败');
  }
}

async function pickOutputDir() {
  const picked = await invoke<string>('pick_output_dir');
  if (picked) {
    settings.value.output_dir = picked;
    ElMessage.success('输出目录已更新');
  }
}

async function pickCookieFile() {
  const picked = await invoke<string>('pick_cookie_file');
  if (picked) {
    settings.value.cookie_path = picked;
    ElMessage.success('Cookie 文件已选择');
  }
}

function clearCookieFile() {
  settings.value.cookie_path = '';
}

async function saveSettings() {
  await invoke('set_settings', {
    settings: {
      output_dir: settings.value.output_dir,
      cookie_path: settings.value.cookie_path,
      retry_count: settings.value.retry_count,
      vcodec: settings.value.vcodec || 'auto',
    },
  });
  ElMessage.success('设置已保存');
  openSettings.value = false;
}

onMounted(async () => {
  await refreshList();
  await refreshSettings();

  await listen<ProgressEvent>('task_progress', (event) => {
    const payload = event.payload;
    const task = tasks.value.find((item) => item.id === payload.task_id);
    if (task) {
      task.status = payload.status;
      task.progress = payload.percent;
      task.speed = payload.speed;
      task.eta = payload.eta;
      task.updated_at = new Date().toISOString();
    }
    pushLog({
      task_id: payload.task_id,
      level: 'info',
      message: trimLog(payload.message || `下载中 ${payload.percent}%`),
    });
  });

  await listen<LogLine>('task_log', (event) => {
    pushLog(event.payload);
  });

  await listen<ResultEvent>('task_done', async (event) => {
    await refreshList();
    const { success, error, task_id, raw } = event.payload;
    if (success) {
      ElMessage.success(`任务 ${task_id} 已完成`);
    } else {
      const rawSummary = trimLog(raw || '', 180);
      const socketHint =
        raw && /WinError\s*10013/i.test(raw)
          ? '（提示：可能被系统网络策略、防火墙或代理冲突拦截）'
          : '';
      ElMessage.error(`任务 ${task_id} 失败：${error || '未知错误'}${socketHint}${rawSummary ? ` | ${rawSummary}` : ''}`);
    }
  });
});
</script>

<style scoped lang="scss">
.panel :deep(.el-card__body) {
  padding: 0;
}

.create-form {
  padding: 0 16px 16px;
}

.form-grid {
  display: grid;
  grid-template-columns: 1.6fr 0.8fr repeat(auto-fit, minmax(140px, 0.7fr));
  gap: 12px;
}

.form-actions {
  display: flex;
  gap: 10px;
  margin-top: 6px;
}

.stats {
  margin: 16px;
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
}

.stat {
  border: 1px solid rgba(121, 104, 81, 0.2);
  border-radius: 12px;
  padding: 14px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.82), rgba(255, 248, 231, 0.82));
}

.stat h3 {
  margin: 0;
  font-size: 0.86rem;
  color: #7b6251;
  font-weight: 600;
}

.stat p {
  margin: 8px 0 0;
  font-size: 1.14rem;
  font-weight: 800;
  color: #2f241a;
}

.logs {
  max-height: 280px;
  overflow: auto;
  border-top: 1px solid #ebddc3;
  padding: 10px;
}

.log-line {
  display: grid;
  grid-template-columns: 92px 150px 1fr;
  gap: 8px;
  align-items: center;
  font-size: 12px;
  color: #2f2620;
  border-bottom: 1px dashed #e4d6bb;
  padding: 6px 0;
}

.log-task {
  color: #5d4a38;
  word-break: break-all;
}

.log-level {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 74px;
  padding: 2px 6px;
  border-radius: 999px;
  border: 1px solid;
  font-weight: 700;
  font-size: 11px;
}

.logs-empty {
  color: #6f6357;
  margin: 10px;
}

.setting-tip {
  margin: 0;
  font-size: 12px;
  color: #6f6454;
}

.format-hint {
  margin: 4px 0 0;
  font-size: 12px;
  color: #b45309;
}

.settings {
  padding: 0 10px 10px;
}

.settings__row {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 8px;
}

.settings__row--split {
  grid-template-columns: 1fr auto auto;
}

.btn-glow {
  box-shadow: 0 10px 24px rgba(108, 84, 43, 0.22);
}

.panel__subtitle {
  margin: 2px 0 0;
  color: #7b6650;
}

.footer {
  max-width: 1160px;
  margin: 12px auto 0;
  color: #6f5f4c;
  font-size: 12px;
  text-align: center;
}

@media (max-width: 1024px) {
  .form-grid,
  .stats {
    grid-template-columns: 1fr 1fr;
  }
}

@media (max-width: 768px) {
  .form-grid,
  .stats {
    grid-template-columns: 1fr;
  }

  .form-actions {
    flex-wrap: wrap;
  }
}
</style>
