### Повідомлення про помилки. Ядро повертає КОД; тут — слова для нього.
error-thicken-added-nothing = Пластина зайшла всередину тіла й нічого не додала — вкажіть товщину більше нуля
error-draft-angle-zero = Ухил 0 градусів нічого не нахиляє — вкажіть кут, відмінний від нуля
error-torus-through-itself = Трубка не тонша за кільце — такий тор перетинає сам себе; задайте радіус трубки меншим за радіус кільця
error-array-of-one = Масив з однієї копії — це саме тіло; задайте дві копії або більше
### Підстановки { $name } передають дані з ядра — їх не можна вилучати, це не прикраса.

## Операція не вдалася в геометричному ядрі.
## Підказка в дужках — типова причина; вона заощаджує звернення до підтримки.

error-op-failed-extrude = Видавлювання не вдалося
error-op-failed-extrude-profile = Видавлювання не вдалося (перевірте профіль)
error-op-failed-extrude-contour = Видавлювання не вдалося (перевірте контур)
error-op-failed-revolve = Обертання не вдалося
error-op-failed-revolve-profile = Обертання не вдалося (перевірте профіль)
error-op-failed-revolve-axis = Обертання навколо датум-осі не вдалося (вісь у площині ескізу?)
error-op-failed-sweep = Протягування не вдалося (профіль на початку шляху й приблизно перпендикулярний до нього?)
error-op-failed-loft = Лофт не вдався (перерізи мають бути замкненими й узгодженими)
error-op-failed-loft-boolean = Лофт-булева операція над тілом не вдалася
error-op-failed-boolean = Булева операція не вдалася
error-op-failed-body-boolean = Булева операція над тілами не вдалася (немає перетину або тіла не пов'язані?)
error-op-failed-fillet = Скруглення не вдалося (завеликий радіус або ребра?)
error-op-failed-fillet-var = Змінне скруглення не вдалося (радіуси або ребра?)
error-op-failed-chamfer = Фаска не вдалася (завеликий розмір або ребра?)
error-op-failed-chamfer-asym = Асиметрична фаска не вдалася (катет або кут завеликий?)
error-op-failed-shell = Оболонка не вдалася (товщина або грань?)
error-op-failed-shell-center = Оболонка по центру не вдалася (зміщення або грань?)
error-op-failed-draft = Ухил не вдався (чи нахиляється ця грань під таким кутом від цієї нейтралі?)
error-op-failed-push-face = Грань не зміщується (криволінійна грань або самоперетин)
error-op-failed-remove-faces = Грані не вдалося видалити
error-op-failed-replace-faces = Поверхня не закрила отвір — грань не замінюється
error-op-failed-copy-faces = Грань не копіюється окремою поверхнею
error-op-failed-offset-surface = Зміщення не будується: грань на такій відстані вивертається або зникає — візьміть меншу відстань
error-op-failed-stitch = Листи не зшиваються: жодна кромка не збіглася — схоже, вони не торкаються одне одного
error-op-failed-mesh-recognise = Розпізнати сітку не вдалося: не побудовано жодної грані
error-op-failed-mesh-solid = Сітка не стала тілом: у ній немає жодного трикутника з площею
error-op-failed-trim = Обрізання не вдалося: поверхня й інструмент не перетинаються або різати нічого
error-op-failed-thicken = Грань не потовщується (зміщення перетинає саме себе?)
error-op-failed-split-body = Площина не ріже тіло (пройшла повз або лежить на грані)
error-op-failed-split-faces = Площина не ділить жодної грані (пройшла повз тіло)
error-op-failed-hole = Отвір не вдався (діаметри або глибини?)
error-op-failed-holes = Отвори не вдалися (точки, діаметри або глибини?)
error-op-failed-thread = Різьба не вдалася
error-op-failed-helix = Гвинтове протягування не вдалося
error-op-failed-auger = Шнек не вдався
error-op-failed-mirror = Дзеркало не вдалося
error-op-failed-mirror-plane = Дзеркало за площиною не вдалося
error-op-failed-array = Масив не вдався
error-op-failed-move = Перенесення не вдалося
error-op-failed-transform = Перетворення не вдалося
error-op-failed-cylinder = Циліндр не вдався
error-op-failed-sphere = Сфера не вдалася
error-op-failed-cone = Конус не вдався
error-op-failed-torus = Тор не вдався
error-op-failed-prism = Призма не вдалася
error-op-failed-fuse-profiles = Злиття контурів не вдалося
error-op-failed-place = Розміщення не вдалося

## Операції потрібне справжнє ядро OCCT (відповіла заглушка).
## Користувач зазвичай цього не бачить — отже, збірка без ядра.

error-kernel-required-extrude = Видавлювання вміє лише ядро OCCT
error-kernel-required-mesh-recognise = Розпізнавати сітку вміє лише ядро OCCT
error-kernel-required-mesh-solid = Перетворювати сітку на тіло вміє лише ядро OCCT
error-kernel-required-extrude-profile = Видавлювання вміє лише ядро OCCT
error-kernel-required-extrude-contour = Видавлювання вміє лише ядро OCCT
error-kernel-required-revolve = Обертання вміє лише ядро OCCT
error-kernel-required-revolve-profile = Обертання вміє лише ядро OCCT
error-kernel-required-revolve-axis = Обертання вміє лише ядро OCCT
error-kernel-required-sweep = Протягування вміє лише ядро OCCT
error-kernel-required-loft = Лофт вміє лише ядро OCCT
error-kernel-required-loft-boolean = Лофт-булеву операцію вміє лише ядро OCCT
error-kernel-required-boolean = Булеву операцію вміє лише ядро OCCT
error-kernel-required-body-boolean = Булеву операцію тіл вміє лише ядро OCCT
error-kernel-required-fillet = Скруглення вміє лише ядро OCCT
error-kernel-required-fillet-var = Змінне скруглення вміє лише ядро OCCT
error-kernel-required-chamfer = Фаску вміє лише ядро OCCT
error-kernel-required-chamfer-asym = Асиметричну фаску вміє лише ядро OCCT
error-kernel-required-shell = Оболонку вміє лише ядро OCCT
error-kernel-required-shell-center = Оболонку по центру вміє лише ядро OCCT
error-kernel-required-draft = Ухил вміє лише ядро OCCT
error-kernel-required-push-face = Тягнути грань вміє лише ядро OCCT
error-kernel-required-remove-faces = Видалення граней вміє лише ядро OCCT
error-kernel-required-replace-faces = Заміну грані поверхнею вміє лише ядро OCCT
error-kernel-required-copy-faces = Копію грані вміє лише ядро OCCT
error-kernel-required-offset-surface = Зміщення поверхні вміє лише ядро OCCT
error-kernel-required-thicken = Потовщення вміє лише ядро OCCT
error-kernel-required-stitch = Зшивання вміє лише ядро OCCT
error-kernel-required-trim = Обрізання вміє лише ядро OCCT
error-kernel-required-patch = Латочку вміє лише ядро OCCT
error-kernel-required-split-body = Розділити тіло вміє лише ядро OCCT
error-kernel-required-split-faces = Розподіл граней вміє лише ядро OCCT
error-kernel-required-hole = Отвір вміє лише ядро OCCT
error-kernel-required-holes = Отвори вміє лише ядро OCCT
error-kernel-required-thread = Різьбу вміє лише ядро OCCT
error-kernel-required-helix = Гвинтове протягування вміє лише ядро OCCT
error-kernel-required-auger = Шнек вміє лише ядро OCCT
error-kernel-required-mirror = Дзеркало вміє лише ядро OCCT
error-kernel-required-mirror-plane = Дзеркало вміє лише ядро OCCT
error-kernel-required-array = Масив вміє лише ядро OCCT
error-kernel-required-move = Перенесення вміє лише ядро OCCT
error-kernel-required-transform = Перетворення вміє лише ядро OCCT
error-kernel-required-cylinder = Циліндр вміє лише ядро OCCT
error-kernel-required-sphere = Сферу вміє лише ядро OCCT
error-kernel-required-cone = Конус вміє лише ядро OCCT
error-kernel-required-torus = Тор вміє лише ядро OCCT
error-kernel-required-prism = Призму вміє лише ядро OCCT
error-kernel-required-fuse-profiles = Злиття контурів вміє лише ядро OCCT
error-kernel-required-place = Розміщення вміє лише ядро OCCT

## Входи, яких немає або які застаріли

error-source-body-not-built = Тіло-джерело не побудовано — спочатку виправте побудову вище на стрічці
error-source-body-deleted = Те, на чому воно побудоване, видалено — виберіть інше тіло або видаліть цю побудову
error-body-in-pieces = Операція розбиває деталь на окремі шматки — деталь це одне тіло; зробіть так, щоб додаток торкався тіла, або створіть нову деталь
error-body-in-one-piece = Тіло з одного шматка — відокремлювати в деталь нічого
error-source-part-has-no-body = У деталі-джерела немає тіла
error-body-a-not-built = Тіло A не побудовано
error-body-b-not-built = Тіло B не побудовано
error-face-not-found = Грані більше немає в тілі-джерелі — посилання застаріло
error-faces-not-found = Граней більше немає в тілі-джерелі — посилання застаріли
error-profile-not-found = Профіль ескізу не знайдено
error-revolve-profile-crosses-axis = Профіль перетинає вісь обертання — так тіло обертання не будується в жодному CAD. Притисніть профіль до осі (половина перерізу: півколо замість кола) або посуньте вісь за профіль.
error-sweep-profile-missing = Профіль протягування не знайдено
error-sweep-path-missing = Траєкторію протягування не знайдено
error-no-isolated-points-for-holes = В ескізі немає ізольованих точок для розташування отворів
error-no-points-for-holes = Немає точок для розташування отворів

## Площини-посилання

error-cut-plane-deleted = Площину різу видалено — виберіть іншу або видаліть розріз
error-sketch-face-gone = Грань, на якій стоїть ескіз, зникла: тіло, якому вона належала, видалено. Перенесіть ескіз на іншу грань або площину, або скасуйте видалення
error-sketch-plane-gone = Робочу площину, на якій стоїть ескіз, видалено. Перенесіть ескіз на іншу площину або грань, або скасуйте видалення
error-mirror-plane-deleted = Площину дзеркала видалено — виберіть іншу або видаліть дзеркало
error-split-plane-deleted = Площину поділу видалено — виберіть іншу або видаліть операцію
error-mirror-plane-unset = Площину дзеркала не задано — створіть дзеркальну деталь наново
error-zero-normal = Нормаль площини нульова — напрямок не задано

## Значення, у яких немає сенсу

error-zero-thickness = Нульова товщина — пластини не вийде
error-zero-push-distance = Нульове зміщення — тягнути грань нікуди
error-broken-solid = Ядро повернуло непридатне тіло — операцію скасовано, деталь залишилася попередньою. Найчастіше так буває, коли грань межує зі скругленням або фаскою: спробуйте менше зміщення або розмістіть операцію в стрічці ДО скруглення
error-split-piece-count = Площина ріже тіло на { $got } частини замість { $want } — поверніть площину або створіть розріз наново
error-loft-needs-two-sections = Лофту потрібно щонайменше два замкнених перерізи
error-draft-needs-faces = Для ухилу потрібні грані для нахилу та нейтральна грань
error-no-contours = Немає контурів для операції
error-all-edges-smooth = Усі вибрані ребра — гладкі стики (межі скруглень): скругляти й знімати фаску ніде
error-fillet-radius-too-big = Скруглення R{ $radius } не вдалося: { $issues }{ $smooth }
# Одне ребро з цього списку. «бере до» підказує найбільший радіус, який би підійшов.
error-fillet-edge-takes-up-to = ребро { $edge } (бере до { $max })
error-fillet-edge-takes-none = ребро { $edge } (не бере жодного радіуса — впирається в дотичний стик попереднього скруглення; зніміть це ребро або скругліть сусіднє раніше)
error-fillet-smooth-skipped = ; гладких стиків пропущено автоматично: { $n }
error-fillet-edges-one-by-one = Скруглення R{ $radius }: ці ребра беруться лише по одному — сусідні скруглення перетинаються
error-chamfer-too-big = Фаска { $dist } мм не вдалася — катет більший за сторону
error-surface-does-not-close = Поверхня не збіглася з прорізом: { $n } кромок залишилися без пари. Схоже, вибрано різні межі — будувати латочку потрібно за тими ж кромками, що обводять замінну грань
error-push-face-on-sheet = Тягнути грань поверхні неможливо: це операція для тіл. Щоб надати поверхні товщину, застосуйте «Потовщення»
error-needs-solid-not-sheet = Це інструмент для тіл: до поверхні він не застосовується. Надайте поверхні товщину — і працюйте з нею як зі звичайним тілом
error-draft-failed = Ухил { $angle }° не взявся на цих гранях. Найчастіше заважає тонка стінка: після оболонки нахиляти майже нічого — встановіть ухил ДО оболонки або візьміть менший кут

## Різьба та шнек

error-thread-rim-not-found = Обід циліндра або отвору (кругле ребро) не знайдено
error-thread-length-unset = Довжину різьби не задано
error-thread-pitch-too-small = Крок { $pitch } мм замалий
error-thread-too-many-turns = { $turns } витків — забагато; збільшіть крок або вкоротіть різьбу
error-thread-longer-than-face = Різьба завдовжки { $length } мм довша за циліндр ({ $face } мм). Вкоротіть різьбу
error-thread-depth-too-deep = Глибина витка { $depth } мм не менша за радіус { $radius } мм: для Ø{ $dia } крок { $pitch } завеликий
error-thread-not-its-size = Різьба Ø{ $nominal } не підходить до грані Ø{ $face } — виберіть розмір за гранню або грань за розміром
warn-edges-dropped = { $dropped } з { $asked } ребер не взялися й залишилися гострими, решту зроблено — відкрийте вузол подвійним клацанням, щоб вибрати інші ребра або інший розмір
error-thread-removed-nothing = Різьба нічого не зняла ({ $before } -> { $after } мм³) — перевірте вибрану грань, крок і довжину
error-thread-failed = Різьба не побудувалася (перевірте крок, довжину й діаметр)
error-auger-rim-not-found = Обід вала (кругле ребро) не знайдено
error-auger-bad-pitch-or-length = У шнека крок і довжина мають бути більшими за нуль
error-auger-outer-not-bigger = Зовнішній Ø{ $outer } шнека не більший за вал Ø{ $shaft }
error-auger-added-nothing = Стрічка шнека нічого не додала ({ $before } -> { $after } мм³) — перевірте зовнішній Ø і вибраний вал
error-auger-flight-failed = Стрічка шнека не побудувалася (перевірте крок, товщину й зовнішній діаметр)

## Ізоляція: геометрія належить деталі

error-body-only-in-part = Тіло можна будувати лише всередині Деталі (Збірка тіл не тримає)
error-cross-component-input = Кроскомпонентне посилання заборонено: вхід { $input } належить іншому компоненту
error-sketch-on-foreign-face = Ескіз входу { $input } посаджено на грань тіла іншого компонента без зовнішнього посилання
error-sketch-face-ref-lost = Опорну грань ескізу на тілі { $body } не знайдено за назвою після перебудови — взято найближчий збіг, перевірте розміщення елемента

## Порожні результати

error-array-empty = Масив нічого не дав
error-empty-result = Результат — порожнє тіло
error-remove-faces-failed = Грані не видалити: { $why }

## Збірка

error-joint-unsatisfied = З'єднання не виконано — нев'язка { $residual } мм

## Вирази

error-expr-unknown-char = Невідомий символ «{ $what }»
error-expr-unknown-fn = Невідома функція «{ $what }»
error-expr-unknown-name = невідома назва: { $what } — немає такого параметра
error-expr-needs-one-arg = { $what }() очікує один аргумент
error-expr-needs-two-args = { $what }() очікує два аргументи
error-expr-expected-paren = Очікувалася «)»
error-expr-expected-paren-after-args = Очікувалася «)» після аргументів
error-expr-unexpected-token = Неочікуваний токен { $what }
error-expr-unexpected-end = вираз обривається: очікувалося число або назва
error-expr-trailing-input = Зайве введення біля «{ $what }»
error-expr-not-a-number = Результат не є числом (ділення на нуль?)

