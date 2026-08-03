//! 自适应防抖
//!
//! 从 GOTO Engine 移植的自适应刷新机制。
//! 通过跟踪用户的打字速度、退格频率等指标，动态调整搜索防抖/节流延迟。

/// 时间间隔窗口大小
const INTERVAL_WINDOW_SIZE: usize = 20;
/// 最小间隔（毫秒）
const INTERVAL_MIN_MS: f64 = 10.0;
/// 最大间隔（毫秒）
const INTERVAL_MAX_MS: f64 = 5000.0;
/// EMA 平滑因子
const EMA_ALPHA: f64 = 0.3;

/// 默认防抖延迟（毫秒）
const DEBOUNCE_DEFAULT_MS: f64 = 200.0;
/// 防抖上限（毫秒）
const DEBOUNCE_UPPER_MS: f64 = 400.0;
/// 默认节流延迟（毫秒）
const THROTTLE_DEFAULT_MS: f64 = 100.0;
/// 节流下限（毫秒）
const THROTTLE_LOWER_MS: f64 = 30.0;
/// 打字速度跟踪器
///
/// 记录用户输入间隔，计算 EMA 速度和错误率，
/// 用于自适应调整搜索延迟。
#[derive(Debug, Clone)]
pub struct TypingSpeedTracker {
    /// 时间间隔环形缓冲区
    intervals: Vec<f64>,
    /// EMA 速度（毫秒/字符）
    ema_speed: f64,
    /// 退格计数
    backspace_count: usize,
    /// 总输入计数
    total_input_count: usize,
    /// 上次输入时间戳
    last_timestamp: Option<u64>,
}

impl Default for TypingSpeedTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypingSpeedTracker {
    /// 创建一个新的 TypingSpeedTracker
    pub fn new() -> Self {
        TypingSpeedTracker {
            intervals: Vec::with_capacity(INTERVAL_WINDOW_SIZE),
            ema_speed: DEBOUNCE_DEFAULT_MS,
            backspace_count: 0,
            total_input_count: 0,
            last_timestamp: None,
        }
    }

    /// 记录一次输入事件
    pub fn record_input(&mut self, timestamp_ms: u64) {
        self.total_input_count += 1;

        if let Some(last) = self.last_timestamp {
            if timestamp_ms > last {
                let interval = (timestamp_ms - last) as f64;
                // 限制间隔范围
                let interval = interval.clamp(INTERVAL_MIN_MS, INTERVAL_MAX_MS);

                // 添加到环形缓冲区
                if self.intervals.len() >= INTERVAL_WINDOW_SIZE {
                    self.intervals.remove(0);
                }
                self.intervals.push(interval);

                // 更新 EMA 速度
                self.ema_speed = self.ema_speed * (1.0 - EMA_ALPHA) + interval * EMA_ALPHA;
            }
        }

        self.last_timestamp = Some(timestamp_ms);
    }

    /// 记录一次退格事件
    pub fn record_backspace(&mut self, timestamp_ms: u64) {
        self.backspace_count += 1;
        // 退格也视为一次输入，更新时间戳
        self.record_input(timestamp_ms);
    }

    /// 获取当前 EMA 速度
    pub fn get_ema_speed(&self) -> f64 {
        if self.ema_speed > DEBOUNCE_DEFAULT_MS || self.intervals.is_empty() {
            DEBOUNCE_DEFAULT_MS
        } else {
            self.ema_speed
        }
    }

    /// 计算错误率（退格数 / 总输入数）
    pub fn error_rate(&self) -> f64 {
        if self.total_input_count == 0 {
            return 0.0;
        }
        self.backspace_count as f64 / self.total_input_count as f64
    }

    /// 获取自适应防抖延迟
    pub fn get_debounce_ms(&self) -> f64 {
        let speed = self.get_ema_speed();
        let error_rate = self.error_rate();

        // 基础延迟 = 速度的 1.5 倍
        let base_delay = speed * 1.5;

        // 错误率修正：错误率越高，延迟越长
        let error_penalty = error_rate * 100.0;

        // 总延迟
        let delay = base_delay + error_penalty;

        // 限制范围
        delay.clamp(DEBOUNCE_DEFAULT_MS, DEBOUNCE_UPPER_MS)
    }

    /// 获取自适应节流延迟
    pub fn get_throttle_ms(&self) -> f64 {
        let speed = self.get_ema_speed();

        // 节流延迟 = 速度的 0.5 倍
        let throttle = speed * 0.5;

        // 限制范围
        throttle.clamp(THROTTLE_LOWER_MS, THROTTLE_DEFAULT_MS)
    }

