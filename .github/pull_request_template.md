<!-- English or Russian, as is easier. / По-английски или по-русски, как удобнее. -->

## What changes

<!-- What was wrong or missing, and what the change does. For a reported fault, the issue number. -->

## What works and what does not

<!-- What was tried by hand and works. What is left unfinished or known not to work - said plainly. -->

## Checks

<!-- The names of the checks that cover the change: a #[test] beside the code, a file in a crate's tests/,
     for a tool its acceptance contract and a step of user_case. -->

- [ ] The new or changed checks are red without the change and green with it (how it was seen: …)
- [ ] `python3 tools/gate.py fast` is green
- [ ] `cargo test --workspace` and `cargo check --workspace --all-targets` are green
- [ ] Every commit is signed off (`git commit -s`)
