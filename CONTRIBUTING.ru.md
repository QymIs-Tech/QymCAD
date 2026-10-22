# Участие в разработке

[English](CONTRIBUTING.md) · **Русский**

Проект принимает правки на общих условиях: код идёт под той же лицензией, что и весь остальной —
[AGPL-3.0-or-later](LICENSE).

## Как взять задачу

Работа перечислена в [issues](https://github.com/QymIs-Tech/QymCAD/issues), и больше нигде.

- **Что можно брать**, помечено `help wanted`, или `good first issue`, когда задача маленькая и лежит в одном
  месте. Issue без этих меток — отчёт или предложение, о котором ещё не договорились: спросите в нём, прежде
  чем начинать.
- **Напишите в issue «беру»**, и сопровождающий назначит её на вас — чтобы одно дело не делали двое.
- **Две недели.** Назначенной задаче, о которой 12 дней нет вестей, бот задаёт вопрос; на 14-й день она снова
  свободна. Комментарий оставляет её за вами, а открытый pull request со ссылкой на неё — всё время, пока он
  открыт.
- **Pull request называет задачу**: `Fixes #N` — и issue закроется, когда его вольют.
- **Крупное** — новый инструмент, формат файла, верстак — лучше сначала обсудить в issue. Крупный pull
  request без обсуждения тоже примем: issue под него заведём тогда.
- **Новое issue** проверяет тот, кто до него добрался первым: комментарий «у меня повторяется / нет» со
  сборкой и системой полезнее, чем палец вверх.

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

- **История остаётся линией.** `main` принимает pull request только перемоткой, и мерж-коммит при этом
  выбрасывается. Догоняйте `main` командой `git rebase origin/main`, а не вливанием `main` в свою ветку.

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
- **Новые строки интерфейса обязательны только на английском.** Если вы знаете другие языки каталога,
  добавьте строки и на них. Остальные переводы допишут сопровождающие.
- **Код отформатирован rustfmt.** Перед отправкой запустите `cargo fmt`; раскладку задаёт `rustfmt.toml`,
  и проверки не пропустят дерево, которое `cargo fmt --check` изменил бы. Один раз после клонирования:
  `git config core.hooksPath tools/hooks` — тогда каждый коммит проверит раскладку (секунда), а каждый пуш
  поищет личное.
- **clippy молчит.** Каждое замечание clippy — ошибка в манифесте; перед отправкой запустите `cargo clippy
  --workspace --all-targets`. Замечание исправляют, а не глушат: никаких `#[allow(clippy::...)]`.
- **Значения говорят, что они такое.** Группа значений, которая ходит вместе, — структура с именованными
  полями, а не кортеж, не список кортежей и не псевдоним типа для кортежа. Переключатель — перечисление, а
  не `bool`. Функция не берёт больше семи аргументов и не берёт подряд аргументы одного типа (`bool, bool`,
  `f64, f64, f64`): два соседа одного типа меняются местами, и это всё равно компилируется.

  ```rust
  // не принимается
  fn field(ui: &mut Ui, id: Id, text: &str, hint: &str, valid: &dyn Fn(&str) -> bool, with_list: bool, autofocus: bool)
  let rows: Vec<(&str, fn(&mut App))> = vec![("move", |a| ...)];

  // принимается
  pub struct FieldRules<'a> { pub valid: &'a dyn Fn(&str) -> bool, pub list: NameList, pub focus: Focus }
  fn field(ui: &mut Ui, id: Id, text: &str, hint: &str, rules: FieldRules)
  struct Edit { what: &'static str, make: fn(&mut App) }
  ```
- **Предупреждений ноль.** `dead_code` и `unused_must_use` запрещены в манифесте: написанное и не
  подключённое краснит сборку сразу.
- **Каждый pull request проверяется на Linux** теми же воротами, что и у разработчика: `python3
  tools/gate.py fast` (раскладка, сборка, clippy, правила кода, слова интерфейса, справка, лёгкие
  приёмочные пробы) и собственные тесты каждого крейта. Быстрый уровень, запущенный до отправки,
  сберегает круг. Если проверка покраснела, задание называет её имя, а что она сказала — в артефактах
  прогона `gate-logs-…`.

Сборка из исходников и упаковка описаны в [`packaging/README.md`](packaging/README.md). Ядро — OCCT 7.9.3,
собранное из исходников строкой `cmake` оттуда же (раздел «The kernel»): та версия, на которой программа проверяется,
с `BUILD_RELEASE_DISABLE_EXCEPTIONS=OFF`, чтобы проверки OCCT остались, и с переводом сигналов, чтобы сбой внутри OCCT
становился отказом. Другая версия собирается с предупреждением.
