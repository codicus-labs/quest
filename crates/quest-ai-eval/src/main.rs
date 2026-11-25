//! Runs the capture eval set against a real model and prints a score per case and a summary.
//!
//! `mise run ai-eval --model <id>` (the key comes from `OPENROUTER_API_KEY` in the repo's `.env`).
//! Each case is one paid provider call; the summary shows what the run cost.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono_tz::Tz;
use clap::Parser;
use quest_ai_eval::cases::{Case, EvalSet};
use quest_ai_eval::score::{Score, score};
use quest_ai_eval::stats::percentile;
use quest_server::{
    LlmProvider, OpenAiCompatible, OpenAiSettings, OpenRouterOptions, ProviderError, ReasoningEffort, Usage,
    is_openrouter, structured_request,
};
use serde_json::json;

/// Scores AI capture on synthetic dictations against a real model.
#[derive(Parser)]
struct Args {
    /// Model id as the provider spells it, e.g. anthropic/claude-haiku-5.5.
    #[arg(long)]
    model: String,
    /// OpenAI-compatible API root.
    #[arg(long, env = "QUEST_AI_BASE_URL", default_value = "https://openrouter.ai/api/v1")]
    base_url: String,
    /// OpenRouter: reasoning effort (none, minimal, low, medium, high, default).
    #[arg(long, default_value = "none")]
    reasoning: ReasoningEffort,
    /// Only these cases (by id); all when left out.
    #[arg(long = "case")]
    cases: Vec<String>,
    /// Also write every result as JSON to this file.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Seconds per call.
    #[arg(long, default_value_t = 60)]
    timeout_secs: u64,
}

enum Outcome {
    Scored(Score),
    /// The answer is no usable JSON, or validation refused it as a whole.
    SchemaError,
    Failed(ProviderError),
}

struct CaseResult<'a> {
    case: &'a Case,
    outcome: Outcome,
    latency: Duration,
    usage: Option<Usage>,
    dropped: usize,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    // The OpenRouter key only ever goes to OpenRouter.
    let key_name = if is_openrouter(&args.base_url) { "OPENROUTER_API_KEY" } else { "QUEST_AI_API_KEY" };
    let Some(api_key) = std::env::var(key_name).ok().filter(|key| !key.trim().is_empty()) else {
        eprintln!("ai-eval: set {key_name} (mise run ai-eval loads OPENROUTER_API_KEY from the repo's .env)");
        return ExitCode::FAILURE;
    };
    let set = match EvalSet::load() {
        Ok(set) => set,
        Err(error) => {
            eprintln!("ai-eval: cases.json is invalid: {error}");
            return ExitCode::FAILURE;
        }
    };
    let provider = match provider(&args, api_key) {
        Ok(provider) => provider,
        Err(error) => {
            eprintln!("ai-eval: {error}");
            return ExitCode::FAILURE;
        }
    };
    let selected: Vec<&Case> =
        set.cases.iter().filter(|case| args.cases.is_empty() || args.cases.contains(&case.id)).collect();
    println!("model {} · {} cases · reasoning {:?}", args.model, selected.len(), args.reasoning);
    let timeout = Duration::from_secs(args.timeout_secs.max(1));
    let mut results = Vec::new();
    for case in selected {
        let result = run_case(&set, case, provider.as_ref(), timeout).await;
        print_case(&result);
        results.push(result);
    }
    let summary = summary(&args, &results);
    println!("\n{}", serde_json::to_string_pretty(&summary).unwrap_or_default());
    if let Some(path) = &args.out {
        let cases: Vec<_> = results.iter().map(case_json).collect();
        let report = json!({ "summary": summary, "cases": cases });
        if let Err(error) = std::fs::write(path, serde_json::to_string_pretty(&report).unwrap_or_default()) {
            eprintln!("ai-eval: cannot write {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn provider(args: &Args, api_key: String) -> Result<Arc<dyn LlmProvider>, String> {
    let openrouter = OpenRouterOptions::for_base_url(&args.base_url, None, vec![], args.reasoning)
        .map_err(|error| error.to_string())?;
    let settings = OpenAiSettings {
        base_url: args.base_url.clone(),
        api_key,
        model: args.model.clone(),
        extra_headers: vec![("X-Title".into(), "Quest eval".into())],
        allow_insecure_http: false,
        openrouter,
    };
    Ok(Arc::new(OpenAiCompatible::new(settings).map_err(|error| error.to_string())?))
}

async fn run_case<'a>(set: &EvalSet, case: &'a Case, provider: &dyn LlmProvider, timeout: Duration) -> CaseResult<'a> {
    let request = set.request(case);
    let time_zone: Tz = quest_capture::check_request(&request).expect("cases.json is checked by the tests");
    let structured = structured_request(&request, time_zone);
    let started = Instant::now();
    let completion = tokio::time::timeout(timeout, provider.complete_structured(&structured))
        .await
        .unwrap_or(Err(ProviderError::Timeout));
    let latency = started.elapsed();
    let (outcome, usage, dropped) = match completion {
        Err(ProviderError::InvalidResponse) => (Outcome::SchemaError, None, 0),
        Err(error) => (Outcome::Failed(error), None, 0),
        Ok(completion) => match quest_capture::validate(&completion.value, &request, time_zone) {
            Ok(proposal) => (Outcome::Scored(score(&case.expect, &proposal)), completion.usage, proposal.dropped.len()),
            Err(_) => (Outcome::SchemaError, completion.usage, 0),
        },
    };
    CaseResult { case, outcome, latency, usage, dropped }
}

