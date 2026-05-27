/**
 * Dev Tools - 前端主逻辑
 * 通过 window.__TAURI__.invoke() 调用 Rust 后端命令
 */

const { invoke } = window.__TAURI__?.core ?? {};

// ─── 工具函数 ───────────────────────────────────────────────

function $(sel) { return document.querySelector(sel); }
function $$(sel) { return document.querySelectorAll(sel); }

function setStatus(msg) {
  $('#status-text').textContent = msg;
}

function formatSize(bytes) {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(1) + ' ' + units[i];
}

function formatTime() {
  return new Date().toLocaleTimeString('zh-CN');
}

// 时钟更新
setInterval(() => {
  $('#status-time').textContent = formatTime();
}, 1000);
$('#status-time').textContent = formatTime();

// ─── Tab 切换 ───────────────────────────────────────────────

$$('.tab-btn').forEach(btn => {
  btn.addEventListener('click', () => {
    $$('.tab-btn').forEach(b => b.classList.remove('active'));
    $$('.tab-content').forEach(c => c.classList.remove('active'));
    btn.classList.add('active');
    $(`#${btn.dataset.tab}`).classList.add('active');
  });
});

// ─── 仪表盘 - 系统信息 ────────────────────────────────────

async function refreshSystemInfo() {
  try {
    setStatus('正在获取系统信息...');
    const info = await invoke('get_system_info');
    $('#info-os').textContent = info.os;
    $('#info-hostname').textContent = info.hostname;
    $('#info-cpu').textContent = `${info.cpu_cores} 核`;
    $('#info-memory').textContent = formatSize(info.memory_total_mb * 1024 * 1024);
    $('#info-rust').textContent = info.rust_version;
    $('#info-tauri').textContent = info.tauri_version;
    setStatus('系统信息已加载 ✅');
  } catch (err) {
    setStatus(`❌ 获取失败: ${err}`);
  }
}

// 页面加载后自动刷新
refreshSystemInfo();

// ─── 笔记系统 ─────────────────────────────────────────────

let currentNoteId = null;
let allNotes = [];

async function loadNotes() {
  try {
    allNotes = await invoke('load_notes');
    renderNotesList(allNotes);
    setStatus(`已加载 ${allNotes.length} 条笔记`);
  } catch (err) {
    setStatus(`❌ 加载笔记失败: ${err}`);
  }
}

function renderNotesList(notes) {
  const container = $('#notes-list');
  if (notes.length === 0) {
    container.innerHTML = `<div class="empty-state">暂无笔记，点击「新建」创建</div>`;
    return;
  }

  container.innerHTML = notes.map(note => `
    <div class="note-item ${note.id === currentNoteId ? 'active' : ''}"
         onclick="selectNote('${note.id}')">
      <div class="note-item-title">${escapeHtml(note.title) || '无标题'}</div>
      <div class="note-item-preview">${escapeHtml(note.content.substring(0, 60)) || '空内容'}</div>
      <div class="note-item-time">${note.updated_at}</div>
    </div>
  `).join('');
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function filterNotes() {
  const query = $('#note-search').value.toLowerCase();
  if (!query) {
    renderNotesList(allNotes);
    return;
  }
  const filtered = allNotes.filter(n =>
    n.title.toLowerCase().includes(query) ||
    n.content.toLowerCase().includes(query)
  );
  renderNotesList(filtered);
}

function selectNote(id) {
  currentNoteId = id;
  const note = allNotes.find(n => n.id === id);
  if (!note) return;

  $('#note-id').value = note.id;
  $('#note-title').value = note.title;
  $('#note-content').value = note.content;
  $('#note-editor-empty').classList.add('hidden');
  $('#note-editor-form').classList.remove('hidden');
  renderNotesList(allNotes);
}

function showNewNoteForm() {
  currentNoteId = null;
  $('#note-id').value = '';
  $('#note-title').value = '';
  $('#note-content').value = '';
  $('#note-editor-empty').classList.add('hidden');
  $('#note-editor-form').classList.remove('hidden');
  $('#note-title').focus();
  renderNotesList(allNotes);
}

async function saveNote() {
  const title = $('#note-title').value.trim() || '未命名笔记';
  const content = $('#note-content').value;
  const id = $('#note-id').value;

  try {
    if (id) {
      // 更新
      await invoke('update_note', { id, title, content });
      setStatus('✅ 笔记已更新');
    } else {
      // 新建
      await invoke('create_note', { title, content });
      setStatus('✅ 笔记已创建');
    }
    await loadNotes();
    // 保持选中最后一篇
    if (!id) {
      const last = allNotes[0];
      if (last) selectNote(last.id);
    }
  } catch (err) {
    setStatus(`❌ 保存失败: ${err}`);
  }
}

async function deleteCurrentNote() {
  const id = $('#note-id').value;
  if (!id) return;
  if (!confirm('确定删除这篇笔记吗？')) return;

  try {
    await invoke('delete_note', { id });
    currentNoteId = null;
    $('#note-editor-form').classList.add('hidden');
    $('#note-editor-empty').classList.remove('hidden');
    await loadNotes();
    setStatus('🗑️ 笔记已删除');
  } catch (err) {
    setStatus(`❌ 删除失败: ${err}`);
  }
}

// 自动加载笔记
loadNotes();

// ─── 文件浏览 ─────────────────────────────────────────────

let currentDir = '/';

function getDefaultPath() {
  // 根据操作系统返回默认路径
  const os = ($('#info-os').textContent || '').toLowerCase();
  if (os.includes('mac')) return '/Users';
  if (os.includes('windows')) return 'C:\\';
  return '/home';
}

async function browseDirectory(dir) {
  const path = dir || $('#file-path').value || '/';
  currentDir = path;
  $('#file-path').value = path;

  try {
    setStatus(`📂 正在浏览: ${path}`);
    const entries = await invoke('list_directory', { path });

    const tbody = $('#file-list');
    if (entries.length === 0) {
      tbody.innerHTML = '<tr><td colspan="3" class="loading">空目录</td></tr>';
      setStatus(`📂 ${path} (空目录)`);
      return;
    }

    tbody.innerHTML = entries.map(entry => `
      <tr class="${entry.is_dir ? 'dir-row' : 'file-row'}"
          onclick="${entry.is_dir
            ? `browseDirectory('${escapePath(entry.path)}')`
            : `previewFile('${escapePath(entry.path)}')`
          }">
        <td>${entry.is_dir ? '📁 ' : '📄 '} ${escapeHtml(entry.name)}</td>
        <td>${entry.is_dir ? '-' : formatSize(entry.size)}</td>
        <td>${entry.modified}</td>
      </tr>
    `).join('');

    setStatus(`📂 ${path} (${entries.length} 项)`);
  } catch (err) {
    setStatus(`❌ 浏览失败: ${err}`);
    $('#file-list').innerHTML = `<tr><td colspan="3" class="loading">❌ ${err}</td></tr>`;
  }
}

function escapePath(path) {
  return path.replace(/\\/g, '\\\\').replace(/'/g, "\\'");
}

async function previewFile(path) {
  try {
    setStatus(`📄 正在读取: ${path}`);
    const content = await invoke('read_text_file', { path });
    $('#file-preview .empty-state').classList.add('hidden');
    const pre = $('#file-content');
    pre.classList.remove('hidden');
    pre.textContent = content;
    setStatus(`📄 ${path} (${content.length} 字符)`);
  } catch (err) {
    setStatus(`❌ 读取失败: ${err}`);
  }
}

// 初始浏览首页
setTimeout(() => {
  browseDirectory(getDefaultPath());
}, 500);
