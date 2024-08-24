# Build audio plug-ins with Rust and Typescript!

This repo contains two open-source plug-ins built in Rust and Typescript!

Currently we support macOS VST3 format only.

This repo contains _alpha_ software! This has known limitations, and there is no documentation on both the framework and the plug-ins themselves. Contributions are welcome.

Eventually, we aspire to grow this codebase into a general purpose framework for building plug-ins using Rust and TypeScript, as well as add features and more plug-ins to the collection.

## Repo organization

Currently we have two top-level folders:


 - `framework`: this includes framework code that we hope to eventually publish to npm and cargo, although it's not quite ready yet. When and if this happens, this code may be moved to a separate repository.
 - `plugs`: this includes code specific to audio plug-ins currently built on this framework.