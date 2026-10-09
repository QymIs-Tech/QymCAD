### Mensajes de error. El núcleo devuelve un CÓDIGO; aquí están las palabras correspondientes.
error-thicken-added-nothing = La placa quedó dentro del cuerpo y no añadió nada — especifique un espesor mayor que cero
error-draft-angle-zero = Un ángulo de desmoldeo de 0 grados no inclina nada — especifique un ángulo distinto de cero
error-torus-through-itself = El tubo es tan grueso como el anillo o más — tal toro se interseca a sí mismo; defina el radio del tubo menor que el radio del anillo
error-array-of-one = Un patrón de una sola copia es el propio cuerpo — indique dos o más copias
### Los marcadores de posición { $name } transmiten datos desde el núcleo — no los elimine, no son decorativos.

## La operación falló en el núcleo geométrico.
## La sugerencia entre paréntesis es la causa habitual — ahorra una consulta al soporte técnico.

error-op-failed-extrude = La extrusión no se pudo realizar
error-op-failed-extrude-profile = La extrusión no se pudo realizar (compruebe el perfil)
error-op-failed-extrude-contour = La extrusión no se pudo realizar (compruebe el contorno)
error-op-failed-revolve = La revolución no se pudo realizar
error-op-failed-revolve-profile = La revolución no se pudo realizar (compruebe el perfil)
error-op-failed-revolve-axis = La revolución alrededor del eje de referencia no se pudo realizar (¿está el eje en el plano del boceto?)
error-op-failed-sweep = El barrido no se pudo realizar (¿está el perfil al inicio de la trayectoria y aproximadamente perpendicular a ella?)
error-op-failed-loft = La soelevación no se pudo realizar (las secciones deben ser cerradas y coherentes)
error-op-failed-loft-boolean = La operación booleana de soelevación con el cuerpo no se pudo realizar
error-op-failed-boolean = La operación booleana no se pudo realizar
error-op-failed-body-boolean = La operación booleana entre cuerpos no se pudo realizar (¿no hay intersección o los cuerpos no están relacionados?)
error-op-failed-fillet = El redondeo no se pudo realizar (¿el radio es demasiado grande o son las aristas?)
error-op-failed-fillet-var = El redondeo variable no se pudo realizar (¿los radios o las aristas?)
error-op-failed-chamfer = El chaflán no se pudo realizar (¿el tamaño es demasiado grande o son las aristas?)
error-op-failed-chamfer-asym = El chaflán asimétrico no se pudo realizar (¿el cateto o el ángulo es demasiado grande?)
error-op-failed-shell = El vaciado no se pudo realizar (¿el espesor o la cara?)
error-op-failed-shell-center = El vaciado centrado no se pudo realizar (¿el desplazamiento o la cara?)
error-op-failed-draft = El desmoldeo no se pudo realizar (¿se puede inclinar esta cara con este ángulo desde esta posición neutra?)
error-op-failed-push-face = La cara no se puede desplazar (cara curvada o autointersección)
error-op-failed-remove-faces = No se pueden eliminar las caras
error-op-failed-replace-faces = La superficie no cerró la abertura — la cara no se sustituirá
error-op-failed-copy-faces = La cara no se puede copiar como una superficie independiente
error-op-failed-offset-surface = El desfase no se puede generar: a esta distancia la cara se invierte o desaparece — elija una distancia menor
error-op-failed-stitch = Las láminas no se pueden coser: ninguna arista coincide — parecen no tocarse
error-op-failed-mesh-recognise = No se pudo reconocer la malla: no se pudo generar ninguna cara
error-op-failed-mesh-solid = La malla no se convirtió en un sólido: no contiene ningún triángulo con área
error-op-failed-trim = El recorte no se pudo realizar: la superficie y la herramienta no se intersecan, o no hay nada que cortar
error-op-failed-thicken = La cara no se puede engrosar (¿el desfase se autointerseca?)
error-op-failed-split-body = El plano no corta el cuerpo (pasa de largo o coincide con una cara)
error-op-failed-split-faces = El plano no divide ninguna cara (no interseca el cuerpo)
error-op-failed-hole = El taladro no se pudo realizar (¿los diámetros o las profundidades?)
error-op-failed-holes = Los taladros no se pudieron realizar (¿los puntos, los diámetros o las profundidades?)
error-op-failed-thread = La rosca no se pudo realizar
error-op-failed-helix = El barrido helicoidal no se pudo realizar
error-op-failed-auger = El tornillo sin fin no se pudo realizar
error-op-failed-mirror = La simetría no se pudo realizar
error-op-failed-mirror-plane = La simetría respecto al plano no se pudo realizar
error-op-failed-array = El patrón no se pudo realizar
error-op-failed-move = El desplazamiento no se pudo realizar
error-op-failed-transform = La transformación no se pudo realizar
error-op-failed-cylinder = El cilindro no se pudo generar
error-op-failed-sphere = La esfera no se pudo generar
error-op-failed-cone = El cono no se pudo generar
error-op-failed-torus = El toro no se pudo generar
error-op-failed-prism = El prisma no se pudo generar
error-op-failed-fuse-profiles = La unión de contornos no se pudo realizar
error-op-failed-place = La colocación no se pudo realizar

