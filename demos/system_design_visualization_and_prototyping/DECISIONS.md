# Decisions made without the user

The user is away and asked that each decision that would have been theirs be chosen to fit
`game_design/OUTSIDE_GAME_DESIGN.md` and [`HANDOFF.md`](HANDOFF.md), written here with its reason, and the work go
on. Numbered, newest last; each says which part (PR) made it. To overturn one, say its number.

## Process

1. **Stacking by hand.** `gh stack` isn't installed in the cloud agent, and its `gh` is read-only, so each part is a
   branch on the one below, opened with the agent's pull request tool with the layer below as its base and stack
   parent. HANDOFF said to ask first; the user said not to wait. (Part 2)
2. **Cursor agent's credit.** Commits by the Cursor cloud agent say `Co-authored-by: Cursor Agent
   <cursoragent@cursor.com>`, and the `pr-description` skill's attribution table gains a row for it. (Part 2)

## Accounts (part 2)

3. **Components share one bus.** `bus.py` holds the database, the clock and the message log; each component
   (`accounts.py`, `purchases.py`, `identity_provider.py`, `payment_provider.py`, `secret_manager.py`) is its own
   class that talks to the others only through it, so each one is visible in the system view. `COMPONENTS` moved
   from `world.py` to `bus.py`.
4. **Sign-in is OpenID Connect's authorization code flow with PKCE (S256), `state` and `nonce`,** redirecting to a
   loopback address on any port. Source: RFC 8252 (OAuth for native apps) and Google's guide for desktop apps,
   which is how the Mac app will do it.
5. **The backend's own tokens:** a 15-minute signed access token, and an opaque 60-day refresh token that is
   replaced on every use; using a replaced one again revokes every token of that sign-in. Source: RFC 9700 (OAuth
   security best practice), refresh token rotation.
6. **One session per account.** A new sign-in ends the others, and the other computer says "signed out because
   your account signed in on another computer". Source: the design ("one session per account"); Steam does the
   same for one account playing on two computers.
7. **What sign-up keeps.** The birth date is checked (13 or older on the simulated day) and thrown away; only the
   time of the confirmation is stored. The sign-up step must finish within 10 minutes of Google's answer.
   Someone born on 29 February turns a year older on 1 March in other years.
8. **Display names.** 3 to 16 of `A-Z a-z 0-9 _`, unique in any mix of case, and no staff words (`admin`,
   `warped`, `staff`, `moderator`, `official`, `support`, `developer`, anywhere in the name). A change of case
   alone still counts as the one change in 30 days. The list of offensive words is three placeholders; a real
   one is left for the fair-play part.
9. **Icons:** eight presets (six colours, a ring, a diamond), to replace with drawn ones later.
10. **The European Union** is refused at sign-up, from the country the load balancer sees (`X-Client-Region`,
    which Google Cloud's load balancer adds). Signing in to an account made elsewhere isn't checked yet. Source:
    the design ("not offered in the EU").
11. **The free allowance** is counted per account, finished matches and seconds played. Deleting an account
    keeps a hash of its Google identity with what it used, so making a new account doesn't give a new allowance.
12. **Deleting an account** removes its name, icon and sign-in, keeps its row (past matches show "deleted
    player") and its purchase records (needed for refunds and taxes), and refunds nothing.
13. **Payment is a hosted checkout page and a signed webhook,** as Stripe Checkout does: the backend unlocks play
    only from the webhook, never from the browser coming back. The webhook is signed `t=<time>,v1=<HMAC-SHA256>`
    and refused more than 300 s old; each event is applied once, by its id; a bad signature answers 401 so that
    Stripe retries. Source: Stripe's webhook documentation (signature scheme, default tolerance, retries).
14. **A refund or a lost dispute locks online play again,** and the player may buy again after it. Events that
    arrive out of order (a refund before the payment) leave the account locked.
15. **After paying, the browser lands on a website page** ("Thank you, you can close this tab") and the app finds
    out by asking the backend every second. Reason: the Mac app would otherwise need its own URL scheme; the
    real app can ask over the connection it already keeps.
16. **The checkout opens in a new tab without the app's session** (`noopener`), so that the tab can't share and
    reuse the app's refresh token.
17. **Secrets live in a Secret Manager component** (the backend's signing keys, Stripe's webhook secret) and the
    system view shows "(hidden)" in their place. Source: Google Cloud Secret Manager, the real counterpart.
18. **Each browser tab is one computer:** the session is kept in the tab's `sessionStorage`, so two tabs show two
    computers sharing an account.
19. **Fake Stripe accepts Stripe's own test cards:** 4242 4242 4242 4242 pays, 4000 0000 0000 0002 is declined.
