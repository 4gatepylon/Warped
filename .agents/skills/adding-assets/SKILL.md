---
name: adding-assets
description: Find, choose, get the user's approval for, add and credit assets from outside this repo - models, textures, sounds, fonts, pictures, sky maps. Use whenever a change needs an asset the repo doesn't have, the user asks for a model or texture, you compare candidate assets, or you add, replace or remove an asset in a game or demo.
---

# Adding assets

An asset from outside the repository brings a license and an author with it. The user approves every download, and
every asset in a game is credited on its CREDITS page.

## Finding candidates

- **Look in the repo first.** An asset already committed (e.g. `demos/fps_demo/assets/`) needs no download and no new
  license; offer it as an option even when it isn't a perfect fit, and say honestly how it fits.
- **Where to look:**
  - **Free licenses:** Poly Haven (CC0: textures, HDRIs, models), Kenney (CC0, low poly), Quaternius (CC0), Poly Pizza.
  - **Credit required:** Sketchfab models marked downloadable (often CC BY; downloading needs a free account), and
    ESO's images (CC BY 4.0).
  - **NASA images** are usually in the public domain, but check each page.
- **For each candidate, record:**
  - its exact page URL, title and author;
  - its license and what it requires;
  - its file format and size;
  - whether downloading needs an account;
  - why it fits.
- **Formats:** models in glTF or GLB (the games load glTF with the `gltf` crate). Textures in JPG or PNG at the
  resolution the game uses.
- **Rule out:**
  - non-commercial licenses (NC) for anything that may ship;
  - models tagged NoAI;
  - copies of other people's designs: a ship from a film, a weapon from a game. When the user asks for something "in
    the style of" a franchise, make an original design in that spirit, with generic names in the code.

## Showing and choosing

- **Show before choosing.** Render each candidate that can be rendered in the game (`graphical-changes`). One that
  can't be rendered without downloading it is marked "not rendered", with its page link.
- **Recommend one,** saying why, and let the user pick.

## Downloading

- **Ask first.** Download nothing until the user approves that file, and name its file name, source and size when
  you ask. When a download needs the user's account, say so: they download it, or set up access themselves.
- **Big files:** ask the user before committing a file larger than about 10 MB.

## Adding it

- **Next to its kind,** e.g. a Sketchfab model under `demos/fps_demo/assets/sketchfab/<name>/`, with a `license.txt`
  beside it: title, source URL, author, license and its requirements.
- **Fail clearly when missing.** The game fails at start, naming the missing file and the command that restores it,
  as `fps_demo_assets::check_asset` does.
- **Credit it in the same PR.** Add the asset to the game's CREDITS page (`CREDITS_LINES` in
  `demos/hyperbolic_wormholes_rust/src/credits.rs`): what it is, its author, its link and its license. Meet any
  attribution the license requires (CC BY needs the author's name), and keep each line within 64 characters. A change
  that removes an asset removes its credit.
- **Place and scale it by eye.** Show it in the game at its final size and placement (`graphical-changes`), e.g. a
  long rifle scaled so its body shows, not just its barrel.
