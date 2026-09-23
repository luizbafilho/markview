# Jira Cross-Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Pi and Claude Code the same `gh pr create` → Jira-ticket reminder behavior OpenCode has, and give Pi first-class `acli`-backed Jira tools, by extracting shared dependency-free cores consumed through thin per-harness adapters.

**Architecture:** Two pure cores in `shared/lib/` (`jira-core.ts` for `acli` operations, `jira-pr-ticket.ts` for PR detection + the generic reminder). OpenCode reaches them via committed source symlinks that survive its copy-and-flatten bootstrap; Pi (symlinked extensions) and Claude (`bin/` script) reach them by repo-relative realpath imports. Each harness keeps its native hook and schema system; only the `acli` runner and parameter schemas live in adapters.

**Tech Stack:** TypeScript run under Bun (OpenCode plugins/tools, tests) and Pi's extension runtime; `acli` (Atlassian CLI); zsh bootstrap (`mise/tasks/bootstrap/{opencode,pi,claude}`); `bun:test`; `typebox` (Pi tool schemas); `@opencode-ai/plugin` (OpenCode tool schemas).

## Global Constraints

- **NO FALLBACKS.** Add no fallback/legacy/back-compat branches. Implement only what each task specifies.
- **Gating is runtime.** OpenCode: `process.env.OPENCODE_CLIENT === "doximity"`. Pi: `process.env.PI_CLIENT === "doximity"`. Claude: basename of `process.env.CLAUDE_CONFIG_DIR` === `"doximity"`.
- **Cores are dependency-free.** They may import only Node builtins (`node:fs`, `node:os`, `node:path`) and nothing harness-specific. Process spawning is injected, never done inside a core.
- **Allowlist (verbatim):** `["agentic-runtime", "agentic-workspaces"]`.
- **Sprint custom field (verbatim):** `customfield_10020`. **Sprint id (verbatim):** `11180`. **Jira project (verbatim):** `AGENTIC`. **Jira site (verbatim):** `https://doximity.atlassian.net`.
- **Fail-safe.** Any error inside a PR-ticket adapter is swallowed so the agent is never disrupted. The Claude shim exits 0 on any error.
- **Verification before "done":** run `cd opencode && bunx tsc --noEmit`, `cd opencode/plugins && bunx tsc --noEmit`, and `make test`. Fix all errors before claiming completion.
- **Commit style:** subject states intent; body explains *why*. Include the updated `docs/superpowers/` files in the commit that changes them. Use `git add -f` for `docs/superpowers/` paths (globally gitignored, but prior specs/plans are tracked).
- **Pre-work hygiene:** before editing any file, re-read it (context may be stale).

---

## Task 1: Jira core — validators and argv builders

**Files:**
- Create: `shared/lib/jira-core.ts`
- Test: `shared/lib/jira-core.test.ts`

**Interfaces:**
- Consumes: nothing (pure).
- Produces:
  - `const SPRINT_FIELD_ID = "customfield_10020"`
  - `type AcliRunner = (args: string[]) => Promise<string>` — resolves to acli stdout; rejects on non-zero exit.
  - `type CreateWorkItemArgs = { project: string; type: string; summary: string; description?: string; parent?: string; labels?: string[]; sprint?: number; assignee?: string; status?: string }`
  - `type SearchArgs = { jql: string; fields?: string; limit?: number; paginate?: boolean }`
  - `type JiraResult = { title: string; output: string; metadata: Record<string, unknown> }`
  - `function assertKey(key: string): void` — throws `Error("key must be a Jira work item key like PROJ-123")` unless `/^[A-Z][A-Z0-9]+-[0-9]+$/`.
  - `function assertProject(project: string): void` — throws `Error("project must be a Jira project key like PROJ")` unless `/^[A-Z][A-Z0-9]+$/`.
  - `function parseJson(input: string): unknown` — `input.trim() ? JSON.parse(input) : null`.
  - `function toAdf(text: string): object` — ADF doc with one paragraph.
  - `function jsonResult(title: string, data: unknown): JiraResult`
  - `function buildViewArgs(key: string, fields: string): string[]`
  - `function buildCommentListArgs(key: string): string[]`
  - `function buildCommentCreateArgs(key: string, body: string): string[]`
  - `function buildCreateArgs(args: CreateWorkItemArgs): string[]` — the non-sprint create path (no `--from-json`).
  - `function buildTransitionArgs(key: string, status: string): string[]`
  - `function buildSearchArgs(args: SearchArgs): string[]`

- [ ] **Step 1: Write the failing test**

```ts
// shared/lib/jira-core.test.ts
import { describe, expect, it } from "bun:test"
import {
  assertKey,
  assertProject,
  buildCreateArgs,
  buildSearchArgs,
  buildTransitionArgs,
  buildViewArgs,
  jsonResult,
  parseJson,
  toAdf,
} from "./jira-core"

describe("jira-core validators", () => {
  it("accepts a valid key and rejects a bad one", () => {
    expect(() => assertKey("AGENTIC-123")).not.toThrow()
    expect(() => assertKey("bad key")).toThrow(/PROJ-123/)
  })

  it("accepts a valid project and rejects a bad one", () => {
    expect(() => assertProject("AGENTIC")).not.toThrow()
    expect(() => assertProject("Agentic-1")).toThrow(/PROJ/)
  })
})

describe("jira-core argv builders", () => {
  it("builds a view argv with fields and --json", () => {
    expect(buildViewArgs("AGENTIC-1", "key,summary")).toEqual([
      "jira", "workitem", "view", "AGENTIC-1", "--fields", "key,summary", "--json",
    ])
  })

  it("builds a create argv from optional fields in declaration order", () => {
    expect(
      buildCreateArgs({ project: "AGENTIC", type: "Task", summary: "S", description: "D", assignee: "@me" }),
    ).toEqual([
      "jira", "workitem", "create", "--project", "AGENTIC", "--type", "Task",
      "--summary", "S", "--description", "D", "--assignee", "@me", "--json",
    ])
  })

  it("builds a transition argv", () => {
    expect(buildTransitionArgs("AGENTIC-2", "In Progress")).toEqual([
      "jira", "workitem", "transition", "--key", "AGENTIC-2", "--status", "In Progress", "--yes", "--json",
    ])
  })

  it("builds a search argv with optional flags", () => {
    expect(buildSearchArgs({ jql: "project = AGENTIC", fields: "key", limit: 5, paginate: true })).toEqual([
      "jira", "workitem", "search", "--jql", "project = AGENTIC",
      "--fields", "key", "--limit", "5", "--paginate", "--json",
    ])
  })

  it("parses json and shapes a result", () => {
    expect(parseJson("")).toBeNull()
    expect(parseJson('{"a":1}')).toEqual({ a: 1 })
    const r = jsonResult("T", { a: 1 })
    expect(r.title).toBe("T")
    expect(JSON.parse(r.output)).toEqual({ a: 1 })
    expect(toAdf("hi")).toMatchObject({ type: "doc", version: 1 })
  })
})
```

