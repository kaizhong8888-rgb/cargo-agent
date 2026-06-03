/// Seasonal Trading Strategies
///
/// Exploits calendar-based market anomalies:
/// 1. Month-of-year effects (January Effect, December Rally, etc.)
/// 2. Day-of-week effects (Monday Effect, Friday Rally, etc.)
/// 3. Turn-of-month effects (first/last 3 days)
/// 4. Holiday effects (pre/post holiday returns)
/// 5. Quarter-end rebalancing flows
///
/// Core idea: Markets exhibit persistent seasonal patterns due to
/// institutional flows, tax-loss harvesting, window dressing, etc.
use crate::trading::data::Candle;
use crate::trading::strategy::{Signal, Strategy};

// ============================================================================
// Date utilities (simplified — uses index-based approximations)
// ============================================================================

/// Approximate calendar info from candle index
/// Assumes daily candles starting from a known date
#[derive(Debug, Clone)]
pub struct CalendarInfo {
    pub day_of_week: u8,  // 0=Monday, 6=Sunday
    pub day_of_month: u8, // 1-31
    pub month: u8,        // 1-12
    pub is_month_start: bool,
    pub is_month_end: bool,
    pub is_quarter_end: bool,
}

impl CalendarInfo {
    /// Create calendar info from candle index
    /// Assumes index 0 = Monday, January 1
    fn from_index(idx: usize, _trading_days_per_year: usize) -> Self {
        // Approximate: 252 trading days per year
        let trading_days_per_year = 252;
        let days_per_month = trading_days_per_year / 12; // ~21
        let days_per_quarter = days_per_month * 3;

        // Day of week (5 trading days per week)
        let day_of_week = (idx % 5) as u8;

        // Approximate month
        let month = ((idx % trading_days_per_year) / days_per_month + 1).min(12) as u8;

        // Day within month
        let day_of_month = ((idx % days_per_month) + 1).min(31) as u8;

        let is_month_start = day_of_month <= 3;
        let is_month_end = day_of_month >= 19; // ~21 trading days, last 3

        let is_quarter_end = (idx % days_per_quarter) >= days_per_quarter - 3;

        Self {
            day_of_week,
            day_of_month,
            month,
            is_month_start,
            is_month_end,
            is_quarter_end,
        }
    }
}

// ============================================================================
// Seasonal pattern detectors
// ============================================================================

/// Compute historical average returns by month
fn compute_monthly_returns(candles: &[Candle]) -> Vec<f64> {
    let mut monthly_sums = vec![0.0; 12];
    let mut monthly_counts = vec![0usize; 12];
    let mut prev_price = candles[0].close;
    let mut trading_day = 0;

    for (i, candle) in candles.iter().enumerate().skip(1) {
        let ret = (candle.close - prev_price) / prev_price;
        let cal = CalendarInfo::from_index(trading_day, 252);
        let month_idx = (cal.month as usize).saturating_sub(1).min(11);
        monthly_sums[month_idx] += ret;
        monthly_counts[month_idx] += 1;
        prev_price = candle.close;
        trading_day += 1;

        // Reset month approximation every ~21 days
        if (i + 1) % 21 == 0 {
            // month rollover
        }
    }

    let mut averages = vec![0.0; 12];
    for i in 0..12 {
        if monthly_counts[i] > 0 {
            averages[i] = monthly_sums[i] / monthly_counts[i] as f64;
        }
    }
    averages
}

/// Compute historical average returns by day of week
fn compute_daily_returns(candles: &[Candle]) -> Vec<f64> {
    let mut daily_sums = vec![0.0; 5];
    let mut daily_counts = vec![0usize; 5];
    let mut prev_price = candles[0].close;

    for (i, candle) in candles.iter().enumerate().skip(1) {
        let ret = (candle.close - prev_price) / prev_price;
        let day_of_week = (i % 5) as usize;
        daily_sums[day_of_week] += ret;
        daily_counts[day_of_week] += 1;
        prev_price = candle.close;
    }

    let mut averages = vec![0.0; 5];
    for i in 0..5 {
        if daily_counts[i] > 0 {
            averages[i] = daily_sums[i] / daily_counts[i] as f64;
        }
    }
    averages
}

