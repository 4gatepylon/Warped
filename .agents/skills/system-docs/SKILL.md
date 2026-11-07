---
name: system-docs
description: Record the system a result was produced on (machine and chip, OS, browser, network, toolchain versions) so benchmark and experiment results can be reproduced, and leave out temporary observations. Use whenever you write down or update system details in a README, DECISIONS.md, PR or results file, or report a measured result.
---

# Documenting the system

Documentation of the system (machine and chip, OS, browser, network such as Wi-Fi or wired, and
toolchain versions such as Xcode, Python and uv) exists so that results, such as benchmark numbers,
can be reproduced. Record the settings that determine or explain a result. Don't record
observations that are likely temporary or that don't matter for reproducing results, such as a tool
failing to run, a permission or install quirk, or which other programs are running right now. If
something like that gets in your way, tell the user instead of writing it into the docs.