- [ ] **Step 2: Run the test, expect failure**

Run: `cd /Users/luiz/.dotfiles && bun test shared/lib/jira-core.test.ts`
Expected: FAIL — `Cannot find module './jira-core'`.

- [ ] **Step 3: Implement `shared/lib/jira-core.ts`**

```ts
export const SPRINT_FIELD_ID = "customfield_10020"

export type AcliRunner = (args: string[]) => Promise<string>

export type CreateWorkItemArgs = {
  project: string
  type: string
  summary: string
  description?: string
  parent?: string
  labels?: string[]
  sprint?: number
  assignee?: string
  status?: string
}

export type SearchArgs = {
  jql: string
  fields?: string
  limit?: number
  paginate?: boolean
}

export type JiraResult = {
  title: string
  output: string
  metadata: Record<string, unknown>
}

const keyPattern = /^[A-Z][A-Z0-9]+-[0-9]+$/
const projectPattern = /^[A-Z][A-Z0-9]+$/

export function assertKey(key: string): void {
  if (!keyPattern.test(key)) throw new Error("key must be a Jira work item key like PROJ-123")
}

export function assertProject(project: string): void {
  if (!projectPattern.test(project)) throw new Error("project must be a Jira project key like PROJ")
}

export function parseJson(input: string): unknown {
  return input.trim() ? JSON.parse(input) : null
}

export function toAdf(text: string): object {
  return { type: "doc", version: 1, content: [{ type: "paragraph", content: [{ type: "text", text }] }] }
}

export function jsonResult(title: string, data: unknown): JiraResult {
  return { title, output: JSON.stringify(data, null, 2), metadata: data as Record<string, unknown> }
}

export function buildViewArgs(key: string, fields: string): string[] {
  return ["jira", "workitem", "view", key, "--fields", fields, "--json"]
}

export function buildCommentListArgs(key: string): string[] {
  return ["jira", "workitem", "comment", "list", "--key", key, "--paginate", "--json"]
}

export function buildCommentCreateArgs(key: string, body: string): string[] {
  return ["jira", "workitem", "comment", "create", "--key", key, "--body", body, "--json"]
}

export function buildCreateArgs(args: CreateWorkItemArgs): string[] {
  const argv = ["jira", "workitem", "create", "--project", args.project, "--type", args.type, "--summary", args.summary]
  if (args.description) argv.push("--description", args.description)
  if (args.parent) argv.push("--parent", args.parent)
  if (args.labels) for (const label of args.labels) argv.push("--label", label)
  if (args.assignee) argv.push("--assignee", args.assignee)
  argv.push("--json")
  return argv
}

export function buildTransitionArgs(key: string, status: string): string[] {
  return ["jira", "workitem", "transition", "--key", key, "--status", status, "--yes", "--json"]
}

export function buildSearchArgs(args: SearchArgs): string[] {
  const argv = ["jira", "workitem", "search", "--jql", args.jql]
  if (args.fields) argv.push("--fields", args.fields)
  if (args.limit !== undefined) argv.push("--limit", String(args.limit))
  if (args.paginate) argv.push("--paginate")
  argv.push("--json")
  return argv
}
```

- [ ] **Step 4: Run the test, expect pass**

Run: `cd /Users/luiz/.dotfiles && bun test shared/lib/jira-core.test.ts`
Expected: PASS (all assertions).

- [ ] **Step 5: Commit**

```bash
cd /Users/luiz/.dotfiles
git add shared/lib/jira-core.ts shared/lib/jira-core.test.ts
git commit -m "Add dependency-free Jira acli core for cross-harness reuse

OpenCode's Jira tools wrap acli with argv building, validation, and JSON
parsing. Extracting that logic into a pure, runtime-neutral core lets Pi
reuse it without duplicating the acli command surface, and keeps the only
per-harness difference (schema system, process spawn) in thin adapters."
```

---

## Task 2: Jira core — high-level operations with injected runner

**Files:**
- Modify: `shared/lib/jira-core.ts`
- Modify: `shared/lib/jira-core.test.ts`

**Interfaces:**
- Consumes: everything from Task 1.
- Produces:
  - `async function fetchContext(run: AcliRunner, key: string): Promise<JiraResult>`
  - `async function postComment(run: AcliRunner, key: string, body: string): Promise<JiraResult>`
  - `async function createWorkItem(run: AcliRunner, args: CreateWorkItemArgs): Promise<JiraResult>`
  - `async function searchWorkItems(run: AcliRunner, args: SearchArgs): Promise<JiraResult>`
  - These reproduce the current OpenCode tool behavior exactly: `fetchContext` views fields `key,issuetype,summary,status,assignee,description,comment` then lists comments and returns `{ key, work_item, comments }`; `createWorkItem` uses the `--from-json` sprint path when `sprint !== undefined` (writing a temp file via `node:fs`/`node:os`/`node:path`), else `buildCreateArgs`, then optionally transitions, then views `key,issuetype,summary,status,assignee` and returns `{ created, transitioned, verified }`.

- [ ] **Step 1: Add failing operation tests**

Append to `shared/lib/jira-core.test.ts`:

```ts
import { createWorkItem, fetchContext, postComment, searchWorkItems } from "./jira-core"

const fakeRunner = (responses: Record<string, string>) => {
  const calls: string[][] = []
  const run = async (args: string[]) => {
    calls.push(args)
    const key = args.join(" ")
    if (!(key in responses)) throw new Error("unexpected acli call: " + key)
    return responses[key]
  }
  return { run, calls }
}

describe("jira-core operations", () => {
  it("fetches work item context with comments", async () => {
    const { run } = fakeRunner({
      "jira workitem view AGENTIC-1 --fields key,issuetype,summary,status,assignee,description,comment --json":
        JSON.stringify({ key: "AGENTIC-1" }),
      "jira workitem comment list --key AGENTIC-1 --paginate --json": JSON.stringify([{ body: "hi" }]),
    })
    const result = await fetchContext(run, "AGENTIC-1")
    const data = JSON.parse(result.output)
    expect(data.work_item.key).toBe("AGENTIC-1")
    expect(data.comments).toEqual([{ body: "hi" }])
  })

  it("posts a comment", async () => {
    const { run } = fakeRunner({
      "jira workitem comment create --key AGENTIC-1 --body Looks good --json": JSON.stringify({ id: "1" }),
    })
    const result = await postComment(run, "AGENTIC-1", "Looks good")
    expect(JSON.parse(result.output).id).toBe("1")
  })

  it("creates, transitions, and verifies a work item (no sprint)", async () => {
    const { run, calls } = fakeRunner({
      "jira workitem create --project AGENTIC --type Task --summary S --description D --assignee @me --json":
        JSON.stringify({ key: "AGENTIC-9" }),
      "jira workitem transition --key AGENTIC-9 --status In Progress --yes --json":
        JSON.stringify({ key: "AGENTIC-9" }),
      "jira workitem view AGENTIC-9 --fields key,issuetype,summary,status,assignee --json":
        JSON.stringify({ key: "AGENTIC-9" }),
    })
    const result = await createWorkItem(run, {
      project: "AGENTIC", type: "Task", summary: "S", description: "D", assignee: "@me", status: "In Progress",
    })
    const data = JSON.parse(result.output)
    expect(data.created.key).toBe("AGENTIC-9")
    expect(data.verified.key).toBe("AGENTIC-9")
    expect(calls.some((c) => c[2] === "transition")).toBe(true)
  })

  it("searches work items", async () => {
    const { run } = fakeRunner({
      "jira workitem search --jql project = AGENTIC --json": JSON.stringify({ issues: [] }),
    })
    const result = await searchWorkItems(run, { jql: "project = AGENTIC" })
    expect(JSON.parse(result.output).issues).toEqual([])
  })
})
```