    /// 获取自适应延迟（结合防抖和节流）
    pub fn get_adaptive_delay(&self) -> f64 {
        let debounce = self.get_debounce_ms();
        let throttle = self.get_throttle_ms();

        // 综合延迟 = 防抖 + 节流 / 2
        (debounce + throttle * 0.5).min(DEBOUNCE_UPPER_MS)
    }

    /// 重置跟踪器
    pub fn reset(&mut self) {
        self.intervals.clear();
        self.ema_speed = DEBOUNCE_DEFAULT_MS;
        self.backspace_count = 0;
        self.total_input_count = 0;
        self.last_timestamp = None;
    }

    /// 获取退格计数
    pub fn backspace_count(&self) -> usize {
        self.backspace_count
    }

    /// 获取总输入计数
    pub fn total_input_count(&self) -> usize {
        self.total_input_count
    }
}

/// 搜索编排器
///
/// 结合 TypingSpeedTracker 决定是否应该执行搜索。
#[derive(Debug, Clone)]
pub struct SearchOrchestrator {
    /// 打字速度跟踪器
    tracker: TypingSpeedTracker,
    /// 上次搜索时间戳
    last_search_time: Option<u64>,
}

impl Default for SearchOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchOrchestrator {
    /// 创建一个新的 SearchOrchestrator
    pub fn new() -> Self {
        SearchOrchestrator {
            tracker: TypingSpeedTracker::new(),
            last_search_time: None,
        }
    }

    /// 处理输入事件
    pub fn on_input(&mut self, timestamp_ms: u64) {
        self.tracker.record_input(timestamp_ms);
    }

    /// 处理退格事件
    pub fn on_backspace(&mut self, timestamp_ms: u64) {
        self.tracker.record_backspace(timestamp_ms);
    }

    /// 判断是否应该执行搜索
    ///
    /// 基于当前打字速度和上次搜索时间决定。
    pub fn should_search(&self, current_time_ms: u64) -> bool {
        let debounce = self.tracker.get_debounce_ms();

        match self.last_search_time {
            Some(last_search) => {
                let elapsed = (current_time_ms as f64) - (last_search as f64);
                elapsed >= debounce
            }
            None => true, // 从未搜索过，立即搜索
        }
    }

    /// 标记一次搜索已执行
    pub fn mark_searched(&mut self, timestamp_ms: u64) {
        self.last_search_time = Some(timestamp_ms);
    }

    /// 获取当前防抖延迟
    pub fn get_debounce_ms(&self) -> f64 {
        self.tracker.get_debounce_ms()
    }

    /// 获取当前节流延迟
    pub fn get_throttle_ms(&self) -> f64 {
        self.tracker.get_throttle_ms()
    }

    /// 获取内部跟踪器的引用
    pub fn tracker(&self) -> &TypingSpeedTracker {
        &self.tracker
    }

    /// 获取内部跟踪器的可变引用
    pub fn tracker_mut(&mut self) -> &mut TypingSpeedTracker {
        &mut self.tracker
    }