## La operación requiere el núcleo OCCT real (respondió una rutina ficticia).
## Normalmente el usuario nunca ve esto — significa que la versión no tiene núcleo.

error-kernel-required-extrude = La extrusión requiere el núcleo OCCT
error-kernel-required-mesh-recognise = Solo el núcleo OCCT reconoce una malla
error-kernel-required-mesh-solid = Solo el núcleo OCCT convierte una malla en un sólido
error-kernel-required-extrude-profile = La extrusión requiere el núcleo OCCT
error-kernel-required-extrude-contour = La extrusión requiere el núcleo OCCT
error-kernel-required-revolve = La revolución requiere el núcleo OCCT
error-kernel-required-revolve-profile = La revolución requiere el núcleo OCCT
error-kernel-required-revolve-axis = La revolución requiere el núcleo OCCT
error-kernel-required-sweep = El barrido requiere el núcleo OCCT
error-kernel-required-loft = La soelevación requiere el núcleo OCCT
error-kernel-required-loft-boolean = La operación booleana de soelevación requiere el núcleo OCCT
error-kernel-required-boolean = La operación booleana requiere el núcleo OCCT
error-kernel-required-body-boolean = La operación booleana entre cuerpos requiere el núcleo OCCT
error-kernel-required-fillet = El redondeo requiere el núcleo OCCT
error-kernel-required-fillet-var = El redondeo variable requiere el núcleo OCCT
error-kernel-required-chamfer = El chaflán requiere el núcleo OCCT
error-kernel-required-chamfer-asym = El chaflán asimétrico requiere el núcleo OCCT
error-kernel-required-shell = El vaciado requiere el núcleo OCCT
error-kernel-required-shell-center = El vaciado centrado requiere el núcleo OCCT
error-kernel-required-draft = El desmoldeo requiere el núcleo OCCT
error-kernel-required-push-face = Empujar/tirar cara requiere el núcleo OCCT
error-kernel-required-remove-faces = Eliminar caras requiere el núcleo OCCT
error-kernel-required-replace-faces = Sustituir una cara por una superficie requiere el núcleo OCCT
error-kernel-required-copy-faces = Copiar una cara requiere el núcleo OCCT
error-kernel-required-offset-surface = El desfase de superficie requiere el núcleo OCCT
error-kernel-required-thicken = Dar espesor requiere el núcleo OCCT
error-kernel-required-stitch = Coser requiere el núcleo OCCT
error-kernel-required-trim = Recortar requiere el núcleo OCCT
error-kernel-required-patch = Crear parche requiere el núcleo OCCT
error-kernel-required-split-body = Dividir cuerpo requiere el núcleo OCCT
error-kernel-required-split-faces = Dividir caras requiere el núcleo OCCT
error-kernel-required-hole = El taladro requiere el núcleo OCCT
error-kernel-required-holes = Los taladros requieren el núcleo OCCT
error-kernel-required-thread = La rosca requiere el núcleo OCCT
error-kernel-required-helix = El barrido helicoidal requiere el núcleo OCCT
error-kernel-required-auger = El tornillo sin fin requiere el núcleo OCCT
error-kernel-required-mirror = La simetría requiere el núcleo OCCT
error-kernel-required-mirror-plane = La simetría requiere el núcleo OCCT
error-kernel-required-array = El patrón requiere el núcleo OCCT
error-kernel-required-move = Mover requiere el núcleo OCCT
error-kernel-required-transform = Transformar requiere el núcleo OCCT
error-kernel-required-cylinder = El cilindro requiere el núcleo OCCT
error-kernel-required-sphere = La esfera requiere el núcleo OCCT
error-kernel-required-cone = El cono requiere el núcleo OCCT
error-kernel-required-torus = El toro requiere el núcleo OCCT
error-kernel-required-prism = El prisma requiere el núcleo OCCT
error-kernel-required-fuse-profiles = Unir contornos requiere el núcleo OCCT
error-kernel-required-place = Colocar requiere el núcleo OCCT