- [ ] **Step 2: Run, expect failure**

Run: `cd /Users/luiz/.dotfiles && bun test shared/lib/jira-core.test.ts`
Expected: FAIL — operations not exported.

- [ ] **Step 3: Implement operations in `shared/lib/jira-core.ts`**

Add imports at the top of the file and the operations at the bottom:

```ts
import { rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

async function createViaJson(run: AcliRunner, args: CreateWorkItemArgs): Promise<unknown> {
  const payload: Record<string, unknown> = {
    projectKey: args.project,
    type: args.type,
    summary: args.summary,
    additionalAttributes: { [SPRINT_FIELD_ID]: args.sprint },
  }
  if (args.description) payload.description = toAdf(args.description)
  if (args.labels) payload.labels = args.labels
  if (args.parent) payload.parentIssueId = args.parent
  if (args.assignee) payload.assignee = args.assignee
  const file = join(tmpdir(), `acli-workitem-${Date.now()}-${Math.random().toString(36).slice(2)}.json`)
  await writeFile(file, JSON.stringify(payload))
  try {
    return parseJson(await run(["jira", "workitem", "create", "--from-json", file, "--json"]))
  } finally {
    await rm(file, { force: true })
  }
}

export async function fetchContext(run: AcliRunner, key: string): Promise<JiraResult> {
  assertKey(key)
  const workItem = parseJson(await run(buildViewArgs(key, "key,issuetype,summary,status,assignee,description,comment")))
  const comments = parseJson(await run(buildCommentListArgs(key)))
  return jsonResult("Jira context", { key, work_item: workItem, comments })
}

export async function postComment(run: AcliRunner, key: string, body: string): Promise<JiraResult> {
  assertKey(key)
  const created = parseJson(await run(buildCommentCreateArgs(key, body)))
  return jsonResult("Jira comment posted", created)
}

export async function createWorkItem(run: AcliRunner, args: CreateWorkItemArgs): Promise<JiraResult> {
  assertProject(args.project)
  if (args.parent) assertKey(args.parent)
  const created: any = args.sprint !== undefined ? await createViaJson(run, args) : parseJson(await run(buildCreateArgs(args)))
  const key = created?.key
  if (typeof key !== "string") throw new Error("acli did not return a work item key")
  const transitioned = args.status ? parseJson(await run(buildTransitionArgs(key, args.status))) : undefined
  const verified = parseJson(await run(buildViewArgs(key, "key,issuetype,summary,status,assignee")))
  return jsonResult("Jira work item created", { created, transitioned, verified })
}

export async function searchWorkItems(run: AcliRunner, args: SearchArgs): Promise<JiraResult> {
  const result = parseJson(await run(buildSearchArgs(args)))
  return jsonResult("Jira search results", result)
}
```

- [ ] **Step 4: Run, expect pass**

Run: `cd /Users/luiz/.dotfiles && bun test shared/lib/jira-core.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/luiz/.dotfiles
git add shared/lib/jira-core.ts shared/lib/jira-core.test.ts
git commit -m "Add Jira core operations parameterized by an injected acli runner

Dependency injection keeps the core runtime-neutral: OpenCode supplies a
Bun.spawn runner, Pi supplies a pi.exec runner, and the operations stay
identical. Mirrors the exact acli call sequence the OpenCode tools use today
so the refactor in the next task is behavior-preserving."
```

---

## Task 3: OpenCode jira tool refactor + committed symlink

**Files:**
- Create (symlink): `opencode/tool/shared/jira-core.ts` → `../../../shared/lib/jira-core.ts`
- Modify: `opencode/tool/shared/jira.ts`
- Test: `opencode/tool/tool.test.ts` (must keep passing unchanged)

**Interfaces:**
- Consumes: `shared/lib/jira-core.ts` operations + `JiraResult`.
- Produces: unchanged tool exports `fetch_context`, `post_comment`, `create_work_item`, `search` with identical observable output.

- [ ] **Step 1: Create the committed symlink and confirm it resolves**

```bash
cd /Users/luiz/.dotfiles
ln -s ../../../shared/lib/jira-core.ts opencode/tool/shared/jira-core.ts
test -f opencode/tool/shared/jira-core.ts && echo "resolves" || echo "BROKEN"
```
Expected: `resolves`.

- [ ] **Step 2: Run existing Jira tool tests to capture the green baseline**

Run: `cd /Users/luiz/.dotfiles/opencode && bun test tool/tool.test.ts`
Expected: PASS (baseline before refactor).

- [ ] **Step 3: Rewrite `opencode/tool/shared/jira.ts` to delegate to the core**

Replace the entire file with the adapter below. It keeps the `tool.schema.*` parameter
schemas and a `Bun.spawn` runner, delegating all logic to the core:

```ts
import { tool } from "@opencode-ai/plugin"
import { type AcliRunner, createWorkItem, fetchContext, postComment, searchWorkItems } from "./jira-core"

const makeRunner = (cwd: string): AcliRunner => async (args) => {
  const proc = Bun.spawn(["acli", ...args], { cwd, env: process.env, stdout: "pipe", stderr: "pipe" })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) throw new Error((stderr || stdout || `acli exited with ${exitCode}`).trim())
  return stdout.trimEnd()
}

export const fetch_context = tool({
  description: "Fetch Jira work item fields and comments for planning, grilling, and issue-slicing workflows.",
  args: { key: tool.schema.string().describe("Jira work item key") },
  async execute(args, context) {
    return fetchContext(makeRunner(context.directory), args.key)
  },
})

export const post_comment = tool({
  description: "Post a plain-text or ADF comment to a Jira work item.",
  args: {
    key: tool.schema.string().describe("Jira work item key"),
    body: tool.schema.string().min(1).describe("Comment body"),
  },
  async execute(args, context) {
    return postComment(makeRunner(context.directory), args.key, args.body)
  },
})

export const create_work_item = tool({
  description: "Create and verify a Jira work item using acli.",
  args: {
    project: tool.schema.string().describe("Jira project key"),
    type: tool.schema.string().min(1).describe("Jira work item type, such as Story, Task, or Bug"),
    summary: tool.schema.string().min(1).describe("Work item summary"),
    description: tool.schema.string().optional().describe("Work item description"),
    parent: tool.schema.string().optional().describe("Optional parent work item key"),
    labels: tool.schema.array(tool.schema.string()).optional().describe("Optional labels"),
    sprint: tool.schema.number().int().positive().optional().describe("Optional sprint id to place the work item in"),
    assignee: tool.schema.string().optional().describe("Optional assignee email/account ID, or @me for the current user"),
    status: tool.schema.string().optional().describe("Optional status to transition the created work item to"),
  },
  async execute(args, context) {
    return createWorkItem(makeRunner(context.directory), args)
  },
})

export const search = tool({
  description: "Search Jira work items with a JQL query using acli.",
  args: {
    jql: tool.schema.string().min(1).describe("JQL query, e.g. project = AGENTIC AND statusCategory != Done"),
    fields: tool.schema.string().optional().describe("Comma-separated fields to return, e.g. key,summary,status"),
    limit: tool.schema.number().int().positive().optional().describe("Maximum number of work items to fetch"),
    paginate: tool.schema.boolean().optional().describe("Fetch all pages of results"),
  },
  async execute(args, context) {
    return searchWorkItems(makeRunner(context.directory), args)
  },
})
```

