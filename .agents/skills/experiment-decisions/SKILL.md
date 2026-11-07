---
name: experiment-decisions
description: Record the decisions behind an experiment or benchmark in a standard template, in the PR description or in a per-experiment DECISIONS.md. Use whenever you build or change anything that compares options or measures something - a performance benchmark (tick time, frame time, latency, bandwidth), a comparison of physics integrators or network transports, a playtest of two mechanics, and so on - and you open or update a PR for it.
---

# Experiment decisions

An experiment is only as trustworthy as the choices behind it: which baselines, which scenarios,
how many repeats, which metric, what to hold fixed.

**The user makes the decisions.** Be as eager as possible to let them decide: before building,
ask about every choice that matters, with your suggested option and the alternatives. Decide
yourself only when:

- the user says the choice doesn't matter, or explicitly asks you to decide;
- or the choice is so small it can't plausibly change the conclusion.

Every decision you do make must be visible to the reviewer, so every such PR lists them.

## When this applies

Any PR that adds or changes an experiment or benchmark: anything that compares two or more options
or measures how well something works. Examples: timing the game server's tick, measuring latency
over Wi-Fi, comparing physics integrators or network transports, a demo that compares two
mechanics or weapons.

## What counts as a decision

A decision is any choice that could change the results or how they read, and that the user didn't
state exactly. Typical ones:

- **Scope**: which options are compared, which are left out, and the question being answered.
- **Baselines**: what each option is compared against.
- **Workload**: scenarios, maps, black holes, player and bullet counts, recorded inputs, network
  conditions.
- **Procedure**: warmup, repeats, random seeds, durations, what is held fixed.
- **Metrics**: what is measured, how it is aggregated (mean, median, percentiles), and the units;
  for a playtest, what players are asked.
- **Environment**: machine, OS, browser, network (Wi-Fi or wired), library versions, anything
  pinned or assumed.
- **Analysis**: how results are summarized, plotted or judged, and why we believe a
  difference is real rather than noise (repeats, variance, confidence intervals).

Don't list choices the user made explicitly; put those in the **Given** bullet instead. Don't list
pure implementation details that can't affect the results (file names, variable names).

## Where the decisions go

- **One folder per experiment** (the PR adds or changes experiment folders, such as
  `demos/<name>/`): write the decisions to `DECISIONS.md` in each experiment's folder. The PR
  description gets only a `## Decisions` section that links each file, one line per experiment.
- **Otherwise**: put the decisions in the PR description under `## Decisions`.

Either way, use the template below unchanged in structure. If a decision changes after review,
update it in place (the file or the PR description) rather than appending a new list.

## Template

In `DECISIONS.md`, the top heading is `# Decisions: <experiment name>`; in a PR description, it is
`## Decisions` and every heading below it moves down one level (as in the example).

```markdown
# Decisions: <experiment name>

- **Question:** <one sentence: what the experiment compares or measures, and why>
- **Given:** <what the user specified; "none" if nothing>
- **Outcomes:** <what we're looking for: the numbers reported and the results they could show,
  e.g. faster or slower than a reference, better on one axis and worse on another>

## <Category, e.g. Scope, Baselines, Workload, Procedure, Metrics, Environment, Analysis>

### D1. <short name of the decision>

- **Choice:** <what was chosen>
- **Alternatives:** <what else was considered; "none considered" if so>
- **Rationale:** <one or two sentences on why>
- **Revisit if:** <optional: what result or constraint would change this choice>

### D2. ...
```

Rules for filling it in:

- Number decisions `D1`, `D2`, ... across the whole list (not per category), so review comments
  can refer to them. Don't renumber after review; mark dropped ones `(removed)`.
- Only include categories that have decisions, in the order listed above.
- Keep each rationale short. If a choice was arbitrary, say so ("arbitrary; any value in 5-20
  would do") rather than inventing a reason.
- Flag decisions you are unsure about with **(needs input)** after the name, and mention them to
  the user.

### Example

```markdown
## Decisions

- **Question:** Which integrator keeps bullets that pass a black hole on their true path at the
  game server's 60 Hz tick: explicit Euler, semi-implicit Euler or RK4?
- **Given:** compare those three at 60 Hz.
- **Outcomes:** each integrator's position error after 5 s. Semi-implicit Euler may be accurate
  enough everywhere, RK4 may be needed only for close passes, or all three may drift too far.

### Baselines

#### D1. RK4 at 6000 Hz as the true path

- **Choice:** the reference path is RK4 with a time step 100 times smaller.
- **Alternatives:** an analytic path (exists only for a single black hole).
- **Rationale:** it works for any map, and its own error is far below the errors compared.

### Workload

#### D2. 100 random shots past one black hole

- **Choice:** random positions and directions, with closest approaches of 1 to 10 times the
  black hole's radius.
- **Alternatives:** several black holes; hand-picked shots.
- **Rationale:** one black hole isolates the integrator's error; several is a separate question.
- **Revisit if:** maps usually have black holes close together.

### Analysis

#### D3. Accurate enough means a 95th-percentile error below a player's radius

- **Choice:** report the median and 95th percentile of the error over the 100 shots.
- **Alternatives:** the mean; the maximum.
- **Rationale:** a bullet that misses by less than a player's radius still hits what it should,
  and the 95th percentile shows close passes without one outlier deciding.
```
