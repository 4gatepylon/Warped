# Wishlist history

Every wishlist commit carries, below its transition line, the history of the same item: a list of the item's
earlier wishlist commits. A reader follows an item's whole life, and spots a reversion, from one commit
message. The transition line and the keywords are in [`wishlist-transitions.md`](wishlist-transitions.md),
and the other commit rules are in the "Commits" section of [`SKILL.md`](SKILL.md).

## The section

```text
[<wishlist>/<id> (<short name>)] HISTORY

* <hash>: (<from> -> <to>) <KEYWORD>: <text>
* <hash>: (<from> -> <to>) <KEYWORD>: <text>
```

- The header is the item bracket of the transition line, then ` HISTORY`. It has no leading `#`, because git
  strips lines that start with `#` when it opens an editor for the message.
- Each bullet is one earlier commit about the item, oldest first: the first 12 characters of its hash, then
  its transition line without the item bracket, with the text cut to 72 characters (ending in `...`).
- A section lists the commits before its own commit. The commit's own change is its transition line, and the
  next commit adds its bullet, because a commit can't contain its own hash.
- A section is append-only. A new section is the previous commit's section plus one bullet at the end. No
  one edits, reorders or drops a bullet, not even after a revert (see "Reverting").
- An item whose earlier commits don't use this format starts with the header and no bullets.

A subject and body (the trailers that end the message are in
[`commit-fields.md`](commit-fields.md)):

```text
wishlist(demos/hyperbolic_wormholes_rust): claim W-203 (pull toward black holes)

* [demos/hyperbolic_wormholes_rust/W-203 (pull toward black holes)] ([ ] -> [~]) CLAIM ON WISHLIST: Claimed by <agent> (session <id>), branch <branch>, 2026-11-10.

[demos/hyperbolic_wormholes_rust/W-203 (pull toward black holes)] HISTORY

* 1a2b3c4d5e6f: (null -> [ ]) ADD TO WISHLIST: W-203 (pull toward black holes): "<the user's words>"
```

## Writing it: the search command

Never write the section by hand. Before each wishlist commit, run

```bash
python3 .agents/skills/pr-description/wishlist_history.py <wishlist> <id> "<short name>"
```

and paste what it prints below the transition line. The script finds the newest commit reachable from `HEAD`
that names the item (`git log -1 --fixed-strings --grep='[<wishlist>/<id> ('`), takes that commit's section,
adds that commit's bullet, and prints the new section.

- `--rev <commit>` starts the search from another commit, and `--json` prints `section` and
  `previous_commit` as JSON.
- The script fails, naming the commit, when the newest commit about the item has no transition line or
  several. Read that commit, then ask the user.
- `git log --fixed-strings --grep='[<wishlist>/<id> (' --format='%h %s'` lists every commit about the item,
  newest first.

## Reverting

A revert is a new commit and never a rewrite. Run `git revert --no-commit <hash>`, then commit with the
`REVERT ON WISHLIST` line (its text is `reverts <hash>`) and the history from the script. The history then
shows the original commit and its reversion, and `git log --grep='REVERT ON WISHLIST'` lists every reversion.

## Limits

- Rebasing or amending a commit changes its hash, so a bullet written before keeps the old hash. Its
  transition and keyword still say what happened, and `git log --grep` finds the commit by its subject.
- A history starts at the first commit in this format, so an older commit about the item is not in it.