## Entradas que faltan o están desactualizadas

error-source-body-not-built = El cuerpo de origen no se construyó — corrija primero la operación anterior
error-source-body-deleted = El elemento sobre el que se construyó fue eliminado — elija otro cuerpo o elimine esta operación
error-body-in-pieces = La operación divide la pieza en partes separadas — una pieza consta de un solo cuerpo; haga que la adición toque el cuerpo o cree una pieza nueva
error-body-in-one-piece = El cuerpo es de una sola pieza — no hay ninguna parte para convertir en pieza
error-source-part-has-no-body = La pieza de origen no tiene cuerpo
error-body-a-not-built = El cuerpo A no se construyó
error-body-b-not-built = El cuerpo B no se construyó
error-face-not-found = La cara ya no está en el cuerpo de origen — la referencia quedó obsoleta
error-faces-not-found = Las caras ya no están en el cuerpo de origen — las referencias quedaron obsoletas
error-profile-not-found = No se encontró el perfil del boceto
error-revolve-profile-crosses-axis = El perfil cruza el eje de revolución — ningún software CAD puede construir esto. Ajuste el perfil contra el eje (media sección: un semicírculo en lugar de un círculo) o aleje el eje del perfil.
error-sweep-profile-missing = No se encontró el perfil de barrido
error-sweep-path-missing = No se encontró la trayectoria de barrido
error-no-isolated-points-for-holes = El boceto no contiene puntos aislados para colocar taladros
error-no-points-for-holes = No hay puntos para colocar taladros

## Planos de referencia

error-cut-plane-deleted = El plano de corte fue eliminado — elija otro o elimine la división
error-sketch-face-gone = La cara sobre la que se encuentra el boceto desapareció: el cuerpo al que pertenecía fue eliminado. Mueva el boceto a otra cara o plano, o deshaga la eliminación
error-sketch-plane-gone = El plano de trabajo sobre el que se encuentra el boceto fue eliminado. Mueva el boceto a otro plano o cara, o deshaga la eliminación
error-mirror-plane-deleted = El plano de simetría fue eliminado — elija otro o elimine la simetría
error-split-plane-deleted = El plano de división fue eliminado — elija otro o elimine la operación
error-mirror-plane-unset = El plano de simetría no está definido — vuelva a crear la pieza simétrica
error-zero-normal = La normal del plano es cero — no se ha definido ninguna dirección

## Valores que no tienen sentido

