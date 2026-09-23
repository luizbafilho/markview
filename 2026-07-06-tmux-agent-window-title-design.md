# Design: LLM-generated tmux window titles for agent sessions

**Date:** 2026-07-06
**Status:** Approved for implementation
**Context:** dotfiles repo (`tmux`, Pi, OpenCode, Claude Code)

## Goal

Agent windows in tmux currently keep generic command names such as `opencode`, `node`, or `pi`. When several agent sessions are open, the window list does not say what each agent is doing.

Add a hook-driven title updater that renames the current tmux window from the latest user prompt. The title should be a short, LLM-generated, kebab-case label such as `fix-tmux-theme`, `commit-changes`, or `review-pr`.

The OpenRouter API key must live in one place: `~/.secrets` as `OPENROUTER_API_KEY`. The new title updater reads that variable.

## Decisions

| Area | Decision |
| --- | --- |
| Display target | Rename the actual tmux window, not a status badge or pane title. |
| Title source | Direct OpenRouter chat completion call. |
| Title model | `deepseek/deepseek-v4-flash` for tmux titles. |
| Key source | `OPENROUTER_API_KEY` from `~/.secrets`. No Keychain lookup. |
| Format | Lowercase kebab-case, 1-3 words, ASCII letters and digits only. |
| Timing | Generate on new user prompt / submitted input. |
| Failure behavior | Hook-facing commands enqueue work, exit 0 quickly, and never surface OpenRouter failures to the agent. If no key exists, the API call fails, or the title fails validation, do not rename. |
| Persistence | A generated title remains until the next prompt changes it. |

## Architecture

```
Pi extension ───────┐
OpenCode plugin ────┼── latest user prompt ─▶ bin/tmux-agent-title ─▶ OpenRouter
Claude hook ────────┘                                  │
                                                       ▼
                                            tmux rename-window <title>
```

`bin/tmux-agent-title` owns all shell, OpenRouter, validation, and tmux logic. Hook-facing paths only enqueue a background worker and exit 0; the worker performs the OpenRouter request and rename. Harness integrations stay thin: they pass the latest user prompt to the script and ignore failures so title generation never disrupts the agent.

## Secret handling

`~/.secrets` becomes the only source of the OpenRouter key:

```sh
export OPENROUTER_API_KEY="..."
```

The title updater sources that file before calling OpenRouter. No other file stores or derives the key.

## Title generation script

New file: `bin/tmux-agent-title`.

Inputs:

- Reads the prompt from stdin.
- Requires `$TMUX_PANE`; outside tmux it exits 0 with no action.
- Reads `OPENROUTER_API_KEY` from the environment after sourcing `~/.secrets`.
- Writes the prompt to a private temp file, starts a background worker, and exits 0 before any OpenRouter request starts.

Worker behavior:

- Reads and deletes the temp prompt file.
- Performs the OpenRouter request with stdout/stderr suppressed.
- Exits 0 for all operational failures.

OpenRouter request:

- URL: `https://openrouter.ai/api/v1/chat/completions`
- Model: `TMUX_AGENT_TITLE_MODEL`, default `deepseek/deepseek-v4-flash`
- Small token cap, temperature 0
- Prompt asks for exactly one tmux window title:
  - 1-3 words
  - lowercase kebab-case
  - no quotes, punctuation, or explanation
  - examples: `fix-tmux-theme`, `commit-changes`, `review-pr`

Validation:

- Trim whitespace.
- Accept only `^[a-z0-9]+(-[a-z0-9]+){0,2}$`.
- Reject empty output and anything longer than 40 characters.

Rename:

```sh
tmux set-option -w -t "$TMUX_PANE" automatic-rename off
tmux rename-window -t "$TMUX_PANE" "$title"
```

The script does not restore automatic rename. Agent-generated names are intentional session labels and should stay visible until the next prompt.

## Harness integrations

### Pi

Add `pi/extensions/tmux-window-title.ts`.

- Listen on `before_agent_start`, using `event.prompt` as the title input.
- Skip subagent child processes (`PI_SUBAGENT_CHILD=1`).
- Fire-and-forget the prompt to `bin/tmux-agent-title`; do not wait for the title script or OpenRouter.
- Ignore script failures.

### OpenCode

Add `opencode/plugins/tmux-window-title.ts`.

- Use the `chat.message` hook.
- Convert `output.parts` to prompt text with the same text-part extraction logic.
- Fire-and-forget that text to `bin/tmux-agent-title`; do not wait for the title script or OpenRouter.
- Ignore script failures.

### Claude Code

Add a command hook to `UserPromptSubmit` in `claude/config/base.json`.

- The hook receives Claude's JSON on stdin.
- `bin/tmux-agent-title claude-hook` extracts `.prompt`, enqueues the worker, and exits 0 immediately.
- The existing `tmux-attention claude-hook` remains in place.

## Documentation and bootstrap changes

Update:

- `docs/bootstrap.md`

Docs should tell the operator to add this once:

```sh
printf '%s\n' 'export OPENROUTER_API_KEY="<openrouter-key>"' >> ~/.secrets
```

## Testing

Add tests for:

- `bin/tmux-agent-title` validation accepts only 1-3 word kebab-case titles.
- Missing `OPENROUTER_API_KEY` makes the script exit without renaming.
- Invalid model output does not call `tmux rename-window`.
- Valid model output disables automatic rename and renames the current window.
- Pi/OpenCode plugins shell out only inside tmux and ignore subagent child processes.
- Claude hook extracts the prompt from representative hook JSON.

Run the existing project checks after implementation:

```sh
make test
```

For focused checks during development:

```sh
cd pi && bun test tests/*.test.ts
cd opencode/plugins && bun test *.test.ts
bats tests/tmux-agent-title.bats
```

## Risks

- Hook payload shapes differ across harnesses. Implementation should inspect real event payloads already used by the existing attention hooks before wiring each producer.
- OpenRouter latency happens only in the background worker. A late rename is acceptable because it labels the window by the user's latest prompt.
- A bad model response must not produce ugly tmux names. Strict validation prevents that without adding a non-LLM title fallback.
