// i18n dictionary for the whole app.
// Welcome page, workspace, settings, history, checkpoint dialog.
// Default locale is English per product spec.

export type Locale = "zh" | "en";

const LOCALE_KEY = "route:locale";

export function getInitialLocale(): Locale {
  if (typeof localStorage !== "undefined") {
    const saved = localStorage.getItem(LOCALE_KEY) as Locale | null;
    if (saved === "zh" || saved === "en") return saved;
  }
  return "en";
}

export function persistLocale(locale: Locale) {
  try {
    localStorage.setItem(LOCALE_KEY, locale);
  } catch {
    // ignore — localStorage may be disabled
  }
}

type DictEntry = {
  addProject: string;
  advancedSection: string;
  aiActiveMode: string;
  aiActiveModeHint: string;
  aiApiEndpointLabel: string;
  aiApiEndpointPlaceholder: string;
  aiApiKeyHint: string;
  aiApiKeyLabel: string;
  aiApiKeyPlaceholder: string;
  aiAssistantHint: string;
  aiAssistantSection: string;
  aiSection: string;
  versionControlSection: string;
  aiConflictEmpty: string;
  aiConflictHint: string;
  aiConflictKeepAi: string;
  aiConflictKeepBoth: string;
  aiConflictKeepOld: string;
  aiConflictNote: string;
  aiConflictSave: string;
  aiConflictSkip: string;
  aiConflictTitle: string;
  aiControlActive: string;
  aiControlLabel: string;
  aiFollowExisting: string;
  aiFollowExistingHint: string;
  aiGenerateHint: string;
  aiGenerateMark: string;
  aiGenerateNoChanges: string;
  aiGenerateNotConfigured: string;
  aiGeneratingMark: string;
  aiIndexCopy: string;
  aiIndexPathHint: string;
  aiIndexPathLabel: string;
  aiIndexRefresh: string;
  aiIndexSection: string;
  aiIndexSectionHint: string;
  aiModelLabel: string;
  aiModelPlaceholder: string;
  aiPromptHint: string;
  aiProviderAnthropic: string;
  aiProviderAnthropicHint: string;
  aiProviderLabel: string;
  aiProviderOllama: string;
  aiProviderOllamaHint: string;
  aiProviderOpenai: string;
  aiProviderOpenaiHint: string;
  aiTestConnection: string;
  aiTesting: string;
  aiTestOk: string;
  appearanceSection: string;
  autostartEnable: string;
  autostartEnableHint: string;
  autostartPriority: string;
  autostartPriorityHigh: string;
  autostartPriorityLow: string;
  autostartPriorityNormal: string;
  autostartSection: string;
  autostartSilent: string;
  autostartSilentHint: string;
  backupDone: string;
  backupFail: string;
  basicSection: string;
  betaBadge: string;
  branchEmpty: string;
  branchKindInherited: string;
  branchKindMain: string;
  branchKindSandbox: string;
  branchName: string;
  branchTreeTitle: string;
  brand: string;
  cancelBranch: string;
  clearData: string;
  clearDataDone: string;
  clearDataHint: string;
  clearDataHold: string;
  clearDataIrreversible: string;
  clickToCopy: string;
  cliMcpBinaryPath: string;
  cliMcpBuildHint: string;
  cliMcpConfigCopied: string;
  cliMcpConfigSnippet: string;
  cliMcpCopyConfig: string;
  cliMcpEnable: string;
  cliMcpEnableHint: string;
  cliMcpNoProject: string;
  cliMcpOff: string;
  cliMcpOn: string;
  cliMcpStartHint: string;
  close: string;
  closeBehaviorHide: string;
  closeBehaviorHint: string;
  closeBehaviorLabel: string;
  closeBehaviorQuit: string;
  copyPath: string;
  copyToNewBranch: string;
  createBranch: string;
  cta: string;
  currentBranchLabel: string;
  dangerZone: string;
  dataExportHint: string;
  dataExportLabel: string;
  dataImportExport: string;
  dataImportExportHint: string;
  dataLocationHint: string;
  dataLocationLabel: string;
  dataSection: string;
  diffShowFiles: string;
  errorTauriBridge: string;
  filesAdded: string;
  filesModified: string;
  filesRemoved: string;
  filterAi: string;
  filterAll: string;
  filterMarker: string;
  filterUser: string;
  gitModeAside: string;
  gitModeAvailable: string;
  gitModeDetect: string;
  gitModeDetectLabel: string;
  gitModeHint: string;
  gitModeLabel: string;
  gitModeNoPushHint: string;
  gitModeOff: string;
  gitModeOn: string;
  gitModeUnavailable: string;
  gitRestore: string;
  gitRestoreConfirm: string;
  gitRestored: string;
  gitRestoreNothing: string;
  gitStashNothing: string;
  gitStashPop: string;
  gitStashPopped: string;
  gitStashPush: string;
  gitStashStashed: string;
  gitTagAdd: string;
  gitTagDelete: string;
  gitTagPlaceholder: string;
  initBackupCloud: string;
  initBackupLocal: string;
  initIncremental: string;
  initIncrementalHint: string;
  initMirror: string;
  initMirrorDelay: string;
  initMirrorHint: string;
  initStandard: string;
  initStandardHint: string;
  integrationsSection: string;
  languageHint: string;
  languageLabel: string;
  manualBackup: string;
  mark: string;
  markBody: string;
  markCancel: string;
  markTitle: string;
  masterOff: string;
  masterOn: string;
  maximize: string;
  mergeBranch: string;
  minimize: string;
  modeAi: string;
  modeAiDev: string;
  modeAiHint: string;
  modeSection: string;
  modeSectionHint: string;
  modeStandard: string;
  modeStandardHint: string;
  newBranch: string;
  noMessage: string;
  noProjects: string;
  pageSettings: string;
  pageWorkspace: string;
  pathCopied: string;
  pickFolderHint: string;
  pickTarget: string;
  privacyP1: string;
  privacyP2: string;
  privacyP3: string;
  privacyTitle: string;
  remove: string;
  resetSettings: string;
  resetSettingsDone: string;
  resetSettingsHint: string;
  resetSettingsHold: string;
  restore: string;
  rollbackOne: string;
  routeaCheatsheetTitle: string;
  routeaColMarker: string;
  routeaColMeaning: string;
  routeaCommentNote: string;
  routeaLabel: string;
  routeaOff: string;
  routeaOn: string;
  routeaRow1Marker: string;
  routeaRow1Meaning: string;
  routeaRow2Marker: string;
  routeaRow2Meaning: string;
  routeaRow3Marker: string;
  routeaRow3Meaning: string;
  routeaSettingHint: string;
  searchPlaceholder: string;
  selectProjectFirst: string;
  settings: string;
  sortAlpha: string;
  sortCustom: string;
  sortReverse: string;
  sortTime: string;
  switchBranch: string;
  tagline: string;
  themeDark: string;
  themeLight: string;
  themeSection: string;
  themeSectionHint: string;
  timeline: string;
  timelineEmpty: string;
  trackAllHint: string;
  trackAllLabel: string;
  trackingOff: string;
  trackingRunning: string;
  trackingStarting: string;
  trackMemoryBufferHint: string;
  trackMemoryBufferLabel: string;
  trackPrefixesHint: string;
  trackPrefixesLabel: string;
  trackSection: string;
  trackSuffixesHint: string;
  trackSuffixesLabel: string;
  trackVerifySha256Hint: string;
  trackVerifySha256Label: string;
  uncommittedChanges: string;
  welcome: string;
  workbenchBackupMethod: string;
  workbenchBackupTarget: string;
  workbenchConfigTitle: string;
  workbenchConfirm: string;
  workbenchEdit: string;
  workbenchSourceLabel: string;
  workbenchTargetPath: string;
};

