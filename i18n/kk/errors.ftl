### Error messages. The core reports a CODE; these are the words for it.
error-thicken-added-nothing = Пластина дененің ішіне кіріп, ештеңе қоспады — қалыңдықты нөлден жоғары беріңіз
error-draft-angle-zero = 0 градус еңіс ештеңені еңкейтпейді — нөлден өзгеше бұрыш беріңіз
error-torus-through-itself = Түтік сақинамен бірдей не одан қалың — мұндай тор өзін-өзі қиып өтеді; түтік радиусын сақина радиусынан кіші етіңіз
error-array-of-one = Бір көшірмеден тұратын массив — тек дененің өзі; екі не одан көп көшірме беріңіз
### { $name } placeholders carry data from the core — keep them, they are not decoration.

## Operation failed in the geometry kernel.
## The hint in parentheses is the usual cause — it saves a support round-trip.

error-op-failed-extrude = Созу орындалмады
error-op-failed-extrude-profile = Созу орындалмады (профильді тексеріңіз)
error-op-failed-extrude-contour = Созу орындалмады (контурды тексеріңіз)
error-op-failed-revolve = Айналдыру орындалмады
error-op-failed-revolve-profile = Айналдыру орындалмады (профильді тексеріңіз)
error-op-failed-revolve-axis = Датум-осьті айнала айналдыру орындалмады (ось эскиз жазықтығында ма?)
error-op-failed-sweep = Траектория бойымен созу орындалмады (профиль траектория басында және оған шамамен перпендикуляр ма?)
error-op-failed-loft = Лофт орындалмады (қималар тұйық және келісімді болуы керек)
error-op-failed-loft-boolean = Лофттың денемен буль операциясы орындалмады
error-op-failed-boolean = Буль операциясы орындалмады
error-op-failed-body-boolean = Денелердің буль операциясы орындалмады (қиылысу жоқ па, не денелер байланыссыз ба?)
error-op-failed-fillet = Дөңгелектеу орындалмады (радиус тым үлкен не қырларда ма?)
error-op-failed-fillet-var = Айнымалы дөңгелектеу орындалмады (радиустар не қырлар ма?)
error-op-failed-chamfer = Фаска орындалмады (өлшемі тым үлкен не қырларда ма?)
error-op-failed-chamfer-asym = Асимметриялы фаска орындалмады (катет не бұрыш тым үлкен бе?)
error-op-failed-shell = Қабық орындалмады (қалыңдық не бет пе?)
error-op-failed-shell-center = Центрленген қабық орындалмады (ығысу не бет пе?)
error-op-failed-draft = Еңіс орындалмады (бұл бетті осы бейтараптан осы бұрышпен еңкейтуге бола ма?)
error-op-failed-push-face = Бет жылжымайды (қисық бет не өзін-өзі қиюы)
error-op-failed-remove-faces = Беттерді жою мүмкін емес
error-op-failed-replace-faces = Бет-қабат тесікті жаппады — бет ауыстырылмайды
error-op-failed-copy-faces = Бет бөлек бет-қабат болып көшірілмейді
error-op-failed-offset-surface = Ығысу құрылмайды: бұл қашықтықта бет айналып кетеді не жоғалады — кішірек қашықтықты алыңыз
error-op-failed-stitch = Парақтар тігілмейді: бірде-бір қыр сәйкес келмеді — олар жанаспайтын сияқты
error-op-failed-mesh-recognise = Тор танылмады: бірде-бір бет құрылмады
error-op-failed-mesh-solid = Тор денеге айналмады: онда ауданы бар бірде-бір үшбұрыш жоқ
error-op-failed-trim = Қию орындалмады: бет-қабат пен құрал қиылыспайды не кесетін ештеңе жоқ
error-op-failed-thicken = Бет қалыңдамайды (ығысу өзін-өзі қия ма?)
error-op-failed-split-body = Жазықтық денені кеспейді (оның жанынан өтеді не бетте жатыр)
error-op-failed-split-faces = Жазықтық ешбір бетті бөлмейді (денеден тыс өтеді)
error-op-failed-hole = Тесік орындалмады (диаметрлер не тереңдіктер ме?)
error-op-failed-holes = Тесіктер орындалмады (нүктелер, диаметрлер не тереңдіктер ме?)
error-op-failed-thread = Бұранда орындалмады
error-op-failed-helix = Бұрандалы созу орындалмады
error-op-failed-auger = Шнек орындалмады
error-op-failed-mirror = Айна орындалмады
error-op-failed-mirror-plane = Жазықтыққа қатысты айна орындалмады
error-op-failed-array = Массив орындалмады
error-op-failed-move = Жылжыту орындалмады
error-op-failed-transform = Түрлендіру орындалмады
error-op-failed-cylinder = Цилиндр орындалмады
error-op-failed-sphere = Сфера орындалмады
error-op-failed-cone = Конус орындалмады
error-op-failed-torus = Тор орындалмады
error-op-failed-prism = Призма орындалмады
error-op-failed-fuse-profiles = Контурларды біріктіру орындалмады
error-op-failed-place = Орналастыру орындалмады

