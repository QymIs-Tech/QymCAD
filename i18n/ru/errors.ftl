### Сообщения об ошибках. Ядро сообщает КОД; здесь — слова к нему.
### Подстановки { $name } несут данные из ядра — их нельзя выкидывать, это не украшение.

## Операция не удалась в геометрическом ядре.
## Подсказка в скобках — обычная причина; она экономит обращение в поддержку.

error-op-failed-extrude = Выдавливание не удалось
error-op-failed-extrude-profile = Выдавливание не удалось (проверьте профиль)
error-op-failed-extrude-contour = Выдавливание не удалось (проверьте контур)
error-op-failed-revolve = Вращение не удалось
error-op-failed-revolve-profile = Вращение не удалось (проверьте профиль)
error-op-failed-revolve-axis = Вращение вокруг датум-оси не удалось (ось в плоскости эскиза?)
error-op-failed-sweep = Протягивание не удалось (профиль в начале пути и примерно перпендикулярен ему?)
error-op-failed-loft = Лофт не удался (сечения должны быть замкнуты и согласованы)
error-op-failed-loft-boolean = Лофт-булева над телом не удалась
error-op-failed-boolean = Булева не удалась
error-op-failed-body-boolean = Булева тел не удалась (нет пересечения или тела не связаны?)
error-op-failed-fillet = Скругление не удалось (радиус велик или рёбра?)
error-op-failed-fillet-var = Переменное скругление не удалось (радиусы или рёбра?)
error-op-failed-chamfer = Фаска не удалась (размер велик или рёбра?)
error-op-failed-chamfer-asym = Асимметричная фаска не удалась (катет или угол велик?)
error-op-failed-shell = Оболочка не удалась (толщина или грань?)
error-op-failed-shell-center = Оболочка по центру не удалась (смещение или грань?)
error-op-failed-draft = Уклон не удался (эта грань наклоняема под таким углом от такой нейтрали?)
error-op-failed-push-face = Грань не тянется (кривая грань или самопересечение)
error-op-failed-remove-faces = Грани не убрать
error-op-failed-replace-faces = Поверхность не закрыла отверстие — грань не заменяется
error-op-failed-copy-faces = Грань не копируется отдельной поверхностью
error-op-failed-stitch = Листы не сшиваются: ни одна кромка не совпала — похоже, они друг друга не касаются
error-op-failed-trim = Обрезка не удалась: поверхность и инструмент не пересекаются или резать нечего
error-op-failed-thicken = Грань не утолщается (офсет сам себя пересёк?)
error-op-failed-split-body = Плоскость не режет тело (прошла мимо или лежит по грани)
error-op-failed-split-faces = Плоскость не делит ни одной грани (прошла мимо тела)
error-op-failed-hole = Отверстие не удалось (диаметры или глубины?)
error-op-failed-holes = Отверстия не удались (точки, диаметры или глубины?)
error-op-failed-thread = Резьба не удалась
error-op-failed-helix = Винтовая протяжка не удалась
error-op-failed-auger = Шнек не удался
error-op-failed-mirror = Зеркало не удалось
error-op-failed-mirror-plane = Зеркало по плоскости не удалось
error-op-failed-array = Массив не удался
error-op-failed-move = Перенос не удался
error-op-failed-transform = Преобразование не удалось
error-op-failed-cylinder = Цилиндр не удался
error-op-failed-sphere = Сфера не удалась
error-op-failed-cone = Конус не удался
error-op-failed-torus = Тор не удался
error-op-failed-prism = Призма не удалась
error-op-failed-fuse-profiles = Слияние контуров не удалось
error-op-failed-place = Размещение не удалось

## Операции нужно настоящее ядро OCCT (ответила заглушка).
## Пользователь обычно этого не видит — значит, сборка без ядра.