error-zero-thickness = Espesor cero — no se crearía ninguna placa
error-zero-push-distance = Distancia cero — no hay hacia dónde mover la cara
error-broken-solid = El núcleo devolvió un sólido no utilizable — la operación se canceló y la pieza no ha cambiado. Esto suele ocurrir cuando la cara limita con un redondeo o chaflán: pruebe con una distancia menor o mueva la operación antes del redondeo en el árbol de operaciones
error-split-piece-count = El plano divide ahora el cuerpo en { $got } partes en lugar de { $want } — desplace el plano hacia atrás o vuelva a crear la división
error-loft-needs-two-sections = Una soelevación requiere al menos dos secciones cerradas
error-draft-needs-faces = Un desmoldeo requiere caras para inclinar y una cara neutra
error-no-contours = No hay contornos para la operación
error-all-edges-smooth = Todas las aristas seleccionadas forman una unión suave (el límite de un redondeo) — no hay nada que redondear o achaflanar
error-fillet-radius-too-big = El redondeo R{ $radius } no se pudo aplicar en: { $issues }{ $smooth }
# Una arista en esa lista. «takes up to» indica al usuario el radio máximo que SÍ funcionaría.
error-fillet-edge-takes-up-to = arista { $edge } (admite hasta { $max })
error-fillet-edge-takes-none = arista { $edge } (no admite ningún radio — interseca una unión tangente de un redondeo previo; elimine esta arista o redondee primero su vecina)
error-fillet-smooth-skipped = ; { $n } uniones suaves se omitieron automáticamente
error-fillet-edges-one-by-one = Redondeo R{ $radius }: estas aristas solo se pueden aplicar una a una — los redondeos adyacentes se solapan
error-chamfer-too-big = El chaflán de { $dist } mm falló — el cateto es mayor que el lado
error-surface-does-not-close = La superficie no coincide con la abertura: quedan { $n } aristas sin emparejar. Los límites difieren — construya el parche sobre las mismas aristas que delimitan la cara que se sustituye
error-push-face-on-sheet = Una cara de superficie no se puede desplazar: esta es una operación para sólidos. Para dar espesor a una superficie, utilice "Engrosar"
error-needs-solid-not-sheet = Esta es una herramienta para sólidos: no se aplica a una superficie. Dé espesor a la superficie y trabaje con ella como un cuerpo normal
error-draft-failed = No se puede aplicar un ángulo de desmoldeo de { $angle }° en estas caras. Por lo general, una pared delgada lo impide: tras un vaciado apenas queda material para inclinar — aplique el desmoldeo antes del vaciado o utilice un ángulo menor

## Roscas y tornillos sin fin

error-thread-rim-not-found = No se encontró el borde del cilindro o del taladro (una arista circular)
error-thread-length-unset = No se ha definido la longitud de la rosca
error-thread-pitch-too-small = El paso de { $pitch } mm es demasiado pequeño
error-thread-too-many-turns = { $turns } vueltas son demasiadas — aumente el paso o reduzca la longitud de la rosca
error-thread-longer-than-face = Una rosca de { $length } mm es más larga que el cilindro ({ $face } mm). Reduzca la longitud de la rosca
error-thread-depth-too-deep = La profundidad de rosca de { $depth } mm alcanza o supera el radio de { $radius } mm: para Ø{ $dia } el paso de { $pitch } es demasiado grueso
error-thread-not-its-size = Una rosca de Ø{ $nominal } no encaja en una cara de Ø{ $face } — elija el tamaño de la cara o la cara según el tamaño
warn-edges-dropped = No se pudieron tomar { $dropped } de las { $asked } aristas indicadas y quedaron vivas; el resto se completó — haga doble clic en el nodo para elegir otras aristas u otro tamaño
error-thread-removed-nothing = La rosca no eliminó material ({ $before } -> { $after } mm³) — compruebe la cara seleccionada, el paso y la longitud
error-thread-failed = La rosca no se pudo generar (compruebe el paso, la longitud y el diámetro)
error-auger-rim-not-found = No se encontró el borde del eje (una arista circular)
error-auger-bad-pitch-or-length = El paso y la longitud del tornillo sin fin deben ser mayores que cero
error-auger-outer-not-bigger = El Ø exterior { $outer } del tornillo sin fin no es mayor que el Ø del eje { $shaft }
error-auger-added-nothing = La espiral del tornillo sin fin no añadió material ({ $before } -> { $after } mm³) — compruebe el Ø exterior y el eje seleccionado
error-auger-flight-failed = La espiral del tornillo sin fin no se pudo generar (compruebe el paso, el espesor y el diámetro exterior)

