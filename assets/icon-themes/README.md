# QymCAD Icon Theme System

QymCAD features a modular, dynamic vector icon theme engine. It supports both built-in icon packs and external user-contributed packs distributed either as plain directories or compressed `.qicons` archives (ZIP-based).

---

## 1. Important: Partial Packs & Fallback Cascade

> **A theme does not need to provide every icon.**
>
> Theme packs can be **partial** and contain only a subset of icons (for example, only sketch tools, only custom constraint badges, or only specific 3D solid operations).
>
> When an icon is requested, QymCAD resolves it through a multi-tier **Fallback Cascade**:
>
> $$\text{Active Custom Pack} \longrightarrow \text{Next Active Pack} \longrightarrow \dots \longrightarrow \text{Built-in Default Pack (shapr-alike)}$$
>
> If an icon is missing from a custom theme, QymCAD automatically and silently falls back to the next pack in the active stack, down to the built-in default theme (`shapr-alike`, which provides 100% complete SVG coverage). No buttons will ever appear blank or broken.

---

## 2. Directory & Bundle Structure

An icon theme is either a directory on disk or a ZIP archive renamed with the `.qicons` extension. It consists of a root metadata file `manifest.ron` and an `icons/` folder containing SVG files organized by category:

```text
my-theme.qicons (or directory my-theme/)
├── manifest.ron
├── icon.svg
├── README.md
├── README.ru.md
└── icons/
    ├── sketch/
    │   ├── line.svg
    │   ├── circle.svg
    │   └── ...
    ├── constraint/
    │   ├── coincident.svg
    │   ├── horizontal.svg
    │   └── ...
    ├── part/
    │   ├── extrude.svg
    │   ├── fillet.svg
    │   └── ...
    ├── assembly/
    │   ├── joint.svg
    │   ├── ground.svg
    │   └── ...
    └── datum/
        ├── plane.svg
        └── ...
```

Place `icon.svg` beside `manifest.ron` to give the theme a thumbnail in the manager. It must be a valid square SVG using a `viewBox`. The built-in thumbnail is shown when this file is absent or invalid. The packager includes a valid `icon.svg` in `.qicons` bundles.

---

## 3. Manifest Specification (`manifest.ron`)