## Повідомлення самого ядра — не перекладається: це діагностика, а не фраза для користувача.

error-kernel-message = Ядро: { $message }

# ── МІСТ ДО ЯДРА ГЕОМЕТРІЇ (OCCT): ядро мови не має, воно віддає коди ──
cad-no-faces-picked = не вибрано жодної грані
cad-faces-not-in-body = вибраних граней немає в тілі (посилання застаріло)
cad-neighbours-not-extendable = сусідні поверхні не подовжуються — видаляється суцільний елемент (отвір, бобишка)
cad-file-not-found = Файл не знайдено: { $v }
cad-step-no-shapes = STEP: не вдалося прочитати тіла
cad-step-nothing-to-export = STEP: немає тіл для експорту
cad-step-write-failed = STEP: запис не вдався (код { $v })
cad-step-read-failed = STEP: не вдалося прочитати або передати геометрію
cad-iges-no-shapes = IGES: не вдалося прочитати тіла
cad-iges-read-failed = IGES: не вдалося прочитати або передати геометрію
cad-iges-empty-tessellation = IGES: у файлі немає поверхні, яку можна показати
cad-iges-nothing-to-export = IGES: записувати нічого
cad-iges-write-failed = IGES: запис не вдався (код { $v })
io-iges-read-failed = IGES: файл не читається ({ $v })
io-iges-not-iges = Це не IGES: у файлі немає розділів, з яких він складається
io-iges-no-curves = IGES: у файлі немає ані поверхонь, ані кривих, які можна показати
io-obj-read-failed = OBJ: файл не читається ({ $v })
io-obj-bad-line = OBJ: рядок { $v } не розібрано
io-obj-bad-index = OBJ: у рядку { $v } грань посилається на вершину, якої немає
io-obj-no-faces = OBJ: у файлі немає жодної грані
io-obj-no-triangles = OBJ: записувати нічого
io-obj-write-failed = OBJ: запис не вдався ({ $v })
io-obj-not-finite-line = OBJ: значення в рядку { $v } не є скінченним числом ({ $w })
io-ply-read-failed = PLY: файл не читається ({ $v })
io-ply-not-ply = Це не PLY: файл не починається із заголовка PLY
io-ply-bad-header = PLY: заголовок не розібрано
io-ply-truncated = PLY: файл обривається раніше, ніж обіцяє його заголовок
io-ply-bad-index = PLY: грань посилається на вершину, якої немає
io-ply-no-faces = PLY: у файлі немає жодної грані
io-ply-no-triangles = PLY: записувати нічого
io-ply-write-failed = PLY: запис не вдався ({ $v })
io-ply-not-finite-line = PLY: значення в рядку { $v } не є скінченним числом ({ $w })
io-ply-not-finite-vertex = PLY: координата вершини { $v } (байт { $w }) не є скінченним числом ({ $x })
io-gltf-read-failed = glTF: файл не читається ({ $v })
io-gltf-not-gltf = Це не glTF: у файлі немає опису сцени
io-gltf-truncated = glTF: двійковий файл обривається
io-gltf-bad-node = glTF: вузол сцени посилається на те, чого немає
io-gltf-no-positions = glTF: у сітки немає координат вершин
io-gltf-bad-index = glTF: трикутник посилається на вершину, якої немає
io-gltf-bad-accessor = glTF: дані сітки описано неправильно
io-gltf-no-buffer = glTF: у файлу немає двійкової частини, яку він називає
io-gltf-bad-buffer = glTF: вбудовані дані не розшифровуються
io-gltf-missing-buffer = glTF: поруч із файлом немає його даних «{ $v }»
io-gltf-no-meshes = glTF: у сцені немає жодної сітки
io-gltf-no-triangles = glTF: записувати нічого
io-gltf-write-failed = glTF: запис не вдався ({ $v })
io-gltf-not-finite = glTF: елемент { $w } аксесора { $v } (байт { $x }) не є скінченним числом ({ $y })
io-3mf-read-failed = 3MF: файл не читається ({ $v })
io-3mf-not-3mf = Це не 3MF: файл не є архівом пакета
io-3mf-no-model = 3MF: у пакеті немає моделі
io-3mf-bad-model = 3MF: модель описано неправильно
io-3mf-unknown-unit = 3MF: невідома одиниця «{ $v }»
io-3mf-bad-transform = 3MF: перетворення записано неправильно
io-3mf-no-object = 3MF: модель посилається на об'єкт { $v }, якого немає
io-3mf-bad-index = 3MF: трикутник посилається на вершину, якої немає
io-3mf-no-meshes = 3MF: у моделі немає жодної сітки
io-3mf-no-triangles = 3MF: записувати нічого
io-3mf-write-failed = 3MF: запис не вдався ({ $v })
io-3mf-not-finite-line = 3MF: значення в рядку { $v } не є скінченним числом ({ $w })
io-amf-read-failed = AMF: файл не читається ({ $v })
io-amf-not-amf = Це не AMF: у файлі немає розмітки AMF
io-amf-unknown-unit = AMF: невідома одиниця «{ $v }»
io-amf-bad-vertex = AMF: вершину записано неправильно
io-amf-bad-triangle = AMF: трикутник записано неправильно
io-amf-bad-index = AMF: трикутник посилається на вершину, якої немає
io-amf-bad-constellation = AMF: сузір'я описано неправильно
io-amf-no-object = AMF: сузір'я посилається на об'єкт { $v }, якого немає
io-amf-no-meshes = AMF: у файлі немає жодної сітки
io-amf-no-triangles = AMF: записувати нічого
io-amf-write-failed = AMF: запис не вдався ({ $v })
io-amf-not-finite-line = AMF: значення в рядку { $v } не є скінченним числом ({ $w })
cad-step-empty-tessellation = STEP: порожня теселяція (немає тіл/граней?)
cad-extrude-needs-3-points = профіль для видавлювання повинен мати >=3 точок
cad-extrude-failed = OCCT: не вдалося видавити профіль (самоперетин?)
cad-extrude-empty = видавлювання дало порожнє тіло
cad-revolve-needs-3-points = профіль для обертання повинен мати >=3 точок
cad-revolve-failed = OCCT: обертання не вдалося (профіль перетинає вісь?)
cad-revolve-empty = обертання дало порожнє тіло
cad-boolean-needs-3-points = обидва профілі повинні мати >=3 точок
cad-boolean-failed = OCCT: булева операція не вдалася
cad-boolean-empty = булева операція дала порожнє тіло