    /// 重置编排器
    pub fn reset(&mut self) {
        self.tracker.reset();
        self.last_search_time = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tracker() {
        let tracker = TypingSpeedTracker::new();
        assert_eq!(tracker.get_ema_speed(), DEBOUNCE_DEFAULT_MS);
        assert_eq!(tracker.error_rate(), 0.0);
        assert_eq!(tracker.backspace_count(), 0);
        assert_eq!(tracker.total_input_count(), 0);
    }

    #[test]
    fn test_record_input() {
        let mut tracker = TypingSpeedTracker::new();
        tracker.record_input(1000);
        assert_eq!(tracker.total_input_count(), 1);
        // 只有一次输入，没有间隔，使用默认速度
        assert_eq!(tracker.get_ema_speed(), DEBOUNCE_DEFAULT_MS);
    }

    #[test]
    fn test_record_multiple_inputs() {
        let mut tracker = TypingSpeedTracker::new();
        tracker.record_input(1000);
        tracker.record_input(1200); // 200ms 间隔
        tracker.record_input(1350); // 150ms 间隔

        assert_eq!(tracker.total_input_count(), 3);
        // EMA 速度应该小于默认值
        assert!(tracker.get_ema_speed() < DEBOUNCE_DEFAULT_MS);
    }

    #[test]
    fn test_record_backspace() {
        let mut tracker = TypingSpeedTracker::new();
        tracker.record_input(1000);
        tracker.record_backspace(1200);

        assert_eq!(tracker.backspace_count(), 1);
        assert_eq!(tracker.total_input_count(), 2);
        assert!(tracker.error_rate() > 0.0);
    }

    #[test]
    fn test_error_rate() {
        let mut tracker = TypingSpeedTracker::new();
        assert_eq!(tracker.error_rate(), 0.0);

        tracker.record_input(1000);
        tracker.record_input(1100);
        tracker.record_backspace(1200);

        assert!((tracker.error_rate() - 1.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_get_debounce_ms() {
        let mut tracker = TypingSpeedTracker::new();
        // 默认情况
        let debounce = tracker.get_debounce_ms();
        assert!(debounce >= DEBOUNCE_DEFAULT_MS);
        assert!(debounce <= DEBOUNCE_UPPER_MS);

        // 快速打字
        tracker.record_input(1000);
        tracker.record_input(1050); // 50ms 间隔
        tracker.record_input(1090); // 40ms 间隔
        let fast_debounce = tracker.get_debounce_ms();
        assert!(fast_debounce >= DEBOUNCE_DEFAULT_MS);
    }

    #[test]
    fn test_get_throttle_ms() {
        let tracker = TypingSpeedTracker::new();
        let throttle = tracker.get_throttle_ms();
        assert!(throttle >= THROTTLE_LOWER_MS);
        assert!(throttle <= THROTTLE_DEFAULT_MS);
    }

    #[test]
    fn test_get_adaptive_delay() {
        let tracker = TypingSpeedTracker::new();
        let delay = tracker.get_adaptive_delay();
        assert!(delay >= THROTTLE_LOWER_MS);
        assert!(delay <= DEBOUNCE_UPPER_MS);
    }

    #[test]
    fn test_reset() {
        let mut tracker = TypingSpeedTracker::new();
        tracker.record_input(1000);
        tracker.record_backspace(1200);
        tracker.reset();

        assert_eq!(tracker.total_input_count(), 0);
        assert_eq!(tracker.backspace_count(), 0);
        assert_eq!(tracker.error_rate(), 0.0);
        assert_eq!(tracker.get_ema_speed(), DEBOUNCE_DEFAULT_MS);
    }

    #[test]
    fn test_search_orchestrator_should_search() {
        let orchestrator = SearchOrchestrator::new();
        // 首次应该搜索
        assert!(orchestrator.should_search(1000));
    }

    #[test]
    fn test_search_orchestrator_should_not_search() {
        let mut orchestrator = SearchOrchestrator::new();
        orchestrator.mark_searched(1000);
        // 刚搜索完，不应立即搜索
        assert!(!orchestrator.should_search(1050));
    }

    #[test]
    fn test_search_orchestrator_should_search_after_debounce() {
        let mut orchestrator = SearchOrchestrator::new();
        orchestrator.mark_searched(1000);
        // 等待足够长的时间
        assert!(orchestrator.should_search(2000));
    }

    #[test]
    fn test_search_orchestrator_on_input() {
        let mut orchestrator = SearchOrchestrator::new();
        orchestrator.on_input(1000);
        orchestrator.on_input(1200);
        assert_eq!(orchestrator.tracker().total_input_count(), 2);
    }

    #[test]
    fn test_search_orchestrator_on_backspace() {
        let mut orchestrator = SearchOrchestrator::new();
        orchestrator.on_backspace(1000);
        assert_eq!(orchestrator.tracker().backspace_count(), 1);
    }

    #[test]
    fn test_search_orchestrator_reset() {
        let mut orchestrator = SearchOrchestrator::new();
        orchestrator.on_input(1000);
        orchestrator.mark_searched(1500);
        orchestrator.reset();

        assert_eq!(orchestrator.tracker().total_input_count(), 0);
        assert!(orchestrator.should_search(2000));
    }

    #[test]
    fn test_constants() {
        assert_eq!(INTERVAL_WINDOW_SIZE, 20);
        assert!(INTERVAL_MIN_MS < INTERVAL_MAX_MS);
        assert!(EMA_ALPHA > 0.0 && EMA_ALPHA < 1.0);
        assert!(DEBOUNCE_DEFAULT_MS < DEBOUNCE_UPPER_MS);
        assert!(THROTTLE_LOWER_MS < THROTTLE_DEFAULT_MS);
    }
}