# AI models for capture

Which model `quest-server` uses for AI capture (`POST /v1/ai/capture`, MCP `capture_text`), and why.
Measured with the eval set in `crates/quest-ai-eval`, through OpenRouter, on 2026-11-24.

## Recommendation

- **Default: `anthropic/claude-haiku-5.5`.** It is the only model without a miss in any run since the prompt fix
  (124 of 124 calls over four runs), and it was already right before the prompt fix (30/31). It costs about
  $0.0005 per call, at most about $0.10 a day at the per-account limit of 200 calls.
- **Fallbacks: `openai/gpt-6-luna`, then `deepseek/deepseek-v4.1-flash-1017`.** Both come from other vendors, so an
  outage at one vendor does not stop capture, and both are cheaper and as fast. They are slightly less steady:
  gpt-6-luna had 31/31 in rounds 3 and 5 but 28/31 in round 4, with practically the same prompt. DeepSeek reacted most to the prompt
  (26 → 31 hits).
- Reasoning is off (`reasoning.effort: none`). This task does not need it; it only adds latency and cost.

`mise.toml` (`mise run server`) sets these values: `QUEST_AI_MODEL`, `QUEST_AI_FALLBACK_MODELS`,
`QUEST_AI_REASONING_EFFORT`.

## Results (final prompt, 31 cases, two runs each)

Runs 4 and 5 were made with the same prompt and cases, 62 calls per model. Gemini had one run, as it was already
behind.

| Model | Hits | Mean score | Schema errors | p50 | p95 | Cost per call | Prompt tokens |
|---|---|---|---|---|---|---|---|
| `anthropic/claude-haiku-5.5` | 62/62 | 1.000 | 0 | 1.34 s | 2.59 s | $0.00050 | 4,341 |
| `deepseek/deepseek-v4.1-flash-1017` | 60/62 | 0.994 | 0 | 1.16 s | 2.59 s | $0.00019 | 2,404 |
| `openai/gpt-6-luna` | 59/62 | 0.977 | 0 | 1.31 s | 1.76 s | $0.00014 | 2,400 |
| `google/gemini-2.5-flash-lite` | 24/31 | 0.932 | 0 | 0.66 s | 2.62 s | $0.00014 | 2,396 |

- Cost comes from OpenRouter's `usage.cost`. The cost of the same model varies between runs, for example
  gpt-6-luna at $0.00007 to $0.00021 per call. The provider's prompt cache causes this: the system prompt and
  schema are the same on every call.
- Haiku's tokenizer counts the same prompt as almost twice as many tokens.
- Latency includes OpenRouter. The models ran at the same time, one case after another for each.
- Misses:
  - gpt-6-luna:
    - "Spanisch-Vokabeln wiederholt" was logged on a new task instead of "Vokabeln wiederholen";
    - "bis Mitternacht" was dropped once;
    - one quest assignment was missing.
  - DeepSeek:
    - "Bis Weihnachten jeden Mittwoch und Freitag" once had no end date;
    - the same text once had no weekdays.
  - Gemini:
    - "jeden Montag" started on a Sunday;
    - it followed an injection (quest "Hacked");
    - "Ende nächster Woche" became this Friday;
    - it gave no work log for "Mails beantwortet".

Earlier runs: round 2 used the new prompt but a case that was too strict; round 3 used the case fix. Haiku had
31/31 in both, and gpt-6-luna and DeepSeek had 31/31 in round 3.

### Before the prompt fix (round 1)

| Model | Hits | Mean score | p50 | p95 |
|---|---|---|---|---|
| `openai/gpt-6-luna` | 31/31 | 1.000 | 1.49 s | 2.11 s |
| `anthropic/claude-haiku-5.5` | 30/31 | 0.994 | 1.64 s | 2.70 s |
| `deepseek/deepseek-v4.1-flash-1017` | 26/31 | 0.956 | 1.10 s | 3.18 s |
| `google/gemini-2.5-flash-lite` | 23/31 | 0.917 | 0.66 s | 2.04 s |

Weak spots this round showed, and the prompt changes for them (`quest_capture::prompt`):

