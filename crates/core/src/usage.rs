//! Tokens and what they would cost at list prices (API-equivalent cost).
//!
//! Subscription users do not pay per token; the cost here is the public API
//! price of the same tokens, so days and projects can be compared. A model
//! with no known price keeps its tokens and adds nothing to the cost.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where the prices below come from, shown with the numbers.
pub const PRICES_AS_OF: &str = "2026-10-02";

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
    /// Input not served from cache.
    #[serde(default)]
    pub input: u64,
    #[serde(default)]
    pub output: u64,
    #[serde(default)]
    pub cache_read: u64,
    /// Cache writes kept five minutes / one hour (they price differently).
    #[serde(default)]
    pub cache_write_5m: u64,
    #[serde(default)]
    pub cache_write_1h: u64,
}

impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write_5m + self.cache_write_1h
    }

    /// Everything the model read, cached or not: what fills a context window.
    pub fn read(&self) -> u64 {
        self.input + self.cache_read + self.cache_write_5m + self.cache_write_1h
    }

    pub fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_read += o.cache_read;
        self.cache_write_5m += o.cache_write_5m;
        self.cache_write_1h += o.cache_write_1h;
    }

    /// Anthropic's `usage` object from a Claude Code transcript.
    pub fn from_claude(u: &serde_json::Value) -> Tokens {
        let n = |v: &serde_json::Value| v.as_u64().unwrap_or(0);
        let write = n(&u["cache_creation_input_tokens"]);
        let h1 = n(&u["cache_creation"]["ephemeral_1h_input_tokens"]).min(write);
        Tokens {
            input: n(&u["input_tokens"]),
            output: n(&u["output_tokens"]),
            cache_read: n(&u["cache_read_input_tokens"]),
            cache_write_5m: write - h1,
            cache_write_1h: h1,
        }
    }

    /// OpenAI's token usage from a Codex rollout, where cached input is part of input.
    pub fn from_codex(u: &serde_json::Value) -> Tokens {
        let n = |v: &serde_json::Value| v.as_u64().unwrap_or(0);
        let input = n(&u["input_tokens"]);
        let cached = n(&u["cached_input_tokens"]).min(input);
        Tokens { input: input - cached, output: n(&u["output_tokens"]), cache_read: cached, ..Default::default() }
    }

    /// Growth from `before` to `self` of a running total (Codex reports totals).
    pub fn since(&self, before: &Tokens) -> Tokens {
        Tokens {
            input: self.input.saturating_sub(before.input),
            output: self.output.saturating_sub(before.output),
            cache_read: self.cache_read.saturating_sub(before.cache_read),
            cache_write_5m: self.cache_write_5m.saturating_sub(before.cache_write_5m),
            cache_write_1h: self.cache_write_1h.saturating_sub(before.cache_write_1h),
        }
    }
}

/// USD per million tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
}

const fn anthropic(input: f64, output: f64, cache_read: f64) -> Price {
    Price { input, output, cache_read, cache_write_5m: input * 1.25, cache_write_1h: input * 2.0 }
}

const fn openai(input: f64, cached: f64, output: f64) -> Price {
    Price { input, output, cache_read: cached, cache_write_5m: input, cache_write_1h: input }
}

