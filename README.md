# Waitress

Minecraft hosting for the modern day -- dish out servers on a silver platter.

## What is Waitress?

Minecraft server hosting has a problem. There's no good panel for hosting servers on a single machine! Pterodactyl targets companies with multiple machines with their node abstraction, and PufferPanel is often buggy and hard to set up. Enter Waitress!

Waitress is a free and open source full-stack Minecraft server hosting solution. The backend is written in Rust, and the frontend is _(going to be)_ written in Svelte, both very good frameworks for building performant applications.

## How does it work?

Waitress works very similarly to existing solutions, only without the technical debt their old stacks encumber upon them. Under the hood, it sandboxes all servers via Docker and allocates ports to the servers to prevent collisions. In future, Waitress will be transformed into a more generic framework for hosting any game server, not just Minecraft.

## How can I install it?

Waitress is not ready for public use right now. Only the backend is available right now. If you wish to try it, run the following

```sh
# install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# clone the repo
git clone https://github.com/VERT-sh/waitress && cd waitress
# build and run the project with release optimizations
cargo run --release
```

## Security

If you have found a critical security flaw, **do not post it on the issues tracker.** Submit it through to `hello@vert.sh` and I will personally get back to you regarding the issue. If you do not see a response within a day, join the VERT Discord server and DM me, `notnullptr`.
