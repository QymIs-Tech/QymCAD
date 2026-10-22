# Contributing

**English** · [Русский](CONTRIBUTING.ru.md)

Contributions are accepted on the usual terms: the code goes under the same licence as everything
else — [AGPL-3.0-or-later](LICENSE).

## Taking an issue

The work is listed in the [issues](https://github.com/QymIs-Tech/QymCAD/issues), and nowhere else.

- **What can be taken** carries `help wanted`, or `good first issue` when it is small and lies in one place.
  An issue without either is a report or a proposal not yet agreed on: ask in it before starting.
- **Say "I take it"** in the issue, and a maintainer assigns it to you — so two people do not do one thing.
- **Two weeks.** An assigned issue with no news for 12 days gets a question from a bot; at 14 days it is
  free again. A comment keeps it yours, and an open pull request referring to it keeps it as long as it is
  open.
- **The pull request names the issue** with `Fixes #N`, and the issue closes when it is merged.
- **Something large** — a new tool, a file format, a workbench — is best talked over in an issue first. A
  large pull request that comes without one is welcome too: an issue is opened for it then.
- **A new issue** is checked by whoever gets to it first: a comment saying whether it happens for you, with
  the build and the system, helps more than a thumbs-up.

## Signing off a commit (DCO)

Every commit must carry a `Signed-off-by` line. The `-s` flag adds it:

```bash
git commit -s -m "a short description"
```

The line means the author agrees to the Developer Certificate of Origin 1.1 — that they have the
right to submit this code to the project. Authors keep the copyright in their own work; what is given
is permission to distribute it under the project's licence.

The agreement is quoted verbatim, as is customary:

```
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.

Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
```

## Before sending a change

- **History stays a line.** `main` takes a pull request by fast-forward only, so a merge commit is dropped
  when it is taken. Bring your branch up to date with `git rebase origin/main`, not with a merge of `main`.

- **The run must be green.** `cargo test --workspace`; read cargo's exit code rather than the tail of
  the output.
- **Code comes with its checks.** What the existing checks do not cover, the pull request covers itself:
  a `#[test]` beside the code or a file in the crate's `tests/`; a new tool also brings its acceptance
  contract (`crates/qymcad-acceptance/src/tools`) and a step of the hand-driven scenario
  (`crates/qymcad/src/gui/user_case.rs`). The code and its checks are accepted together; a pull request
  that changes the code of a crate and no check is stopped by the `tests come with code` job. A change
  that needs no check — a comment, a word, a typo — gets the `no-test-needed` label from a maintainer.
- **A fix comes with a check** in the same layer the trouble lives in. That check must be RED before
  the fix: one that is green beforehand proves nothing. The pull request names the checks and says how
  the red was seen.
- **All checks green** — the existing ones and the new ones. A pull request with a red check is not merged.
- **Comments and assertion messages are in English.** Interface strings go through the `i18n/`
  catalogue only, never inline in the code.
- **New interface strings are required in English only.** If you know other languages of the catalogue, add
  the strings in them too; the maintainers fill in the rest.
- **Formatted by rustfmt.** Run `cargo fmt` before sending; the layout is set by `rustfmt.toml` and the
  checks refuse a tree `cargo fmt --check` would change. Once after cloning, `git config core.hooksPath
  tools/hooks` makes every commit check the layout (a second) and every push search for anything personal.
- **clippy has nothing to say.** Every clippy remark is an error in the manifest; run `cargo clippy
  --workspace --all-targets` before sending. A remark is mended, not silenced: no `#[allow(clippy::...)]`.
- **Values say what they are.** A group of values that travels together is a struct with named fields — not
  a tuple, not a list of tuples, not a type alias of a tuple. A switch is an enum, not a `bool`. No function
  takes more than seven arguments, and none takes a run of one type (`bool, bool`, `f64, f64, f64`): two
  neighbours of one type swap and still compile.

  ```rust
  // refused
  fn field(ui: &mut Ui, id: Id, text: &str, hint: &str, valid: &dyn Fn(&str) -> bool, with_list: bool, autofocus: bool)
  let rows: Vec<(&str, fn(&mut App))> = vec![("move", |a| ...)];

  // accepted
  pub struct FieldRules<'a> { pub valid: &'a dyn Fn(&str) -> bool, pub list: NameList, pub focus: Focus }
  fn field(ui: &mut Ui, id: Id, text: &str, hint: &str, rules: FieldRules)
  struct Edit { what: &'static str, make: fn(&mut App) }
  ```
- **Zero warnings.** `dead_code` and `unused_must_use` are denied in the manifest: something written
  and never wired up reddens the build at once.
- **Every pull request is checked on Linux** by the same gate a developer runs: `python3 tools/gate.py
  fast` (the layout, the build, clippy, the rules of the code, the interface words, the help, the light
  acceptance probes) and every crate's own tests. Running the fast level before sending saves a round trip.
  When a check goes red, the job lists its name; what it said is in the `gate-logs-…` artifacts of the run.

Building from source and packaging are described in [`packaging/README.md`](packaging/README.md). The kernel is
OCCT 7.9.3 built from source with the `cmake` line given there ("The kernel"): the version the program is tested
against, with `BUILD_RELEASE_DISABLE_EXCEPTIONS=OFF` so that OCCT's checks stay in, and with signals converted so that a
fault inside OCCT becomes a refusal. Another version builds with a warning.
