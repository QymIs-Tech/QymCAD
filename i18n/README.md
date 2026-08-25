# Localisation of QymCAD

**Adding a language means putting a folder here.** Not a single edit in the interface code is needed: the list
of languages is built from this directory, so forgetting to add a language to the menu is impossible - the
menu IS the directory.

## Adding your language

1. Copy `en/` into a folder named with your language code (`de`, `fr`, `pl`, `zh-CN` - BCP-47 tags).
2. Translate the values to the right of `=`. **Leave the keys on the left alone** - the program finds a
   string by them.
3. Be sure to translate `language-name`: that is how the language is named in the menu, in the language
   itself ("Deutsch", not "German").
4. Build the program - the catalogue is compiled into the binary.

## What you may leave untranslated

**An incomplete translation does not break anything.** A string missing from your language is taken from
`en`. So a translation can arrive in parts: 60% is better than nothing, and the interface will not come out
full of holes.

`cargo test -p qymcad i18n` prints the coverage of every language, so what is left is visible.

## The voice: a program names things, it does not chat

This is not a matter of taste: an engineering program does not address a person casually. The rule was broken
wholesale once - 221 strings in the familiar form against 20 in the formal one.

| | how it should read | how it should not |
|---|---|---|
| a command, a button, a menu item | **an infinitive or a noun**: "Extrude", "Revolve", "Fillet" | "Extrude it!" |
| a hint, the status line | **formal and impersonal**: "Select a contour", "Pick a plane" | "click the plane, mate" |
| the arrow of a step | `->` in ASCII | `->` as raw unicode: it draws as a box, see `gui/font_coverage.rs` |
| emphasis | ordinary words | SHOUTING IN CAPITALS mid-sentence |
| a refusal | what is wrong and what to do: "The profile crosses the axis of revolution. Move the profile off the axis." | "Oops, something went wrong" |

Chatty words have no place in the catalogue: what suits a message to a friend reads as familiarity in the
window of a program.

**This is held by a check** rather than by memory: `the_catalogue_speaks_like_a_program` in
`crates/qymcad/src/i18n/tests.rs` reddens a new casual string at once, and
`the_catalogue_holds_words_not_symbols` answers for icons standing in place of words.

## The rules that save time

- **Substitutions** are written `{ $name }` and must match the original by the SET of names: a string where a
  substitution is lost or renamed breaks at run time, not at build time. The test catches it.
- **Plural forms** go through `{ $n ->` (see the `fluent` selectors). Russian has three of them, English has
  two; that is normal, every language has its own.
- The `#` comments need not be carried over, though it helps: they say where a string appears.

## The files

| file | what is inside |
|---|---|
| `main.ftl` | the CAD interface: menus, windows, tools, messages |
| `errors.ftl` | what the program answers when an operation refuses |
| `cam.ftl` | the machining module (CAM). Translating it is OPTIONAL: the module is off by default and most people do not need it |