error-kernel-required-extrude = Выдавливание умеет только ядро OCCT
error-kernel-required-extrude-profile = Выдавливание умеет только ядро OCCT
error-kernel-required-extrude-contour = Выдавливание умеет только ядро OCCT
error-kernel-required-revolve = Вращение умеет только ядро OCCT
error-kernel-required-revolve-profile = Вращение умеет только ядро OCCT
error-kernel-required-revolve-axis = Вращение умеет только ядро OCCT
error-kernel-required-sweep = Протягивание умеет только ядро OCCT
error-kernel-required-loft = Лофт умеет только ядро OCCT
error-kernel-required-loft-boolean = Лофт-булева умеет только ядро OCCT
error-kernel-required-boolean = Булева умеет только ядро OCCT
error-kernel-required-body-boolean = Булева тел умеет только ядро OCCT
error-kernel-required-fillet = Скругление умеет только ядро OCCT
error-kernel-required-fillet-var = Переменное скругление умеет только ядро OCCT
error-kernel-required-chamfer = Фаска умеет только ядро OCCT
error-kernel-required-chamfer-asym = Асимметричная фаска умеет только ядро OCCT
error-kernel-required-shell = Оболочка умеет только ядро OCCT
error-kernel-required-shell-center = Оболочка по центру умеет только ядро OCCT
error-kernel-required-draft = Уклон умеет только ядро OCCT
error-kernel-required-push-face = Тянуть грань умеет только ядро OCCT
error-kernel-required-remove-faces = Удаление граней умеет только ядро OCCT
error-kernel-required-replace-faces = Замена грани поверхностью умеет только ядро OCCT
error-kernel-required-copy-faces = Копия грани умеет только ядро OCCT
error-kernel-required-thicken = Утолщение умеет только ядро OCCT
error-kernel-required-split-body = Разделить тело умеет только ядро OCCT
error-kernel-required-split-faces = Деление граней умеет только ядро OCCT
error-kernel-required-hole = Отверстие умеет только ядро OCCT
error-kernel-required-holes = Отверстия умеет только ядро OCCT
error-kernel-required-thread = Резьба умеет только ядро OCCT
error-kernel-required-helix = Винтовая протяжка умеет только ядро OCCT
error-kernel-required-auger = Шнек умеет только ядро OCCT
error-kernel-required-mirror = Зеркало умеет только ядро OCCT
error-kernel-required-mirror-plane = Зеркало умеет только ядро OCCT
error-kernel-required-array = Массив умеет только ядро OCCT
error-kernel-required-move = Перенос умеет только ядро OCCT
error-kernel-required-transform = Преобразование умеет только ядро OCCT
error-kernel-required-cylinder = Цилиндр умеет только ядро OCCT
error-kernel-required-sphere = Сфера умеет только ядро OCCT
error-kernel-required-cone = Конус умеет только ядро OCCT
error-kernel-required-torus = Тор умеет только ядро OCCT
error-kernel-required-prism = Призма умеет только ядро OCCT
error-kernel-required-fuse-profiles = Слияние контуров умеет только ядро OCCT
error-kernel-required-place = Размещение умеет только ядро OCCT

## Входы, которых нет или которые устарели

error-source-body-not-built = Тело-источник не построено — сначала почини фичу выше по ленте
error-source-part-has-no-body = У детали-источника нет тела
error-body-a-not-built = Тело A не построено
error-body-b-not-built = Тело B не построено
error-face-not-found = Грани больше нет в теле-источнике — ссылка устарела
error-faces-not-found = Граней больше нет в теле-источнике — ссылки устарели
error-profile-not-found = Профиль эскиза не найден
error-revolve-profile-crosses-axis = Профиль пересекает ось вращения — так тело вращения не строится ни в одном CAD. Прижмите профиль к оси (половина сечения: полукруг вместо круга) или отодвинь ось за профиль.
error-sweep-profile-missing = Профиль протяжки не найден
error-sweep-path-missing = Траектория протяжки не найдена
error-no-isolated-points-for-holes = В эскизе нет изолированных точек, куда ставить отверстия
error-no-points-for-holes = Нет точек, куда ставить отверстия

## Плоскости-ссылки

error-cut-plane-deleted = Плоскость реза удалена — выберите другую или удалите разрез
error-mirror-plane-deleted = Плоскость зеркала удалена — выберите другую или удалите зеркало
error-split-plane-deleted = Плоскость деления удалена — выберите другую или удалите операцию
error-mirror-plane-unset = Плоскость зеркала не задана — пересоздай зеркальную деталь
error-zero-normal = Нормаль плоскости нулевая — направление не задано

## Значения, в которых нет смысла