- [ ] **Step 4: Run tests + typecheck, expect pass**

Run: `cd /Users/luiz/.dotfiles/opencode && bun test tool/tool.test.ts && bunx tsc --noEmit`
Expected: PASS (the existing Jira tool tests are unchanged and still green); tsc clean.

- [ ] **Step 5: Commit**

```bash
cd /Users/luiz/.dotfiles
git add opencode/tool/shared/jira.ts opencode/tool/shared/jira-core.ts
git commit -m "Refactor OpenCode Jira tools onto the shared acli core

The tool bodies now delegate to shared/lib/jira-core via a committed symlink
that survives OpenCode's copy-and-flatten bootstrap (the tool registry imports
the file, finds no tool exports, and skips it). Behavior is unchanged; the
logic is now shared with the upcoming Pi adapter instead of duplicated."
```

---

## Task 4: Pi Jira tools adapter

**Files:**
- Create: `pi/extensions/jira-tools.ts`
- Create: `pi/tests/jira-tools.test.ts`

**Interfaces:**
- Consumes: `shared/lib/jira-core.ts` operations (`../../shared/lib/jira-core`).
- Produces: a default extension function registering Pi tools `fetch_context`, `post_comment`, `create_work_item`, `search`, each backed by a `pi.exec("acli", ...)` runner. Export a pure helper `makeRunner(exec)` for unit testing the runner-to-core wiring without a live Pi.

**Reference:** read `pi/extensions/tmux-attention.ts` for the `ExtensionAPI` import and `pi.exec` usage, and the Pi docs custom-tools section (`docs/extensions.md`, `pi.registerTool`, `typebox` `Type.Object`, `StringEnum` from `@earendil-works/pi-ai` for enums — not needed here since these tools use only strings/numbers/booleans/arrays).

- [ ] **Step 1: Write the failing test**

```ts
// pi/tests/jira-tools.test.ts
import { describe, expect, it } from "bun:test"
import { makeRunner } from "../extensions/jira-tools"

describe("pi jira-tools runner", () => {
  it("returns trimmed stdout from pi.exec", async () => {
    const exec = async (_cmd: string, args: string[]) => {
      expect(args).toEqual(["jira", "workitem", "view", "AGENTIC-1", "--fields", "key", "--json"])
      return { stdout: '{"key":"AGENTIC-1"}\n', stderr: "", exitCode: 0 }
    }
    const run = makeRunner(exec as any)
    expect(await run(["jira", "workitem", "view", "AGENTIC-1", "--fields", "key", "--json"])).toBe('{"key":"AGENTIC-1"}')
  })

  it("throws on non-zero exit", async () => {
    const exec = async () => ({ stdout: "", stderr: "boom", exitCode: 1 })
    const run = makeRunner(exec as any)
    await expect(run(["jira", "workitem", "view", "X", "--json"])).rejects.toThrow(/boom/)
  })
})
```

- [ ] **Step 2: Run, expect failure**

Run: `cd /Users/luiz/.dotfiles/pi && bun test tests/jira-tools.test.ts`
Expected: FAIL — cannot find `../extensions/jira-tools`.

- [ ] **Step 3: Implement `pi/extensions/jira-tools.ts`**

```ts
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent"
import { Type } from "typebox"
import {
  type AcliRunner,
  createWorkItem,
  fetchContext,
  postComment,
  searchWorkItems,
} from "../../shared/lib/jira-core"

type ExecResult = { stdout: string; stderr: string; exitCode: number }
type Exec = (command: string, args: string[], options?: { signal?: AbortSignal; timeout?: number }) => Promise<ExecResult>

export const makeRunner = (exec: Exec, signal?: AbortSignal): AcliRunner => async (args) => {
  const result = await exec("acli", args, { signal, timeout: 60000 })
  if (result.exitCode !== 0) {
    throw new Error((result.stderr || result.stdout || `acli exited with ${result.exitCode}`).trim())
  }
  return result.stdout.trimEnd()
}

export default function jiraTools(pi: ExtensionAPI) {
  const run = makeRunner((command, args, options) => pi.exec(command, args, options))

  pi.registerTool({
    name: "fetch_context",
    description: "Fetch Jira work item fields and comments for planning, grilling, and issue-slicing workflows.",
    parameters: Type.Object({ key: Type.String({ description: "Jira work item key" }) }),
    async execute(_id, params) {
      return fetchContext(run, params.key)
    },
  })

  pi.registerTool({
    name: "post_comment",
    description: "Post a plain-text or ADF comment to a Jira work item.",
    parameters: Type.Object({
      key: Type.String({ description: "Jira work item key" }),
      body: Type.String({ description: "Comment body" }),
    }),
    async execute(_id, params) {
      return postComment(run, params.key, params.body)
    },
  })

  pi.registerTool({
    name: "create_work_item",
    description: "Create and verify a Jira work item using acli.",
    parameters: Type.Object({
      project: Type.String({ description: "Jira project key" }),
      type: Type.String({ description: "Jira work item type, such as Story, Task, or Bug" }),
      summary: Type.String({ description: "Work item summary" }),
      description: Type.Optional(Type.String({ description: "Work item description" })),
      parent: Type.Optional(Type.String({ description: "Optional parent work item key" })),
      labels: Type.Optional(Type.Array(Type.String(), { description: "Optional labels" })),
      sprint: Type.Optional(Type.Number({ description: "Optional sprint id to place the work item in" })),
      assignee: Type.Optional(Type.String({ description: "Optional assignee email/account ID, or @me" })),
      status: Type.Optional(Type.String({ description: "Optional status to transition the created work item to" })),
    }),
    async execute(_id, params) {
      return createWorkItem(run, params)
    },
  })

  pi.registerTool({
    name: "search",
    description: "Search Jira work items with a JQL query using acli.",
    parameters: Type.Object({
      jql: Type.String({ description: "JQL query, e.g. project = AGENTIC AND statusCategory != Done" }),
      fields: Type.Optional(Type.String({ description: "Comma-separated fields to return" })),
      limit: Type.Optional(Type.Number({ description: "Maximum number of work items to fetch" })),
      paginate: Type.Optional(Type.Boolean({ description: "Fetch all pages of results" })),
    }),
    async execute(_id, params) {
      return searchWorkItems(run, params)
    },
  })
}
```