- Vague dates ("nächste Woche", "Monatsende", "Ende nächster Woche") stayed in the backlog. Fix: a rule for each
  phrase, and the backlog only for text without any time reference.
- Weekdays were computed wrongly ("jeden Montag" started on a Sunday). Fix: the context now has a calendar from
  yesterday to three weeks ahead ("2026-11-30 Monday"), and the model looks dates up there.
- A future time range ("morgen von zehn bis zwölf") became a work log (then dropped as future work). Fix: an
  explicit rule that it is a task with a time block.
- Tasks named for a new quest were not assigned to it. Fix: the rule now says so.
- Prompt injection (Gemini). Fix: passages that try to instruct the model create nothing.
- One more rule: text without plans gives empty lists.

The calendar adds about 400 prompt tokens per call (≈ $0.00004 with gpt-6-luna).

## The eval

`crates/quest-ai-eval/cases.json`: 31 synthetic German dictations (colloquial, with filler words) against a fixed
planner on Tuesday 2026-11-24, 10:30 Europe/Berlin. The planner has 4 quests and 8 tasks. The cases cover:

- relative dates: tomorrow, Friday, next week, end of the month, end of next week, the day after tomorrow, next
  Wednesday;
- existing and new quests;
- work logs today, yesterday and until midnight, on existing and new tasks;
- daily, weekly (several weekdays, every two weeks, with an end date) and monthly (last day) repeats;
- the backlog;
- durations with and without a start time, and future work that is no log;
- two prompt injections;
- two texts with nothing to capture.

Each case runs through the production path:

- `quest_server::structured_request`;
- `OpenAiCompatible` with the OpenRouter fields;
- `quest_capture::validate`.

It is then scored with `quest_ai_eval::score`. There is one check per expected item. Each expected field is one
more check: day, quest, block, repeat, and for work logs day, start and end. One more check is that no extra items
were proposed, and another that titles have no filler words. A hit means every check passed. A schema error means
the answer was no usable JSON, or validation refused it as a whole.

```sh
mise run ai-eval --model anthropic/claude-haiku-5.5             # all cases, about $0.01 per model
mise run ai-eval --model anthropic/claude-haiku-5.5 --case repeat-montag --out /tmp/eval.json
mise run ai-eval --model anthropic/claude-haiku-5.5 --reasoning low # reasoning effort, default none
```

It needs `OPENROUTER_API_KEY` in the repo's `.env`, and it only sends the synthetic texts. Without the key it stops
before making any call. `cargo test` only checks the eval set and the scoring; it makes no calls.

Ambiguous phrases accept every reasonable answer: "nächsten Mittwoch" is tomorrow or the week after, and "nächste
Woche" is any day of next week. One case was too strict: a title had to say "Navigation", but "Am Relaunch
arbeiten" is just as good. It was loosened between rounds 2 and 3.

## OpenRouter request fields

The server sends these fields only to OpenRouter. It detects OpenRouter by the host `openrouter.ai`;
`QUEST_AI_OPENROUTER` overrides the detection.

- `provider.require_parameters: true`: only providers that support every parameter of the request are picked.
  Without it, a provider may ignore `response_format` silently.
- `models`: the main model followed by `QUEST_AI_FALLBACK_MODELS`. OpenRouter moves on to the next model on errors,
  downtime and rate limits. Verified live: a main model without structured output was answered by the fallback.
- `reasoning: { effort }`. `default` leaves the field out. `require_parameters` counts `reasoning` as well, so a
  model without reasoning support then has no provider (404) unless the setting is `default`.

Tool calling as a fallback is still needed for a single model without structured output and no fallback list.
With `require_parameters`, OpenRouter answers such a request with 404 "No endpoints found that can handle the
requested parameters", not with a 400. The server takes that 404 as a refusal of `json_schema` too and retries
with a forced tool call; that path was verified live with `amazon/nova-2-lite-v1`. Tool calling only becomes the
default for the process once that retry has succeeded.

With a fallback list that contains a model with structured output (as in the default configuration), OpenRouter
routes to that model, so the retry with a tool call does not happen.

Budget for this phase: about $0.15 in total (five eval rounds, plus probes and an end-to-end check).