error-zero-thickness = Нулевая толщина — пластины не выйдет
error-zero-push-distance = Нулевое смещение — тянуть грань некуда
error-broken-solid = Ядро вернуло негодное тело — операция отменена, деталь осталась прежней. Чаще всего так выходит, когда грань граничит со скруглением или фаской: попробуйте меньше смещение или поставьте операцию в ленте ДО скругления
error-split-piece-count = Плоскость режет тело на { $got } части вместо { $want } — верни плоскость или пересоздай разрез
error-loft-needs-two-sections = Лофту нужно минимум два замкнутых сечения
error-draft-needs-faces = Уклону нужны наклоняемые грани и нейтральная грань
error-no-contours = Нет контуров для операции
error-all-edges-smooth = Все выбранные рёбра — гладкие стыки (границы скруглений): скруглять и снимать фаску нечего
error-fillet-radius-too-big = Скругление R{ $radius } не взялось: { $issues }{ $smooth }
# Одно ребро из этого списка. «берёт до» подсказывает наибольший радиус, который бы прошёл.
error-fillet-edge-takes-up-to = ребро { $edge } (берёт до { $max })
error-fillet-edge-takes-none = ребро { $edge } (не берёт ни один радиус — упирается в касательный стык прежнего скругления; сними это ребро или скругли соседнее раньше)
error-fillet-smooth-skipped = ; гладких стыков пропущено автоматически: { $n }
error-fillet-edges-one-by-one = Скругление R{ $radius }: эти рёбра берутся только по одному — соседние скругления пересекаются
error-chamfer-too-big = Фаска { $dist } мм не удалась — катет больше стороны
error-surface-does-not-close = Поверхность не совпала с проёмом: { $n } кромок остались без пары. Похоже, выбраны разные границы — строить заплатку надо по тем же кромкам, что обводят заменяемую грань
error-push-face-on-sheet = Тянуть грань поверхности нечем: это операция для тел. Чтобы дать поверхности толщину, примените «Утолщение»
error-needs-solid-not-sheet = Это инструмент для тел: к поверхности он не применяется. Дайте поверхности толщину — и работайте с ней как с обычным телом
error-draft-failed = Уклон { $angle }° не взялся на этих гранях. Чаще всего мешает тонкая стенка: после оболочки наклонять почти нечего — поставьте уклон ДО оболочки или возьмите угол меньше

## Резьба и шнек

error-thread-rim-not-found = Обод цилиндра или отверстия (круглое ребро) не найден
error-thread-length-unset = Длина резьбы не задана
error-thread-pitch-too-small = Шаг { $pitch } мм слишком мал
error-thread-too-many-turns = { $turns } витков — слишком много; увеличь шаг или укороти резьбу
error-thread-depth-too-deep = Глубина витка { $depth } мм не меньше радиуса { $radius } мм: для Ø{ $dia } шаг { $pitch } слишком крупный
error-thread-removed-nothing = Резьба ничего не сняла ({ $before } -> { $after } мм³) — проверьте выбранную грань, шаг и длину
error-thread-failed = Резьба не построилась (проверьте шаг, длину и диаметр)
error-auger-rim-not-found = Обод вала (круглое ребро) не найден
error-auger-bad-pitch-or-length = У шнека шаг и длина обязаны быть больше нуля
error-auger-outer-not-bigger = Наружный Ø{ $outer } шнека не больше вала Ø{ $shaft }
error-auger-added-nothing = Лента шнека ничего не добавила ({ $before } -> { $after } мм³) — проверьте наружный Ø и выбранный вал
error-auger-flight-failed = Лента шнека не построилась (проверьте шаг, толщину и наружный диаметр)

## Изоляция: геометрия принадлежит детали

error-body-only-in-part = Тело можно строить только внутри Детали (Сборка тел не держит)
error-cross-component-input = Кросс-компонентная ссылка запрещена: вход { $input } принадлежит другому компоненту
error-sketch-on-foreign-face = Эскиз входа { $input } посажен на грань тела другого компонента без внешней ссылки
error-sketch-face-ref-lost = Опорная грань эскиза на теле { $body } не найдена по имени после пересборки — взят ближайший матч, проверьте посадку фичи

## Пустые результаты

error-array-empty = Массив ничего не дал
error-empty-result = Результат — пустое тело
error-remove-faces-failed = Грани не убрать: { $why }

## Сборка

error-joint-unsatisfied = Связь не выполнена — невязка { $residual } мм

## Выражения

error-expr-unknown-char = Неизвестный символ «{ $what }»
error-expr-unknown-fn = Неизвестная функция «{ $what }»
error-expr-unknown-name = неизвестное имя: { $what } — нет такого параметра
error-expr-needs-one-arg = { $what }() ждёт один аргумент
error-expr-needs-two-args = { $what }() ждёт два аргумента
error-expr-expected-paren = Ожидалась «)»
error-expr-expected-paren-after-args = Ожидалась «)» после аргументов
error-expr-unexpected-token = Неожиданный токен { $what }
error-expr-unexpected-end = выражение обрывается: ждали число или имя
error-expr-trailing-input = Лишний ввод у «{ $what }»
error-expr-not-a-number = Результат не число (деление на ноль?)

