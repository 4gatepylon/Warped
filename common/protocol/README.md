# Protocol

Every message between clients, the management server and game servers, as Protocol Buffers
(`.proto`). Room management and game server control are gRPC services. Real-time play uses the
same messages, but its transport is chosen separately. Each component generates its own code into
its git-ignored `.build/`.
