# Contributing

Issues are welcome: bug reports, questions, and ideas for livedot. Please include your livedot version (`cat VERSION` or the `livedot --version` output), your desktop and tray host, and the relevant part of your `config.yml`.

Pull requests are by request. Open an issue first and describe the change; if it fits the project, the maintainer will say so and a pull request is welcome. An accepted change is re-applied by the maintainer by hand rather than merged through the GitHub interface, and you are credited in the changelog.

Before proposing code:

- Read `README.md` and `specs/app.yml`, which state what livedot promises.
- Build with `cargo build --release` and keep `cargo clippy` clean.
- Write no code comments; the project keeps its explanations in the README and the spec.
- Do not add dependencies without discussing it in the issue first.