## The operation needs the real OCCT kernel (a stub answered).
## The user normally never sees these — they mean the build has no kernel.

error-kernel-required-extrude = Созуға OCCT ядросы керек
error-kernel-required-mesh-recognise = Торды тек OCCT ядросы таниды
error-kernel-required-mesh-solid = Торды денеге тек OCCT ядросы айналдырады
error-kernel-required-extrude-profile = Созуға OCCT ядросы керек
error-kernel-required-extrude-contour = Созуға OCCT ядросы керек
error-kernel-required-revolve = Айналдыруға OCCT ядросы керек
error-kernel-required-revolve-profile = Айналдыруға OCCT ядросы керек
error-kernel-required-revolve-axis = Айналдыруға OCCT ядросы керек
error-kernel-required-sweep = Траектория бойымен созуға OCCT ядросы керек
error-kernel-required-loft = Лофтқа OCCT ядросы керек
error-kernel-required-loft-boolean = Лофттың буль операциясына OCCT ядросы керек
error-kernel-required-boolean = Буль операциясына OCCT ядросы керек
error-kernel-required-body-boolean = Денелердің буль операциясына OCCT ядросы керек
error-kernel-required-fillet = Дөңгелектеуге OCCT ядросы керек
error-kernel-required-fillet-var = Айнымалы дөңгелектеуге OCCT ядросы керек
error-kernel-required-chamfer = Фаскаға OCCT ядросы керек
error-kernel-required-chamfer-asym = Асимметриялы фаскаға OCCT ядросы керек
error-kernel-required-shell = Қабыққа OCCT ядросы керек
error-kernel-required-shell-center = Центрленген қабыққа OCCT ядросы керек
error-kernel-required-draft = Еңіске OCCT ядросы керек
error-kernel-required-push-face = Бетті жылжытуға OCCT ядросы керек
error-kernel-required-remove-faces = Беттерді жоюға OCCT ядросы керек
error-kernel-required-replace-faces = Бетті бет-қабатпен ауыстыруға OCCT ядросы керек
error-kernel-required-copy-faces = Бетті көшіруге OCCT ядросы керек
error-kernel-required-offset-surface = Бет-қабатты ығыстыруға OCCT ядросы керек
error-kernel-required-thicken = Қалыңдатуға OCCT ядросы керек
error-kernel-required-stitch = Тігуге OCCT ядросы керек
error-kernel-required-trim = Қиюға OCCT ядросы керек
error-kernel-required-patch = Жамауға OCCT ядросы керек
error-kernel-required-split-body = Денені бөлуге OCCT ядросы керек
error-kernel-required-split-faces = Беттерді бөлуге OCCT ядросы керек
error-kernel-required-hole = Тесікке OCCT ядросы керек
error-kernel-required-holes = Тесіктерге OCCT ядросы керек
error-kernel-required-thread = Бұрандаға OCCT ядросы керек
error-kernel-required-helix = Бұрандалы созуға OCCT ядросы керек
error-kernel-required-auger = Шнекке OCCT ядросы керек
error-kernel-required-mirror = Айнаға OCCT ядросы керек
error-kernel-required-mirror-plane = Айнаға OCCT ядросы керек
error-kernel-required-array = Массивке OCCT ядросы керек
error-kernel-required-move = Жылжытуға OCCT ядросы керек
error-kernel-required-transform = Түрлендіруге OCCT ядросы керек
error-kernel-required-cylinder = Цилиндрге OCCT ядросы керек
error-kernel-required-sphere = Сфераға OCCT ядросы керек
error-kernel-required-cone = Конусқа OCCT ядросы керек
error-kernel-required-torus = Торға OCCT ядросы керек
error-kernel-required-prism = Призмаға OCCT ядросы керек
error-kernel-required-fuse-profiles = Контурларды біріктіруге OCCT ядросы керек
error-kernel-required-place = Орналастыруға OCCT ядросы керек