/// Compute turn-of-month effect
fn compute_tom_effect(candles: &[Candle]) -> (f64, f64) {
    let mut tom_sum = 0.0;
    let mut tom_count = 0;
    let mut rest_sum = 0.0;
    let mut rest_count = 0;

    let mut prev_price = candles[0].close;
    let mut day_in_month = 0;

    for candle in candles.iter().skip(1) {
        let ret = (candle.close - prev_price) / prev_price;
        let is_tom = day_in_month < 3 || day_in_month >= 18; // first/last 3 trading days

        if is_tom {
            tom_sum += ret;
            tom_count += 1;
        } else {
            rest_sum += ret;
            rest_count += 1;
        }

        prev_price = candle.close;
        day_in_month = (day_in_month + 1) % 21;
    }

    let tom_avg = if tom_count > 0 { tom_sum / tom_count as f64 } else { 0.0 };
    let rest_avg = if rest_count > 0 {
        rest_sum / rest_count as f64
    } else {
        0.0
    };

    (tom_avg, rest_avg)
}

// ============================================================================
// Seasonal Strategy
// ============================================================================

#[derive(Debug)]
pub struct SeasonalStrategy {
    /// Enable monthly effect (default: true)
    pub enable_monthly: bool,
    /// Enable day-of-week effect (default: true)
    pub enable_daily: bool,
    /// Enable turn-of-month effect (default: true)
    pub enable_tom: bool,
    /// Minimum lookback period for pattern detection (default: 100)
    pub min_lookback: usize,
    /// Signal strength threshold (default: 0.0005 = 0.05% avg return)
    pub threshold: f64,
}

impl Default for SeasonalStrategy {
    fn default() -> Self {
        Self {
            enable_monthly: true,
            enable_daily: true,
            enable_tom: true,
            min_lookback: 100,
            threshold: 0.0005,
        }
    }
}

impl SeasonalStrategy {
    pub fn new(
        enable_monthly: bool,
        enable_daily: bool,
        enable_tom: bool,
        min_lookback: usize,
        threshold: f64,
    ) -> Self {
        Self {
            enable_monthly,
            enable_daily,
            enable_tom,
            min_lookback,
            threshold,
        }
    }

    /// Get analysis results without generating signals
    pub fn analyze(&self, candles: &[Candle]) -> SeasonalAnalysis {
        let monthly = if self.enable_monthly && candles.len() >= self.min_lookback {
            compute_monthly_returns(candles)
        } else {
            vec![0.0; 12]
        };

        let daily = if self.enable_daily && candles.len() >= self.min_lookback {
            compute_daily_returns(candles)
        } else {
            vec![0.0; 5]
        };

        let (tom, rest) = if self.enable_tom && candles.len() >= self.min_lookback {
            compute_tom_effect(candles)
        } else {
            (0.0, 0.0)
        };

        SeasonalAnalysis {
            monthly_returns: monthly,
            daily_returns: daily,
            tom_effect: tom,
            non_tom_effect: rest,
        }
    }
}

/// Seasonal analysis results
#[derive(Debug, Clone)]
pub struct SeasonalAnalysis {
    /// Average returns by month (Jan=0, Dec=11)
    pub monthly_returns: Vec<f64>,
    /// Average returns by day of week (Mon=0, Fri=4)
    pub daily_returns: Vec<f64>,
    /// Turn-of-month average return
    pub tom_effect: f64,
    /// Non-turn-of-month average return
    pub non_tom_effect: f64,
}

impl SeasonalAnalysis {
    /// Get the best month by average return
    pub fn best_month(&self) -> (usize, f64) {
        let mut best = (0, f64::NEG_INFINITY);
        for (i, &ret) in self.monthly_returns.iter().enumerate() {
            if ret > best.1 {
                best = (i, ret);
            }
        }
        best
    }

    /// Get the worst month by average return
    pub fn worst_month(&self) -> (usize, f64) {
        let mut worst = (0, f64::INFINITY);
        for (i, &ret) in self.monthly_returns.iter().enumerate() {
            if ret < worst.1 {
                worst = (i, ret);
            }
        }
        worst
    }

