<div align="center">

<img src="assets/logo.png" width="112" alt="QymCAD">

# QymCAD

**Параметричний асоціативний CAD на ядрі B-rep**

Ескіз → деталь → збірка. Одна програма, один файл проєкту, без хмари та підписки.

[cad.qymis.tech](https://cad.qymis.tech)

[![CI](https://github.com/QymIs-Tech/QymCAD/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/QymIs-Tech/QymCAD/actions/workflows/checks.yml)
[![Release](https://img.shields.io/github/v/release/QymIs-Tech/QymCAD?logo=github)](https://github.com/QymIs-Tech/QymCAD/releases)
[![License: AGPL 3.0](https://img.shields.io/badge/License-AGPL_3.0-blue.svg)](LICENSE)
<br>
[![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011%20x64-0078D6?logo=windows&logoColor=white)](https://github.com/QymIs-Tech/QymCAD/releases)
[![Linux](https://img.shields.io/badge/Linux-x86__64-FCC624?logo=linux&logoColor=black)](https://github.com/QymIs-Tech/QymCAD/releases)
[![macOS](https://img.shields.io/badge/macOS-Apple%20Silicon-000000?logo=apple&logoColor=white)](https://github.com/QymIs-Tech/QymCAD/releases)

[English](README.md) | [Русский](README.ru.md) | **Українська** | [Қазақша](README.kk.md)

<img src="docs/screenshots/01-assembly.png" width="900" alt="Верстат, зібраний у QymCAD: компоненти, з'єднання та їхні ступені вільності">

</div>

## Що це

QymCAD — настільний САПР для механічних деталей та збірок: корпусів, кронштейнів, механізмів,
друкованих та фрезерованих виробів.

Ескіз перетворюється на тіло видавлюванням, обертанням або замітанням (sweep); тіло доповнюється отворами,
скругленнями, оболонкою, масивами. Деталі збираються у вузол з'єднаннями — обертальними, повзунами,
жорсткими. Модель параметрична: зміна розміру в будь-якій операції перебудовує все, що розташоване
нижче за шкалою часу (timeline), включно зі збіркою.

Геометрія точна, об'ємна (B-rep) на ядрі [OpenCASCADE](https://dev.opencascade.org/) — тому самому, на
якому працює FreeCAD. Звідси точні поверхні замість полігональних сіток, коректні скруглення та обмін
через STEP з іншими САПР.

Інтерфейс доступний українською, англійською, казахською та російською мовами.

## Стан

Версія для розробки. Програма працює та придатна для реальних деталей, але оновлюється щодня.

- **Формат документа змінюється без зворотної сумісності.** Файл, збережений у попередній версії, може
  не відкритися. Для таких файлів є скрипт `convert_qcad.py` (див. нижче).
- **Модуль ЧПК (CAM) не працює.** У налаштуваннях є прапорець «Вкладка „Обробка“ (CAM)», але це заділ
  на майбутнє: частина коду залишилася від попередньої версії та не підтримується. Модуль повернеться
  у стабільній альфі.
- **Збірка для macOS не має підпису Apple.** macOS позначає завантажену програму як поміщену в карантин
  і відмовляється відкривати, повідомляючи про пошкодження — це не так. Позначка знімається один раз
  однією командою, покроково описаною в інструкціях усередині архіву. Тільки для Apple Silicon; збірки
  під Intel немає.

## Можливості

**Ескіз.** Лінії, дуги, кола, еліпси, сплайни, багатокутники, пази, текст. Зв'язки та розміри
вирішуються спільно; розміри параметричні (`w/2`, `sin(a)`). Є допоміжна геометрія та
проєкція ребер тіла в ескіз.

**Деталь.** Видавлювання, обертання, замітання (sweep), лофт, скруглення (зокрема змінного радіуса за
вершинами), фаска, оболонка, ухил, отвори (прості, цекування, зенкування), потовщення, латка,
зшивання, обрізання, поділ граней і тіла, копіювання та штовхання грані, лінійні й кругові масиви,
дзеркало, булеві операції. Є примітиви: паралелепіпед, циліндр, сфера, конус, тор, призма.
Різьба будується справжнім гвинтовим профілем зі збігами.

**Збірка.** Деталі й підзбірки, з'єднання (жорстке, обертальне, повзун, циліндричне, плоске,
кульове, штифт у пазу, паралельність), обмеження та приводи, ступені вільності, перевірка
інтерференції (перетинів). Ескіз можна будувати на грані сусідньої деталі як асоціативне посилання:
зміна сусідньої деталі автоматично перебудовує залежну.

**Обмін.** STEP (імпорт та експорт), STL (імпорт та експорт), DXF і SVG (імпорт та експорт
ескізів). Експортувати можна як увесь проєкт, так і окрему деталь чи підзбірку з дерева побудови.

## Встановлення

Збірки розміщені у розділі [Releases](https://github.com/QymIs-Tech/QymCAD/releases). Додаткові бібліотеки не потрібні: OpenCASCADE та залежності вже запаковані всередині.

<details>
<summary><b>Windows (10 / 11 x64)</b></summary>

- **Портативна версія**: завантажте `qymcad-win64.zip`, розархівуйте в будь-яку папку та запустіть `qymcad.exe`.
- **MSI-інсталятор**: завантажте та запустіть `qymcad-*-x64.msi` для встановлення в систему.
- **winget**:
  ```powershell
  winget install QymIsTech.QymCAD
  ```

</details>

<details>
<summary><b>Linux (x86_64)</b></summary>

Потребує glibc 2.35 або новішої (Ubuntu 22.04+, Debian 12+, Fedora 36+, Arch Linux).

- **AppImage** (один файл):
  ```bash
  chmod +x qymcad-*.AppImage
  ./qymcad-*.AppImage
  ```
- **Arch Linux (AUR)**:
  ```bash
  yay -S qymcad-bin
  ```
- **Flatpak**:
  ```bash
  flatpak install flathub tech.qymis.cad
  ```

</details>

<details>
<summary><b>macOS 12+ (Apple Silicon)</b></summary>

Завантажте `qymcad-*-macos-arm64.zip` та розархівуйте його. Збірка не має підпису Apple, тому перед першим запуском слід один раз зняти позначку карантину в Терміналі:

```bash
xattr -cr QymCAD.app
```

Після цього відкрийте `QymCAD.app` звичайним подвійним клацанням.

*Примітка: тільки для Apple Silicon (M1/M2/M3/M4); збірки під Intel немає.*

</details>

## Довідка

Вбудована довідка відкривається клавішею **F1**. Ілюстровані статті розташовані в [`docs/help`](docs/help).

## Файли проєкту

Документ зберігається як `.qcad`. Формат змінюється напряму, без шару сумісності; документи
попередніх версій перетворюються окремим скриптом:

```bash
python3 convert_qcad.py part.qcad    # оновлює файл на місці, поруч залишає part.qcad.bak
```

Скрипт переводить документ будь-якої попередньої версії до поточної за один прохід і є ідемпотентним.

## Збирання з вихідного коду

Linux — `just pkg-linux`, потрібен Docker. Windows — MSVC, ядро збирається з вихідного коду. macOS — ядро
також із вихідного коду, далі `packaging/macos/bundle.sh`. Детальніше:
[`packaging/README.md`](packaging/README.md).

## Участь у розробці

Ми раді будь-якому внеску — коду, покращенню документації, новим перекладам та звітам про помилки. Ознайомтеся з [інструкцією для контриб'юторів](CONTRIBUTING.md) і оберіть задачу з міткою [good first issue](https://github.com/QymIs-Tech/QymCAD/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22), щоб почати.

## Контриб'ютори

<a href="https://github.com/QymIs-Tech/QymCAD/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=QymIs-Tech/QymCAD" alt="Contributors" />
</a>

## Історія зірок

<a href="https://star-history.com/#QymIs-Tech/QymCAD&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=QymIs-Tech/QymCAD&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=QymIs-Tech/QymCAD&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=QymIs-Tech/QymCAD&type=Date" />
 </picture>
</a>

## Ліцензія

Код розповсюджується під ліцензією [AGPL-3.0-or-later](LICENSE): форк та будь-яка похідна збірка залишаються
під тією ж ліцензією з відкритим вихідним кодом.

Ядро OpenCASCADE постачається під ліцензією LGPL-2.1 із винятком та лінкується динамічно; шрифти
й значки — під власними ліцензіями. Усе це перелічено в
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md), цей файл додається до кожної збірки поруч із програмою.

---

Майстерня **QymIs Tech** — [qymis.tech](https://qymis.tech). Автор: Денис Казаченков.