## Inputs that are missing or stale

error-source-body-not-built = Бастапқы дене құрылмады — алдымен одан жоғарыдағы операцияны түзетіңіз
error-source-body-deleted = Ол құрылған негіз жойылды — басқа денені таңдаңыз не бұл операцияны жойыңыз
error-body-in-pieces = Операция бөлшекті бөлек бөліктерге бөледі — бөлшек бір дене болуы керек; қосылатын бөлікті денеге тигізіңіз не жаңа бөлшек жасаңыз
error-body-in-one-piece = Дене бір бөліктен тұрады — бөлшекке айналдыратын бөлік жоқ
error-source-part-has-no-body = Бастапқы бөлшекте дене жоқ
error-body-a-not-built = A денесі құрылмады
error-body-b-not-built = B денесі құрылмады
error-face-not-found = Бет бастапқы денеде енді жоқ — сілтеме ескірді
error-faces-not-found = Беттер бастапқы денеде енді жоқ — сілтемелер ескірді
error-profile-not-found = Эскиз профилі табылмады
error-revolve-profile-crosses-axis = Профиль айналу осін қиып өтеді — мұны ешбір CAD құра алмайды. Профильді оське тақаңыз (қиманың жартысы: шеңбер емес, жарты шеңбер) не осьті профильден ары жылжытыңыз.
error-sweep-profile-missing = Траектория бойымен созу профилі табылмады
error-sweep-path-missing = Траектория бойымен созу траекториясы табылмады
error-no-isolated-points-for-holes = Эскизде тесік қоятын оқшау нүктелер жоқ
error-no-points-for-holes = Тесік қоятын нүктелер жоқ

## Reference planes

error-cut-plane-deleted = Кесетін жазықтық жойылды — басқасын таңдаңыз не бөлуді жойыңыз
error-sketch-face-gone = Эскиз тұрған бет жоғалды: ол тиесілі дене жойылды. Эскизді басқа бетке не жазықтыққа жылжытыңыз немесе жоюды болдырмаңыз
error-sketch-plane-gone = Эскиз тұрған жұмыс жазықтығы жойылды. Эскизді басқа жазықтыққа не бетке жылжытыңыз немесе жоюды болдырмаңыз
error-mirror-plane-deleted = Айна жазықтығы жойылды — басқасын таңдаңыз не айнаны жойыңыз
error-split-plane-deleted = Бөлу жазықтығы жойылды — басқасын таңдаңыз не операцияны жойыңыз
error-mirror-plane-unset = Айна жазықтығы берілмеген — айналық бөлшекті қайта жасаңыз
error-zero-normal = Жазықтық нормалі нөлге тең — бағыт анықталмаған

## Values that make no sense

