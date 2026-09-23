/* A browser-only sample workspace. No Lore CLI, filesystem, or network calls. */
(() => {
  const root = document.querySelector('#demo-root');
  if (!root) return;

  const files = [
    { path: 'README.md', icon: 'description', before: ['# LoreLens', '', 'A desktop client for Lore VCS.'], after: ['# LoreLens', '', 'A fast desktop client for Lore VCS.', '', 'Browse, review, and commit from one workspace.'] },
    { path: 'src/main.rs', icon: 'code', before: ['fn main() {', '  println!("Hello, Lore!");', '}'], after: ['fn main() {', '  let workspace = lore::open("sample-project");', '  lorelens::run(workspace);', '}'] },
    { path: 'src/ui/theme.rs', icon: 'palette', before: ['pub const ACCENT: &str = "#3279f9";', 'pub const RADIUS: u8 = 8;'], after: ['pub const ACCENT: &str = "#3279f9";', 'pub const RADIUS: u8 = 10;', 'pub const FOLLOW_SYSTEM: bool = true;'] },
    { path: 'assets/mark.svg', icon: 'image', after: ['<svg viewBox="0 0 32 32">', '  <path d="M16 2 30 16 16 30 2 16Z" />', '</svg>'] },
    { path: 'Cargo.toml', icon: 'settings', after: ['[package]', 'name = "lorelens"', 'edition = "2024"'] }
  ];
  const byPath = Object.fromEntries(files.map(file => [file.path, file]));
  const seedHistory = [
    { revision: 'r148', message: 'Update workspace overview', author: 'law80', date: 'Sep 16', paths: ['README.md'] },
    { revision: 'r147', message: 'Add file preview panel', author: 'law80', date: 'Sep 15', paths: ['src/main.rs'] },
    { revision: 'r146', message: 'Refine theme colors', author: 'law80', date: 'Sep 14', paths: ['src/ui/theme.rs'] }
  ];
  const dictionaries = {
    ko: {
      'Interactive demo': '인터랙티브 데모', 'Sample data only': '샘플 데이터 전용', 'Reset demo': '데모 초기화',
      'Repository': '저장소', 'Changes': '변경사항', 'History': '기록', 'File preview': '파일 미리보기', 'Unpushed commits': '푸시 대기 커밋',
      'Refresh': '새로고침', 'Sync': '동기화', 'Push': '푸시', 'Files': '파일', 'Filter files…': '파일 검색…', 'Filter changes…': '변경사항 검색…',
      'All': '전체', 'Staged': '스테이지됨', 'Unstaged': '스테이지 안 됨', 'Stage all': '모두 스테이지', 'Unstage all': '모두 해제',
      'Stage': '스테이지', 'Unstage': '해제', 'changed files': '개 파일 변경', 'files staged': '개 파일 스테이지됨',
      'No files match this search.': '검색 결과가 없습니다.', 'No changes in this view.': '이 보기에는 변경사항이 없습니다.',
      'Review a changed file to see its diff.': '변경된 파일을 선택해 차이를 확인하세요.', 'Diff preview': '차이 미리보기',
      'Write a commit message…': '커밋 메시지 입력…', 'Commit staged': '스테이지된 파일 커밋', 'Select a file to preview it.': '미리 볼 파일을 선택하세요.',
      'Local commits waiting to be pushed': '푸시 대기 중인 로컬 커밋', 'No local commits waiting to be pushed.': '푸시 대기 중인 로컬 커밋이 없습니다.',
      'Commit details': '커밋 세부 정보', 'Select a revision to inspect it.': '리비전을 선택해 확인하세요.',
      'Sample workspace ready. Changes stay in this browser session.': '샘플 작업 공간이 준비되었습니다. 변경사항은 이 브라우저 세션에만 남습니다.',
      'Demo reset.': '데모를 초기화했습니다.', 'Workspace is up to date.': '작업 공간이 최신 상태입니다.',
      'Sample status refreshed.': '샘플 상태를 새로고침했습니다.', 'No local commits to push.': '푸시할 로컬 커밋이 없습니다.',
      'Local commits pushed in the demo.': '데모에서 로컬 커밋을 푸시했습니다.', 'Commit created locally.': '로컬 커밋을 만들었습니다.',
      'Working tree clean': '작업 공간에 변경사항 없음', 'Modified': '수정됨', 'Branch': '브랜치', 'Account loaded': '계정 연결됨',
      'Preview follows the selected file.': '선택한 파일을 미리 봅니다.'
    },
    zh: {
      'Interactive demo': '交互式演示', 'Sample data only': '仅使用示例数据', 'Reset demo': '重置演示',
      'Repository': '仓库', 'Changes': '更改', 'History': '历史', 'File preview': '文件预览', 'Unpushed commits': '未推送提交',
      'Refresh': '刷新', 'Sync': '同步', 'Push': '推送', 'Files': '文件', 'Filter files…': '筛选文件…', 'Filter changes…': '筛选更改…',
      'All': '全部', 'Staged': '已暂存', 'Unstaged': '未暂存', 'Stage all': '全部暂存', 'Unstage all': '全部取消暂存',
      'Stage': '暂存', 'Unstage': '取消暂存', 'changed files': '个文件已更改', 'files staged': '个文件已暂存',
      'No files match this search.': '没有匹配的文件。', 'No changes in this view.': '此视图中没有更改。',
      'Review a changed file to see its diff.': '选择已更改的文件查看差异。', 'Diff preview': '差异预览',
      'Write a commit message…': '输入提交信息…', 'Commit staged': '提交已暂存文件', 'Select a file to preview it.': '选择文件进行预览。',
      'Local commits waiting to be pushed': '等待推送的本地提交', 'No local commits waiting to be pushed.': '没有等待推送的本地提交。',
      'Commit details': '提交详情', 'Select a revision to inspect it.': '选择修订版本查看详情。',
      'Sample workspace ready. Changes stay in this browser session.': '示例工作区已就绪。更改只保留在当前浏览器会话中。',
      'Demo reset.': '演示已重置。', 'Workspace is up to date.': '工作区已是最新状态。',
      'Sample status refreshed.': '示例状态已刷新。', 'No local commits to push.': '没有可推送的本地提交。',
      'Local commits pushed in the demo.': '已在演示中推送本地提交。', 'Commit created locally.': '已创建本地提交。',
      'Working tree clean': '工作区没有更改', 'Modified': '已修改', 'Branch': '分支', 'Account loaded': '账户已连接',
      'Preview follows the selected file.': '预览所选文件。'
    }
  };
  const escapeHtml = value => String(value).replace(/[&<>"']/g, character => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character]);
  const icon = name => `<span class="symbol" aria-hidden="true">${name}</span>`;
  const locale = () => document.documentElement.lang.slice(0, 2);
  const t = key => dictionaries[locale()]?.[key] ?? key;
  const label = key => escapeHtml(t(key));

  function initialBranches() {
    return {
      main: { changes: { 'README.md': false, 'src/main.rs': false, 'src/ui/theme.rs': false }, commits: [], history: structuredClone(seedHistory) },
      'feature/desktop-ui': { changes: { 'README.md': false, 'src/ui/theme.rs': false }, commits: [], history: structuredClone(seedHistory) }
    };
  }
  let branches = initialBranches();
  const state = { branch: 'main', tab: 'changes', filter: 'all', fileFilter: '', changeFilter: '', selected: 'src/main.rs', revision: 'r148', message: '', notice: 'Sample workspace ready. Changes stay in this browser session.' };
  const current = () => branches[state.branch];
  const changedPaths = () => Object.keys(current().changes);
  const stagedPaths = () => changedPaths().filter(path => current().changes[path]);
  const localizedNotice = () => label(state.notice);

  function fileRows() {
    const matches = files.filter(file => file.path.toLowerCase().includes(state.fileFilter.toLowerCase()));
    if (!matches.length) return `<p class="demo-empty">${label('No files match this search.')}</p>`;
    return matches.map(file => `<button type="button" class="demo-file ${state.selected === file.path ? 'is-selected' : ''}" data-action="select-file" data-path="${escapeHtml(file.path)}" aria-current="${state.selected === file.path ? 'true' : 'false'}">${icon(file.icon)}<span>${escapeHtml(file.path)}</span>${file.path in current().changes ? '<i class="demo-modified" aria-hidden="true"></i>' : ''}</button>`).join('');
  }

  function changeRows() {
    const matches = changedPaths().filter(path => path.toLowerCase().includes(state.changeFilter.toLowerCase()) && (state.filter === 'all' || (state.filter === 'staged') === current().changes[path]));
    if (!matches.length) return `<p class="demo-empty">${label('No changes in this view.')}</p>`;
    return matches.map(path => `<div class="demo-change ${state.selected === path ? 'is-selected' : ''}"><button type="button" class="demo-stage" data-action="toggle-stage" data-path="${escapeHtml(path)}" aria-label="${label(current().changes[path] ? 'Unstage' : 'Stage')} ${escapeHtml(path)}" aria-pressed="${current().changes[path]}">${current().changes[path] ? icon('check_box') : icon('check_box_outline_blank')}</button><button type="button" class="demo-change-file" data-action="select-change" data-path="${escapeHtml(path)}">${icon(byPath[path].icon)}<span>${escapeHtml(path)}</span></button><span class="demo-state">${label(current().changes[path] ? 'Staged' : 'Modified')}</span></div>`).join('');
  }

  function diff(path) {
    const file = byPath[path];
    if (!file?.before || !(path in current().changes)) return `<p class="demo-empty">${label('Review a changed file to see its diff.')}</p>`;
    const before = file.before.map(line => `<span class="demo-line removed">− ${escapeHtml(line)}</span>`).join('');
    const after = file.after.map(line => `<span class="demo-line added">+ ${escapeHtml(line)}</span>`).join('');
    return `<div class="demo-diff-head">${icon('difference')}<span>${escapeHtml(path)}</span><small>${label('Diff preview')}</small></div><pre class="demo-code">${before}${after}</pre>`;
  }

  function changesView() {
    const count = changedPaths().length;
    return `<div class="demo-changes-toolbar"><input class="demo-input" data-role="change-filter" type="search" value="${escapeHtml(state.changeFilter)}" placeholder="${label('Filter changes…')}" aria-label="${label('Filter changes…')}"><div class="demo-filter" role="group" aria-label="${label('Changes')}">${['all', 'staged', 'unstaged'].map(filter => `<button type="button" data-action="filter" data-filter="${filter}" aria-pressed="${state.filter === filter}">${label(filter[0].toUpperCase() + filter.slice(1))}</button>`).join('')}</div></div><div class="demo-change-summary"><span>${count ? `${count} ${label('changed files')}` : label('Working tree clean')}</span><button type="button" data-action="stage-all" ${count ? '' : 'disabled'}>${label(stagedPaths().length === count && count ? 'Unstage all' : 'Stage all')}</button></div><div class="demo-change-list" data-role="change-list">${changeRows()}</div><div class="demo-diff">${diff(state.selected)}</div><div class="demo-commit"><span>${icon('account_tree')} ${stagedPaths().length} ${label('files staged')}</span><input class="demo-input" data-role="commit-message" type="text" maxlength="100" value="${escapeHtml(state.message)}" placeholder="${label('Write a commit message…')}" aria-label="${label('Write a commit message…')}"><button type="button" data-action="commit" ${stagedPaths().length && state.message.trim() ? '' : 'disabled'}>${label('Commit staged')}</button></div>`;
  }

  function previewView() {
    const file = byPath[state.selected];
    if (!file) return `<p class="demo-empty">${label('Select a file to preview it.')}</p>`;
    return `<div class="demo-view-heading">${icon(file.icon)}<strong>${escapeHtml(file.path)}</strong><span>${label('File preview')}</span></div><pre class="demo-code demo-preview-code">${file.after.map((line, index) => `<span class="demo-line"><i>${String(index + 1).padStart(2, '0')}</i>${escapeHtml(line)}</span>`).join('')}</pre><p class="demo-hint">${label('Preview follows the selected file.')}</p>`;
  }

  function historyRows(rows) {
    return rows.map(entry => `<button type="button" class="demo-history-row ${state.revision === entry.revision ? 'is-selected' : ''}" data-action="select-revision" data-revision="${escapeHtml(entry.revision)}"><span>${escapeHtml(entry.revision)}</span><strong>${escapeHtml(entry.message)}</strong><span>${escapeHtml(entry.author)}</span><span>${escapeHtml(entry.date)}</span></button>`).join('');
  }

  function historyView() {
    const entry = current().history.find(item => item.revision === state.revision);
    return `<div class="demo-view-heading">${icon('history')}<strong>${label('History')}</strong><span>${escapeHtml(state.branch)}</span></div><div class="demo-history-list">${historyRows(current().history)}</div><div class="demo-history-detail"><strong>${label('Commit details')}</strong>${entry ? `<p>${escapeHtml(entry.revision)} · ${escapeHtml(entry.message)}</p><small>${escapeHtml(entry.paths.join(', '))}</small>` : `<p>${label('Select a revision to inspect it.')}</p>`}</div>`;
  }

  function unpushedView() {
    const entry = current().commits.find(item => item.revision === state.revision);
    return `<div class="demo-view-heading">${icon('upload')}<strong>${label('Local commits waiting to be pushed')}</strong><span>${current().commits.length}</span></div>${current().commits.length ? `<div class="demo-history-list">${historyRows(current().commits)}</div>${entry ? `<div class="demo-history-detail"><strong>${label('Commit details')}</strong><p>${escapeHtml(entry.message)}</p><small>${escapeHtml(entry.paths.join(', '))}</small></div>` : ''}` : `<p class="demo-empty demo-large-empty">${label('No local commits waiting to be pushed.')}</p>`}`;
  }

  function render() {
    const tabViews = { changes: changesView, history: historyView, preview: previewView, unpushed: unpushedView };
    root.innerHTML = `<div class="demo-app"><div class="demo-titlebar"><span class="demo-brand"><img src="favicon.svg" alt="" width="17" height="17"> LoreLens</span><span class="demo-title">${label('Interactive demo')} <b>·</b> ${label('Sample data only')}</span><button type="button" class="demo-reset" data-action="reset">${icon('restart_alt')} ${label('Reset demo')}</button></div><div class="demo-toolbar"><span class="demo-repository">${icon('database')} sample-project</span><label class="demo-branch">${icon('account_tree')}<span class="sr-only">${label('Branch')}</span><select data-role="branch" aria-label="${label('Branch')}"><option value="main" ${state.branch === 'main' ? 'selected' : ''}>main</option><option value="feature/desktop-ui" ${state.branch === 'feature/desktop-ui' ? 'selected' : ''}>feature/desktop-ui</option></select></label><button type="button" data-action="refresh">${icon('refresh')} ${label('Refresh')}</button><button type="button" data-action="sync">${icon('sync')} ${label('Sync')}</button><button type="button" data-action="push" ${current().commits.length ? '' : 'disabled'}>${icon('north')} ${label('Push')} <em>${current().commits.length}</em></button><span class="demo-revision">${escapeHtml(current().history[0]?.revision ?? 'r148')}</span></div><div class="demo-body"><aside class="demo-sidebar"><div class="demo-panel-title">${label('Files')} <span>${files.length}</span></div><input class="demo-input" data-role="file-filter" type="search" value="${escapeHtml(state.fileFilter)}" placeholder="${label('Filter files…')}" aria-label="${label('Filter files…')}"><div class="demo-file-list" data-role="file-list">${fileRows()}</div></aside><section class="demo-workspace" aria-label="${label('Interactive demo')}"><div class="demo-tabs" role="tablist" aria-label="${label('Repository')}">${[['changes', 'Changes'], ['history', 'History'], ['preview', 'File preview'], ['unpushed', 'Unpushed commits']].map(([tab, text]) => `<button type="button" role="tab" data-action="tab" data-tab="${tab}" aria-selected="${state.tab === tab}">${label(text)}${tab === 'unpushed' && current().commits.length ? `<span>${current().commits.length}</span>` : ''}</button>`).join('')}</div><div class="demo-tab-panel" role="tabpanel">${tabViews[state.tab]()}</div></section></div><div class="demo-bottom"><span class="demo-connection"><i></i>${label('Account loaded')}</span><span role="status" aria-live="polite">${localizedNotice()}</span><span>Rust + GPUI · LoreLens</span></div></div>`;
  }

  root.addEventListener('click', event => {
    const button = event.target.closest('[data-action]');
    if (!button || !root.contains(button)) return;
    const { action, path } = button.dataset;
    if (action === 'reset') {
      branches = initialBranches();
      Object.assign(state, { branch: 'main', tab: 'changes', filter: 'all', fileFilter: '', changeFilter: '', selected: 'src/main.rs', revision: 'r148', message: '', notice: 'Demo reset.' });
    } else if (action === 'select-file') { state.selected = path; state.tab = 'preview'; }
    else if (action === 'select-change') state.selected = path;
    else if (action === 'toggle-stage') current().changes[path] = !current().changes[path];
    else if (action === 'stage-all') {
      const stage = stagedPaths().length !== changedPaths().length;
      for (const changed of changedPaths()) current().changes[changed] = stage;
    } else if (action === 'filter') state.filter = button.dataset.filter;
    else if (action === 'tab') state.tab = button.dataset.tab;
    else if (action === 'select-revision') state.revision = button.dataset.revision;
    else if (action === 'refresh') state.notice = 'Sample status refreshed.';
    else if (action === 'sync') state.notice = 'Workspace is up to date.';
    else if (action === 'push') {
      state.notice = current().commits.length ? 'Local commits pushed in the demo.' : 'No local commits to push.';
      current().commits = [];
    } else if (action === 'commit' && stagedPaths().length && state.message.trim()) {
      const branch = current();
      const paths = stagedPaths();
      const entry = { revision: `r${149 + branch.history.length - seedHistory.length}`, message: state.message.trim(), author: 'you', date: 'Now', paths };
      branch.history.unshift(entry);
      branch.commits.unshift(entry);
      for (const staged of paths) delete branch.changes[staged];
      state.selected = Object.keys(branch.changes)[0] ?? state.selected;
      state.message = '';
      state.revision = entry.revision;
      state.notice = 'Commit created locally.';
    } else return;
    render();
  });

  root.addEventListener('change', event => {
    if (event.target.dataset.role !== 'branch') return;
    state.branch = event.target.value;
    state.selected = changedPaths()[0] ?? 'README.md';
    state.revision = current().history[0]?.revision;
    state.filter = 'all';
    state.message = '';
    render();
  });

  root.addEventListener('input', event => {
    if (event.target.dataset.role === 'file-filter') {
      state.fileFilter = event.target.value;
      root.querySelector('[data-role="file-list"]').innerHTML = fileRows();
    } else if (event.target.dataset.role === 'change-filter') {
      state.changeFilter = event.target.value;
      root.querySelector('[data-role="change-list"]').innerHTML = changeRows();
    } else if (event.target.dataset.role === 'commit-message') {
      state.message = event.target.value;
      root.querySelector('[data-action="commit"]').disabled = !state.message.trim() || !stagedPaths().length;
    }
  });

  document.querySelectorAll('[data-language]').forEach(select => select.addEventListener('change', render));
  render();
})();