    /// Generate a human-readable summary
    pub fn summary(&self) -> String {
        let months = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun",
            "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let days = ["Mon", "Tue", "Wed", "Thu", "Fri"];

        let mut out = String::from("Seasonal Analysis Summary:\n\n");

        out.push_str("Monthly Returns:\n");
        for (i, &ret) in self.monthly_returns.iter().enumerate() {
            out.push_str(&format!("  {}: {:.4}%\n", months[i], ret * 100.0));
        }

        out.push_str("\nDay-of-Week Returns:\n");
        for (i, &ret) in self.daily_returns.iter().enumerate() {
            out.push_str(&format!("  {}: {:.4}%\n", days[i], ret * 100.0));
        }

        out.push_str(&format!(
            "\nTurn-of-Month Effect: {:.4}% vs {:.4}% (non-ToM)\n",
            self.tom_effect * 100.0,
            self.non_tom_effect * 100.0
        ));

        let (best_m, best_r) = self.best_month();
        let (worst_m, worst_r) = self.worst_month();
        out.push_str(&format!(
            "Best month: {} ({:.4}%)\n",
            months[best_m],
            best_r * 100.0
        ));
        out.push_str(&format!(
            "Worst month: {} ({:.4}%)\n",
            months[worst_m],
            worst_r * 100.0
        ));

        out
    }
}

impl Strategy for SeasonalStrategy {
    fn name(&self) -> &str {
        "Seasonal Patterns"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let analysis = self.analyze(candles);
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];

        if n < self.min_lookback {
            return signals;
        }