error-zero-thickness = Нөлдік қалыңдық — пластина болмайды
error-zero-push-distance = Нөлдік арақашықтық — бетті жылжытатын жер жоқ
error-broken-solid = Ядро жарамсыз дене қайтарды — операция болдырылмады, бөлшек өзгермеді. Бұл әдетте бет дөңгелектеуге не фаскаға шектескенде болады: кішірек арақашықтықты байқап көріңіз не операцияны тізбекте дөңгелектеуден бұрын қойыңыз
error-split-piece-count = Жазықтық енді денені { $want } орнына { $got } бөлікке кеседі — жазықтықты кері жылжытыңыз не бөлуді қайта жасаңыз
error-loft-needs-two-sections = Лофтқа кемінде екі тұйық қима керек
error-draft-needs-faces = Еңіске еңкейтілетін беттер мен бейтарап бет керек
error-no-contours = Операцияға контурлар жоқ
error-all-edges-smooth = Таңдалған барлық қырлар тегіс түйіс (дөңгелектеу шекарасы) — дөңгелектейтін не фаска алатын ештеңе жоқ
error-fillet-radius-too-big = R{ $radius } дөңгелектеуі қолданылмады: { $issues }{ $smooth }
# One edge in that list. «takes up to» tells the user the largest radius that WOULD work.
error-fillet-edge-takes-up-to = { $edge } қыры ({ $max } дейін алады)
error-fillet-edge-takes-none = { $edge } қыры (ешқандай радиус алмайды — ертеректегі дөңгелектеудің жанама түйісіне тіреледі; бұл қырды алып тастаңыз не алдымен көршісін дөңгелектеңіз)
error-fillet-smooth-skipped = ; { $n } тегіс түйіс автоматты өткізілді
error-fillet-edges-one-by-one = R{ $radius } дөңгелектеуі: бұл қырлар тек бір-бірден алынады — көрші дөңгелектеулер қабаттасады
error-chamfer-too-big = { $dist } мм фаска орындалмады — катет қабырғадан үлкен
error-surface-does-not-close = Бет-қабат тесікке сәйкес келмейді: { $n } қыр жұпсыз қалды. Шекаралар әртүрлі — жамауды ауыстырылатын бетті шектейтін сол қырлар бойынша құрыңыз
error-push-face-on-sheet = Бет-қабаттың бетін жылжытуға болмайды: бұл тұтас денеге арналған операция. Бет-қабатқа қалыңдық беру үшін «Қалыңдату» командасын пайдаланыңыз
error-needs-solid-not-sheet = Бұл тұтас денеге арналған құрал: ол бет-қабатқа қолданылмайды. Бет-қабатқа қалыңдық беріп, онымен әдеттегі дене сияқты жұмыс істеңіз
error-draft-failed = { $angle }° еңіс бұл беттерде алынбайды. Әдетте жұқа қабырға кедергі келтіреді: қабықтан кейін еңкейтетін аз қалады — еңісті қабықтан бұрын қолданыңыз не кішірек бұрыш алыңыз

## Threads and augers

error-thread-rim-not-found = Цилиндрдің не тесіктің жиегі (дөңгелек қыр) табылмады
error-thread-length-unset = Бұранда ұзындығы берілмеген
error-thread-pitch-too-small = { $pitch } мм қадам тым кіші
error-thread-too-many-turns = { $turns } орам тым көп — қадамды үлкейтіңіз не бұранданы қысқартыңыз
error-thread-longer-than-face = Ұзындығы { $length } мм бұранда цилиндрден ({ $face } мм) ұзын. Бұранданы қысқартыңыз
error-thread-depth-too-deep = { $depth } мм бұранда тереңдігі { $radius } мм радиусқа тең не одан асады: Ø{ $dia } үшін { $pitch } қадам тым ірі
error-thread-not-its-size = Ø{ $nominal } бұранда Ø{ $face } бетке сәйкес келмейді — өлшемді бетке қарай не бетті өлшемге қарай таңдаңыз
warn-edges-dropped = { $asked } қырдың { $dropped } алынбады және өткір қалды; қалғаны жасалды — басқа қырларды не басқа өлшемді таңдау үшін түйінді екі рет шертіңіз
error-thread-removed-nothing = Бұранда ештеңе алмады ({ $before } -> { $after } мм³) — таңдалған бетті, қадамды және ұзындықты тексеріңіз
error-thread-failed = Бұранда құрылмады (қадамды, ұзындықты және диаметрді тексеріңіз)
error-auger-rim-not-found = Білік жиегі (дөңгелек қыр) табылмады
error-auger-bad-pitch-or-length = Шнектің қадамы мен ұзындығы нөлден үлкен болуы керек
error-auger-outer-not-bigger = Шнектің сыртқы Ø{ $outer } біліктің Ø{ $shaft } үлкен емес
error-auger-added-nothing = Шнек қалағы ештеңе қоспады ({ $before } -> { $after } мм³) — сыртқы Ø мен таңдалған білікті тексеріңіз
error-auger-flight-failed = Шнек қалағы құрылмады (қадамды, қалыңдықты және сыртқы диаметрді тексеріңіз)

## Isolation: a part owns its geometry

