# Claude Code configuration reference

Checked on **2026-10-30** against the locally installed **Claude Code 2.1.312**.
This is a reference for composing ccs profiles, not a validation schema enforced
by ccs. ccs accepts arbitrary argument names and environment variable names;
Claude Code interprets them.

## Version and sources

- `claude --version`: `2.1.312 (Claude Code)`.
- [Complete local `claude --help` snapshot](claude-code-2.1.312-help.txt): the
  public startup options and commands advertised by this installed version.
- [Official CLI reference](https://code.claude.com/docs/en/cli-reference).
- [Official environment variable reference](https://code.claude.com/docs/en/env-vars).
- [Model configuration](https://code.claude.com/docs/en/model-config).
- [Subagent model selection](https://code.claude.com/docs/en/sub-agents#choose-a-model).
- [Gateway authentication](https://code.claude.com/docs/en/llm-gateway-connect).
- [Authentication](https://code.claude.com/docs/en/authentication).

The tables below record the variables investigated for profile composition;
they are not an exhaustive inventory of Claude Code environment variables.
Environment behavior comes from the official documentation. It was not verified
by making model requests or connecting to a gateway. Live documentation can
change independently of the installed version; minimum versions are recorded
where explicitly documented. The help snapshot is evidence of the local CLI
surface, not a guarantee of model or provider capabilities.

## Environment variables

### Endpoint and authentication

| Variable | Meaning |
| --- | --- |
| `ANTHROPIC_BASE_URL` | API endpoint, including an Anthropic-compatible gateway |
| `ANTHROPIC_AUTH_TOKEN` | Credential sent as `Authorization: Bearer <value>` |
| `ANTHROPIC_API_KEY` | Credential sent as `x-api-key: <value>` |

When both authentication variables are nonempty, `ANTHROPIC_AUTH_TOKEN` takes
precedence over `ANTHROPIC_API_KEY`. A gateway determines which mechanism it
accepts; the name of the shell variable holding a credential does not determine
the header.

ccs clears inherited `ANTHROPIC_*` before applying the merged profile. An empty
`ANTHROPIC_API_KEY` is unnecessary when no shared entry supplies a nonempty key.
An explicit empty value remains useful for overriding such a shared entry.
Other inherited variables, including `CLAUDE_CODE_*`, remain unless overridden.

### Session model and alias mappings

| Variable | Meaning |
| --- | --- |
| `ANTHROPIC_MODEL` | Session startup model; accepts an alias or full model ID |
| `ANTHROPIC_DEFAULT_MODEL` | Lower-priority default for new sessions; requires v2.1.236+ |
| `ANTHROPIC_DEFAULT_FABLE_MODEL` | Full model ID behind `fable`; also used for Fable recognition in automatic fallback on third-party providers |
| `ANTHROPIC_DEFAULT_OPUS_MODEL` | Full model ID behind `opus`, and the Plan Mode phase of `opusplan` |
| `ANTHROPIC_DEFAULT_SONNET_MODEL` | Full model ID behind `sonnet`, and the non-Plan Mode phase of `opusplan` |
| `ANTHROPIC_DEFAULT_HAIKU_MODEL` | Full model ID behind `haiku`, also used for some background functionality |

Session model selection, from highest priority to lowest:

1. `/model` during the session.
2. Startup `--model`.
3. `ANTHROPIC_MODEL`.
4. The settings-file `model` field.
5. `ANTHROPIC_DEFAULT_MODEL`, subject to organization defaults, availability,
   model restrictions, and the documented accepted values.

`ANTHROPIC_DEFAULT_MODEL` is a fallback for starting sessions, not an additional
alias mapping. The documentation excludes values including `default`, `inherit`,
`opusplan`, and `haiku` for that variable. Setting it alongside an explicit
`ANTHROPIC_MODEL` does not change the selected startup model.

`ANTHROPIC_SMALL_FAST_MODEL` is deprecated; use
`ANTHROPIC_DEFAULT_HAIKU_MODEL` instead.

### Subagents

| Variable | Meaning |
| --- | --- |
| `CLAUDE_CODE_SUBAGENT_MODEL` | Default model for subagents, agent team teammates, and workflow agents without another model selection; accepts an alias or full model ID |
| `CLAUDE_CODE_SUBAGENT_MODEL_FORCE` | Set to `"1"` with the preceding variable to override ordinary subagent model selections; requires v2.1.257+ |

Without force, ordinary subagent selection follows this priority:

1. The invocation's model, subject to any installed mod's `agent.spawn` hook.
2. The subagent definition's `model`, including `inherit`.
3. `CLAUDE_CODE_SUBAGENT_MODEL`.
4. The main conversation's model.

Before v2.1.251, the environment variable had higher priority, including over
`model: inherit`. With force enabled, Claude cannot pass a per-invocation model
and definition model fields are ignored. The documented exception is that forks
and skills running in a subagent with `model: inherit` still use the main model.
Thus force should not be described as overriding every possible child session.

To share a default auxiliary tier while allowing each profile to choose its
actual model:

```toml
[shared.gateway.env]
ANTHROPIC_BASE_URL = "http://localhost:3425"
ANTHROPIC_AUTH_TOKEN = { from_env = "GATEWAY_API_KEY" }
CLAUDE_CODE_SUBAGENT_MODEL = "haiku"

[profiles.coding]
uses = ["gateway"]

[profiles.coding.env]
ANTHROPIC_MODEL = "gateway/main-model"
ANTHROPIC_DEFAULT_HAIKU_MODEL = "gateway/auxiliary-model"
```

This sets a default. Leave force unset when invocation and agent-definition
model choices should remain effective.

### Context, compaction, output, and effort

| Variable | Meaning and constraints |
| --- | --- |
| `CLAUDE_CODE_MAX_CONTEXT_TOKENS` | Changes the context capacity Claude Code assumes; applicability depends on model ID recognition |
| `CLAUDE_CODE_AUTO_COMPACT_WINDOW` | Auto-compact window of 100000–1000000 tokens, capped by the model's context capacity; `200k`/`1M` suffixes require v2.1.304+ |
| `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` | Percentage from 1–100 for earlier compaction; cannot raise the threshold above the default; applies only to sessions that compact before the context limit, including applicable subagents |
| `CLAUDE_CODE_MAX_OUTPUT_TOKENS` | Maximum output for most requests; values above a known model's cap are lowered; larger values reduce context available before compaction |
| `CLAUDE_CODE_EFFORT_LEVEL` | `low`, `medium`, `high`, `xhigh`, or `max`, depending on the model; `auto` or `unset` omits the API effort level |
| `CLAUDE_CODE_DISABLE_1M_CONTEXT` | `"1"` disables 1M context variants and limits native 1M models to 200K, subject to the custom-ID correction rules below |

`CLAUDE_CODE_EFFORT_LEVEL` overrides `--effort`, `/effort`, `modelSettings`, and
`effortLevel`; a `maxEffortLevel` cap still applies. Avoid fixing it in shared
environment configuration when sessions should freely adjust effort.

`CLAUDE_CODE_AUTO_COMPACT_WINDOW` takes precedence over `--autocompact`,
`/autocompact`, and the settings-file window. A plain numeric environment value
is measured in tokens; use an explicit count such as `512000`.

For context-capacity correction in the current documentation:

- An unrecognized custom/provider ID without a `claude-` prefix or `[1m]`
  suffix can use `CLAUDE_CODE_MAX_CONTEXT_TOKENS` directly, while retaining
  proactive compaction.
- An unrecognized custom/provider ID with `[1m]` also needs
  `CLAUDE_CODE_DISABLE_1M_CONTEXT=1` for the capacity override to apply.
- A recognized model ID, or a bare `claude-` ID without a stripped suffix,
  only uses this override together with `DISABLE_COMPACT`, which disables
  compaction. See the official rules before using this combination.

These values describe the client's assumptions and compaction policy. They do
not increase the backend model's capacity. Configure capacity per actual model;
a shared window is appropriate only when the referenced backends support it.
Do not infer an exact compaction token count solely by multiplying the window
by `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE`.

### Custom model display and capabilities

`ANTHROPIC_CUSTOM_MODEL_OPTION` adds a custom model to the `/model` picker.
Optional `ANTHROPIC_CUSTOM_MODEL_OPTION_NAME` and
`ANTHROPIC_CUSTOM_MODEL_OPTION_DESCRIPTION` label that entry.

Each `ANTHROPIC_DEFAULT_*_MODEL` family and `ANTHROPIC_CUSTOM_MODEL_OPTION`
supports these companion suffixes:

| Suffix | Meaning |
| --- | --- |
| `_NAME` | Picker display name |
| `_DESCRIPTION` | Picker description |
| `_SUPPORTED_CAPABILITIES` | Comma-separated capability declaration |

Documented capability values are `effort`, `xhigh_effort`, `max_effort`,
`thinking`, `adaptive_thinking`, and `interleaved_thinking`. When a capability
list is set, unlisted capabilities are disabled for the matching pinned model.
Only declare features that the backend implements. Display overrides also apply
to gateways; capability applicability is documented for third-party providers.
See the official model configuration reference for provider-specific limits.

## Startup arguments

The [local help snapshot](claude-code-2.1.312-help.txt) records all public options
advertised by `claude --help`, including their aliases, values, defaults where
shown, and mode restrictions. The following are particularly useful in profiles:

| Long option | Value | Purpose |
| --- | --- | --- |
| `model` | String | Session model, overriding `ANTHROPIC_MODEL` |
| `effort` | String | `low`, `medium`, `high`, `xhigh`, `max`; model-dependent and overridden by `CLAUDE_CODE_EFFORT_LEVEL` |
| `autocompact` | String | `auto` or a token window such as `200k`; overridden by `CLAUDE_CODE_AUTO_COMPACT_WINDOW` |
| `fallback-model` | String | Model ID or comma-separated fallback model IDs |
| `permission-mode` | String | `acceptEdits`, `auto`, `bypassPermissions`, `manual`, `dontAsk`, or `plan` |
| `add-dir` | String or string array | Additional tool-access directories |
| `plugin-dir` | String or string array | Plugin directory or zip; one path per repeated flag |
| `plugin-url` | String or string array | Plugin zip URL; repeatable |
| `allowedTools` / `allowed-tools` | String | Comma- or space-separated tool entries |
| `disallowedTools` / `disallowed-tools` | String | Comma- or space-separated denied tool entries |
| `tools` | String | Comma-separated built-in tools, `default`, or an empty string to disable them |
| `agent` | String | Session agent, overriding the setting |
| `agents` | String | JSON definitions, or a file path with print mode |
| `append-system-prompt` | String | Append to the default system prompt |
| `system-prompt` | String | Replace the session system prompt |
| `system-prompt-snapshot` | String | `on` or `off`; controls prompt recording and reuse across requests and resumes |
| `settings` | String | Settings JSON path or inline JSON |
| `setting-sources` | String | Comma-separated `user`, `project`, `local` sources |
| `mcp-config` | String | MCP configuration file or JSON string |
| `strict-mcp-config` | Boolean | Use only explicitly supplied MCP configuration |
| `verbose` | Boolean | Enable verbose output |
| `print` | Boolean | Noninteractive response and exit |
| `output-format` | String | Print mode: `text`, `json`, or `stream-json` |
| `input-format` | String | Print mode: `text` or `stream-json` |
| `json-schema` | String | Print-mode structured output JSON schema |
| `max-budget-usd` | String | Print-mode API budget, represented as a string in ccs |
| `continue` | Boolean | Continue the latest conversation in this directory |
| `resume` | String, or Boolean for picker | Resume by session ID/name or open the picker |
| `fork-session` | Boolean | Create a new session ID when continuing or resuming |
| `name` | String | Session display name |
| `worktree` | String, or Boolean for automatic name | Create a git worktree |
| `bare` | Boolean | Minimal startup; changes customization loading and authentication behavior |
| `safe-mode` | Boolean | Disable customizations for troubleshooting |
| `restricted` | Boolean | Restrict tools, settings sources, and file access |

Use either spelling of aliased options consistently; ccs merges by the literal
TOML key and does not normalize aliases. For options whose help says
`<values...>`, ccs arrays emit repeated flags rather than one flag followed by
many values. Use documented repeatable flags such as `plugin-dir` for arrays;
use a single accepted list string for tool lists. Pass other multi-value forms
on the command line when needed.

For example:

```toml
[shared.common.args]
permission-mode = "plan"
plugin-dir = ["./plugins/review", "./plugins/tools"]

[profiles.review]
uses = ["common"]

[profiles.review.args]
model = "opus"
effort = "high"
```

ccs renders strings as `--key=value`, `true` as `--key`, and arrays as repeated
`--key=value` arguments. `false` and empty arrays emit nothing; they can clear a
shared ccs entry but do not negate a Claude setting. Numeric-looking values,
including budgets, must be TOML strings. Prompt text and Claude subcommands belong
on the invocation command line rather than in the structured argument table.

Final process arguments are the merged profile arguments followed by the
unchanged invocation arguments. ccs does not implement Claude Code's handling
of duplicate options. See [the project configuration guide](../README.md#configuration)
for merge, environment inheritance, and argument rendering rules.

## Refreshing this reference

When checking another version, run `claude --version` and `claude --help`, save
its help under a matching versioned filename, and update this page's date and
links. Recheck the official model, environment, and CLI documentation for changed
precedence and minimum versions. Do not infer support from variable names alone.