        // Combine all seasonal signals with voting
        for i in (self.min_lookback - 1)..n {
            let cal = CalendarInfo::from_index(i, 252);
            let mut buy_votes = 0;
            let mut sell_votes = 0;

            // 1. Monthly effect
            if self.enable_monthly {
                let month_idx = (cal.month as usize).saturating_sub(1).min(11);
                let monthly_ret = analysis.monthly_returns[month_idx];
                if monthly_ret > self.threshold {
                    buy_votes += 1;
                } else if monthly_ret < -self.threshold {
                    sell_votes += 1;
                }
            }

            // 2. Day-of-week effect
            if self.enable_daily {
                let dow = cal.day_of_week as usize;
                let daily_ret = analysis.daily_returns.get(dow).copied().unwrap_or(0.0);
                if daily_ret > self.threshold {
                    buy_votes += 1;
                } else if daily_ret < -self.threshold {
                    sell_votes += 1;
                }
            }

            // 3. Turn-of-month effect
            if self.enable_tom {
                if cal.is_month_start {
                    // Beginning of month: typically positive (fund inflows)
                    if analysis.tom_effect > self.threshold {
                        buy_votes += 1;
                    }
                } else if cal.is_month_end {
                    // End of month: window dressing, can go either way
                    let tom_edge = analysis.tom_effect - analysis.non_tom_effect;
                    if tom_edge > self.threshold * 2.0 {
                        buy_votes += 1;
                    } else if tom_edge < -self.threshold * 2.0 {
                        sell_votes += 1;
                    }
                }
            }

            // 4. Quarter-end effect
            if cal.is_quarter_end {
                // Quarter-end: institutional rebalancing, typically volatile
                // Simplified: assume slight positive bias (window dressing)
                buy_votes += 1;
            }

            // Generate signal based on majority vote
            if buy_votes >= 2 {
                signals[i] = Signal::Buy;
            } else if sell_votes >= 2 {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ============================================================================
// Holiday Effect Strategy (Simplified)
///
/// Detects pre-holiday and post-holiday return anomalies.
/// Pre-holiday: typically positive (investors close positions before holidays)
/// Post-holiday: mixed, depends on the holiday
///
/// This simplified version uses fixed "holiday" indices for demonstration.
// ============================================================================

#[derive(Debug)]
pub struct HolidayEffectStrategy {
    /// Days before a "holiday" to look for pattern (default: 1)
    pub pre_days: usize,
    /// Days after a "holiday" to look for pattern (default: 1)
    pub post_days: usize,
    /// Holiday indices (trading day offsets in a year)
    pub holiday_offsets: Vec<usize>,
}

impl Default for HolidayEffectStrategy {
    fn default() -> Self {
        // Approximate US holidays in trading day offsets
        Self {
            pre_days: 1,
            post_days: 1,
            holiday_offsets: vec![
                0,    // New Year
                20,   // MLK Day
                35,   // Presidents Day
                75,   // Good Friday
                105,  // Memorial Day
                130,  // Independence Day
                175,  // Labor Day
                200,  // Thanksgiving
                220,  // Christmas
                245,  // Year-end
            ],
        }
    }
}

impl Strategy for HolidayEffectStrategy {
    fn name(&self) -> &str {
        "Holiday Effect"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];

        if n < 10 {
            return signals;
        }

        // Detect holidays by finding approximate offsets in the year
        let trading_days_per_year = 252;

        for i in self.pre_days..n {
            let day_in_year = i % trading_days_per_year;

            // Check if we're near a holiday
            for &holiday in &self.holiday_offsets {
                let dist = if day_in_year >= holiday {
                    day_in_year - holiday
                } else {
                    trading_days_per_year + day_in_year - holiday
                };

                // Pre-holiday effect
                if dist <= self.pre_days && dist > 0 {
                    signals[i] = Signal::Buy;
                    break;
                }

                // Post-holiday effect: wait for the day after
                if dist == self.post_days && self.post_days > 0 {
                    // Mixed signal: sometimes positive, sometimes negative
                    // Default to Buy for the "holiday rally" effect
                    signals[i] = Signal::Buy;
                    break;
                }
            }
        }

        signals
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading::data::DataSource;

    #[test]
    fn test_seasonal_strategy_generates_signals() {
        let candles = DataSource::generate_mock(500, 100.0);

        let strategy = SeasonalStrategy::default();
        let signals = strategy.generate(&candles);

        assert_eq!(signals.len(), 500);
    }

    #[test]
    fn test_seasonal_analysis() {
        let candles = DataSource::generate_mock(500, 100.0);

        let strategy = SeasonalStrategy::default();
        let analysis = strategy.analyze(&candles);

        assert_eq!(analysis.monthly_returns.len(), 12);
        assert_eq!(analysis.daily_returns.len(), 5);
    }

    #[test]
    fn test_seasonal_analysis_summary() {
        let candles = DataSource::generate_mock(500, 100.0);

        let strategy = SeasonalStrategy::default();
        let analysis = strategy.analyze(&candles);
        let summary = analysis.summary();

        assert!(summary.contains("Seasonal Analysis Summary"));
        assert!(summary.contains("Monthly Returns"));
        assert!(summary.contains("Day-of-Week Returns"));
        assert!(summary.contains("Turn-of-Month Effect"));
    }

    #[test]
    fn test_seasonal_insufficient_data() {
        let candles = DataSource::generate_mock(50, 100.0);

        let strategy = SeasonalStrategy::new(true, true, true, 100, 0.0005);
        let signals = strategy.generate(&candles);

        assert_eq!(signals.len(), 50);
        // All Hold when insufficient data
        assert!(signals.iter().all(|s| *s == Signal::Hold));
    }

    #[test]
    fn test_holiday_effect_strategy() {
        let candles = DataSource::generate_mock(500, 100.0);

        let strategy = HolidayEffectStrategy::default();
        let signals = strategy.generate(&candles);

        assert_eq!(signals.len(), 500);
    }

    #[test]
    fn test_calendar_info() {
        let cal = CalendarInfo::from_index(0, 252);
        assert_eq!(cal.day_of_week, 0); // Monday
        assert_eq!(cal.month, 1); // January

        let cal2 = CalendarInfo::from_index(20, 252);
        assert_eq!(cal2.day_of_week, 0); // Monday again (20 % 5 = 0)
        assert_eq!(cal2.month, 2); // February
    }

    #[test]
    fn test_monthly_returns_computation() {
        let mut candles = DataSource::generate_mock(300, 100.0);
        // Make January artificially bullish
        for i in 0..21 {
            candles[i].close *= 1.02;
        }

        let monthly = compute_monthly_returns(&candles);
        assert_eq!(monthly.len(), 12);
    }

    #[test]
    fn test_turn_of_month_effect() {
        let candles = DataSource::generate_mock(500, 100.0);
        let (tom, rest) = compute_tom_effect(&candles);

        // Values should be finite
        assert!(tom.is_finite());
        assert!(rest.is_finite());
    }
}