error-body-only-in-part = Денені тек Бөлшек ішінде құруға болады (Жинақта денелер болмайды)
error-cross-component-input = Компоненттер арасындағы сілтемеге рұқсат жоқ: { $input } кірісі басқа компонентке тиесілі
error-sketch-on-foreign-face = { $input } кіріс эскизі басқа компонент денесінің бетінде сыртқы сілтемесіз тұр
error-sketch-face-ref-lost = { $body } денесіндегі эскиз бетінің сілтемесі қайта құрудан кейін атауы бойынша табылмады — ең жақын сәйкесі алынды, операция қайда түскенін тексеріңіз

## Results that are empty

error-array-empty = Массив ештеңе бермеді
error-empty-result = Нәтиже — бос дене
error-remove-faces-failed = Беттерді жою мүмкін емес: { $why }

## Assembly

error-joint-unsatisfied = Қосылыс орындалмаған — қалдық { $residual } мм

## Expressions

error-expr-unknown-char = Белгісіз таңба «{ $what }»
error-expr-unknown-fn = Белгісіз функция «{ $what }»
error-expr-unknown-name = белгісіз атау: { $what } — мұндай параметр жоқ
error-expr-needs-one-arg = { $what }() бір аргумент алады
error-expr-needs-two-args = { $what }() екі аргумент алады
error-expr-expected-paren = «)» күтілді
error-expr-expected-paren-after-args = Аргументтерден кейін «)» күтілді
error-expr-unexpected-token = Күтілмеген таңба { $what }
error-expr-unexpected-end = өрнек тым ерте аяқталады: сан не атау күтілді
error-expr-trailing-input = «{ $what }» жанында артық енгізу
error-expr-not-a-number = Нәтиже сан емес (нөлге бөлу ме?)

## A message from the kernel itself — passed through untranslated: it is diagnostics, not prose.

error-kernel-message = Ядро: { $message }