/// Convenience alias — components receive `tr: Dict` as a prop.
export type Dict = DictEntry;

export const dict: Record<Locale, DictEntry> = {
  zh: {
    addProject: "添加项目",
    advancedSection: "进阶",
    aiActiveMode: "主动 AI 模式",
    aiActiveModeHint: "接入独立 API，由专属 AI 操作软件",
    aiApiEndpointLabel: "接口地址",
    aiApiEndpointPlaceholder: "https://api.example.com/v1",
    aiApiKeyHint: "主动 AI 模式的服务密钥",
    aiApiKeyLabel: "API 密钥",
    aiApiKeyPlaceholder: "sk-...",
    aiAssistantHint: "配置 AI 如何辅助操作软件",
    aiAssistantSection: "AI 辅助模式",
    aiSection: "AI",
    versionControlSection: "版本控制",
    aiConflictEmpty: "最近没有 AI 提交，或 AI 没有标记任何冲突",
    aiConflictHint: "AI 修改了你的文件吗？这里可以选择保留旧逻辑、用 AI 新写的、或者两份都保留",
    aiConflictKeepAi: "用 AI 新写的",
    aiConflictKeepBoth: "两份都保留",
    aiConflictKeepOld: "保留旧逻辑",
    aiConflictNote: "备注（可选）",
    aiConflictSave: "保存决定",
    aiConflictSkip: "跳过",
    aiConflictTitle: "智能取舍",
    aiControlActive: "AI 正在控制",
    aiControlLabel: "外部控制",
    aiFollowExisting: "跟随原有设置",
    aiFollowExistingHint: "由 Web Coding 的 AI 通过现有接口自行操作软件",
    aiGenerateHint: "根据未提交变更自动生成标题",
    aiGenerateMark: "AI 生成",
    aiGenerateNoChanges: "没有未提交的变更",
    aiGenerateNotConfigured: "请先在设置中配置主动 AI 模式",
    aiGeneratingMark: "生成中...",
    aiIndexCopy: "复制索引路径",
    aiIndexPathHint: "路径提示：AI 通过此文件了解当前项目结构、追踪状态与配置",
    aiIndexPathLabel: "路径",
    aiIndexRefresh: "刷新索引",
    aiIndexSection: "AI 索引文件",
    aiIndexSectionHint: "每个项目根目录下都会写入 .route/index.json，供 AI 快速了解项目结构",
    aiModelLabel: "模型",
    aiModelPlaceholder: "gpt-4o / claude-3-5-sonnet / llama3.2",
    aiPromptHint: "AI 操控本软件时遵循的内置提示词。AI 会在每次提交的 body 中自动写入意图、文件、逻辑、冲突四段",
    aiProviderAnthropic: "Anthropic",
    aiProviderAnthropicHint: "Claude 系列模型",
    aiProviderLabel: "AI 提供商",
    aiProviderOllama: "Ollama",
    aiProviderOllamaHint: "本地大模型，无需 API 密钥",
    aiProviderOpenai: "OpenAI",
    aiProviderOpenaiHint: "OpenAI 及兼容服务（DeepSeek / Groq / vLLM 等）",
    aiTestConnection: "测试连接",
    aiTesting: "测试中...",
    aiTestOk: "连接成功",
    appearanceSection: "外观",
    autostartEnable: "开机时自动启动",
    autostartEnableHint: "系统登录后自动运行 Route",
    autostartPriority: "启动优先级",
    autostartPriorityHigh: "高",
    autostartPriorityLow: "低",
    autostartPriorityNormal: "标准",
    autostartSection: "开机自启动",
    autostartSilent: "静默启动",
    autostartSilentHint: "启动时隐藏窗口，在后台运行",
    backupDone: "备份完成",
    backupFail: "备份失败",
    basicSection: "基本",
    betaBadge: "Beta",
    branchEmpty: "暂无分支",
    branchKindInherited: "继承分支",
    branchKindMain: "主分支",
    branchKindSandbox: "沙盒分支",
    branchName: "分支名称",
    branchTreeTitle: "分支",
    brand: "route",
    cancelBranch: "取消",
    clearData: "清除全部数据",
    clearDataDone: "已清除",
    clearDataHint: "删除已记录的项目、历史与所有缓存。此操作不可撤销",
    clearDataHold: "长按 10 秒确认清除（不可撤销）",
    clearDataIrreversible: "此操作不可撤销。",
    clickToCopy: "点击复制路径",
    cliMcpBinaryPath: "二进制路径",
    cliMcpBuildHint: "若未找到二进制，请先运行 cargo build",
    cliMcpConfigCopied: "已复制",
    cliMcpConfigSnippet: "配置片段",
    cliMcpCopyConfig: "复制配置",
    cliMcpEnable: "启用 CLI / MCP 接口",
    cliMcpEnableHint: "启动后，外部 AI 可通过命令行或 MCP 协议调用 Route 的版本管理能力",
    cliMcpNoProject: "请先选择一个项目",
    cliMcpOff: "已停用",
    cliMcpOn: "已启用",
    cliMcpStartHint: "启动后请保持 Route 在前台或最小化运行",
    close: "关闭",
    closeBehaviorHide: "隐藏到后台",
    closeBehaviorHint: "选择关闭窗口时的行为",
    closeBehaviorLabel: "关闭窗口时",
    closeBehaviorQuit: "退出应用",
    copyPath: "复制路径",
    copyToNewBranch: "复制到新分支",
    createBranch: "创建",
    cta: "选择文件夹",
    currentBranchLabel: "当前分支",
    dangerZone: "危险操作",
    dataExportHint: "下载 JSON 格式的时间线记录",
    dataExportLabel: "导出时间线",
    dataImportExport: "时间线数据",
    dataImportExportHint: "导出时间线或查看本地数据位置",
    dataLocationHint: "项目数据存储在 .route 目录中",
    dataLocationLabel: "数据位置",
    dataSection: "数据",
    diffShowFiles: "查看文件变化",
    errorTauriBridge: "请在 Route 桌面应用中运行（需要 `cargo tauri dev` 或安装版）",
    filesAdded: "新增",
    filesModified: "修改",
    filesRemoved: "删除",
    filterAi: "AI",
    filterAll: "全部",
    filterMarker: "标记",
    filterUser: "用户",
    gitModeAside: "Beta · 不执行 push 或远程操作",
    gitModeAvailable: "Git 可用",
    gitModeDetect: "检测",
    gitModeDetectLabel: "检测 Git",
    gitModeHint: "使用本地 Git 作为版本控制后端",
    gitModeLabel: "Git 模式",
    gitModeNoPushHint: "Route 不会执行 git push 或连接远程仓库",
    gitModeOff: "关闭",
    gitModeOn: "开启",
    gitModeUnavailable: "未检测到 Git",
    gitRestore: "丢弃改动",
    gitRestoreConfirm: "确认丢弃？",
    gitRestored: "已丢弃改动",
    gitRestoreNothing: "没有可丢弃的改动",
    gitStashNothing: "没有可暂存的更改",
    gitStashPop: "应用暂存",
    gitStashPopped: "已应用暂存",
    gitStashPush: "暂存更改",
    gitStashStashed: "已暂存更改",
    gitTagAdd: "添加标签",
    gitTagDelete: "删除标签",
    gitTagPlaceholder: "标签名（如 v1.0）",
    initBackupCloud: "云端",
    initBackupLocal: "本地文件夹",
    initIncremental: "增量",
    initIncrementalHint: "只增不减，保留所有历史文件",
    initMirror: "镜像",
    initMirrorDelay: "镜像延迟（秒）",
    initMirrorHint: "完全复制到目标路径，保持两端一致",
    initStandard: "标准",
    initStandardHint: "版本管理，支持快照与回退",
    integrationsSection: "外部接入",
    languageHint: "立即生效，刷新时也保留",
    languageLabel: "语言",
    manualBackup: "手动全量备份",
    mark: "标记点",
    markBody: "正文",
    markCancel: "取消",
    markTitle: "标题",
    masterOff: "已关闭",
    masterOn: "运行中",
    maximize: "最大化",
    mergeBranch: "合并到当前",
    minimize: "最小化",
    modeAi: "AI 协同模式",
    modeAiDev: "开发中",
    modeAiHint: "让 AI 接管版本管理，配置 AI 后自动记录",
    modeSection: "模式",
    modeSectionHint: "选择此项目使用哪种工作模式",
    modeStandard: "标准模式",
    modeStandardHint: "本地自动追踪，可手动打点、回退、撤销回退",
    newBranch: "新建分支",
    noMessage: "(无消息)",
    noProjects: "暂无项目",
    pageSettings: "设置",
    pageWorkspace: "工作台",
    pathCopied: "已复制",
    pickFolderHint: "开始追踪你的项目文件",
    pickTarget: "选择",
    privacyP1: "route 标记模式开启后，Route 将会读取您所选文件中的注释内容，以识别标记并归类修改。",
    privacyP2: "软件为本地计算与运行：所有解析仅在您本机进行，文件内容不会上传到任何远程服务器。",
    privacyP3: "但配置数据部分将会记录您所选项目的部分文件元数据（例如路径、修改计数、文件类型），请注意保护隐私。",
    privacyTitle: "隐私与本地计算",
    remove: "移除",
    resetSettings: "重置设置",
    resetSettingsDone: "已重置",
    resetSettingsHint: "把语言、端点、备份等偏好恢复为默认值",
    resetSettingsHold: "长按 1 秒确认重置",
    restore: "还原",
    rollbackOne: "回退到此节点",
    routeaCheatsheetTitle: "route 标记语法",
    routeaColMarker: "标记",
    routeaColMeaning: "含义",
    routeaCommentNote: "支持 //、#、--、/* */ 等常见注释风格",
    routeaLabel: "route 标记",
    routeaOff: "未开启",
    routeaOn: "已开启",
    routeaRow1Marker: "// route X",
    routeaRow1Meaning: "此文件所有修改归属 X 分类",
    routeaRow2Marker: "// route start X ... // route over X",
    routeaRow2Meaning: "两行之间的修改归属 X 分类",
    routeaRow3Marker: "// route no start ... // route no",
    routeaRow3Meaning: "这段记录不会被追踪",
    routeaSettingHint: "开启后，软件将读取文档与代码内容以识别 route 注释",
    searchPlaceholder: "搜索记录",
    selectProjectFirst: "请先在左侧选择一个项目",
    settings: "设置",
    sortAlpha: "字母顺序",
    sortCustom: "自定义排序",
    sortReverse: "倒序",
    sortTime: "按时间",
    switchBranch: "切换",
    tagline: "From Route to Routine",
    themeDark: "深色",
    themeLight: "浅色",
    themeSection: "主题模式",
    themeSectionHint: "在浅色与深色之间切换",
    timeline: "时间线",
    timelineEmpty: "暂无修改记录",
    trackAllHint: "关闭后只追踪下方列出的后缀 / 前缀",
    trackAllLabel: "追踪所有文件",
    trackingOff: "未追踪",
    trackingRunning: "追踪中",
    trackingStarting: "启动中",
    trackMemoryBufferHint: "连续敲代码时延迟落盘的时长。设为 0 立即落盘",
    trackMemoryBufferLabel: "内存缓冲区（毫秒）",
    trackPrefixesHint: "可选；只追踪以这些路径前缀开头的文件",
    trackPrefixesLabel: "追踪的前缀",
    trackSection: "文件追踪",
    trackSuffixesHint: "逗号分隔，例如 .py,.js,.tsx,.css",
    trackSuffixesLabel: "追踪的后缀",
    trackVerifySha256Hint: "默认关闭。开启后每次提交都额外记录 SHA-256 哈希供外部系统校验",
    trackVerifySha256Label: "SHA-256 校验",
    uncommittedChanges: "未提交变更",
    welcome: "欢迎来到",
    workbenchBackupMethod: "备份方式",
    workbenchBackupTarget: "备份目标",
    workbenchConfigTitle: "配置",
    workbenchConfirm: "确认",
    workbenchEdit: "编辑",
    workbenchSourceLabel: "源",
    workbenchTargetPath: "目标路径",
  },

  en: {
    addProject: "Add project",
    advancedSection: "Advanced",
    aiActiveMode: "Active AI Mode",
    aiActiveModeHint: "Connect a separate API for a dedicated AI to operate the software",
    aiApiEndpointLabel: "Endpoint",
    aiApiEndpointPlaceholder: "https://api.example.com/v1",
    aiApiKeyHint: "Service key for the active AI mode",
    aiApiKeyLabel: "API Key",
    aiApiKeyPlaceholder: "sk-...",
    aiAssistantHint: "Configure how AI assists with the software",
    aiAssistantSection: "AI Assistant Mode",
    aiSection: "AI",
    versionControlSection: "Version Control",
    aiConflictEmpty: "No recent AI commit, or none of them flagged any conflict.",
    aiConflictHint: "Did the AI change your files? Pick the old logic, the AI's new version, or keep both.",
    aiConflictKeepAi: "Use AI's version",
    aiConflictKeepBoth: "Keep both",
    aiConflictKeepOld: "Keep old logic",
    aiConflictNote: "Note (optional)",
    aiConflictSave: "Save decision",
    aiConflictSkip: "Skip",
    aiConflictTitle: "Smart resolution",
    aiControlActive: "AI in control",
    aiControlLabel: "External control",
    aiFollowExisting: "Follow Existing Settings",
    aiFollowExistingHint: "Your Web Coding AI operates the software via existing interfaces",
    aiGenerateHint: "Auto-draft a title from uncommitted changes",
    aiGenerateMark: "AI generate",
    aiGenerateNoChanges: "No uncommitted changes",
    aiGenerateNotConfigured: "Configure Active AI mode in Settings first",
    aiGeneratingMark: "Generating...",
    aiIndexCopy: "Copy index path",
    aiIndexPathHint: "Path hint: AIs read this file to understand project structure, tracked files, and current settings.",
    aiIndexPathLabel: "Path",
    aiIndexRefresh: "Refresh index",
    aiIndexSection: "AI index file",
    aiIndexSectionHint: "A `.route/index.json` is written inside each project so AIs can discover the project structure quickly",
    aiModelLabel: "Model",
    aiModelPlaceholder: "gpt-4o / claude-3-5-sonnet / llama3.2",
    aiPromptHint: "The built-in prompt every AI agent must follow when driving the app. Each commit body must include INTENT, FILES, LOGIC, CONFLICTS.",
    aiProviderAnthropic: "Anthropic",
    aiProviderAnthropicHint: "Claude family models",
    aiProviderLabel: "Provider",
    aiProviderOllama: "Ollama",
    aiProviderOllamaHint: "Local LLMs, no API key needed",
    aiProviderOpenai: "OpenAI",
    aiProviderOpenaiHint: "OpenAI and compatible services (DeepSeek / Groq / vLLM etc.)",
    aiTestConnection: "Test connection",
    aiTesting: "Testing...",
    aiTestOk: "Connected",
    appearanceSection: "Appearance",
    autostartEnable: "Launch on boot",
    autostartEnableHint: "Run Route automatically after system login",
    autostartPriority: "Startup priority",
    autostartPriorityHigh: "High",
    autostartPriorityLow: "Low",
    autostartPriorityNormal: "Normal",
    autostartSection: "Auto-start",
    autostartSilent: "Silent start",
    autostartSilentHint: "Start hidden, run in the background",
    backupDone: "Backup complete",
    backupFail: "Backup failed",
    basicSection: "Basic",
    betaBadge: "Beta",
    branchEmpty: "No branches",
    branchKindInherited: "Inherited",
    branchKindMain: "Main",
    branchKindSandbox: "Sandbox",
    branchName: "Branch name",
    branchTreeTitle: "Branches",
    brand: "route",
    cancelBranch: "Cancel",
    clearData: "Clear all data",
    clearDataDone: "Cleared",
    clearDataHint: "Delete every recorded project, history, and cache. This is irreversible.",
    clearDataHold: "Hold 10s to confirm (irreversible)",
    clearDataIrreversible: "This cannot be undone.",
    clickToCopy: "Click to copy path",
    cliMcpBinaryPath: "Binary path",
    cliMcpBuildHint: "If the binary is missing, run cargo build first",
    cliMcpConfigCopied: "Copied",
    cliMcpConfigSnippet: "Config snippet",
    cliMcpCopyConfig: "Copy config",
    cliMcpEnable: "Enable CLI / MCP interface",
    cliMcpEnableHint: "Once on, external AIs can call Route's version-management primitives through the command line or the MCP protocol",
    cliMcpNoProject: "Select a project first",
    cliMcpOff: "Disabled",
    cliMcpOn: "Enabled",
    cliMcpStartHint: "Keep Route running in the foreground or minimized while integrations are active",
    close: "Close",
    closeBehaviorHide: "Hide to tray",
    closeBehaviorHint: "Choose what happens when closing the window",
    closeBehaviorLabel: "On window close",
    closeBehaviorQuit: "Quit application",
    copyPath: "Copy path",
    copyToNewBranch: "Copy to new branch",
    createBranch: "Create",
    cta: "Choose Folder",
    currentBranchLabel: "Current branch",
    dangerZone: "Danger zone",
    dataExportHint: "Download timeline records as JSON",
    dataExportLabel: "Export timeline",
    dataImportExport: "Timeline data",
    dataImportExportHint: "Export the timeline or view local data location",
    dataLocationHint: "Project data is stored in the .route directory",
    dataLocationLabel: "Data location",
    dataSection: "Data",
    diffShowFiles: "Show file changes",
    errorTauriBridge: "Please run this in the Route desktop app (requires `cargo tauri dev` or the installed build)",
    filesAdded: "Added",
    filesModified: "Modified",
    filesRemoved: "Removed",
    filterAi: "AI",
    filterAll: "All",
    filterMarker: "Marker",
    filterUser: "User",
    gitModeAside: "Beta · no push or remote operations",
    gitModeAvailable: "Git available",
    gitModeDetect: "Detect",
    gitModeDetectLabel: "Detect Git",
    gitModeHint: "Use local Git as the version control backend",
    gitModeLabel: "Git Mode",
    gitModeNoPushHint: "Route will not run git push or connect to remotes",
    gitModeOff: "Off",
    gitModeOn: "On",
    gitModeUnavailable: "Git not found",
    gitRestore: "Discard changes",
    gitRestoreConfirm: "Confirm discard?",
    gitRestored: "Changes discarded",
    gitRestoreNothing: "Nothing to discard",
    gitStashNothing: "Nothing to stash",
    gitStashPop: "Pop stash",
    gitStashPopped: "Stash applied",
    gitStashPush: "Stash changes",
    gitStashStashed: "Changes stashed",
    gitTagAdd: "Tag version",
    gitTagDelete: "Delete tag",
    gitTagPlaceholder: "Tag name (e.g. v1.0)",
    initBackupCloud: "Cloud",
    initBackupLocal: "Local folder",
    initIncremental: "Incremental",
    initIncrementalHint: "Only add, never remove — keeps all history",
    initMirror: "Mirror",
    initMirrorDelay: "Mirror delay (seconds)",
    initMirrorHint: "Complete copy to target, keeping both sides in sync",
    initStandard: "Standard",
    initStandardHint: "Version management with snapshots and rollback",
    integrationsSection: "External integrations",
    languageHint: "Takes effect immediately and persists",
    languageLabel: "Language",
    manualBackup: "Full backup",
    mark: "Marker",
    markBody: "Body",
    markCancel: "Cancel",
    markTitle: "Title",
    masterOff: "Off",
    masterOn: "Running",
    maximize: "Maximize",
    mergeBranch: "Merge into current",
    minimize: "Minimize",
    modeAi: "AI Collaboration Mode",
    modeAiDev: "In development",
    modeAiHint: "Let an AI agent handle version management after configuration",
    modeSection: "Mode",
    modeSectionHint: "Pick how this project operates",
    modeStandard: "Standard Mode",
    modeStandardHint: "Local auto-tracking, with manual mark, rollback and undo",
    newBranch: "New branch",
    noMessage: "(no message)",
    noProjects: "No projects yet",
    pageSettings: "Settings",
    pageWorkspace: "Workspace",
    pathCopied: "Copied",
    pickFolderHint: "Start tracking your project files",
    pickTarget: "Pick",
    privacyP1: "When route tag mode is on, Route reads comments in your files to detect markers and tag changes.",
    privacyP2: "All parsing happens locally on your machine. No file content is uploaded to any remote server.",
    privacyP3: "However, configuration data will record partial file metadata (paths, modification counts, file types) for the projects you select. Please protect your privacy accordingly.",
    privacyTitle: "Privacy & local compute",
    remove: "Remove",
    resetSettings: "Reset settings",
    resetSettingsDone: "Reset",
    resetSettingsHint: "Restore language, endpoint and backup preferences to defaults",
    resetSettingsHold: "Hold 1s to confirm reset",
    restore: "Restore",
    rollbackOne: "Rollback to this node",
    routeaCheatsheetTitle: "route tag syntax",
    routeaColMarker: "Marker",
    routeaColMeaning: "Meaning",
    routeaCommentNote: "Supports //, #, --, /* */, and other comment styles",
    routeaLabel: "route tags",
    routeaOff: "Off",
    routeaOn: "On",
    routeaRow1Marker: "// route X",
    routeaRow1Meaning: "All changes in this file are tagged X",
    routeaRow2Marker: "// route start X ... // route over X",
    routeaRow2Meaning: "Changes between these markers are tagged X",
    routeaRow3Marker: "// route no start ... // route no",
    routeaRow3Meaning: "This range is not tracked",
    routeaSettingHint: "Reads comments in your files to detect route markers",
    searchPlaceholder: "Search records",
    selectProjectFirst: "Select a project from the sidebar first",
    settings: "Settings",
    sortAlpha: "Alphabetical",
    sortCustom: "Custom order",
    sortReverse: "Reverse",
    sortTime: "By time",
    switchBranch: "Switch",
    tagline: "From Route to Routine",
    themeDark: "Dark",
    themeLight: "Light",
    themeSection: "Theme Mode",
    themeSectionHint: "Switch between light and dark",
    timeline: "Timeline",
    timelineEmpty: "No modifications yet",
    trackAllHint: "When off, only the listed suffixes / prefixes are tracked.",
    trackAllLabel: "Track all files",
    trackingOff: "Not tracking",
    trackingRunning: "Tracking",
    trackingStarting: "Starting",
    trackMemoryBufferHint: "How long the watcher waits for typing to settle before flushing to disk. 0 = flush immediately.",
    trackMemoryBufferLabel: "Memory buffer (ms)",
    trackPrefixesHint: "Optional; only files whose path starts with one of these are tracked.",
    trackPrefixesLabel: "Tracked prefixes",
    trackSection: "File tracking",
    trackSuffixesHint: "Comma-separated, e.g. .py,.js,.tsx,.css",
    trackSuffixesLabel: "Tracked suffixes",
    trackVerifySha256Hint: "Off by default. When on, every commit additionally records a SHA-256 of every tracked file for external verification.",
    trackVerifySha256Label: "SHA-256 verification",
    uncommittedChanges: "Uncommitted changes",
    welcome: "Welcome to",
    workbenchBackupMethod: "Backup method",
    workbenchBackupTarget: "Backup target",
    workbenchConfigTitle: "Configuration",
    workbenchConfirm: "Confirm",
    workbenchEdit: "Edit",
    workbenchSourceLabel: "Source",
    workbenchTargetPath: "Target path",
  },
};

/// Return the dictionary for the given locale. Used by components that
/// need to resolve the active locale themselves (e.g. Titlebar).
export function t(locale: Locale): Dict {
  return dict[locale];
}