# ── ФАЙЛОВИЙ ШАР: коди надходять з qymcad-io, аргумент — шлях і текст ОС ──
io-file-create = не вдалося створити { $v }
io-file-replace = не вдалося замінити { $v }
io-file-read = не вдалося прочитати { $v }
io-not-a-qpart = це не .qpart (не zip-контейнер)
io-not-a-qcad = це не .qcad (не zip-контейнер): старий формат не підтримується
io-refuse-empty-over-full = відмова: порожній документ поверх непорожнього файлу ({ $v } вузлів) — збережіть як новий файл
io-stl-read-failed = STL: файл не читається ({ $v })
io-stl-truncated = STL: файл обривається раніше, ніж закінчуються трикутники за його заголовком
io-stl-bad-facet = STL: грань не розібрано
io-stl-no-faces = STL: у файлі немає жодного трикутника
io-stl-no-triangles = STL: немає трикутників для експорту
io-stl-too-many-triangles = STL: забагато трикутників
io-stl-write-failed = STL: запис не вдався: { $v }
io-stl-not-finite-line = STL: значення в рядку { $v } не є скінченним числом ({ $w })
io-stl-not-finite-triangle = STL: координата трикутника { $v } (байт { $w }) не є скінченним числом ({ $x })

io-svg-empty-sketch = SVG: порожній ескіз
io-svg-write-failed = SVG: запис не вдався: { $v }
io-dxf-empty-sketch = DXF: порожній ескіз
io-dxf-write-failed = DXF: запис не вдався: { $v }
error-edges-not-found = З { $asked } названих кромок у тілі не залишилося жодної. Їхні назви видала операція вище на стрічці, і вона змінилася — виберіть кромки наново.
error-described-edges-not-found = Кромки були вибрані через грань або кромку, якої в тілі більше немає: її змінила операція вище на стрічці — виберіть кромки наново.
error-op-failed-patch = Поверхня не натягується на ці кромки
error-shell-thickness-over-round = Стінка { $t } мм товща за найдрібніше скруглення на тілі ({ $r } мм): зміщення з'їдає його повністю, і оболонка не будується. Візьміть стінку тоншу за { $r } мм або збільште скруглення
error-operation-split-body = Операція розділила деталь на { $n } тіл(а): у деталі може бути лише одне тіло. Зменште значення або застосуйте операцію до іншої грані
error-mirror-of-hollow-body = Дзеркало порожнистої деталі за її власною гранню ядру поки не вдається: злиття половинок залишає зайві оболонки. Віддзеркальте деталь до оболонки або виберіть іншу площину
error-shell-of-multi-shell-body = Оболонку на тілі з { $n } оболонок ядро не будує: тіло вже порожнисте або зібране з копій (масив, дзеркало). Зробіть оболонку раніше — до масиву, дзеркала або другої оболонки
error-shell-not-built-here = Оболонку на цьому тілі побудувати не вдалося: зміщення граней зазнає збою всередині ядра. Спробуйте іншу товщину стінки або зробіть оболонку раніше в історії, поки тіло простіше
error-cut-removed-nothing = Виріз не зняв нічого: інструмент не перетинає деталь. Перевірте, де розташований інструмент і на яку глибину йде виріз
error-stitch-nothing-joined = Зшивати нічого: у вибраних поверхонь немає спільних кромок — вони не стикаються. Після скруглень сусідні грані розділені скругленою смугою; беріть поверхні, які дійсно стикуються