## Aislamiento: la geometría pertenece a la pieza

error-body-only-in-part = Solo se puede construir un cuerpo dentro de una Pieza (un Ensamblaje no contiene cuerpos)
error-cross-component-input = No se permite la referencia entre componentes: la entrada { $input } pertenece a otro componente
error-sketch-on-foreign-face = El boceto de la entrada { $input } se apoya en una cara del cuerpo de otro componente sin una referencia externa
error-sketch-face-ref-lost = La cara de referencia del boceto en el cuerpo { $body } no se encontró por su nombre tras reconstruir — se utilizó la coincidencia más cercana; compruebe la posición de la operación

## Resultados vacíos

error-array-empty = El patrón no generó ningún resultado
error-empty-result = El resultado es un cuerpo vacío
error-remove-faces-failed = No se pueden eliminar las caras: { $why }

## Ensamblaje

error-joint-unsatisfied = La unión no se cumple — residuo de { $residual } mm

## Expresiones

error-expr-unknown-char = Carácter desconocido «{ $what }»
error-expr-unknown-fn = Función desconocida «{ $what }»
error-expr-unknown-name = nombre desconocido: { $what } — no existe tal parámetro
error-expr-needs-one-arg = { $what }() requiere un argumento
error-expr-needs-two-args = { $what }() requiere dos argumentos
error-expr-expected-paren = Se esperaba «)»
error-expr-expected-paren-after-args = Se esperaba «)» tras los argumentos
error-expr-unexpected-token = Token inesperado { $what }
error-expr-unexpected-end = la expresión termina antes de tiempo: se esperaba un número o un nombre
error-expr-trailing-input = Entrada sobrante en «{ $what }»
error-expr-not-a-number = El resultado no es un número (¿división por cero?)

## Mensaje del propio núcleo — se transmite sin traducir: es diagnóstico, no prosa.

error-kernel-message = Núcleo: { $message }

