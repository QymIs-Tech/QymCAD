# Contributing

**English** · [Русский](CONTRIBUTING.ru.md)

Contributions are accepted on the usual terms: the code goes under the same licence as everything
else — [AGPL-3.0-or-later](LICENSE).

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

- **The run must be green.** `cargo test --workspace`; read cargo's exit code rather than the tail of
  the output.
- **A fix comes with a check** in the same layer the trouble lives in. That check must be RED before
  the fix: one that is green beforehand proves nothing.
- **Comments and assertion messages are in English.** Interface strings go through the `i18n/`
  catalogue only, never inline in the code.
- **Zero warnings.** `dead_code` and `unused_must_use` are denied in the manifest: something written
  and never wired up reddens the build at once.

Building from source and packaging are described in [`packaging/README.md`](packaging/README.md).