/// Public list prices, standard tier, from platform.claude.com/docs/en/about-claude/pricing
/// and developers.openai.com/api/docs/pricing as of `PRICES_AS_OF`. Matched by
/// prefix after dropping a date suffix; the longest match wins.
const PRICES: &[(&str, Price)] = &[
    ("claude-fable-5-1", anthropic(10.0, 50.0, 0.25)),
    ("claude-mythos-5-1", anthropic(10.0, 50.0, 0.25)),
    ("claude-fable-5", anthropic(10.0, 50.0, 1.0)),
    ("claude-mythos-5", anthropic(10.0, 50.0, 1.0)),
    ("claude-opus-5-5", anthropic(4.0, 20.0, 0.20)),
    ("claude-opus-5", anthropic(5.0, 25.0, 0.50)),
    ("claude-opus-4-8", anthropic(5.0, 25.0, 0.50)),
    ("claude-opus-4-7", anthropic(5.0, 25.0, 0.50)),
    ("claude-opus-4-6", anthropic(5.0, 25.0, 0.50)),
    ("claude-opus-4-5", anthropic(5.0, 25.0, 0.50)),
    ("claude-opus-4-1", anthropic(15.0, 75.0, 1.50)),
    ("claude-opus-4", anthropic(15.0, 75.0, 1.50)),
    ("claude-sonnet-5-5", anthropic(2.0, 10.0, 0.20)),
    ("claude-sonnet-5", anthropic(2.0, 10.0, 0.20)),
    ("claude-sonnet-4-6", anthropic(3.0, 15.0, 0.30)),
    ("claude-sonnet-4-5", anthropic(3.0, 15.0, 0.30)),
    ("claude-sonnet-4", anthropic(3.0, 15.0, 0.30)),
    ("claude-haiku-4-5", anthropic(1.0, 5.0, 0.10)),
    ("claude-3-5-haiku", anthropic(0.80, 4.0, 0.08)),
    ("gpt-6-astra", openai(10.0, 1.0, 50.0)),
    ("gpt-5.6-sol", openai(4.0, 0.40, 20.0)),
    ("gpt-5.6-terra", openai(2.0, 0.20, 12.0)),
    ("gpt-5.6-luna", openai(0.20, 0.02, 1.20)),
    ("gpt-5.5", openai(5.0, 0.50, 30.0)),
    ("gpt-5.4", openai(2.50, 0.25, 15.0)),
    ("gpt-5.3-codex", openai(1.75, 0.175, 14.0)),
    ("gpt-5-mini", openai(0.25, 0.025, 2.0)),
];

/// Exact names priced as themselves only (a prefix would also catch gpt-5-codex, which has no listed price).
const EXACT: &[(&str, Price)] = &[
    ("gpt-5", openai(1.25, 0.125, 10.0)),
    // OpenRouter public catalog snapshot, 2026-10-01:
    // https://openrouter.ai/api/v1/models (id: deepseek/deepseek-v4-flash).
    // USD/token: prompt 0.00000004186, completion 0.00000008372,
    // input_cache_read 0.000000008372. Cache writes use normal input price;
    // the catalog lists no separate write surcharge. The catalog price is its
    // cheapest provider; OpenRouter routes each request to a provider of its
    // choosing, so this is a lower bound, not the billed amount.
    // Exact match: variants (:free, later versions) must not inherit this price.
    ("deepseek/deepseek-v4-flash", openai(0.04186, 0.008372, 0.08372)),
    // Same catalog/date, distinct model: prompt 0.00000003,
    // completion 0.0000005, input_cache_read 0.00000001 USD/token.
    ("deepseek/deepseek-v4.1-flash", openai(0.03, 0.01, 0.5)),
    // Coding models people reach through Claude Code (ANTHROPIC_BASE_URL) or a
    // Codex model provider, by the id each provider's setup guide puts in the
    // config. Claude Code records the response's `model`, which these
    // Anthropic-compatible endpoints document as the requested id; Codex
    // records the configured id itself. Exact match: each family has
    // differently priced siblings (-flashx, -highspeed, -prime, :batch).
    //
    // Z.ai, https://docs.z.ai/guides/overview/pricing (ids in
    // https://docs.z.ai/devpack/latest-model). Cached-input storage is
    // "Limited-time Free": no write surcharge.
    ("glm-5.3", openai(1.4, 0.26, 4.4)),
    ("glm-5.3-flash", openai(0.15, 0.03, 0.50)),
    // Moonshot, https://platform.kimi.ai/docs/pricing/chat (ids in
    // https://platform.kimi.ai/docs/guide/claude-code-kimi). K3 bills cache
    // writes by TTL at 1x / 2x input; K2 lists no write price.
    ("kimi-k3", Price { input: 3.0, output: 15.0, cache_read: 0.30, cache_write_5m: 3.0, cache_write_1h: 6.0 }),
    ("kimi-k2.7-code", openai(0.95, 0.19, 4.0)),
    // Alibaba Model Studio, International (Singapore) region,
    // https://www.alibabacloud.com/help/en/model-studio/model-pricing; cache
    // rules in .../model-studio/context-cache: explicit cache hits 10%, writes
    // 125% (5-minute TTL only). Claude Code marks cache_control, so hits are
    // taken as explicit; implicit hits bill 20%, which this undercounts.
    ("qwen3.7-max", Price { input: 2.5, output: 7.5, cache_read: 0.25, cache_write_5m: 3.125, cache_write_1h: 3.125 }),
    // MiniMax, https://platform.minimax.io/docs/guides/pricing-paygo, standard
    // tier at <= 512k input tokens per request; above that the price doubles,
    // so long requests are undercounted. No cache write price is listed.
    ("minimax-m3", openai(0.30, 0.06, 1.20)),
    // OpenRouter catalog ids, https://openrouter.ai/api/v1/models, 2026-10-02.
    // Like DeepSeek above, the catalog price is a lower bound of what the
    // routed provider bills. Only qwen3.7-max lists a cache write price.
    ("z-ai/glm-5.3", openai(1.4, 0.14, 4.4)),
    ("z-ai/glm-5.3-flash", openai(0.15, 0.03, 0.5)),
    ("moonshotai/kimi-k3", openai(2.7, 0.27, 13.5)),
    ("moonshotai/kimi-k2.7-code", openai(0.6712, 0.18, 3.35)),
    ("qwen/qwen3.7-max", Price { input: 1.475, output: 4.425, cache_read: 0.295, cache_write_5m: 1.84375, cache_write_1h: 1.84375 }),
    ("minimax/minimax-m3", openai(0.3, 0.06, 1.2)),
];