The manifest is written in [RON (Rusty Object Notation)](https://github.com/ron-rs/ron) and must be located at the root of the theme folder under the name `manifest.ron`.

### Example `manifest.ron`:

```ron
(
    // Package type for plugin system compatibility (always IconTheme)
    package_type: IconTheme,

    // Unique machine identifier (alphanumeric, hyphens allowed)
    id: "shapr-alike",

    // User-facing display name shown in settings
    name: "Shapr-Alike",

    // Semantic version string
    version: "1.0.0",

    // Author or organization
    author: "braindefender",

    // License identifier (e.g. MIT, Apache-2.0, AGPL-3.0-or-later)
    license: "AGPL-3.0-or-later",

    // Short summary describing the theme
    description: "QymCAD icons inspired by Shapr3D",

    // Optional translated display text, keyed by language tag
    translations: {
        "uk": (
            name: "Shapr-Alike",
            description: "Іконки QymCAD, натхненні Shapr3D",
        ),
        "ru": (
            name: "Shapr-Alike",
            description: "Иконки QymCAD, вдохновлённые Shapr3D",
        ),
        "kk": (
            name: "Shapr-Alike",
            description: "Shapr3D үлгісінде жасалған QymCAD таңбашалары",
        ),
    },
)
```

### Manifest Fields:

| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `package_type` | `PackageType` | No (default `IconTheme`) | Extension type identifier. Always `IconTheme`. |
| `id` | `String` | **Yes** | Unique package ID (e.g., `shapr-alike`, `custom-theme`). |
| `name` | `String` | **Yes** | Human-readable name displayed in Settings -> Icon Themes. |
| `version` | `String` | No | Semantic version string (e.g., `"1.0.0"`). |
| `author` | `String` | No | Author, maintainer, or contributing community. |
| `license` | `String` | No | SPDX license identifier or license name. |
| `description` | `String` | No | Concise description of the pack. |
| `translations` | map of language tags to `(name, description)` | No | Translated display text. Either translated field may be omitted. |

---

### Localized names and documentation

Keep `name` and `description` as the base text in `manifest.ron`. Add translations under BCP 47 language tags such as `ru` or `pt-BR`. The manager displays a matching translation for the current interface language. If there is no exact match, it tries the primary language (`pt-BR` -> `pt`), then the base field. Missing translated fields also fall back separately.

Place a full description in `README.md` and optional translations beside it, named `README.<language-tag>.md`, for example `README.ru.md` or `README.pt-BR.md`. The same language fallback applies to README files. If no README is available, the manager builds a short description from the localized manifest text. Localized README files are included by the built-in packager.

The package ID, author, license, and version are shared across languages. Existing themes without translations continue to use their base text.

---

## 4. Theme Palette Colors & CSS Variables

All icon themes can dynamically adapt to the active UI scheme using CSS variables:
* **Syntax:** `var(--token-name, fallback_color)`
* **Mandatory Fallback Rule:** Every `var(...)` expression **must** specify a valid fallback hex color, e.g. `var(--icon-stroke, #E0E0E0)`. An empty fallback (or omitting the fallback) is rejected by the package validator.
* **Current Color:** `currentColor` is automatically mapped to `--icon-stroke`.
* **Static / Brand Colors:** Hardcoded hex colors (`#FF3333`, `#00AA00`) are fully supported for icons with fixed brand palettes that should not adapt to UI themes.

### Supported Icon Tokens:
QymCAD strictly recognizes the 14 tokens defined in `qymcad_scheme::ICON_TOKENS`. Any token outside this list will fail validation:

| Token Name | Intended Usage | Dark Theme Default | Light Theme Default |
| :--- | :--- | :--- | :--- |
| `--icon-stroke` | Base contour stroke (`currentColor` maps here) | `#000000` | `#808080` |
| `--icon-neutral` | Subtle accents, secondary lines, neutral fills | `#BCBCBC` | `#CCCCCC` |
| `--icon-accent` | General tool highlight / active state | `#18F2F2` | `#18F2F2` |
| `--icon-dimmed` | Muted background geometry | `#7C7C7C` | `#ABABAB` |
| `--icon-sketch-primary` | 2D Sketch primary geometry and tools | `#18F2F2` | `#18F2F2` |
| `--icon-sketch-secondary` | 2D Sketch secondary accents / modifications | `#262626` | `#808080` |
| `--icon-constraint-primary` | Geometric constraints primary badge color | `#F26118` | `#F26118` |
| `--icon-constraint-secondary` | Geometric constraints secondary accents | `#333333` | `#808080` |
| `--icon-part-primary` | 3D Solid feature operations primary color | `#18F2F2` | `#18F2F2` |
| `--icon-part-secondary` | 3D Solid cuts, voids, and auxiliary actions | `#F2AA18` | `#F2AA18` |
| `--icon-assembly-primary` | Assembly components and mate links | `#18F2F2` | `#18F2F2` |
| `--icon-assembly-secondary` | Assembly secondary elements | `#F2AA18` | `#F2AA18` |
| `--icon-datum-primary` | Reference planes and axes primary | `#18F2F2` | `#18F2F2` |
| `--icon-datum-secondary` | Reference datum points and coordinate axes | `#F2AA18` | `#F2AA18` |

---

## 5. SVG File Requirements & Sanitation

All SVG files are validated at compile-time and packaging-time by `validate_svg` (`crates/qymcad-ui-state/src/icons/bundle/hygiene.rs`):

1. **Aspect Ratio & ViewBox:**
   * The root `<svg>` element **must** include a `viewBox` attribute.
   * Aspect ratio must be strictly **1:1 (square)** (tolerance: $0.95 \le \text{ratio} \le 1.05$).
   * Recommended canvas viewBox: `viewBox="0 0 64 64"` (CAD standard) or `viewBox="0 0 24 24"`.
   * Recommended padding: leave 4 to 6 pixels of inner margin on a 64×64 canvas so contours do not touch toolbar button borders.
2. **UI Button Sizes:**
   * Tool button slots in QymCAD toolbars have a fixed layout size of **40×34 pt**.
   * Inside this slot, icons are dynamically rasterized to crisp **20×20 pt** or **24×24 pt** squares matching screen DPI (`pixels_per_point`).
3. **No Embedded Rasters (Zero Raster Rule):**
   * Raster images (`<image>` or `data:image/...` base64 payloads) are **strictly prohibited**. Only pure vector shapes (`<path>`, `<circle>`, `<rect>`, `<polygon>`, `<g>`, `<line>`, `<polyline>`, `<ellipse>`) are allowed.
4. **Security & Clean Hygiene:**
   * Dangerous executable tags (`<script>`, `<foreignObject>`, `<applet>`, `<object>`, `<embed>`, `<iframe>`, `<audio>`, `<video>`, `<metadata>`) are forbidden.
   * Event handlers (`onload=`, `onclick=`, `javascript:...`) and `<!ENTITY>` declarations are blocked.
   * Editor metadata namespaces (`inkscape:`, `sodipodi:`, `adobe:`, `sketch:`, `figma:`) should be stripped before distribution (QymCAD's in-app packager cleans them automatically).
5. **Encoding:**
   * Must be valid **UTF-8** without Byte Order Mark (BOM).

---

## 6. Live Testing & Hot Reload Workflow

QymCAD includes a built-in filesystem watcher for rapid icon theme development:

1. **Place your theme folder** into the user icon themes directory:
   * **Linux:** `~/.config/qymcad/icon_themes/<your-theme>/` or `~/.local/share/qymcad/icon_themes/<your-theme>/`
   * **Windows:** `%APPDATA%\qymcad\config\icon_themes\<your-theme>\` or `%APPDATA%\qymis\qymcad\data\icon_themes\<your-theme>\`
   * **macOS:** `~/Library/Application Support/qymcad/icon_themes/<your-theme>/`
2. **Open QymCAD** and navigate to:
   * **Settings $\rightarrow$ Appearance $\rightarrow$ Manage Icon Themes**
3. Select your theme in the sidebar list and check:
   * **"Watch this folder for live changes (Live reload)"**
4. Keep QymCAD open while editing SVGs in your vector design tool (Inkscape, Illustrator, Figma, Penpot).
5. Whenever you save an SVG file, QymCAD detects the file modification and repaints the toolbar buttons instantly in real time.

---

## 7. Directory Naming & Icon Catalogue

Icon paths are resolved by combining category subdirectories with the icon name:
`icons/<category>/<name>.svg`

Below is the complete dictionary of icons recognized across the 5 categories:

### 7.1. Category `sketch/` — 2D Sketch Creation, Modification & Dimensions (32 icons)

| Icon Path | Function & Description |
| :--- | :--- |
| `sketch/select.svg` | **Select:** Switch sketcher to selection/pointer mode to pick and manipulate geometry. |
| `sketch/point.svg` | **Point:** Place an isolated point entity onto the sketch plane. |
| `sketch/line.svg` | **Line:** Draw straight line segments between two endpoints. |
| `sketch/rect.svg` | **Rectangle:** Draw a 2-point corner-to-corner rectangular box. |
| `sketch/circle.svg` | **Circle:** Draw a circle defined by center point and radius point. |
| `sketch/circle_3pt.svg` | **3-Point Circle:** Draw a circle passing through three perimeter points. |
| `sketch/arc.svg` | **Arc:** Draw a circular arc defined by three points (start, end, and curve point). |
| `sketch/polygon.svg` | **Regular Polygon:** Create an inscribed regular polygon with a configurable number of sides. |
| `sketch/slot.svg` | **Slot:** Draw an elongated racetrack slot (obround) with parallel sides and semicircular caps. |
| `sketch/ellipse.svg` | **Ellipse:** Draw an ellipse defined by center point, major radius, and minor radius. |
| `sketch/spline.svg` | **B-Spline:** Draw smooth non-uniform rational B-spline curves through control points. |
| `sketch/text.svg` | **Text:** Place text geometry curves onto the sketch plane. |
| `sketch/construction.svg` | **Construction Toggle:** Toggle construction mode (switches between solid contour lines and dashed reference geometry). |
| `sketch/fillet.svg` | **Fillet:** Round an acute corner between two intersecting curves with a circular tangent arc. |
| `sketch/fillet_all.svg` | **Fillet All:** Round all sharp corners of the active contour in one action. |
| `sketch/chamfer.svg` | **Chamfer:** Cut an intersecting corner with a straight diagonal bevel. |
| `sketch/trim.svg` | **Trim:** Cut away an intersecting segment of a curve up to adjacent intersections. |
| `sketch/extend.svg` | **Extend:** Lengthen a curve up to its nearest intersection with neighbouring geometry. |
| `sketch/break.svg` | **Break:** Split a continuous curve into two connected segments at a selected point. |
| `sketch/offset.svg` | **Offset:** Create a parallel equidistant offset contour from selected curves. |
| `sketch/mirror.svg` | **Mirror:** Mirror selected sketch entities across a straight symmetry centerline. |
| `sketch/copy.svg` | **Copy:** Translate and replicate selected 2D sketch elements to a new position. |
| `sketch/array_linear.svg` | **Linear Pattern:** Replicate selected entities in a 2D grid/linear rectangular array. |
| `sketch/array_circular.svg` | **Circular Pattern:** Replicate selected entities radially around a center point. |
| `sketch/delete.svg` | **Delete:** Delete selected sketch curves, points, or constraints. |
| `sketch/move.svg` | **Move:** Translate selected sketch curves to a new 2D position. |
| `sketch/rotate.svg` | **Rotate:** Rotate selected sketch entities around a specified center point. |
| `sketch/measure.svg` | **Measure 2D:** Inspect distance, length, and angles between sketch elements. |
| `sketch/project.svg` | **Project 3D Edge:** Project edges or faces of 3D bodies into the active sketch plane as reference curves. |
| `sketch/dim_linear.svg` | **Linear Dimension:** Create a linear distance dimension constraint between points or lines. |
| `sketch/dim_angle.svg` | **Angular Dimension:** Create an angle dimension constraint between two intersecting lines. |
| `sketch/dim_radius.svg` | **Radius Dimension:** Create a radial or diameter dimension constraint for a circle or arc. |

---

### 7.2. Category `constraint/` — Geometric & Dimensional Constraints (12 icons)

| Icon Path | Function & Description |
| :--- | :--- |
| `constraint/coincident.svg` | **Coincident:** Constrain two points to share the same location, or pin a point onto a curve. |
| `constraint/horizontal.svg` | **Horizontal:** Force a line segment or pair of points to align parallel to the X-axis. |
| `constraint/vertical.svg` | **Vertical:** Force a line segment or pair of points to align parallel to the Y-axis. |
| `constraint/parallel.svg` | **Parallel:** Constrain two lines to remain parallel to each other. |
| `constraint/perpendicular.svg` | **Perpendicular:** Constrain two lines to meet at a strict 90-degree right angle. |
| `constraint/equal.svg` | **Equal:** Constrain two lines to equal length, or two circles/arcs to equal radius. |
| `constraint/collinear.svg` | **Collinear:** Constrain two lines to lie along the exact same infinite line. |
| `constraint/concentric.svg` | **Concentric:** Constrain two circles or arcs to share the same center point. |
| `constraint/tangent.svg` | **Tangent:** Constrain a line and curve, or two curves, to be smooth and tangent at their contact point. |
| `constraint/symmetric.svg` | **Symmetric:** Constrain two geometric elements symmetrically across a reference centerline. |
| `constraint/midpoint.svg` | **Midpoint:** Constrain a point to lie at the exact center of a line segment. |
| `constraint/lock.svg` | **Lock / Fix:** Fully fix a point or curve in place, removing all remaining degrees of freedom. |

---

### 7.3. Category `part/` — 3D Modeling, Primitives, Booleans & Surfaces (34 icons)

| Icon Path | Function & Description |
| :--- | :--- |
| `part/extrude.svg` | **Extrude (Pad):** Extrude a 2D sketch profile linearly along its normal into a 3D solid body. |
| `part/revolve.svg` | **Revolve (Shaft):** Revolve a 2D sketch profile around an axis of revolution to generate a solid of revolution. |
| `part/sweep.svg` | **Sweep:** Sweep a planar 2D profile along a 3D guide curve (spine) to form complex channels or tubes. |
| `part/loft.svg` | **Loft:** Smoothly blend two or more planar cross-section profiles into an organic 3D solid. |
| `part/fillet.svg` | **3D Fillet:** Round selected solid body edges with a uniform or variable circular radius. |
| `part/chamfer.svg` | **3D Chamfer:** Bevel selected solid body edges with an angled flat transition. |
| `part/shell.svg` | **Shell:** Hollow out a solid body leaving walls of specified thickness and opening selected faces. |
| `part/hole.svg` | **Hole:** Create parametric simple, counterbored, countersunk, or tapped holes on solid faces. |
| `part/draft.svg` | **Draft Angle:** Taper selected planar faces relative to a mold pull direction for casting. |
| `part/thread.svg` | **Thread:** Generate cosmetic or modeled helical screw threads on cylindrical solid faces. |
| `part/push_face.svg` | **Push/Pull Face:** Direct modeling: offset or move a solid face along its normal vector. |
| `part/remove_face.svg` | **Remove Face:** Direct modeling: delete a solid face and extend neighbouring faces to heal the gap. |
| `part/split_body.svg` | **Split Body:** Divide a single solid body into multiple independent pieces using a plane or surface. |
| `part/split_face.svg` | **Split Face:** Divide a solid face into multiple sub-faces using an intersecting wire or face. |
| `part/thicken.svg` | **Thicken:** Add thickness to an open surface sheet to convert it into a solid 3D body. |
| `part/face_copy.svg` | **Extract Face:** Copy an existing solid face as an independent parametric sheet surface. |
| `part/measure.svg` | **Measure 3D:** Interactive inspection ruler: measure distances, coordinates, and angles in 3D space. |
| `part/box.svg` | **Box Primitive:** Create a parametric box solid (length, width, height). |
| `part/cylinder.svg` | **Cylinder Primitive:** Create a parametric cylinder solid (radius, height). |
| `part/sphere.svg` | **Sphere Primitive:** Create a parametric sphere solid (radius). |
| `part/cone.svg` | **Cone Primitive:** Create a parametric cone or truncated frustum (bottom radius, top radius, height). |
| `part/torus.svg` | **Torus Primitive:** Create a parametric torus ring (major ring radius, minor tube radius). |
| `part/prism.svg` | **Prism Primitive:** Create a parametric regular polygonal prism solid. |
| `part/boolean_bodies.svg` | **Boolean Bodies Tool:** Launch the multi-body boolean operations toolbar. |
| `part/offset_surface.svg` | **Offset Surface:** Create a new surface sheet at an equidistant normal offset from a reference face. |
| `part/surface_replace.svg` | **Replace Face:** Substitute an existing face of a solid body with a curved surface sheet. |
| `part/patch.svg` | **Boundary Patch:** Fill a closed boundary loop of 3D curves with a smooth surface patch. |
| `part/stitch.svg` | **Stitch Surfaces:** Sew neighbouring surface sheets along shared edges into an unified shell. |
| `part/trim_surface.svg` | **Trim Surface:** Trim away surface sheet segments using cutting curves or intersecting faces. |
| `part/recognise.svg` | **Recognize Shapes:** Feature recognition: detect planes, cylinders, and primitives on imported mesh/BREP data. |
| `part/array_linear.svg` | **3D Linear Pattern:** Replicate 3D features or solid bodies along linear directions. |
| `part/array_circular.svg` | **3D Circular Pattern:** Replicate 3D features or solid bodies rotationally around an axis. |
| `part/mirror.svg` | **3D Mirror:** Mirror 3D features or bodies across a planar symmetry datum. |
| `part/section.svg` | **Section View:** Cut a live interactive clipping cross-section through the 3D model. |

---

### 7.4. Category `assembly/` — Components, Mates & Mechanisms (13 icons)

| Icon Path | Function & Description |
| :--- | :--- |
| `assembly/new_part.svg` | **New Part:** Create a new empty part component inside the active assembly hierarchy. |
| `assembly/new_subassembly.svg` | **New Subassembly:** Create a new empty subassembly node in the component tree. |
| `assembly/insert_component.svg` | **Insert Component:** Import an external part or subassembly document into the assembly. |
| `assembly/joint.svg` | **Joint / Mate:** Create an engineering kinematic mate between components (Rigid, Revolute, Slider, Cylindrical, Ball). |
| `assembly/ground.svg` | **Ground:** Lock / anchor a component rigidly to the root assembly coordinate origin. |
| `assembly/group.svg` | **Rigid Group:** Fasten a collection of components together as a rigid cluster without individual mates. |
| `assembly/width.svg` | **Width Mate:** Center a component midway between two opposing parallel boundary walls. |
| `assembly/tangent.svg` | **Tangent Mate:** Mate a cylindrical or curved face in tangent contact with a plane. |
| `assembly/relation.svg` | **Gear / Rack Relation:** Couple kinematic degrees of freedom between two joints (gear ratios, linear racks). |
| `assembly/array_linear.svg` | **Linear Component Pattern:** Replicate components in linear assembly patterns. |
| `assembly/array_circular.svg` | **Circular Component Pattern:** Replicate components in circular assembly patterns. |
| `assembly/mirror.svg` | **Mirror Component:** Mirror parts or subassemblies across a planar symmetry datum. |
| `assembly/section.svg` | **Assembly Section View:** Toggle parametric cross-section cut plane view of the assembly. |

---

### 7.5. Category `datum/` — Reference Planes, Axes & Points (4 icons)

| Icon Path | Function & Description |
| :--- | :--- |
| `datum/axis.svg` | **Datum Axis:** Construct a parametric reference axis (cylinder centerline, line, face normal). |
| `datum/plane.svg` | **Datum Plane:** Construct an auxiliary reference plane (offset, angled, 3-point, tangent). |
| `datum/point.svg` | **Datum Point:** Construct a parametric reference point in 3D space. |
| `datum/sketch_pick_plane.svg` | **New Sketch Plane:** Select a planar face or datum plane to begin drawing a new 2D sketch. |

---

## 8. Packaging & Installation

### 8.1. Packaging into `.qicons`

A `.qicons` bundle can be prepared in two ways:

1. **In-App Packager (Recommended — Verified Archive):**
   Use the built-in Packager UI inside QymCAD (**Settings $\rightarrow$ Appearance $\rightarrow$ Manage Icon Themes $\rightarrow$ Package Theme**).
   The Packager validates SVG elements, enforces square viewBox ratios, verifies token fallback values, builds a clean ZIP archive, and appends a SHA-256 integrity trailer (`QCAD` magic trailer). Bundles packaged with this trailer load with **Verified Archive** status for maximum speed and security.

2. **Standard ZIP Archive (Community Archive):**
   You can also create a plain ZIP archive using standard command-line tools:
   ```bash
   cd my-custom-theme/
   zip -r ../my-custom-theme.qicons manifest.ron README*.md icons/
   ```
   *Note:* Archives packaged with standard `zip` do not include the cryptographic trailer and will be loaded in community **Archive** mode with runtime crash-guards.

### 8.2. User Installation Paths
Place your unpacked theme folder or `.qicons` archive into the user configuration directory:
* **Linux:** `~/.config/qymcad/icon_themes/` or `~/.local/share/qymcad/icon_themes/`
* **Windows:** `%APPDATA%\qymcad\config\icon_themes\` or `%APPDATA%\qymis\qymcad\data\icon_themes\`
* **macOS:** `~/Library/Application Support/qymcad/icon_themes/`

Open **Settings $\rightarrow$ Appearance $\rightarrow$ Manage Icon Themes** in QymCAD to select and activate your theme.