# ── PUENTE AL NÚCLEO GEOMÉTRICO (OCCT) ──
cad-no-faces-picked = no se ha seleccionado ninguna cara
cad-faces-not-in-body = las caras seleccionadas no pertenecen a este cuerpo (la referencia está obsoleta)
cad-neighbours-not-extendable = las superficies adyacentes no se extienden — se está eliminando un elemento completo (taladro, resalte)
cad-file-not-found = Archivo no encontrado: { $v }
cad-step-no-shapes = STEP: no se pudieron leer los cuerpos
cad-step-nothing-to-export = STEP: no hay cuerpos para exportar
cad-step-write-failed = STEP: error al escribir (código { $v })
cad-step-read-failed = STEP: no se pudo leer o transferir la geometría
cad-iges-no-shapes = IGES: no se pudieron leer los cuerpos
cad-iges-read-failed = IGES: no se pudo leer o transferir la geometría
cad-iges-empty-tessellation = IGES: el archivo no contiene ninguna superficie que se pueda mostrar
cad-iges-nothing-to-export = IGES: no hay nada para escribir
cad-iges-write-failed = IGES: error al escribir (código { $v })
io-iges-read-failed = IGES: no se puede leer el archivo ({ $v })
io-iges-not-iges = Esto no es IGES: el archivo no contiene ninguna de las secciones de las que se compone IGES
io-iges-no-curves = IGES: el archivo no contiene superficies ni curvas que se puedan mostrar
io-obj-read-failed = OBJ: no se puede leer el archivo ({ $v })
io-obj-bad-line = OBJ: no se pudo leer la línea { $v }
io-obj-bad-index = OBJ: una cara en la línea { $v } hace referencia a un vértice que no existe
io-obj-no-faces = OBJ: el archivo no contiene ninguna cara
io-obj-no-triangles = OBJ: no hay nada para escribir
io-obj-write-failed = OBJ: error al escribir ({ $v })
io-ply-read-failed = PLY: no se puede leer el archivo ({ $v })
io-ply-not-ply = Esto no es PLY: el archivo no comienza con un encabezado PLY
io-ply-bad-header = PLY: no se pudo leer el encabezado
io-ply-truncated = PLY: el archivo termina antes de lo indicado en su encabezado
io-ply-bad-index = PLY: una cara hace referencia a un vértice que no existe
io-ply-no-faces = PLY: el archivo no contiene ninguna cara
io-ply-no-triangles = PLY: no hay nada para escribir
io-ply-write-failed = PLY: error al escribir ({ $v })
io-gltf-read-failed = glTF: no se puede leer el archivo ({ $v })
io-gltf-not-gltf = Esto no es glTF: el archivo no contiene una descripción de escena
io-gltf-truncated = glTF: el archivo binario está incompleto
io-gltf-bad-node = glTF: un nodo de la escena hace referencia a algo que no existe
io-gltf-no-positions = glTF: la malla no contiene posiciones de vértices
io-gltf-bad-index = glTF: un triángulo hace referencia a un vértice que no existe
io-gltf-bad-accessor = glTF: los datos de la malla están descritos incorrectamente
io-gltf-no-buffer = glTF: el archivo carece de la parte binaria que menciona
io-gltf-bad-buffer = glTF: los datos incrustados no se pueden descodificar
io-gltf-missing-buffer = glTF: sus datos "{ $v }" no están junto al archivo
io-gltf-no-meshes = glTF: la escena no contiene mallas
io-gltf-no-triangles = glTF: no hay nada para escribir
io-gltf-write-failed = glTF: error al escribir ({ $v })
io-3mf-read-failed = 3MF: no se puede leer el archivo ({ $v })
io-3mf-not-3mf = Esto no es 3MF: el archivo no es un archivo de paquete
io-3mf-no-model = 3MF: el paquete no contiene ningún modelo
io-3mf-bad-model = 3MF: el modelo está descrito incorrectamente
io-3mf-unknown-unit = 3MF: unidad desconocida "{ $v }"
io-3mf-bad-transform = 3MF: la transformación está escrita incorrectamente
io-3mf-no-object = 3MF: el modelo hace referencia al objeto { $v }, que no existe
io-3mf-bad-index = 3MF: un triángulo hace referencia a un vértice que no existe
io-3mf-no-meshes = 3MF: el modelo no contiene ninguna malla
io-3mf-no-triangles = 3MF: no hay nada para escribir
io-3mf-write-failed = 3MF: error al escribir ({ $v })
io-amf-read-failed = AMF: no se puede leer el archivo ({ $v })
io-amf-not-amf = Esto no es AMF: el archivo no contiene marcado AMF
io-amf-unknown-unit = AMF: unidad desconocida "{ $v }"
io-amf-bad-vertex = AMF: un vértice está escrito incorrectamente
io-amf-bad-triangle = AMF: un triángulo está escrito incorrectamente
io-amf-bad-index = AMF: un triángulo hace referencia a un vértice que no existe
io-amf-bad-constellation = AMF: la constelación está descrita incorrectamente
io-amf-no-object = AMF: la constelación hace referencia al objeto { $v }, que no existe
io-amf-no-meshes = AMF: el archivo no contiene ninguna malla
io-amf-no-triangles = AMF: no hay nada para escribir
io-amf-write-failed = AMF: error al escribir ({ $v })
cad-step-empty-tessellation = STEP: teselación vacía (¿no hay cuerpos/caras?)
cad-extrude-needs-3-points = un perfil de extrusión necesita >=3 puntos
cad-extrude-failed = OCCT: no se pudo extruir el perfil (¿autointersección?)
cad-extrude-empty = la extrusión produjo un cuerpo vacío
cad-revolve-needs-3-points = un perfil de revolución necesita >=3 puntos
cad-revolve-failed = OCCT: la revolución falló (¿cruza el perfil el eje?)
cad-revolve-empty = la revolución produjo un cuerpo vacío
cad-boolean-needs-3-points = ambos perfiles necesitan >=3 puntos
cad-boolean-failed = OCCT: la operación booleana falló
cad-boolean-empty = la operación booleana produjo un cuerpo vacío