NOTE: confirm the exact `pi.registerTool` execute signature and `pi.exec` return shape against `docs/extensions.md` (sections `pi.registerTool(definition)` and `pi.exec(command, args, options?)`) before finalizing; adjust parameter destructuring/result fields to match the installed Pi version. Do not add fallbacks — match the real API.

- [ ] **Step 4: Run, expect pass**

Run: `cd /Users/luiz/.dotfiles/pi && bun test tests/jira-tools.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/luiz/.dotfiles
git add pi/extensions/jira-tools.ts pi/tests/jira-tools.test.ts
git commit -m "Add Pi Jira tools backed by the shared acli core

Pi previously had no Jira capability and fell back to acli-via-Bash. Register
the same tool set OpenCode exposes (fetch_context, post_comment,
create_work_item, search) over the shared core via a pi.exec runner, giving Pi
first-class Jira parity so the PR-ticket reminder can treat both harnesses the
same way."
```

---

## Task 5: Jira skill — Pi path and PR-linking workflow

**Files:**
- Modify: `shared/skills/jira/SKILL.md`

**Interfaces:** documentation only; no code.

- [ ] **Step 1: Re-read the skill**

Run: `cat /Users/luiz/.dotfiles/shared/skills/jira/SKILL.md`

- [ ] **Step 2: Add the Pi path and the PR-linking workflow**

Edit the `## Harness Paths` list to add a Pi entry identical in spirit to OpenCode:

```markdown
- **OpenCode path**: Use the Jira custom tools.
- **Pi path**: Use the Jira custom tools.
- **Claude Code path**: Use Bash with `acli`. Check `acli --help` and the relevant Jira subcommand help before mutating, because exact command names and flags can vary by installed version.
```

Append a new workflow under `## Common Workflows`:

```markdown
- PR work-item linking (triggered by the PR-ticket reminder): a pull request was created on
  an allowlisted repo. Ensure a single AGENTIC work item covers it and is linked.
  1. Search `project = AGENTIC AND sprint = 11180 AND statusCategory != Done ORDER BY key ASC`
     and compare each summary against the PR's intent (branch, diff, repository) — do not
     rely on keyword matching alone.
  2. If no open item already covers the work, create one: project `AGENTIC`, an appropriate
     type, a concise summary, a 2-4 sentence description that references the PR URL, sprint
     11180, assignee `@me`, status `In Progress`.
  3. Link the PR via Atlassian's GitHub convention: edit the PR title to include the key,
     e.g. `AGENTIC-123: <existing title>`.
  4. Add one Jira link to the PR description, e.g.
     `Jira: https://doximity.atlassian.net/browse/KEY`. Do not add a separate PR comment.
  5. Never create duplicate tickets.
```

- [ ] **Step 3: Verify the file reads coherently**

Run: `cat /Users/luiz/.dotfiles/shared/skills/jira/SKILL.md`
Expected: Pi path present; PR-linking workflow present; no contradictions.

- [ ] **Step 4: Commit**

```bash
cd /Users/luiz/.dotfiles
git add shared/skills/jira/SKILL.md
git commit -m "Document the Pi Jira path and the PR work-item linking workflow

Pi now has first-class Jira tools, so its path matches OpenCode. Folding the
PR-ticket policy (search the sprint, create only if uncovered, link via title +
one description link, no comment, no duplicates) into the skill lets the
PR-ticket reminder stay a short, harness-agnostic pointer instead of embedding
tool-specific instructions in three adapters."
```

---

## Task 6: PR-ticket detection core

**Files:**
- Create: `shared/lib/jira-pr-ticket.ts`
- Create: `shared/lib/jira-pr-ticket.test.ts`
- Delete: `opencode/plugins/jira-pr-ticket.test.ts` (its single assertion is superseded here)

**Interfaces:**
- Consumes: nothing (pure).
- Produces:
  - `const ALLOWLIST: string[]` = `["agentic-runtime", "agentic-workspaces"]`
  - `function matchesPrCreate(command: string): boolean`
  - `function extractPrUrl(output: string): string | undefined`
  - `function parseRepoName(url: string): string | undefined`
  - `function buildMessage(prUrl: string): string`

- [ ] **Step 1: Write the failing test**

```ts
// shared/lib/jira-pr-ticket.test.ts
import { describe, expect, it } from "bun:test"
import { ALLOWLIST, buildMessage, extractPrUrl, matchesPrCreate, parseRepoName } from "./jira-pr-ticket"

describe("matchesPrCreate", () => {
  it("matches a bare gh pr create", () => {
    expect(matchesPrCreate("gh pr create --fill")).toBe(true)
  })
  it("matches through env assignments, env, and command prefixes", () => {
    expect(matchesPrCreate("FOO=1 env BAR=2 command gh pr create")).toBe(true)
  })
  it("matches in a later shell segment", () => {
    expect(matchesPrCreate("git push && gh pr create --web")).toBe(true)
  })
  it("does not match unrelated commands", () => {
    expect(matchesPrCreate("gh pr list")).toBe(false)
    expect(matchesPrCreate("echo gh pr create")).toBe(false)
  })
})

describe("extractPrUrl / parseRepoName", () => {
  const out = "Creating pull request\nhttps://github.com/doximity/agentic-runtime/pull/42\n"
  it("extracts the PR URL", () => {
    expect(extractPrUrl(out)).toBe("https://github.com/doximity/agentic-runtime/pull/42")
  })
  it("parses the repo name", () => {
    expect(parseRepoName("https://github.com/doximity/agentic-runtime/pull/42")).toBe("agentic-runtime")
  })
  it("returns undefined when no URL is present", () => {
    expect(extractPrUrl("no url here")).toBeUndefined()
  })
})

describe("ALLOWLIST + buildMessage", () => {
  it("contains exactly the two agentic repos", () => {
    expect(ALLOWLIST).toEqual(["agentic-runtime", "agentic-workspaces"])
  })
  it("builds a generic reminder that points at the jira skill and the URL", () => {
    const msg = buildMessage("https://github.com/doximity/agentic-runtime/pull/42")
    expect(msg).toContain("https://github.com/doximity/agentic-runtime/pull/42")
    expect(msg).toContain("AGENTIC")
    expect(msg).toContain("jira")
    expect(msg).toContain("<system-reminder>")
    expect(msg).not.toContain("jira_search")
    expect(msg).not.toContain("gh pr comment")
    expect(msg).not.toContain("customfield")
  })
})
```

- [ ] **Step 2: Run, expect failure**

Run: `cd /Users/luiz/.dotfiles && bun test shared/lib/jira-pr-ticket.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `shared/lib/jira-pr-ticket.ts`**