/// Lowercased, without Claude Code's context suffix ("glm-5.3[1m]" -> "glm-5.3").
fn bare(model: &str) -> String {
    let m = model.trim().to_lowercase();
    match m.find('[') {
        Some(i) if m.ends_with(']') => m[..i].to_owned(),
        _ => m,
    }
}

pub fn price(model: &str) -> Option<Price> {
    let m = model.trim().to_lowercase();
    let b = bare(model);
    if let Some((_, p)) = EXACT.iter().find(|(k, _)| *k == b) {
        return Some(*p);
    }
    PRICES
        .iter()
        .filter(|(k, _)| m == *k || m.starts_with(&format!("{k}-")) || m.starts_with(&format!("{k}[")))
        .max_by_key(|(k, _)| k.len())
        .map(|(_, p)| *p)
}

/// List-price cost of `t` on `model`, or None when the model has no known price.
pub fn cost(model: &str, t: &Tokens) -> Option<f64> {
    let p = price(model)?;
    let m = |n: u64, per: f64| n as f64 * per / 1_000_000.0;
    Some(m(t.input, p.input) + m(t.output, p.output) + m(t.cache_read, p.cache_read) + m(t.cache_write_5m, p.cache_write_5m) + m(t.cache_write_1h, p.cache_write_1h))
}

/// Short name for screens: "claude-opus-5-5" -> "opus 5.5", "gpt-5.6-sol" -> "gpt-5.6 sol",
/// "MiniMax-M3[1m]" -> "minimax-m3". Reports group models by this name, so a router
/// prefix ("z-ai/glm-5.3") stays: its cost is a catalog lower bound, not a list price.
pub fn short_model(model: &str) -> String {
    let b = bare(model);
    let m = b.as_str();
    if let Some(rest) = m.strip_prefix("claude-") {
        let mut parts: Vec<&str> = rest.split('-').collect();
        // drop a trailing date like 20251001
        if parts.last().is_some_and(|p| p.len() == 8 && p.chars().all(|c| c.is_ascii_digit())) {
            parts.pop();
        }
        if let Some((family, ver)) = parts.split_first() {
            return if ver.is_empty() { family.to_string() } else { format!("{family} {}", ver.join(".")) };
        }
    }
    match m.rsplit_once('-') {
        Some((base, tail)) if m.starts_with("gpt-") && tail.chars().all(|c| c.is_ascii_alphabetic()) && base.contains('.') => format!("{base} {tail}"),
        _ => m.to_owned(),
    }
}

/// Tokens and list-price cost, split by model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub tokens: Tokens,
    /// Cost of the tokens whose model has a known price.
    pub cost: f64,
    /// Tokens left out of `cost` because their model has no known price.
    #[serde(default)]
    pub unpriced: u64,
    #[serde(default)]
    pub by_model: BTreeMap<String, ModelUsage>,
    /// Cost per local hour of the day, 0..24 (days only; empty when summed over days).
    #[serde(default)]
    pub by_hour: Vec<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub tokens: Tokens,
    pub cost: Option<f64>,
}