fn print_case(result: &CaseResult) {
    let cost = result.usage.and_then(|usage| usage.cost).map_or("-".to_owned(), |cost| format!("${cost:.5}"));
    let verdict = match &result.outcome {
        Outcome::Scored(score) if score.is_hit() => format!("HIT  {}/{}", score.passed, score.total),
        Outcome::Scored(score) => format!("miss {}/{}", score.passed, score.total),
        Outcome::SchemaError => "SCHEMA ERROR".to_owned(),
        Outcome::Failed(error) => format!("FAILED ({error})"),
    };
    let dropped = if result.dropped > 0 { format!(" dropped={}", result.dropped) } else { String::new() };
    println!("{:<26} {:<14} {:>6} ms {cost}{dropped}", result.case.id, verdict, result.latency.as_millis());
    if let Outcome::Scored(score) = &result.outcome {
        for failure in &score.failures {
            println!("    - {failure}");
        }
    }
}

fn summary(args: &Args, results: &[CaseResult]) -> serde_json::Value {
    let scores: Vec<&Score> = results
        .iter()
        .filter_map(|r| match &r.outcome {
            Outcome::Scored(score) => Some(score),
            _ => None,
        })
        .collect();
    let hits = scores.iter().filter(|score| score.is_hit()).count();
    let points: f64 = scores.iter().map(|s| f64::from(s.passed) / f64::from(s.total.max(1))).sum();
    let count = |matches: fn(&Outcome) -> bool| results.iter().filter(|r| matches(&r.outcome)).count();
    let latencies: Vec<Duration> = results.iter().map(|r| r.latency).collect();
    let costs: Vec<f64> = results.iter().filter_map(|r| r.usage.and_then(|u| u.cost)).collect();
    let total_cost: f64 = costs.iter().sum();
    let millis = |fraction| percentile(&latencies, fraction).map(|d| d.as_millis() as u64);
    json!({
        "model": args.model,
        "reasoning": format!("{:?}", args.reasoning),
        "cases": results.len(),
        "hits": hits,
        "mean_score": if results.is_empty() { 0.0 } else { points / results.len() as f64 },
        "schema_errors": count(|o| matches!(o, Outcome::SchemaError)),
        "provider_errors": count(|o| matches!(o, Outcome::Failed(_))),
        "dropped_items": results.iter().map(|r| r.dropped).sum::<usize>(),
        "latency_p50_ms": millis(0.5),
        "latency_p95_ms": millis(0.95),
        "cost_total_usd": total_cost,
        "cost_per_call_usd": if costs.is_empty() { 0.0 } else { total_cost / costs.len() as f64 },
        "prompt_tokens_per_call": average(results.iter().filter_map(|r| r.usage.map(|u| u.prompt_tokens))),
        "completion_tokens_per_call": average(results.iter().filter_map(|r| r.usage.map(|u| u.completion_tokens))),
    })
}

fn average(values: impl Iterator<Item = u64>) -> u64 {
    let (sum, count) = values.fold((0, 0), |(sum, count), value| (sum + value, count + 1));
    sum.checked_div(count).unwrap_or(0)
}

fn case_json(result: &CaseResult) -> serde_json::Value {
    let (verdict, passed, total, failures) = match &result.outcome {
        Outcome::Scored(score) => ("scored", score.passed, score.total, score.failures.clone()),
        Outcome::SchemaError => ("schema_error", 0, 0, vec![]),
        Outcome::Failed(error) => ("failed", 0, 0, vec![error.to_string()]),
    };
    json!({
        "id": result.case.id,
        "verdict": verdict,
        "passed": passed,
        "total": total,
        "failures": failures,
        "latency_ms": result.latency.as_millis() as u64,
        "cost_usd": result.usage.and_then(|u| u.cost),
        "dropped": result.dropped,
    })
}