```ts
export const ALLOWLIST = ["agentic-runtime", "agentic-workspaces"]

const PR_URL_PATTERN = /https:\/\/github\.com\/([A-Za-z0-9._-]+)\/([A-Za-z0-9._-]+)\/pull\/\d+/
const SHELL_SEPARATOR = /\s*(?:&&|\|\||[;|\n])\s*/
const ENV_ASSIGNMENT = /^[A-Za-z_][A-Za-z0-9_]*=/

export function matchesPrCreate(command: string): boolean {
  return command.split(SHELL_SEPARATOR).some((segment) => {
    const words = segment.trim().split(/\s+/).filter(Boolean)
    let index = 0
    while (ENV_ASSIGNMENT.test(words[index] ?? "")) index += 1
    if (words[index] === "env") {
      index += 1
      while (ENV_ASSIGNMENT.test(words[index] ?? "")) index += 1
    }
    if (words[index] === "command") index += 1
    return words[index] === "gh" && words[index + 1] === "pr" && words[index + 2] === "create"
  })
}

export function extractPrUrl(output: string): string | undefined {
  return output.match(PR_URL_PATTERN)?.[0]
}

export function parseRepoName(url: string): string | undefined {
  return url.match(PR_URL_PATTERN)?.[2]
}

export function buildMessage(prUrl: string): string {
  return [
    "<system-reminder>",
    `A pull request was just created: ${prUrl}.`,
    "Before continuing, ensure a Jira work item exists for this work in the AGENTIC project and is linked to the PR.",
    "Follow the jira skill for the correct path on this harness (PR work-item linking workflow).",
    "Do not create duplicate tickets.",
    "</system-reminder>",
  ].join("\n")
}
```

- [ ] **Step 4: Delete the superseded OpenCode plugin test**

```bash
cd /Users/luiz/.dotfiles
git rm opencode/plugins/jira-pr-ticket.test.ts
```

- [ ] **Step 5: Run, expect pass**

Run: `cd /Users/luiz/.dotfiles && bun test shared/lib/jira-pr-ticket.test.ts`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cd /Users/luiz/.dotfiles
git add shared/lib/jira-pr-ticket.ts shared/lib/jira-pr-ticket.test.ts
git commit -m "Extract PR-ticket detection into a shared, dependency-free core

Move the gh-pr-create matcher, PR URL extraction, repo allowlist, and the
reminder builder out of the OpenCode plugin so Pi and Claude adapters can reuse
them. The message is now generic and points at the jira skill rather than
naming OpenCode-only tools, so one string serves all three harnesses. The old
plugin's single buildMessage test is replaced by broader core coverage."
```

---

## Task 7: OpenCode PR-ticket plugin onto the shared core + committed symlink + bootstrap copy

**Files:**
- Create (symlink): `opencode/plugins/lib/jira-pr-ticket-core.ts` → `../../../shared/lib/jira-pr-ticket.ts`
- Modify: `opencode/plugins/jira-pr-ticket.ts`
- Modify: `mise/tasks/bootstrap/opencode`

**Interfaces:**
- Consumes: `./lib/jira-pr-ticket-core` (the symlinked detection core).
- Produces: unchanged `JiraPrTicket` plugin behavior (appends `buildMessage(url)` to bash tool output for allowlisted PRs in the doximity Context, deduped by URL).

- [ ] **Step 1: Create the committed symlink and confirm it resolves**

```bash
cd /Users/luiz/.dotfiles
mkdir -p opencode/plugins/lib
ln -s ../../../shared/lib/jira-pr-ticket.ts opencode/plugins/lib/jira-pr-ticket-core.ts
test -f opencode/plugins/lib/jira-pr-ticket-core.ts && echo "resolves" || echo "BROKEN"
```
Expected: `resolves`.

- [ ] **Step 2: Rewrite `opencode/plugins/jira-pr-ticket.ts` to import the core**

```ts
import type { Plugin } from "@opencode-ai/plugin"
import { ALLOWLIST, buildMessage, extractPrUrl, matchesPrCreate, parseRepoName } from "./lib/jira-pr-ticket-core"

