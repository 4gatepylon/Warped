# Wishlist transitions

Every commit that changes an item's status in a wishlist checklist (`USER_WISHLIST_CHECKLIST.md`) names the
change in one line of the form below, so the user sees what moved by reading the commit message, not the
diff. The statuses and the checklist's sections are defined in "The user's wishlist" in the `AGENTS.md` of
the demo that owns the checklist (for example `demos/hyperbolic_wormholes_rust/AGENTS.md`). The rest of the
commit rules are in the "Commits" section of [`SKILL.md`](SKILL.md). The history that goes below the line
is in [`wishlist-history.md`](wishlist-history.md).

## The line

```text
* [<wishlist>/<id> (<short name>)] (<from> -> <to>) <KEYWORD>: <text>
```

- `<wishlist>` is the folder that holds the checklist, relative to the repository root (for example
  `demos/hyperbolic_wormholes_rust`). Each checklist numbers its own items, so `W-35` exists in more than
  one: the wishlist and the id together name an item.
- `<id> (<short name>)` is the item's id with the bold title it has in the checklist, never a bare id.
- `<from>` is the item's status before the commit, and `<to>` its status after it: `[ ]` open, `[~]`
  claimed, `[?]` pending user review, `[x]` done, or `null` when the item isn't on the list.
- `<KEYWORD>` comes from the table below, in capitals, exactly as written.
- `<text>` is what the table's third column says to put after the colon.

The commit carries the line as its message body, under a subject that names the wishlist too:

```text
wishlist(demos/hyperbolic_wormholes_rust): add W-203 (pull toward black holes)

* [demos/hyperbolic_wormholes_rust/W-203 (pull toward black holes)] (null -> [ ]) ADD TO WISHLIST: W-203 (pull toward black holes): "<the user's words>"
```

## The transitions

`[<s>]` stands for any status, the same on both sides of the arrow. `W-NN` is the item's id.

| Transition | Keyword | Text after the colon | Subject after `wishlist(<wishlist>): ` |
| --- | --- | --- | --- |
| `null -> [ ]` | `ADD TO WISHLIST` | the id, short name and the user's words, fixing only spelling | `add W-NN (<short name>)` |
| `[ ] -> [~]` | `CLAIM ON WISHLIST` | who claims it (agent, session), the branch, the date, and where the work is | `claim W-NN (<short name>)` |
| `[~] -> [?]` | `MOVE TO PENDING ON WISHLIST` | the number of the PR that delivers the item | `W-NN (<short name>) pending review in #<PR>` |
| `[?] -> [x]` | `MARK CONFIRMED DONE ON WISHLIST` | `Confirmed by the user, <YYYY-MM-DD>` and their words | `W-NN (<short name>) confirmed` |
| `[~] -> [ ]` | `UNCLAIM ON WISHLIST` | why the work stopped | `unclaim W-NN (<short name>)` |
| `[~] -> [~]` | `TAKE OVER CLAIM ON WISHLIST` | whose claim it was (no open PR for two days) and who holds it now | `take over W-NN (<short name>)` |
| `[~] -> [~]` | `UPDATE WORK ON WISHLIST` | the new "Where the work is" | `W-NN (<short name>) gains where the work is` |
| `[~] -> [~]` or `[?] -> [?]` | `LINK PR ON WISHLIST` | the number added: a PR that delivers part of a claimed item, or a fix for a pending one | `link #<PR> to W-NN (<short name>)` |
| `[<s>] -> [<s>]` | `ADD USER'S WORDS ON WISHLIST` | the user's new words and their date, such as a problem they report | `W-NN (<short name>) gains the user's words` |
| `[x] -> [ ]` | `REOPEN ON WISHLIST` | the user's words on what broke | `reopen W-NN (<short name>)` |
| `[<s>] -> null` | `FOLD INTO ANOTHER ON WISHLIST` | the id and short name it folds into | `fold W-NN (<short name>) into W-MM (<short name>)` |
| `[<s>] -> [<s>]` | `RENUMBER ON WISHLIST` | the old and the new id, after two items took the same id | `renumber W-NN (<short name>) to W-MM` |
| `[<s>] -> [<s>]` | `REVERT ON WISHLIST` | `reverts <hash>`, the commit it undoes; `<to>` is the status before that commit | `revert W-NN (<short name>)` |

## Rules

- **One commit, one item, one line.** A commit that touches a checklist changes exactly one item in it,
  carries exactly one line, and changes nothing else: no code, no other item. Several changes are several
  commits. A message with three requests is three `ADD TO WISHLIST` commits, and a PR that delivers two
  items has one `MOVE TO PENDING` commit for each. Reverting a commit then reverts exactly one transition.
  - `RENUMBER` is the one exception: it also rewrites the references to the old id in other items.
  - `FOLD INTO ANOTHER` takes two commits: first `ADD USER'S WORDS` on the item it folds into, carrying
    the folded item's words, then the fold, so no words are lost.
- **One step at a time.** An item is claimed, in a `CLAIM` commit, before the commits that deliver it. Only
  the user's confirmation in chat makes `[x]`. A problem the user reports on a `[?]` item keeps it `[?]`
  (`ADD USER'S WORDS ON WISHLIST`).
- **Ask first for two of them.** `FOLD INTO ANOTHER` needs the user's approval, as splitting and merging
  items do. `REOPEN` needs the user to say the item broke.
- **The line matches the diff.** Check `git show <commit> -- <checklist>`: `<from>` is the item's status
  before the commit, `<to>` its status after it, and no other item moves.
- **`wishlist(...)` means the checklist only.** A commit whose subject starts with `wishlist(` edits one
  checklist and nothing else. A commit with code never edits a checklist, so its subject never starts with
  `wishlist(`.
- **`add` means one thing.** The word `add` in a `wishlist(...)` subject means `ADD TO WISHLIST`, a new
  item on the list, and nothing else. A PR number is linked, never added, and `add` never means the item
  is built.
- **Finding the lines.** `git log --grep='ADD TO WISHLIST'` lists every item added (use another keyword
  for another transition), and `git log --grep='<wishlist>/<id> ('` follows one item.