# ── GEOMETRY KERNEL BRIDGE (OCCT) ──
cad-no-faces-picked = бірде-бір бет таңдалмаған
cad-faces-not-in-body = таңдалған беттер бұл денеде жоқ (сілтеме ескірген)
cad-neighbours-not-extendable = көрші беттер ұзармайды — тұтас элемент алынып жатыр (тесік, шығыңқы)
cad-file-not-found = Файл табылмады: { $v }
cad-step-no-shapes = STEP: денелерді оқу мүмкін болмады
cad-step-nothing-to-export = STEP: экспорттайтын денелер жоқ
cad-step-write-failed = STEP: жазу орындалмады (код { $v })
cad-step-read-failed = STEP: геометрияны оқу не беру мүмкін болмады
cad-iges-no-shapes = IGES: денелерді оқу мүмкін болмады
cad-iges-read-failed = IGES: геометрияны оқу не беру мүмкін болмады
cad-iges-empty-tessellation = IGES: файлда көрсетуге болатын бет жоқ
cad-iges-nothing-to-export = IGES: жазатын ештеңе жоқ
cad-iges-write-failed = IGES: жазу орындалмады (код { $v })
io-iges-read-failed = IGES: файл оқылмайды ({ $v })
io-iges-not-iges = Бұл IGES емес: файлда IGES құралатын бөлімдердің бірі де жоқ
io-iges-no-curves = IGES: файлда көрсетуге болатын бет те, қисық та жоқ
io-obj-read-failed = OBJ: файл оқылмайды ({ $v })
io-obj-bad-line = OBJ: { $v }-жолды оқу мүмкін болмады
io-obj-bad-index = OBJ: { $v }-жолдағы бет жоқ төбеге сілтейді
io-obj-no-faces = OBJ: файлда бірде-бір бет жоқ
io-obj-no-triangles = OBJ: жазатын ештеңе жоқ
io-obj-write-failed = OBJ: жазу орындалмады ({ $v })
io-ply-read-failed = PLY: файл оқылмайды ({ $v })
io-ply-not-ply = Бұл PLY емес: файл PLY тақырыбынан басталмайды
io-ply-bad-header = PLY: тақырыпты оқу мүмкін болмады
io-ply-truncated = PLY: файл тақырыбында айтылғаннан ерте аяқталады
io-ply-bad-index = PLY: бет жоқ төбеге сілтейді
io-ply-no-faces = PLY: файлда бірде-бір бет жоқ
io-ply-no-triangles = PLY: жазатын ештеңе жоқ
io-ply-write-failed = PLY: жазу орындалмады ({ $v })
io-gltf-read-failed = glTF: файл оқылмайды ({ $v })
io-gltf-not-gltf = Бұл glTF емес: файлда сахна сипаттамасы жоқ
io-gltf-truncated = glTF: екілік файл кесіліп қалған
io-gltf-bad-node = glTF: сахна түйіні жоқ нәрсеге сілтейді
io-gltf-no-positions = glTF: торда төбе координаталары жоқ
io-gltf-bad-index = glTF: үшбұрыш жоқ төбеге сілтейді
io-gltf-bad-accessor = glTF: тор деректері қате сипатталған
io-obj-not-finite-line = OBJ: { $v } жолындағы мән шекті сан емес ({ $w })
io-ply-not-finite-line = PLY: { $v } жолындағы мән шекті сан емес ({ $w })
io-ply-not-finite-vertex = PLY: { $v } төбесінің координатасы ({ $w } байт) шекті сан емес ({ $x })
io-gltf-not-finite = glTF: { $v } аксессорының { $w } элементі ({ $x } байт) шекті сан емес ({ $y })
io-3mf-not-finite-line = 3MF: { $v } жолындағы мән шекті сан емес ({ $w })
io-amf-not-finite-line = AMF: { $v } жолындағы мән шекті сан емес ({ $w })
io-stl-not-finite-line = STL: { $v } жолындағы мән шекті сан емес ({ $w })
io-stl-not-finite-triangle = STL: { $v } үшбұрышының координатасы ({ $w } байт) шекті сан емес ({ $x })
io-xml-too-deep = Файл { $v } деңгейден тереңірек салынған және оқылмайды
io-gltf-no-buffer = glTF: файлда ол атаған екілік бөлік жоқ
io-gltf-bad-buffer = glTF: кірістірілген деректерді декодтау мүмкін емес
io-gltf-missing-buffer = glTF: оның «{ $v }» деректері файлдың жанында жоқ
io-gltf-no-meshes = glTF: сахнада бірде-бір тор жоқ
io-gltf-no-triangles = glTF: жазатын ештеңе жоқ
io-gltf-write-failed = glTF: жазу орындалмады ({ $v })
io-3mf-read-failed = 3MF: файл оқылмайды ({ $v })
io-3mf-not-3mf = Бұл 3MF емес: файл пакет мұрағаты емес
io-3mf-no-model = 3MF: пакетте модель жоқ
io-3mf-bad-model = 3MF: модель қате сипатталған
io-3mf-unknown-unit = 3MF: белгісіз бірлік «{ $v }»
io-3mf-bad-transform = 3MF: түрлендіру қате жазылған
io-3mf-no-object = 3MF: модель жоқ { $v } нысанына сілтейді
io-3mf-bad-index = 3MF: үшбұрыш жоқ төбеге сілтейді
io-3mf-no-meshes = 3MF: модельде бірде-бір тор жоқ
io-3mf-no-triangles = 3MF: жазатын ештеңе жоқ
io-3mf-write-failed = 3MF: жазу орындалмады ({ $v })
io-amf-read-failed = AMF: файл оқылмайды ({ $v })
io-amf-not-amf = Бұл AMF емес: файлда AMF белгілеуі жоқ
io-amf-unknown-unit = AMF: белгісіз бірлік «{ $v }»
io-amf-bad-vertex = AMF: төбе қате жазылған
io-amf-bad-triangle = AMF: үшбұрыш қате жазылған
io-amf-bad-index = AMF: үшбұрыш жоқ төбеге сілтейді
io-amf-bad-constellation = AMF: шоғыр қате сипатталған
io-amf-no-object = AMF: шоғыр жоқ { $v } нысанына сілтейді
io-amf-no-meshes = AMF: файлда бірде-бір тор жоқ
io-amf-no-triangles = AMF: жазатын ештеңе жоқ
io-amf-write-failed = AMF: жазу орындалмады ({ $v })
cad-step-empty-tessellation = STEP: тесселяция бос (денелер/беттер жоқ па?)
cad-extrude-needs-3-points = созу профиліне >=3 нүкте керек
cad-extrude-failed = OCCT: профильді созу мүмкін болмады (өзін-өзі қию ма?)
cad-extrude-empty = созу бос дене берді
cad-revolve-needs-3-points = айналдыру профиліне >=3 нүкте керек
cad-revolve-failed = OCCT: айналдыру орындалмады (профиль осьті қиып өте ме?)
cad-revolve-empty = айналдыру бос дене берді
cad-boolean-needs-3-points = екі профильге де >=3 нүкте керек
cad-boolean-failed = OCCT: буль операциясы орындалмады
cad-boolean-empty = буль операциясы бос дене берді

