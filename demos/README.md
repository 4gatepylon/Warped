# Demos

Experiments that help decide what to build: whether the curved-space mechanics are fun, which
mechanics and weapons to add, and how to handle bullets or lasers whose paths never end.

- [`three_js_vibecode_demo/`](three_js_vibecode_demo/README.md): single-file three.js demos of curved space (a 3-sphere, hyperbolic space, black holes), with laser versions, and wormholes you can fly and shoot through.
- [`hyperbolic_wormholes/`](hyperbolic_wormholes/README.md): wormholes in hyperbolic space, combining the hyperbolic and wormhole demos above: rails that fan apart, and mouths that bend light and lead to the house, a far pylon, and a view from above.
- [`hyperbolic_wormholes_rust/`](hyperbolic_wormholes_rust/README.md): the hyperbolic wormholes demo in Rust, to measure its speed natively, with gravity and a laser, a benchmark of fixed views, and a scripted route.
- [`warped_shared_rust/`](warped_shared_rust/README.md): the library of code the Rust level demos (Lunar Surface, Dust, Snow Planet, City Sphere and the match) share, so each level is its own program but their common code is written once.
- [`gluebox/`](gluebox/README.md): build a 3D space by gluing together the faces of a cube (3-torus, Klein bottle × circle, projective space, the 3-sphere, …) and fly around inside it. Based on the demos above.
- [`mirror_sphere/`](mirror_sphere/README.md): walk on the inside of a hollow mirror sphere, with a laser that reflects off it and fades out.
- [`fps_demo/`](fps_demo/README.md): a single-player FPS in Rust with SDL3 and a direct Metal renderer (PBR, image-based lighting, cascaded shadows, bloom, SSAO), against robot bots in an indoor/outdoor arena. A look-and-feel test before curved space; macOS only.
- [`levels/`](levels/README.md): one playable demo per level of the default map (Dust, Snow Planet, City Sphere, Dark Forest, Lunar Surface), plus City Sphere with wormholes, each with its layout, pylons and warp objects.
- [`system_design_visualization_and_prototyping/`](system_design_visualization_and_prototyping/README.md): a playable simulation of the system around a match (the game app's and website's windows, a simulated backend with an elastic fleet of game servers, and a view of what the backend is doing), to try the outside-game design before building it.
