# Участие в разработке

[English](CONTRIBUTING.md) · **Русский**

Проект принимает правки на общих условиях: код идёт под той же лицензией, что и весь остальной —
[AGPL-3.0-or-later](LICENSE).

## Подпись коммита (DCO)

Каждый коммит должен нести строку `Signed-off-by`. Она добавляется флагом `-s`:

```bash
git commit -s -m "краткое описание"
```

Строка означает, что автор правки согласен с Developer Certificate of Origin 1.1 — подтверждает, что
вправе передать этот код проекту. Права на свой код автор сохраняет за собой; передаётся только
разрешение распространять его под лицензией проекта.

Текст соглашения приводится дословно, как принято:

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

## Прежде чем присылать правку

- **Прогон должен быть зелёным.** `cargo test --workspace`; смотреть код возврата cargo, а не хвост
  вывода.
- **Код приходит со своими проверками.** То, чего не покрывают уже имеющиеся проверки, pull request
  покрывает сам: `#[test]` рядом с кодом или файл в `tests/` крейта; новый инструмент несёт ещё свой
  приёмочный контракт (`crates/qymcad-acceptance/src/tools`) и шаг сценария рукой
  (`crates/qymcad/src/gui/user_case.rs`). Код и проверки принимаются вместе; pull request, меняющий код
  крейта без единой проверки, останавливает задание `tests come with code`. Правке, которой проверка не
  нужна, — комментарий, слово, опечатка — сопровождающий ставит метку `no-test-needed`.
- **Исправление сопровождается проверкой** в том же слое, где живёт беда. Проверка обязана краснеть
  до исправления: зелёная до него не проверяет ничего. Pull request называет проверки и говорит, как
  видели красноту.
- **Все проверки зелёные** — и имевшиеся, и новые. Pull request с красной проверкой не вливается.
- **Комментарии и сообщения проверок — по-английски.** Строки интерфейса — только через каталог
  `i18n/`, не в коде.
- **Предупреждений ноль.** `dead_code` и `unused_must_use` запрещены в манифесте: написанное и не
  подключённое краснит сборку сразу.
- **Каждый pull request проверяется на Linux** теми же воротами, что и у разработчика: `python3
  tools/gate.py fast` (сборка, правила кода, слова интерфейса, справка, лёгкие приёмочные пробы) и
  собственные тесты каждого крейта. Быстрый уровень, запущенный до отправки, сберегает круг. Если
  проверка покраснела, задание называет её имя, а что она сказала — в артефактах прогона `gate-logs-…`.

Сборка из исходников и упаковка описаны в [`packaging/README.md`](packaging/README.md).