export const JiraPrTicket: Plugin = async () => {
  const handled = new Set<string>()

  return {
    "tool.execute.after": async (input, output) => {
      if (process.env.OPENCODE_CLIENT !== "doximity") return
      if (input.tool !== "bash") return
      const command: string = input.args?.command ?? ""
      if (!matchesPrCreate(command)) return
      const prUrl = extractPrUrl(output.output ?? "")
      if (!prUrl) return
      const repoName = parseRepoName(prUrl)
      if (!repoName || !ALLOWLIST.includes(repoName)) return
      if (handled.has(prUrl)) return
      handled.add(prUrl)
      output.output = `${output.output}\n\n${buildMessage(prUrl)}`
    },
  }
}
```

- [ ] **Step 3: Extend the bootstrap plugin-copy loop to vendor `plugins/lib/`**

Re-read `mise/tasks/bootstrap/opencode` first. Find the plugin-copy loop (copies
`"$plugins_src"/*.ts` into `$plugins_dst`, skipping `*.test.ts`). Immediately after that
loop, add a nested copy for the `lib/` subdirectory:

```zsh
	if [[ -d "$plugins_src/lib" ]]; then
		ensure_dir "$plugins_dst/lib"
		setopt local_options null_glob
		for lib_file in "$plugins_src/lib"/*.ts; do
			[[ "${lib_file:t}" == *.test.ts ]] && continue
			copy_file_if_changed "$lib_file" "$plugins_dst/lib/${lib_file:t}"
		done
	fi
```

(The plugin discovery glob is top-level `{plugin,plugins}/*.{ts,js}`, so files under
`plugins/lib/` are never auto-loaded as plugins. `copy_file_if_changed` follows the
symlink and writes real file content.)

- [ ] **Step 4: Typecheck the plugins package + dry-run the bootstrap**

Run:
```bash
cd /Users/luiz/.dotfiles/opencode/plugins && bunx tsc --noEmit
cd /Users/luiz/.dotfiles && mise bootstrap:opencode --dry-run --yes 2>&1 | rg -i "plugins/lib|jira-pr-ticket" || true
```
Expected: tsc clean. (Dry-run output is best-effort confirmation the new branch is reached; the copy itself is exercised by a real `mise run bootstrap:opencode` in Task 9.)

- [ ] **Step 5: Commit**

```bash
cd /Users/luiz/.dotfiles
git add opencode/plugins/jira-pr-ticket.ts opencode/plugins/lib/jira-pr-ticket-core.ts mise/tasks/bootstrap/opencode
git commit -m "Point the OpenCode PR-ticket plugin at the shared detection core

The plugin imports the core through a committed symlink in a plugins/lib/
subdirectory, which OpenCode's top-level-only plugin scan never auto-loads, and
a new bootstrap copy step vendors that subdir into each Context home. Behavior
is unchanged; the detection logic is now shared with Pi and Claude."
```

---

## Task 8: Pi PR-ticket extension + Claude PostToolUse shim

**Files:**
- Create: `pi/extensions/jira-pr-ticket.ts`
- Create: `bin/jira-pr-ticket-hook`
- Create: `pi/tests/jira-pr-ticket.test.ts`
- Modify: `claude/config/base.json`

**Interfaces:**
- Consumes: `shared/lib/jira-pr-ticket.ts` (Pi via `../../shared/lib/...`, Claude via `../shared/lib/...`).
- Produces: a Pi extension that appends the reminder to a matching bash `tool_result`; a Claude `bin` shim that prints the `additionalContext` JSON. Export a pure `decide(command, output)` helper from the Claude shim for testing.

**Reference:** read `pi/extensions/tmux-attention.ts` for `pi.on(...)` registration; confirm the `tool_result` event field names (command + result text) against `docs/extensions.md` (`tool_result` event under Tool Events). Confirm the Claude `PostToolUse` stdin keys (`tool_name`, `tool_input.command`, `tool_response`) and the `hookSpecificOutput.additionalContext` output shape against `https://code.claude.com/docs/en/hooks.md`.

- [ ] **Step 1: Write the failing Pi extension test**

```ts
// pi/tests/jira-pr-ticket.test.ts
import { describe, expect, it } from "bun:test"
import { reminderFor } from "../extensions/jira-pr-ticket"

describe("pi jira-pr-ticket reminderFor", () => {
  it("returns a reminder for an allowlisted PR created via gh", () => {
    const msg = reminderFor("gh pr create --fill", "https://github.com/doximity/agentic-runtime/pull/7")
    expect(msg).toContain("agentic-runtime/pull/7")
    expect(msg).toContain("<system-reminder>")
  })
  it("returns undefined for a non-allowlisted repo", () => {
    expect(reminderFor("gh pr create", "https://github.com/doximity/other-repo/pull/7")).toBeUndefined()
  })
  it("returns undefined when the command is not gh pr create", () => {
    expect(reminderFor("gh pr list", "https://github.com/doximity/agentic-runtime/pull/7")).toBeUndefined()
  })
})
```

- [ ] **Step 2: Run, expect failure**

Run: `cd /Users/luiz/.dotfiles/pi && bun test tests/jira-pr-ticket.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement `pi/extensions/jira-pr-ticket.ts`**

```ts
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent"
import { ALLOWLIST, buildMessage, extractPrUrl, matchesPrCreate, parseRepoName } from "../../shared/lib/jira-pr-ticket"

// Pure decision helper: returns the reminder to append, or undefined.
export function reminderFor(command: string, output: string): string | undefined {
  if (!matchesPrCreate(command)) return undefined
  const prUrl = extractPrUrl(output)
  if (!prUrl) return undefined
  const repoName = parseRepoName(prUrl)
  if (!repoName || !ALLOWLIST.includes(repoName)) return undefined
  return buildMessage(prUrl)
}

export default function jiraPrTicket(pi: ExtensionAPI) {
  const handled = new Set<string>()

  pi.on("tool_result", (event) => {
    try {
      if (process.env.PI_CLIENT !== "doximity") return
      if (event.toolName !== "bash") return
      const command: string = event.args?.command ?? ""
      const text: string = typeof event.result === "string" ? event.result : (event.result?.output ?? "")
      const message = reminderFor(command, text)
      if (!message) return
      const prUrl = extractPrUrl(text)!
      if (handled.has(prUrl)) return
      handled.add(prUrl)
      return { result: `${text}\n\n${message}` }
    } catch {
      // Reminder injection must never disturb pi.
    }
  })
}
```

NOTE: the `tool_result` event field names (`toolName`, `args.command`, `result`/`result.output`) and the modify-return shape (`{ result }`) MUST be reconciled with `docs/extensions.md` and the running Pi version before finalizing. Adjust field access and the returned object to the real event contract; keep the pure `reminderFor` unchanged. No fallbacks.

- [ ] **Step 4: Run the Pi test, expect pass**

Run: `cd /Users/luiz/.dotfiles/pi && bun test tests/jira-pr-ticket.test.ts`
Expected: PASS.

- [ ] **Step 5: Write the failing Claude shim test**

```ts
// pi/tests/jira-pr-ticket-claude.test.ts  (placed here so make test's pi bun runner picks it up)
import { describe, expect, it } from "bun:test"
import { decide } from "../../bin/jira-pr-ticket-hook"

describe("claude jira-pr-ticket-hook decide", () => {
  it("emits additionalContext for an allowlisted PR", () => {
    const out = decide("Bash", { command: "gh pr create" }, "https://github.com/doximity/agentic-workspaces/pull/3")
    expect(out?.hookSpecificOutput?.hookEventName).toBe("PostToolUse")
    expect(out?.hookSpecificOutput?.additionalContext).toContain("agentic-workspaces/pull/3")
  })
  it("returns undefined for non-Bash tools", () => {
    expect(decide("Edit", { command: "gh pr create" }, "https://github.com/doximity/agentic-workspaces/pull/3")).toBeUndefined()
  })
  it("returns undefined for non-allowlisted repos", () => {
    expect(decide("Bash", { command: "gh pr create" }, "https://github.com/doximity/nope/pull/3")).toBeUndefined()
  })
})
```

- [ ] **Step 6: Run, expect failure**

Run: `cd /Users/luiz/.dotfiles/pi && bun test tests/jira-pr-ticket-claude.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 7: Implement `bin/jira-pr-ticket-hook`**

```ts
#!/usr/bin/env bun
import { ALLOWLIST, buildMessage, extractPrUrl, matchesPrCreate, parseRepoName } from "../shared/lib/jira-pr-ticket"

type Decision = { hookSpecificOutput: { hookEventName: "PostToolUse"; additionalContext: string } }

// Pure decision: returns the PostToolUse JSON to print, or undefined to stay silent.
export function decide(toolName: string, toolInput: { command?: string }, toolResponse: string): Decision | undefined {
  if (toolName !== "Bash") return undefined
  const command = toolInput?.command ?? ""
  if (!matchesPrCreate(command)) return undefined
  const prUrl = extractPrUrl(toolResponse)
  if (!prUrl) return undefined
  const repoName = parseRepoName(prUrl)
  if (!repoName || !ALLOWLIST.includes(repoName)) return undefined
  return { hookSpecificOutput: { hookEventName: "PostToolUse", additionalContext: buildMessage(prUrl) } }
}

async function main() {
  if (process.env.CLAUDE_CONFIG_DIR?.split("/").filter(Boolean).pop() !== "doximity") return
  const raw = await new Response(Bun.stdin.stream()).text()
  const payload = JSON.parse(raw) as {
    tool_name?: string
    tool_input?: { command?: string }
    tool_response?: unknown
  }
  const response = typeof payload.tool_response === "string" ? payload.tool_response : JSON.stringify(payload.tool_response ?? "")
  const decision = decide(payload.tool_name ?? "", payload.tool_input ?? {}, response)
  if (decision) process.stdout.write(JSON.stringify(decision))
}

// Only run main when executed as a script, not when imported by the test.
if (import.meta.main) {
  main().catch(() => process.exit(0))
}
```

Then make it executable:
```bash
chmod +x /Users/luiz/.dotfiles/bin/jira-pr-ticket-hook
```

NOTE: re-verify the `tool_response` type for the Bash tool and the exact `additionalContext`
delivery against the installed Claude version before finalizing. No fallbacks.

- [ ] **Step 8: Run the Claude shim test, expect pass**

Run: `cd /Users/luiz/.dotfiles/pi && bun test tests/jira-pr-ticket-claude.test.ts`
Expected: PASS.

- [ ] **Step 9: Register the hook in `claude/config/base.json`**

Re-read the file, then add a `PostToolUse` entry to the existing `hooks` object (sibling of
`Notification`, `Stop`, `UserPromptSubmit`, `SessionEnd`):

```json
    "PostToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "~/.dotfiles/bin/jira-pr-ticket-hook"
          }
        ]
      }
    ]
```

- [ ] **Step 10: Validate the JSON and the generated settings merge**

Run:
```bash
cd /Users/luiz/.dotfiles
jq -e '.hooks.PostToolUse[0].matcher == "Bash"' claude/config/base.json
jq -s '.[0] * .[1]' claude/config/base.json claude/config/doximity.json | jq -e '.hooks.PostToolUse[0].hooks[0].command'
```
Expected: both `jq -e` succeed (exit 0).

- [ ] **Step 11: Commit**

```bash
cd /Users/luiz/.dotfiles
chmod +x bin/jira-pr-ticket-hook
git add pi/extensions/jira-pr-ticket.ts pi/tests/jira-pr-ticket.test.ts pi/tests/jira-pr-ticket-claude.test.ts bin/jira-pr-ticket-hook claude/config/base.json
git commit -m "Add Pi and Claude PR-ticket adapters over the shared core

Pi appends the reminder to a matching bash tool_result; Claude runs a bun
PostToolUse shim that prints additionalContext. Both reuse the shared detection
core and are runtime-gated to the doximity Context, matching the existing
OpenCode plugin so all three harnesses behave identically on gh pr create."
```

---

## Task 9: Wire shared-lib tests into `make test` and run full verification

**Files:**
- Modify: `Makefile`

**Interfaces:** none (CI wiring).

- [ ] **Step 1: Re-read the test target**

Run: `rg -n "bun test|Pi extension|OpenCode" Makefile`

- [ ] **Step 2: Add a shared-lib test line**

In the `test:` recipe, before the OpenCode tool test line, add:

```makefile
	@echo "Running shared lib unit tests..."
	@bun test shared/lib/*.test.ts
```

- [ ] **Step 3: Run the shared-lib tests through make's command**

Run: `cd /Users/luiz/.dotfiles && bun test shared/lib/*.test.ts`
Expected: PASS (jira-core + jira-pr-ticket suites).

- [ ] **Step 4: Full typecheck + test sweep**

Run:
```bash
cd /Users/luiz/.dotfiles/opencode && bunx tsc --noEmit
cd /Users/luiz/.dotfiles/opencode/plugins && bunx tsc --noEmit
cd /Users/luiz/.dotfiles && make test
```
Expected: tsc clean for both packages; `make test` passes (shared-lib, OpenCode tool, OpenCode plugin, Pi extension, Bats, checkhealth, package parsing).

- [ ] **Step 5: Real bootstrap reconcile (vendors the symlinks + plugins/lib copy)**

Run:
```bash
cd /Users/luiz/.dotfiles
mise run bootstrap:opencode
mise run bootstrap:pi
mise run bootstrap:claude
test -f ~/.config/opencode/doximity/tool/jira-core.ts && echo "opencode tool core vendored"
test -f ~/.config/opencode/doximity/plugins/lib/jira-pr-ticket-core.ts && echo "opencode plugin core vendored"
jq -e '.hooks.PostToolUse[0].hooks[0].command' ~/.config/claude/doximity/settings.json
```
Expected: both `echo` lines print; the `jq -e` succeeds. (Confirms OpenCode resolves the cores at runtime and the Claude hook landed in the doximity home.)

- [ ] **Step 6: Commit**

```bash
cd /Users/luiz/.dotfiles
git add Makefile
git commit -m "Run shared lib unit tests in make test

The dependency-free Jira cores need coverage in CI; add a bun test line for
shared/lib so the matcher, argv builders, and reminder builder are verified on
every make test run alongside the per-harness adapter suites."
```

---

## Task 10: Update the spec status and finalize docs

**Files:**
- Modify: `docs/superpowers/specs/2026-06-21-jira-cross-harness-design.md`

- [ ] **Step 1: Flip the spec status to Implemented**

Change the `*Status*:` line near the top from `Approved (pending spec review)` to
`Implemented`.

- [ ] **Step 2: Commit (force-add; docs/superpowers is gitignored but tracked here)**

```bash
cd /Users/luiz/.dotfiles
git add -f docs/superpowers/specs/2026-06-21-jira-cross-harness-design.md
git commit -m "Mark Jira cross-harness spec implemented

All tasks landed and verified (tsc + make test + bootstrap reconcile); record
the spec as implemented so the design doc reflects shipped state."
```

---

## Self-Review

**Spec coverage:**
- Part 1 shared core → Tasks 1, 2. OpenCode refactor → Task 3. Pi tools → Task 4. Skill Pi path + PR workflow → Task 5. ✓
- Part 2 detection core → Task 6. OpenCode adapter + symlink + bootstrap copy → Task 7. Pi adapter + Claude shim + base.json hook → Task 8. ✓
- Shared-lib test wiring + full verification + bootstrap reconcile → Task 9. ✓
- Runtime gating (all three), generic message, allowlist, fail-safe, committed-symlink placement, plugins/lib copy step → covered in Tasks 6-9 and Global Constraints. ✓

**Placeholder scan:** No TBD/TODO. Two `NOTE:` callouts (Task 4, Task 8) direct the implementer to reconcile exact Pi/Claude event-field and API shapes against the installed versions — these are real verification steps, not deferred work, and each has concrete code to adjust. ✓

**Type consistency:** `AcliRunner`, `CreateWorkItemArgs`, `SearchArgs`, `JiraResult`, and operation signatures defined in Tasks 1-2 are used unchanged in Tasks 3-4. `ALLOWLIST`/`matchesPrCreate`/`extractPrUrl`/`parseRepoName`/`buildMessage` defined in Task 6 are used unchanged in Tasks 7-8. `reminderFor` (Task 8 Pi) and `decide` (Task 8 Claude) are each defined and tested in the same task. ✓
