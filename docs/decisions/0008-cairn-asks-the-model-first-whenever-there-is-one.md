# 0008: CAIRN Asks the Model First Whenever There Is One

Status: proposed

Date: 2026-11-27

Amends: [ADR 0006](0006-cairn-is-a-bounded-tool-loop-over-governed-evidence.md), its consequence that "the default stays the router until the evaluation set passes for the configured model".

## Context

At V0.9.126 a message reaches the tool loop only when the chat's model is in Agent mode, and Agent mode needs a model whose tool calling is known **and** that is on the default list, has a passing catalog evaluation or was forced by an administrator. A model whose tool calling nobody set counts as taking no tools. Every other message goes to the keyword router, which tries its English phrase lists before it asks the model.

In practice a person with a working model can be answered with no AI at all. "Analyze the size of files in this repo" matched the Analyze files job by its words, took "the size of files in this repo" for a file name and replied that no file matches, labelled "matched from your wording". The model never read the request. Even in Agent mode it could not have answered: no tool lists files by size, sorts them or counts them.

People who use an AI agent expect the agent to read what they asked and choose what to do. A keyword match that runs before the model breaks that expectation, and an evaluation gate that silently swaps the agent for keywords hides the cause.

## Decision

1. **The model reads every message when there is one.** With a text model available to the person, CAIRN never answers by keywords. In order:
   1. the loop, on the model's native tool calling, or on the JSON protocol when the model has no native tool calling or nobody has said;
   2. when the model cannot follow the JSON protocol, the model picks one job, the job runs, and the model writes the answer from the job's result;
   3. a model set to take no tools, or a person, workspace or organization that chose Jobs, gets step 2 directly.
2. **Keywords are the no-AI path only.** The router's phrase lists answer when no model is available to the person (none enabled, AI policy, quota spent, provider unreachable), and the reply says it was answered without AI.
3. **Greetings, picked jobs and uploaded files go through the model too.** A job picked from the + menu becomes the first tool the loop must call, not a bypass.
4. **Evaluation is a signal, not a switch.** A model's live evaluation score is shown where people and administrators choose models. A model with no report or a failing one is labelled as such and still runs the loop; administrators who want the older behaviour set Jobs.
5. **Tools are written for the model.** Each tool says when to use it and when not, and a tool that finds nothing names its closest matches and what to try next. CAIRN gets general tools that combine (a file query with filters, sorting, groups and totals) beside its jobs.
6. **Labels say what is true.** No reply says how its job was matched. One standing note says AI responses can be inaccurate, and a reply made without AI says so.

## Consequences

- Every message with a model costs at least one model call, greetings included, and usually two to four. Quotas are spent faster and replies from slow local models take longer. The run budgets and the person's allowance still bound each run.
- Weak models will choose tools worse than the router matched them on simple lookups. The harm is bounded (tools read, proposals wait for confirmation, budgets stop runs), and the score shown beside the model tells people why.
- The loop becomes the common path while its rows are still 🧪. The live walkthrough and evaluation owed by V0.9.119 now cover this path first.
- The router's routing tests become tests of the no-AI path; the loop's fallback order is tested with a scripted provider.
- The keyword lists stop growing for good. Template phrases (V0.2.185) apply only when there is no model.