impl Usage {
    /// Recompute a saved aggregate from its model token counts, without raw
    /// transcripts or a model call. Incomplete legacy aggregates stay intact.
    /// Hourly costs cannot be reconstructed from per-model totals; invalidate
    /// them when prices change. Today's hourly chart is collected fresh.
    pub fn reprice(&mut self) -> bool {
        let mut accounted = Tokens::default();
        for model in self.by_model.values() {
            accounted.add(&model.tokens);
        }
        if self.by_model.is_empty() || accounted != self.tokens {
            return false;
        }
        let changed = self.by_model.iter().any(|(name, model)| {
            match (model.cost, cost(name, &model.tokens)) {
                (Some(a), Some(b)) => (a - b).abs() > 1e-9,
                (None, None) => false,
                _ => true,
            }
        });
        if !changed {
            return false;
        }
        self.cost = 0.0;
        self.unpriced = 0;
        for (name, model) in &mut self.by_model {
            model.cost = cost(name, &model.tokens);
            match model.cost {
                Some(value) => self.cost += value,
                None => self.unpriced += model.tokens.total(),
            }
        }
        self.by_hour.clear();
        true
    }

    /// One model call (or several of the same model) at local hour `hour`.
    pub fn record(&mut self, model: &str, t: &Tokens, hour: Option<usize>) {
        self.tokens.add(t);
        let c = cost(model, t);
        match c {
            Some(c) => self.cost += c,
            None => self.unpriced += t.total(),
        }
        let e = self.by_model.entry(model.to_owned()).or_default();
        e.tokens.add(t);
        e.cost = match (e.cost, c) {
            (Some(a), Some(b)) => Some(a + b),
            (None, Some(b)) if e.tokens.total() == t.total() => Some(b),
            _ => None,
        };
        if let (Some(h), Some(c)) = (hour, c) {
            if self.by_hour.len() < 24 {
                self.by_hour.resize(24, 0.0);
            }
            self.by_hour[h.min(23)] += c;
        }
    }

    pub fn merge(&mut self, o: &Usage) {
        self.tokens.add(&o.tokens);
        self.cost += o.cost;
        self.unpriced += o.unpriced;
        for (m, u) in &o.by_model {
            let e = self.by_model.entry(m.clone()).or_default();
            let fresh = e.tokens.total() == 0;
            e.tokens.add(&u.tokens);
            e.cost = match (e.cost, u.cost) {
                (Some(a), Some(b)) => Some(a + b),
                (None, Some(b)) if fresh => Some(b),
                _ => None,
            };
        }
        if !o.by_hour.is_empty() {
            if self.by_hour.len() < 24 {
                self.by_hour.resize(24, 0.0);
            }
            for (i, c) in o.by_hour.iter().enumerate().take(24) {
                self.by_hour[i] += c;
            }
        }
    }

