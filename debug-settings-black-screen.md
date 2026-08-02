# Debug Session: settings-black-screen

## Status
[OPEN]

## Symptom
在 Route 桌面应用的设置页面里，几乎随便点击任意选项，整个界面就会突然黑屏/窗口消失。

## Environment
- OS: Windows 11
- Frontend: React 18 + Tauri 2.x + Vite
- Backend: Rust (route-tauri)
- Recent changes: `html/body/#root` 加了 `overflow: hidden; overscroll-behavior: none`

## Hypotheses
1. `overflow: hidden` / `overscroll-behavior: none` 应用在 Webview2 中触发渲染崩溃，导致黑屏。
2. 设置页某个 switch/checkbox 的 onChange 触发无限循环或严重 JS 异常，使 React 卸载整棵树。
3. Tauri Webview 在 `data-theme` 切换时因 CSS 变量大量更新产生重绘异常。
4. 点击设置项时后端 IPC 调用失败/返回异常，前端未捕获导致白屏/黑屏。
5. 窗口失去焦点或被系统回收（最小化/隐藏）被误感知为“黑屏”。

## Instrumentation Plan
- 在 `App.tsx` 顶层添加全局错误捕获 + 点击事件日志上报。
- 在主题切换、设置页主要 switch 的 handler 前后埋点。
- 收集崩溃前的最后日志。

## Evidence
<!-- 运行时日志将写入此处 -->

## Fix
<!-- 待证据确认后填写 -->