## Сообщение самого ядра — не переводится: это диагностика, а не фраза для пользователя.

error-kernel-message = Ядро: { $message }

# ── МОСТ К ЯДРУ ГЕОМЕТРИИ (OCCT): ядро языка не имеет, оно отдаёт коды ──
cad-no-faces-picked = не выбрано ни одной грани
cad-faces-not-in-body = выбранных граней нет в теле (ссылка устарела)
cad-neighbours-not-extendable = соседние поверхности не продлеваются — снимается цельный элемент (отверстие, бобышка)
cad-step-no-shapes = STEP: не удалось прочитать тела
cad-step-nothing-to-export = STEP: нет тел для экспорта
cad-step-write-failed = STEP: запись не удалась (код { $v })
cad-step-read-failed = STEP: не удалось прочитать или передать геометрию
cad-step-empty-tessellation = STEP: пустая тесселяция (нет тел/граней?)
cad-extrude-needs-3-points = профиль для выдавливания должен иметь >=3 точки
cad-extrude-failed = OCCT: не удалось выдавить профиль (самопересечение?)
cad-extrude-empty = выдавливание дало пустое тело
cad-revolve-needs-3-points = профиль для вращения должен иметь >=3 точки
cad-revolve-failed = OCCT: вращение не удалось (профиль пересекает ось?)
cad-revolve-empty = вращение дало пустое тело
cad-boolean-needs-3-points = оба профиля должны иметь >=3 точки
cad-boolean-failed = OCCT: булева операция не удалась
cad-boolean-empty = булева операция дала пустое тело

# ── ФАЙЛОВЫЙ СЛОЙ: коды приходят из qymcad-io, аргумент — путь и текст ОС ──
io-file-create = не удалось создать { $v }
io-file-replace = не удалось заменить { $v }
io-file-read = не удалось прочитать { $v }
io-not-a-qpart = это не .qpart (не zip-контейнер)
io-not-a-qcad = это не .qcad (не zip-контейнер): старый формат не поддерживается
io-refuse-empty-over-full = отказ: пустой документ поверх непустого файла ({ $v } узлов) — сохрани как новый файл
io-stl-no-triangles = STL: нет треугольников для экспорта
io-stl-too-many-triangles = STL: слишком много треугольников
io-stl-write-failed = STL: запись не удалась: { $v }

io-svg-empty-sketch = SVG: пустой эскиз
io-svg-write-failed = SVG: запись не удалась: { $v }
io-dxf-empty-sketch = DXF: пустой эскиз
io-dxf-write-failed = DXF: запись не удалась: { $v }
verify-axis-out-of-table = ход выходит за стол — { $v }
post-not-implemented = постпроцессор ещё не реализован
error-edges-not-found = Из { $asked } названных кромок в теле не осталось ни одной. Их имена выдала операция выше по ленте, и она изменилась — выберите кромки заново.
error-op-failed-patch = Поверхность не натягивается на эти кромки
error-shell-thickness-over-round = Стенка { $t } мм толще самого мелкого скругления на теле ({ $r } мм): смещение съедает его целиком, и оболочка не строится. Возьмите стенку тоньше { $r } мм либо увеличьте скругление
error-operation-split-body = Операция развалила деталь на { $n } тел(а): в детали может быть только одно тело. Уменьшите значение или примените операцию к другой грани
error-mirror-of-hollow-body = Зеркало полой детали по её собственной грани ядру пока не даётся: слияние половинок оставляет лишние оболочки. Отзеркальте деталь ДО оболочки либо выберите другую плоскость
error-shell-of-multi-shell-body = Оболочку на теле из { $n } оболочек ядро не строит: тело уже полое либо собрано из копий (массив, зеркало). Сделайте оболочку раньше — до массива, зеркала или второй оболочки
error-shell-not-built-here = Оболочку на этом теле построить не удалось: смещение граней падает внутри ядра. Попробуйте другую толщину стенки либо сделайте оболочку раньше в истории, пока тело проще
error-cut-removed-nothing = Вырез не снял ничего: инструмент не пересекает деталь. Проверьте, где стоит инструмент и на какую глубину идёт вырез
error-stitch-nothing-joined = Сшивать нечего: у выбранных поверхностей нет общих кромок — они не соприкасаются. После скруглений соседние грани разделены скруглённой полосой; берите поверхности, которые правда стыкуются