# ── CAPA DE ARCHIVOS: los códigos proceden de qymcad-io, el argumento es la ruta y el texto del SO ──
io-file-create = no se pudo crear { $v }
io-file-replace = no se pudo reemplazar { $v }
io-file-read = no se pudo leer { $v }
io-not-a-qpart = esto no es un archivo .qpart (no es un contenedor zip)
io-not-a-qcad = esto no es un archivo .qcad (no es un contenedor zip): el formato antiguo no es compatible
io-refuse-empty-over-full = rechazado: documento vacío sobre un archivo no vacío ({ $v } nodos) — guárdelo como un archivo nuevo
io-stl-read-failed = STL: no se puede leer el archivo ({ $v })
io-stl-truncated = STL: el archivo termina antes de los triángulos indicados en su encabezado
io-stl-bad-facet = STL: no se pudo leer una faceta
io-stl-no-faces = STL: el archivo no contiene triángulos
io-stl-no-triangles = STL: no hay triángulos para exportar
io-stl-too-many-triangles = STL: demasiados triángulos
io-stl-write-failed = STL: error al escribir: { $v }

io-svg-empty-sketch = SVG: el boceto está vacío
io-svg-write-failed = SVG: error al escribir: { $v }
io-dxf-empty-sketch = DXF: el boceto está vacío
io-dxf-write-failed = DXF: error al escribir: { $v }
error-edges-not-found = No queda en el cuerpo ninguna de las { $asked } aristas indicadas. Sus nombres procedían de una operación anterior en el historial y esta ha cambiado — vuelva a seleccionar las aristas.
error-described-edges-not-found = Las aristas se seleccionaron mediante una cara o una arista que ya no existe en el cuerpo: una operación anterior en el historial la modificó — vuelva a seleccionar las aristas.
error-op-failed-patch = No se puede tender una superficie entre estas aristas
error-shell-thickness-over-round = Una pared de { $t } mm es más gruesa que el redondeo más pequeño del cuerpo ({ $r } mm): el desfase lo consume por completo y no se puede generar el vaciado. Utilice una pared más delgada que { $r } mm o aumente el redondeo
error-operation-split-body = La operación dividió la pieza en { $n } cuerpos: una pieza contiene exactamente un cuerpo. Reduzca el valor o aplique la operación a otra cara
error-mirror-of-hollow-body = Reflejar una pieza hueca respecto a su propia cara supera la capacidad actual del núcleo: la unión de las mitades deja vaciados huérfanos. Refleje la pieza antes del vaciado o elija otro plano
error-shell-of-multi-shell-body = El núcleo no puede vaciar un cuerpo formado por { $n } vaciados: ya es hueco o está ensamblado a partir de copias (patrón, simetría). Realice el vaciado antes — antes del patrón, la simetría o un segundo vaciado
error-shell-not-built-here = No se pudo construir el vaciado en este cuerpo: el desfase de caras falla internamente en el núcleo. Pruebe con otro espesor de pared o realice el vaciado antes en el historial, mientras la pieza sea más simple
error-cut-removed-nothing = El corte no eliminó material: la herramienta no interseca la pieza. Compruebe la posición de la herramienta y la profundidad del corte
error-stitch-nothing-joined = Nada que coser: las superficies seleccionadas no comparten aristas — no se tocan. Tras los redondeos, las caras vecinas quedan separadas por una franja redondeada; seleccione superficies que realmente se unan
