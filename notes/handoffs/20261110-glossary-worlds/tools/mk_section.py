"""Scratch helper: add one section to game_design/GLOSSARY.md on a new branch and commit it."""
import pathlib, re, subprocess, sys

branch, title, section_file, subject, contribution, body_file, domain = sys.argv[1:8]
scr = pathlib.Path(__file__).parent

def run(*args: str) -> str:
    return subprocess.run(args, capture_output=True, text=True, check=True).stdout

run("git", "checkout", "-q", "-B", branch)
p = pathlib.Path("game_design/GLOSSARY.md")
lines = p.read_text().split("\n")
start = lines.index("## Sections")
last = max(i for i in range(start, len(lines)) if lines[i].startswith("- [") and (i == start + 2 or lines[i - 1].startswith("- [") or lines[i - 1] == ""))
end = start + 2
while end < len(lines) and lines[end].startswith("- ["):
    end += 1
anchor = re.sub(r"[^a-z0-9 -]", "", title.lower()).replace(" ", "-")
lines.insert(end, f"- [{title}](#{anchor})")
text = "\n".join(lines).rstrip("\n") + "\n\n" + pathlib.Path(section_file).read_text().rstrip("\n") + "\n"
p.write_text(text)

cols: dict[str, set[int]] = {}
cur = ""
for l in text.splitlines():
    if l.startswith("#"):
        cur = l
    if l.startswith("| "):
        cols.setdefault(cur, set()).add(l.count("|") - 1)
bad = {k: v for k, v in cols.items() if len(v) > 1}
assert not bad, bad
for link in re.findall(r"\]\(([^)#]+)\)", text):
    assert link.startswith("http") or (p.parent / link).exists(), link

traces = run(str(scr / "trace_block.sh"), contribution)
body = pathlib.Path(body_file).read_text().strip()
message = f"""{subject}

{body}

{traces}
Kind: docs, design
Component: game-design
Domain: {domain}
Area: game_design
Read: game_design/GLOSSARY.md
Verify: every table in the glossary has a consistent column count and every relative link resolves (checked by the script that added the section); nothing was run
Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>
"""
run("git", "add", "game_design")
subprocess.run(["git", "commit", "-q", "-F", "-"], input=message, text=True, check=True)
print(run("git", "log", "--oneline", "-1").strip()[:100], "|", len(text.splitlines()), "lines")
