//! `level_net.rs` is what every level server's network shares (Lunar Surface, Dust, Snow Planet,
//! City Sphere, Dark Forest and the match): how many players a level server takes, and how its
//! snapshot of every player is split into pages that each fit one safe UDP datagram, then put back
//! together in a window.
//!
//! Interface: `MAX_PLAYER_COUNT`, `MAX_SNAPSHOT_DATAGRAM_BYTES`, `SNAPSHOT_PAGE_HEADER_BYTES`,
//! `get_players_per_page`, `SnapshotPages`.
//!
//! Example: `const PLAYERS_PER_PAGE: usize = level_net::get_players_per_page(186, 9);`
//!
//! Must know:
//! - Gotchas: a page is `[kind, page index, page count, player count]`, then the level's own
//!   player records, game state and health bytes; each level's `net.rs` writes and reads those.
//! - Gotchas: a lost page leaves its players where the page before it put them, until the next
//!   snapshot's copy of that page arrives, 1/60 s later.

/// `MAX_PLAYER_COUNT` is how many players a level server takes at once: a 5v5 with one spare
/// player for each team (W-155, play against bots).
pub const MAX_PLAYER_COUNT: usize = 12;

/// `MAX_SNAPSHOT_DATAGRAM_BYTES` is the largest snapshot page a level server sends: under the
/// 1280-byte IPv6 minimum MTU less its headers, so no network along the way splits a page.
pub const MAX_SNAPSHOT_DATAGRAM_BYTES: usize = 1200;

/// `SNAPSHOT_PAGE_HEADER_BYTES` counts a page's kind byte, page index, page count and player count.
pub const SNAPSHOT_PAGE_HEADER_BYTES: usize = 4;

/// `get_players_per_page` gives how many players fit one page, with `bytes_per_player` for each
/// player (their record and health byte) and `bytes_per_page_tail` after them all (the game
/// state).
///
/// Example: `const PLAYERS_PER_PAGE: usize = get_players_per_page(186, 9);`
pub const fn get_players_per_page(bytes_per_player: usize, bytes_per_page_tail: usize) -> usize {
    (MAX_SNAPSHOT_DATAGRAM_BYTES - SNAPSHOT_PAGE_HEADER_BYTES - bytes_per_page_tail)
        / bytes_per_player
}

/// `SnapshotPages` keeps the latest copy of each page of the server's snapshot, so a window shows
/// every player although each datagram carries only some of them.
pub struct SnapshotPages<T> {
    pages: Vec<Vec<T>>,
}

impl<T: Clone> SnapshotPages<T> {
    /// `New` starts with no pages.
    ///
    /// Example: `let mut pages: SnapshotPages<RemotePlayer> = SnapshotPages::New();`
    #[allow(non_snake_case)]
    pub fn New() -> SnapshotPages<T> {
        SnapshotPages { pages: Vec::new() }
    }

    /// `add_page` keeps `players` as page `page_index` of a snapshot of `page_count` pages, forgets
    /// the pages from `page_count` on, and gives every player the pages hold, in page order. A
    /// player who moved to this page (another left the server) is dropped from the other pages, so
    /// nobody shows twice; `get_id` gives a player's id.
    ///
    /// Example: `client.players = pages.add_page(0, 2, players, |player| player.id);`
    pub fn add_page(
        &mut self,
        page_index: usize,
        page_count: usize,
        players: Vec<T>,
        get_id: impl Fn(&T) -> u32,
    ) -> Vec<T> {
        assert!(
            page_index < page_count,
            "add_page: page {page_index} of a snapshot of {page_count} pages"
        );
        self.pages.resize_with(page_count, Vec::new);
        for (index, page) in self.pages.iter_mut().enumerate() {
            if index != page_index {
                page.retain(|old| players.iter().all(|new| get_id(new) != get_id(old)));
            }
        }
        self.pages[page_index] = players;
        self.pages.concat()
    }

    /// `remove_player` forgets player `id` (eliminated or gone) in every page.
    ///
    /// Example: `pages.remove_player(victim_id, |player| player.id);`
    pub fn remove_player(&mut self, id: u32, get_id: impl Fn(&T) -> u32) {
        for page in &mut self.pages {
            page.retain(|player| get_id(player) != id);
        }
    }

    /// `clear` forgets every page.
    ///
    /// Example: `pages.clear();`
    pub fn clear(&mut self) {
        self.pages.clear();
    }
}

#[cfg(test)]
mod tests {
    //! Tests of splitting a snapshot into pages and putting it back together.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Page size | a level's player record at `MAX_PLAYER_COUNT` players (two pages), within `MAX_SNAPSHOT_DATAGRAM_BYTES` | `a_full_server_fits_safe_datagrams` |
    //! | Pages received | one, all, one lost (the other's older copy kept) | `pages_put_every_player_back` |
    //! | A player between pages | stays on a page, moves to another page, eliminated | `a_player_who_moves_page_shows_once`, `removed_players_leave_every_page` |
    //! | Page count | grows, shrinks (later pages forgotten) | `pages_put_every_player_back` |

    use super::*;

    const BYTES_PER_LEVEL_PLAYER: usize = 186;
    const BYTES_PER_LEVEL_GAME: usize = 9;

    /// `get_ids` lists the ids of `players`, which here are their ids.
    fn get_ids(players: &[u32]) -> Vec<u32> {
        players.to_vec()
    }

    #[test]
    fn a_full_server_fits_safe_datagrams() {
        let players_per_page: usize =
            get_players_per_page(BYTES_PER_LEVEL_PLAYER, BYTES_PER_LEVEL_GAME);
        let page_bytes: usize = SNAPSHOT_PAGE_HEADER_BYTES
            + players_per_page * BYTES_PER_LEVEL_PLAYER
            + BYTES_PER_LEVEL_GAME;
        assert!(page_bytes <= MAX_SNAPSHOT_DATAGRAM_BYTES);
        assert_eq!(MAX_PLAYER_COUNT.div_ceil(players_per_page), 2);
    }

    #[test]
    fn pages_put_every_player_back() {
        let mut pages: SnapshotPages<u32> = SnapshotPages::New();
        assert_eq!(get_ids(&pages.add_page(0, 2, vec![1, 2], |&id| id)), [1, 2]);
        assert_eq!(get_ids(&pages.add_page(1, 2, vec![3], |&id| id)), [1, 2, 3]);
        assert_eq!(
            get_ids(&pages.add_page(1, 2, vec![3, 4], |&id| id)),
            [1, 2, 3, 4]
        );
        assert_eq!(get_ids(&pages.add_page(0, 1, vec![1, 2], |&id| id)), [1, 2]);
    }

    #[test]
    fn a_player_who_moves_page_shows_once() {
        let mut pages: SnapshotPages<u32> = SnapshotPages::New();
        pages.add_page(0, 2, vec![1, 2], |&id| id);
        pages.add_page(1, 2, vec![3, 4], |&id| id);
        assert_eq!(
            get_ids(&pages.add_page(0, 2, vec![1, 3], |&id| id)),
            [1, 3, 4]
        );
    }

    #[test]
    fn removed_players_leave_every_page() {
        let mut pages: SnapshotPages<u32> = SnapshotPages::New();
        pages.add_page(0, 2, vec![1, 2], |&id| id);
        pages.add_page(1, 2, vec![3], |&id| id);
        pages.remove_player(2, |&id| id);
        assert_eq!(get_ids(&pages.add_page(1, 2, vec![3], |&id| id)), [1, 3]);
        pages.clear();
        assert_eq!(get_ids(&pages.add_page(1, 2, vec![3], |&id| id)), [3]);
    }
}
