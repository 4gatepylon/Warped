# Cloud architecture proposal

> **Early draft, superseded.** Written on 2026-11-10 before most decisions were made: it has no matchmaking, calls a
> level a "map", limits rated games in a way the user later replaced, and names AWS services nobody has chosen.
> Keep it as raw material for the system design (its diagrams of the whole system, the OAuth sign-in and the data
> model are a starting point), and trust `../OUTSIDE_GAME_DESIGN.md` wherever the two disagree.

A proposal for the official, cloud-hosted version of Warped. It grows the existing management server
(`management_server/`) and game server contract (`common/protocol/game_server_contract.md`) instead of
replacing them. Every diagram is Mermaid, so GitHub renders it.

Status: **draft v2, with open questions at the end.**

## Decisions so far

| Question | Decision |
| --- | --- |
| Who creates games? | Players, only. There is no matchmaking queue: players create, browse and join games. |
| Sign-in | Google at launch. More providers later reuse the same flow. |
| Rating | OpenSkill (team-aware, with an uncertainty value), shown as one number. |
| Size | Hundreds of players, one region. Redis and a message queue are left out until they are needed. |

## What it has to support

| Need | Where it lives |
| --- | --- |
| Create, read, update and delete games, including their settings and which maps they play | [Games module](#3-games-crud-and-joining) |
| Sign up, sign in and sign out, with OAuth | [Auth module](#2-oauth-sign-in-explained) |
| A rating for every player (the "ELO system") | [Rating worker](#4-match-end-and-the-rating-update) |

A **game** here is a room that someone configures (name, settings, an ordered list of maps) and that runs
as one game server process for one match.

## Design principles

- **One modular backend first, not microservices.** The management server grows modules (auth, games,
  maps, lobby, fleet) inside one deployable. Only the rating worker is separate from day one, because it is
  asynchronous by nature. Modules talk through plain function calls and can be split out later.
- **Game servers are cattle.** They hold no durable state. They start from a config, play one match, report
  the result once, and exit. The launcher interface (`launch`, `get`, `list`, `stop`) stays; EC2 or ECS
  becomes one more launcher.
- **Game servers never call the database and rarely call the backend.** A signed join ticket lets a game
  server admit players without a lookup, and one signed result report is the only write it makes.
- **Maps are immutable and content-addressed.** A game points at a map's hash, so editing a map never
  changes a match that is in progress or a past match's meaning.
- **Ratings are a ledger.** Every change is an appended row, so a bad result can be replayed or reversed.
- **Sized for hundreds of players.** One region, one PostgreSQL, no cache and no queue service. The work
  a queue would do (results to the rating worker) is a table in PostgreSQL, and the leaderboard is an
  indexed query. Each of these is a drop-in upgrade later.
- **Players create games, so players can cost money.** Every game runs a server, so each account has a quota
  of live games and every game stops itself when idle.

## 1. The whole system

```mermaid
flowchart LR
    client["Game client<br/>macOS first, then Windows and Linux"]
    site["Website<br/>download, profile, leaderboard"]
    idp["Identity providers<br/>Google, Apple, Discord, Steam"]

    cdn["CDN<br/>client builds, map files"]
    lb["Load balancer<br/>TLS, rate limits, WAF"]
    backend["Backend, one deployable<br/>Auth, Games, Maps, Lobby,<br/>Results intake, Fleet manager"]

    gs["Game servers<br/>one process per match"]

    pg[("PostgreSQL<br/>accounts, games, matches,<br/>ratings, results outbox")]
    s3[("Object storage<br/>map files by hash")]
    kms["Key service<br/>signing keys"]
    worker["Rating worker<br/>OpenSkill, idempotent"]

    client -->|"HTTPS API"| lb
    site -->|"HTTPS API"| lb
    lb --> backend
    client -.->|"sign in, in the system browser"| idp
    backend -.->|"redeem code, fetch public keys"| idp
    client -->|"download maps by hash"| cdn
    cdn --> s3

    backend --> pg
    backend -->|"sign tokens and tickets"| kms
    backend -->|"validated map upload"| s3

    backend -->|"launch, health, stop"| gs
    client <==>|"real-time play, join ticket"| gs
    gs -->|"fetch maps, check hash"| s3
    gs -->|"signed match result"| lb

    worker -->|"poll outbox, write ratings"| pg
```

| Backend module | Job |
| --- | --- |
| Auth | OAuth client for the identity providers, issues and rotates tokens |
| Games | Create, read, update and delete games, their settings and their map list |
| Maps | Accepts, validates, versions and publishes map files |
| Lobby | Game browser, admission rules, quotas, signs join tickets |
| Results intake | Checks a game server's result report and stores the match |
| Fleet manager | The existing launcher: starts, checks and stops game servers |

The two paths to remember:

- **Control path** (slow, secure, transactional): client, load balancer, backend, database.
- **Play path** (fast, authoritative): client to one game server over a real-time connection. The backend is
  not on it. A player's ticket is the only thing connecting the two paths.

## 2. OAuth sign-in, explained

### 2.1 The idea in one paragraph

You never see or store a player's Google password. The player proves who they are *to Google*, and Google
gives Warped a signed statement saying "this is user `1234`, their email is X". Warped treats that statement
as the proof. Everything below is about making sure nobody can steal or forge that statement on its way.

### 2.2 The words

| Term | Meaning here |
| --- | --- |
| Identity provider (IdP) | Google, Apple, Discord or Steam. It owns the password and the login page. |
| Authorization code | A short-lived, one-use ticket the IdP hands back after login. Worthless alone. |
| `id_token` | A signed JSON Web Token from the IdP: who the user is (`sub`), the email, who it was issued for (`aud`), when it expires (`exp`). |
| `sub` | The IdP's permanent ID for that user. Warped keys accounts on `(provider, sub)`, never on email. |
| PKCE | "Proof Key for Code Exchange": the client makes a random `code_verifier`, sends only its hash (`code_challenge`) at the start, and shows the original at the end. Only whoever began the login can finish it. |
| `state` | A random value sent out and checked on return, so a stranger can't push a login into your client. |
| `nonce` | A random value put inside the `id_token`, so an old `id_token` can't be replayed. |
| Loopback redirect | The client briefly listens on `127.0.0.1` so the browser can hand the code back to it. This is the standard for desktop apps (RFC 8252). |

### 2.3 The flow (Authorization Code with PKCE, desktop client)

```mermaid
sequenceDiagram
    autonumber
    actor P as Player
    participant C as Game client
    participant B as System browser
    participant G as Google
    participant W as Warped backend (Auth)
    participant DB as PostgreSQL

    Note over C: Phase 1 - the client makes one-time secrets
    P->>C: Click "Sign in with Google"
    C->>C: state, nonce, code_verifier = random values
    C->>C: code_challenge = BASE64URL(SHA256(code_verifier))
    C->>C: Listen on http://127.0.0.1:PORT/callback (random free port)

    Note over C,G: Phase 2 - the player proves who they are to Google
    C->>B: Open Google's authorize URL
    Note right of C: client_id, redirect_uri=127.0.0.1:PORT,<br/>response_type=code, scope=openid email profile,<br/>state, nonce, code_challenge, method=S256
    B->>G: GET /authorize
    G->>P: Login page (and a consent screen the first time)
    P->>G: Password, passkey or 2FA
    G-->>B: 302 to 127.0.0.1:PORT/callback?code=AUTH_CODE&state=STATE
    B->>C: GET /callback?code=AUTH_CODE&state=STATE
    C->>C: Check returned state equals sent state (else abort)
    C->>B: Show "You can close this tab"

    Note over C,W: Phase 3 - Warped redeems the code, server to server
    C->>W: POST /auth/oauth/google {code, code_verifier, nonce, redirect_uri}
    W->>G: POST /token {code, code_verifier, client_id, client_secret}
    G->>G: SHA256(code_verifier) equals the stored code_challenge?
    G-->>W: {id_token, access_token}
    W->>G: GET /jwks (cached for hours)
    W->>W: Verify id_token: signature, iss, aud=our client_id, exp, nonce
    W->>W: Keep sub and email, discard Google's tokens

    Note over W,DB: Phase 4 - find or create the Warped account
    W->>DB: SELECT account WHERE (provider, sub)
    alt first sign-in
        W->>DB: INSERT account, identity (provider, sub, email)
        W-->>C: 201 {needs_display_name: true}
    else returning player
        DB-->>W: account
    end

    Note over W,C: Phase 5 - Warped issues its own tokens
    W->>DB: INSERT refresh token (hash only), family_id, expires in 60 days
    W-->>C: {access_token (JWT, 15 min), refresh_token (opaque, 60 days)}
    C->>C: Keep refresh token in the macOS Keychain, access token in memory
```

Only Google is wired up at launch. Another OpenID Connect provider (Apple, Discord with an extra API call) repeats
the same five phases with different URLs. Steam uses OpenID 2.0 and would need its own flow.

What each secret protects against:

| Attack | Stopped by |
| --- | --- |
| Another app on the machine grabs the code from the loopback redirect | PKCE: it lacks `code_verifier`, so Google refuses the code |
| A malicious page tricks the client into finishing someone else's login | `state` |
| A leaked old `id_token` is replayed to log in | `nonce`, `exp` and one-use codes |
| Someone sends Warped a forged `id_token` | Signature check against Google's published keys, plus `aud` and `iss` |
| The client binary is unpacked and the secret read | The client holds no secret: `client_secret` stays on the backend |
| Two accounts merged because an email changed or was reused | Accounts are keyed on `(provider, sub)` |

### 2.4 Staying signed in, signing out

```mermaid
sequenceDiagram
    autonumber
    participant C as Game client
    participant W as Warped backend (Auth)
    participant DB as PostgreSQL
    participant GS as Game server

    Note over C,W: Every API call carries the 15 minute access token
    C->>W: GET /games with Authorization Bearer access_token
    W->>W: Verify signature and exp (no database read)
    W-->>C: 200

    Note over C,W: When it expires, rotate the refresh token
    C->>W: POST /auth/refresh {refresh_token R1}
    W->>DB: Look up hash(R1), same family, not used before?
    alt R1 is valid and unused
        W->>DB: Mark R1 used, INSERT R2 in the same family
        W-->>C: {new access_token, refresh_token R2}
    else R1 was already used (stolen and replayed)
        W->>DB: Revoke the whole family
        W-->>C: 401, sign in again
    end

    Note over C,W: Sign out
    C->>W: POST /auth/logout {refresh_token}
    W->>DB: Revoke the family
    W-->>C: 204
    C->>C: Delete tokens from Keychain and memory
    Note over C,GS: Already-issued join tickets expire within 60 s, access tokens within 15 min
```

Sign out everywhere is "revoke every family for this account". An account ban also sets a
`not_before` timestamp that the games module checks before issuing any join ticket.

## 3. Games CRUD and joining

A game moves through the states below. Settings and the map list are editable only while it is a draft or
open.

```mermaid
stateDiagram-v2
    [*] --> draft: create
    draft --> open: publish (players may find it)
    open --> draft: unpublish
    draft --> starting: start
    open --> starting: start
    starting --> live: game server answers /health
    starting --> failed: no answer in 30 s, or it exits
    live --> finished: match ends, result stored
    live --> finished: empty for 10 min, or 2 h limit
    live --> failed: game server dies
    finished --> [*]
    failed --> [*]
    draft --> deleted: delete
    open --> deleted: delete
    live --> deleted: delete (stops the server)
    deleted --> [*]
```

```mermaid
sequenceDiagram
    autonumber
    actor O as Owner
    participant C as Game client
    participant W as Backend (Games, Maps, Lobby)
    participant DB as PostgreSQL
    participant S3 as Object storage
    participant F as Fleet manager
    participant GS as Game server
    actor J as Another player

    Note over O,DB: Create and update
    O->>C: Name the game, pick settings, pick maps in order
    C->>W: POST /games {name, settings, map_ids[]}
    W->>DB: Check each map has a published version
    W->>DB: Check the owner's quota (live and open games)
    W->>W: Validate settings against the schema, map count odd (one purple level)
    W->>DB: INSERT game (draft) and game_maps with map_version_ids
    W-->>C: 201 {game_id, etag}
    C->>W: PATCH /games/{id} If-Match etag {settings or map_ids}
    W->>DB: UPDATE only if state is draft or open and etag matches
    W-->>C: 200 {new etag} or 412 if someone else changed it

    Note over O,GS: Start
    C->>W: POST /games/{id}/start
    W->>DB: Freeze config: settings plus map content hashes, state = starting
    W->>F: launch(game_id, frozen config, max_player_count)
    W-->>C: 202
    F->>GS: Start with the contract's flags plus the ticket public key and a server token
    GS->>S3: Fetch each map by hash, verify SHA-256
    GS-->>F: GET /health returns 200
    F->>DB: state = live, address = host and port

    Note over J,GS: Join
    J->>W: GET /games?state=open (list, with the average rating of players inside)
    J->>W: POST /games/{id}/join
    W->>W: Admission: game is live, not full, player not banned
    W->>W: Sign join ticket {sub, game_id, team, exp in 60 s, jti}
    W-->>J: {ticket, address}
    J->>GS: Connect and present the ticket
    GS->>GS: Verify signature, aud = this game_id, exp, jti unused
    GS-->>J: Welcome, state stream starts

    Note over O,GS: Delete
    O->>C: Delete game
    C->>W: DELETE /games/{id}
    W->>F: stop(game_id)
    F->>GS: SIGTERM, then SIGKILL after 5 s
    W->>DB: state = deleted, soft delete, keep matches
    W-->>C: 204
```

The ticket replaces today's design, where the game server looks the player ID up in the management server on
every connect. That lookup also trusts clients not to steal IDs (`management_server_api.md`). A signed,
short-lived, single-use ticket keeps outsiders out without that trust, and the game server needs no network
call to admit anyone.

Deleting a game never deletes its matches, because ratings refer to them.

## 4. Match end and the rating update

```mermaid
sequenceDiagram
    autonumber
    participant GS as Game server
    participant W as Results intake
    participant DB as PostgreSQL
    participant R as Rating worker
    participant C as Game clients

    GS->>GS: Last round ends, winner decided (or 30 min tie)
    GS->>W: POST /internal/matches {match_id, game_id, rounds, winner, players[]}
    Note right of GS: Authorization: the server token, which is only valid for this game_id
    W->>W: Verify token, check the players were admitted, sanity-check the result
    W->>DB: BEGIN
    W->>DB: INSERT match, match_players ON CONFLICT (match_id) DO NOTHING
    W->>DB: INSERT outbox_event MatchFinished {match_id}
    W->>DB: COMMIT
    W-->>GS: 202
    GS->>GS: Exit

    loop every few seconds
        R->>DB: SELECT unprocessed outbox_events FOR UPDATE SKIP LOCKED
    end
    R->>R: Skip the match unless the game is rated and passes the checks below
    R->>DB: SELECT ratings FOR UPDATE, ordered by account_id (no deadlocks)
    R->>R: OpenSkill rate(blue, red, ranks) gives new mu and sigma per player
    R->>DB: INSERT rating_events, UPDATE ratings, mark the outbox event done, COMMIT
    C->>W: GET /me/rating and GET /leaderboard
    W->>DB: SELECT ... ORDER BY display_rating DESC LIMIT 100
    W-->>C: {display_rating, rank, recent changes}
```

### Why OpenSkill and not plain Elo

Elo is built for one player against one player. A 5 against 5 match forces it to average each team, which
ignores how sure we are about each player. OpenSkill (a Plackett-Luce model, open source and free of
TrueSkill's patent) is built for teams and works like this:

- Each player has two numbers: `mu` (skill estimate) and `sigma` (how unsure we are). New players start at
  `mu = 25`, `sigma = 25 / 3`.
- After a match, the winning team's `mu` rises and the losing team's falls, by more if the result was
  surprising and more for players whose `sigma` is large. `sigma` shrinks with every match.
- A tie is a result where both teams have rank 1. A player who left early can be given a smaller weight
  instead of a full result.
- The number players see is `display_rating = 1000 + 40 * (mu - 3 * sigma)`. A new player shows 1000, and the
  number only climbs once the system is confident, so a lucky streak does not top the leaderboard.

Glicko-2 is the other good choice (it also has an uncertainty value), but it was designed for one on one, so
teams would again be averaged. The ledger stores `mu` and `sigma` before and after, so switching later means
replaying the matches.

### Rules for rating in player-created games

Anyone can create a game, so a rating is easy to farm unless games must earn it:

- A game is **rated** only if its settings equal an official ruleset preset (the owner cannot edit them),
  all its maps are official, and it runs on our fleet. Everything else is unrated.
- A match counts only if each team started with at least a minimum number of players, and the game lasted at
  least a minimum time.
- Repeats against the same opponents count less and less (weight by how many of the last 20 matches the same
  group played together), so two friends cannot trade wins.
- Players cannot rate in a game they own with an account that has fewer than a minimum number of matches
  from other owners' games.
- Each `rating_events` row stores `mu_before`, `mu_after`, `sigma_before`, `sigma_after`, `weight` and
  `match_id`, so a corrected or voided result can be replayed from the ledger.

## 5. Data model

```mermaid
erDiagram
    ACCOUNT ||--o{ IDENTITY : "signs in with"
    ACCOUNT ||--o{ REFRESH_TOKEN : "holds"
    ACCOUNT ||--o{ GAME : "owns"
    GAME ||--|{ GAME_MAP : "plays"
    MAP ||--|{ MAP_VERSION : "has"
    MAP_VERSION ||--o{ GAME_MAP : "used by"
    GAME ||--o{ MATCH : "hosts"
    MATCH ||--|{ MATCH_PLAYER : "has"
    ACCOUNT ||--o{ MATCH_PLAYER : "plays as"
    ACCOUNT ||--o{ RATING : "has"
    RATING ||--o{ RATING_EVENT : "changed by"
    MATCH ||--o{ RATING_EVENT : "causes"
    MATCH ||--o{ OUTBOX_EVENT : "announces"

    ACCOUNT {
        uuid account_id PK
        text display_name UK
        timestamptz created_at
        timestamptz banned_until
        timestamptz tokens_not_before
    }
    IDENTITY {
        uuid identity_id PK
        uuid account_id FK
        text provider "google, apple, discord, steam"
        text subject UK "unique with provider"
        text email "contact only, never a key"
    }
    REFRESH_TOKEN {
        uuid token_id PK
        uuid account_id FK
        uuid family_id
        bytea token_hash UK
        timestamptz used_at
        timestamptz expires_at
        timestamptz revoked_at
    }
    GAME {
        uuid game_id PK
        uuid owner_account_id FK
        text name
        text state "draft, open, starting, live, finished, failed, deleted"
        boolean is_rated "true only for an official ruleset on official maps"
        text ruleset "official preset name, or custom"
        int max_player_count
        jsonb settings
        int settings_version
        int etag
        timestamptz created_at
    }
    MAP {
        uuid map_id PK
        text name UK
        uuid owner_account_id FK
    }
    MAP_VERSION {
        uuid map_version_id PK
        uuid map_id FK
        text content_sha256 UK "key of the file in object storage"
        text status "uploaded, validated, published, rejected"
        timestamptz created_at
    }
    GAME_MAP {
        uuid game_id FK
        int position "order the levels are played in"
        uuid map_version_id FK
    }
    MATCH {
        uuid match_id PK
        uuid game_id FK
        text winner "blue, red, tie"
        int duration_s
        jsonb rounds
        timestamptz finished_at
    }
    MATCH_PLAYER {
        uuid match_id FK
        uuid account_id FK
        text team "blue or red"
        int kill_count
        int death_count
        boolean did_finish
    }
    RATING {
        uuid account_id FK
        text pool "for example default"
        real mu
        real sigma
        real display_rating "1000 + 40 * (mu - 3 * sigma), indexed"
        int games_played_count
        timestamptz updated_at
    }
    RATING_EVENT {
        uuid rating_event_id PK
        uuid account_id FK
        uuid match_id FK
        text pool
        real mu_before
        real mu_after
        real sigma_before
        real sigma_after
        real weight
    }
    OUTBOX_EVENT {
        uuid outbox_event_id PK
        uuid match_id FK
        text kind "MatchFinished"
        timestamptz processed_at "null until the rating worker is done"
    }
```

## 6. Where AWS fits (one possible mapping, sized for hundreds of players)

| Box in diagram 1 | AWS service | Why |
| --- | --- | --- |
| CDN, load balancer | CloudFront, ALB with WAF | managed TLS and rate limits |
| Backend and rating worker | ECS Fargate, 2 tasks and 1 task | stateless, no servers to patch |
| PostgreSQL | RDS PostgreSQL, Multi-AZ | one place for transactions, also the outbox |
| Object storage | S3 | content-addressed maps |
| Key service | KMS (asymmetric keys) | signing keys never leave it |
| Game servers | EC2 per game, started by the `Launcher` | simple, and the launcher interface hides the choice |
| Sign-in | Own Auth module, or Cognito if we would rather not run it | see the open questions |

Upgrades when they are needed: a pre-warmed server pool when starting a game feels slow, Redis when the
leaderboard or game list is queried too often, SQS when more than one consumer needs match results,
read replicas when PostgreSQL is busy.

## 7. How this maps onto what exists

| Today | In this proposal |
| --- | --- |
| `POST /game-servers` | `POST /games` creates, `POST /games/{id}/start` launches |
| `GameServerConfig = {}` | `settings` (versioned) plus the ordered map list |
| Player ID looked up on every connect | A signed join ticket verified offline |
| `Launcher` (local process) | Unchanged, plus an EC2 or ECS launcher |
| `common/maps/` folders | Published as map versions: `validate_map.py` runs at upload, the folder's hash is the key |
| No accounts | Accounts, identities, refresh tokens |

## Open questions

1. **Rated games:** is "rated only on an official ruleset and official maps" the right rule for player-created
   games? Or should custom games rate too, with a separate leaderboard?
2. **Quotas:** how many games may one account have live at once, and how long may one run idle? The draft says
   one, 10 minutes idle and a 2 hour limit.
3. **Who may publish maps:** only us, or players too? Player uploads need size limits, a stricter validator and
   a review step.
4. **Round depth:** should how far the wavefront went change the rating change, or only win, loss and tie?
5. **Leavers and bans:** what is the penalty for leaving a rated match early, and who can ban an account?
6. **Hosting:** is there a monthly budget that should cap the fleet?