    /// Share of input served from cache, 0..1; None with no input at all.
    pub fn cache_hit(&self) -> Option<f64> {
        let read = self.tokens.read();
        (read > 0).then(|| self.tokens.cache_read as f64 / read as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deepseek_v41_actual_model_prices_cached_input_separately() {
        let t = Tokens { input: 756_279, output: 66_905, cache_read: 14_829_940, ..Default::default() };
        assert!((cost("deepseek/deepseek-v4.1-flash", &t).expect("actual model is priced") - 0.20444027).abs() < 1e-12);
        assert_ne!(price("deepseek/deepseek-v4.1-flash"), price("deepseek/deepseek-v4-flash"));
        assert!(price("deepseek/deepseek-v4.1-flash:batch").is_none());
    }

    #[test]
    fn saved_usage_reprices_without_transcripts_and_keeps_unknown_tokens() {
        let mut saved: Usage = serde_json::from_value(json!({
            "tokens": {"input": 2_000_000, "output": 1_000_000},
            "cost": 0.0, "unpriced": 3_000_000, "by_hour": [0.0, 0.0],
            "by_model": {
                "deepseek/deepseek-v4-flash": {"tokens": {"input": 1_000_000, "output": 1_000_000}, "cost": null},
                "unknown/model": {"tokens": {"input": 1_000_000}, "cost": null}
            }
        })).unwrap();
        let original_tokens = saved.tokens;
        assert!(saved.reprice());
        assert!((saved.cost - 0.12558).abs() < 1e-12);
        assert_eq!(saved.unpriced, 1_000_000);
        assert_eq!(saved.tokens, original_tokens);
        assert_eq!(saved.by_model["unknown/model"].cost, None);
        assert!(saved.by_hour.is_empty(), "old hourly cost cannot be apportioned without per-hour model tokens");
        assert!(!saved.reprice(), "repricing is idempotent");
    }

    #[test]
    fn repricing_preserves_legacy_usage_without_complete_model_tokens() {
        let mut saved = Usage { tokens: Tokens { input: 100, ..Default::default() }, cost: 0.5, ..Default::default() };
        let before = saved.clone();
        assert!(!saved.reprice());
        assert_eq!(saved, before);
    }

    #[test]
    fn deepseek_catalog_id_prices_every_token_category() {
        let t = Tokens { input: 1_000_000, output: 1_000_000, cache_read: 1_000_000,
            cache_write_5m: 1_000_000, cache_write_1h: 1_000_000 };
        let expected = 0.04186 * 3.0 + 0.08372 + 0.008372;
        assert!((cost("deepseek/deepseek-v4-flash", &t).expect("catalog model is priced") - expected).abs() < 1e-12);
        assert!(price("deepseek/deepseek-v4-flash:free").is_none());
        assert!(price("deepseek/deepseek-v4-flash-future").is_none());
        assert!(price("other/deepseek-v4-flash").is_none());
    }

    #[test]
    fn prices_match_by_longest_prefix() {
        assert_eq!(price("claude-opus-5-5").unwrap().input, 4.0);
        assert_eq!(price("claude-opus-5").unwrap().input, 5.0);
        assert_eq!(price("claude-haiku-4-5-20251001").unwrap().output, 5.0);
        assert_eq!(price("claude-fable-5-1").unwrap().cache_read, 0.25);
        assert_eq!(price("gpt-5.6-sol").unwrap().output, 20.0);
        assert_eq!(price("gpt-5").unwrap().input, 1.25);
        assert!(price("gpt-5-codex").is_none());
        assert!(price("glm-4.6").is_none());
    }

    #[test]
    fn claude_usage_and_cost() {
        let t = Tokens::from_claude(&json!({
            "input_tokens": 1_000_000, "output_tokens": 1_000_000,
            "cache_read_input_tokens": 1_000_000, "cache_creation_input_tokens": 2_000_000,
            "cache_creation": {"ephemeral_1h_input_tokens": 1_000_000}
        }));
        assert_eq!((t.cache_write_5m, t.cache_write_1h), (1_000_000, 1_000_000));
        // opus 5.5: 4 + 20 + 0.2 + 5 + 8
        assert!((cost("claude-opus-5-5", &t).unwrap() - 37.2).abs() < 1e-9);
    }

    #[test]
    fn codex_cached_input_is_part_of_input() {
        let t = Tokens::from_codex(&json!({"input_tokens": 100, "cached_input_tokens": 80, "output_tokens": 5}));
        assert_eq!((t.input, t.cache_read, t.output), (20, 80, 5));
    }

    #[test]
    fn unknown_models_keep_tokens_out_of_cost() {
        let mut u = Usage::default();
        u.record("claude-opus-5-5", &Tokens { input: 1_000_000, ..Default::default() }, Some(9));
        u.record("glm-4.6", &Tokens { input: 500, ..Default::default() }, Some(9));
        assert!((u.cost - 4.0).abs() < 1e-9);
        assert_eq!(u.unpriced, 500);
        assert_eq!(u.by_model["glm-4.6"].cost, None);
        assert!((u.by_hour[9] - 4.0).abs() < 1e-9);
    }

    #[test]
    fn short_names() {
        assert_eq!(short_model("claude-opus-5-5"), "opus 5.5");
        assert_eq!(short_model("claude-haiku-4-5-20251001"), "haiku 4.5");
        assert_eq!(short_model("gpt-5.6-sol"), "gpt-5.6 sol");
        assert_eq!(short_model("gpt-5-codex"), "gpt-5-codex");
    }

    #[test]
    fn short_names_drop_context_suffix_and_case_but_keep_the_router() {
        assert_eq!(short_model("glm-5.3"), "glm-5.3");
        assert_eq!(short_model("glm-5.3[1m]"), "glm-5.3");
        assert_eq!(short_model("MiniMax-M3[1m]"), "minimax-m3");
        assert_eq!(short_model("kimi-k3[1m]"), "kimi-k3");
        assert_eq!(short_model("claude-opus-5-5[1m]"), "opus 5.5");
        // Routed through OpenRouter: priced differently, so not grouped with the direct id.
        assert_eq!(short_model("z-ai/glm-5.3"), "z-ai/glm-5.3");
        assert_eq!(short_model("minimax/minimax-m3"), "minimax/minimax-m3");
        assert_eq!(short_model("deepseek/deepseek-v4.1-flash"), "deepseek/deepseek-v4.1-flash");
        assert_eq!(short_model("gpt-oss:20b"), "gpt-oss:20b");
    }

    fn per_m(p: Price) -> [f64; 5] {
        [p.input, p.output, p.cache_read, p.cache_write_5m, p.cache_write_1h]
    }

    #[test]
    fn coding_models_on_other_providers_use_their_own_list_prices() {
        let p = |m: &str| per_m(price(m).unwrap_or_else(|| panic!("{m} is priced")));
        // Provider direct ids: official price pages.
        assert_eq!(p("glm-5.3"), [1.4, 4.4, 0.26, 1.4, 1.4]);
        assert_eq!(p("glm-5.3-flash"), [0.15, 0.50, 0.03, 0.15, 0.15]);
        assert_eq!(p("kimi-k3"), [3.0, 15.0, 0.30, 3.0, 6.0]);
        assert_eq!(p("kimi-k2.7-code"), [0.95, 4.0, 0.19, 0.95, 0.95]);
        assert_eq!(p("qwen3.7-max"), [2.5, 7.5, 0.25, 3.125, 3.125]);
        assert_eq!(p("minimax-m3"), [0.30, 1.20, 0.06, 0.30, 0.30]);
        // OpenRouter catalog ids: the catalog's own (different) prices.
        assert_eq!(p("z-ai/glm-5.3"), [1.4, 4.4, 0.14, 1.4, 1.4]);
        assert_eq!(p("z-ai/glm-5.3-flash"), [0.15, 0.5, 0.03, 0.15, 0.15]);
        assert_eq!(p("moonshotai/kimi-k3"), [2.7, 13.5, 0.27, 2.7, 2.7]);
        assert_eq!(p("moonshotai/kimi-k2.7-code"), [0.6712, 3.35, 0.18, 0.6712, 0.6712]);
        assert_eq!(p("qwen/qwen3.7-max"), [1.475, 4.425, 0.295, 1.84375, 1.84375]);
        assert_eq!(p("minimax/minimax-m3"), [0.3, 1.2, 0.06, 0.3, 0.3]);
        // Ids as the setup guides write them: mixed case, Claude Code's 1M-context suffix.
        assert_eq!(price("MiniMax-M3"), price("minimax-m3"));
        assert_eq!(price("MiniMax-M3[1m]"), price("minimax-m3"));
        assert_eq!(price("glm-5.3[1m]"), price("glm-5.3"));
        assert_eq!(price("kimi-k3[1m]"), price("kimi-k3"));
    }

    #[test]
    fn kimi_k3_prices_cache_writes_by_ttl() {
        let t = Tokens { cache_write_5m: 1_000_000, cache_write_1h: 1_000_000, ..Default::default() };
        assert!((cost("kimi-k3", &t).unwrap() - 9.0).abs() < 1e-9);
    }

    #[test]
    fn neighbouring_ids_do_not_inherit_a_coding_model_price() {
        for m in [
            // other tiers and variants with their own prices
            "glm-5.3-flashx", "z-ai/glm-5.3-flashx", "z-ai/glm-5.3-prime", "z-ai/glm-5.3:batch",
            "glm-5.3-flash-x", "glm-5.31", "kimi-k3:batch", "moonshotai/kimi-k3:batch",
            "kimi-k2.7-code-highspeed", "qwen3.7-max-preview", "qwen3.7-max-2026-06-08",
            "qwen3.8-max", "qwen3.7-plus", "minimax-m3.1-flash-preview", "minimax-m3-highspeed",
            // the id without its router prefix, or under another router's prefix
            "glm-5", "other/glm-5.3", "zai/glm-5.3",
            // local models: the name alone does not say where they ran
            "gpt-oss:20b", "gpt-oss:120b", "openai/gpt-oss-20b",
            // a server-picked model has no fixed price
            "auto",
        ] {
            assert!(price(m).is_none(), "{m} must stay unpriced");
        }
    }
}