# ── FILE LAYER: codes come from qymcad-io, the argument is the path and the OS text ──
io-file-create = { $v } жасау мүмкін болмады
io-file-replace = { $v } ауыстыру мүмкін болмады
io-file-read = { $v } оқу мүмкін болмады
io-not-a-qpart = бұл .qpart емес (zip контейнер емес)
io-not-a-qcad = бұл .qcad емес (zip контейнер емес): ескі формат қолдау таппайды
io-refuse-empty-over-full = бас тартылды: бос құжат бос емес файлдың үстіне ({ $v } түйін) — оны жаңа файл ретінде сақтаңыз
io-stl-read-failed = STL: файл оқылмайды ({ $v })
io-stl-truncated = STL: файл тақырыбы санаған үшбұрыштар біткенше аяқталып қалады
io-stl-bad-facet = STL: жақты оқу мүмкін болмады
io-stl-no-faces = STL: файлда бірде-бір үшбұрыш жоқ
io-stl-no-triangles = STL: экспорттайтын үшбұрыштар жоқ
io-stl-too-many-triangles = STL: үшбұрыштар тым көп
io-stl-write-failed = STL: жазу орындалмады: { $v }

io-svg-empty-sketch = SVG: эскиз бос
io-svg-write-failed = SVG: жазу орындалмады: { $v }
io-dxf-empty-sketch = DXF: эскиз бос
io-dxf-write-failed = DXF: жазу орындалмады: { $v }
error-edges-not-found = Аталған { $asked } қырдың бірде-біреуі денеде қалмады. Олардың атаулары тізбектегі жоғарырақ операциядан келген, ал ол өзгерді — қырларды қайта таңдаңыз.
error-described-edges-not-found = Қырлар денеде енді жоқ бет немесе қыр арқылы таңдалған: оны тізбектегі жоғарырақ операция өзгертті — қырларды қайта таңдаңыз.
error-op-failed-patch = Бұл қырлардың үстіне бет-қабат тартылмайды
error-shell-thickness-over-round = { $t } мм қабырға денедегі ең кіші дөңгелектеуден ({ $r } мм) қалың: ығысу оны толық жейді, қабық құрылмайды. { $r } мм-ден жұқа қабырғаны алыңыз не дөңгелектеуді үлкейтіңіз
error-operation-split-body = Операция бөлшекті { $n } денеге бөлді: бөлшекте дәл бір дене болады. Мәнді азайтыңыз не операцияны басқа бетке қолданыңыз
error-mirror-of-hollow-body = Қуыс бөлшекті өз бетіне қатысты айналау әзірге ядроның қолынан келмейді: жартыларды біріктіргенде артық қабықтар қалады. Бөлшекті қабық жасамас бұрын айналаңыз не басқа жазықтықты таңдаңыз
error-shell-of-multi-shell-body = Ядро { $n } қабықтан тұратын денеге қабық жасай алмайды: ол қазірдің өзінде қуыс не көшірмелерден жиналған (массив, айна). Қабықты ертерек жасаңыз — массивтен, айнадан не екінші қабықтан бұрын
error-shell-not-built-here = Бұл денеде қабық құрылмады: беттерді ығыстыру ядро ішінде сәтсіз аяқталады. Басқа қабырға қалыңдығын байқап көріңіз не денеге тарихта ертерек, ол қарапайымырақ кезде қабық жасаңыз
error-cut-removed-nothing = Ойып алу ештеңе алмады: құрал бөлшекті қиып өтпейді. Құрал қайда тұрғанын және ойық қаншалықты терең екенін тексеріңіз
error-stitch-nothing-joined = Тігетін ештеңе жоқ: таңдалған бет-қабаттардың ортақ қырлары жоқ — олар жанаспайды. Дөңгелектеуден кейін көрші беттерді дөңгелек жолақ бөледі; шынымен түйісетін бет-қабаттарды таңдаңыз
