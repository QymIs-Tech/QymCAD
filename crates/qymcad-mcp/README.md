# qymcad-mcp

A [Model Context Protocol](https://modelcontextprotocol.io) server for QymCAD. A language model connected to it builds,
edits, measures and exports QymCAD documents. The document is built by the same OCCT kernel the window drives, and
the file it saves is an ordinary `.qcad` that the window opens.

It runs without a window. It goes through the core (`Project`, `finish_base_body`, `regenerate`), as the core's own
checks build a part, and does not touch `App`.

## What it does

| Tool | What it does |
| --- | --- |
| `new_document`, `open_document`, `save_document` | Start, open or save a `.qcad`. |
| `describe_document` | Show the parts, the sketches with their contours, and the timeline with the nodes that failed and why. Bodies come with volume and box; parameters are listed too. |
| `describe_body` | List faces (centre, normal, area) and edges (ends, middle, direction, radius) by their persistent ids, which `fillet`, `chamfer`, `shell` and `hole` take. |
| `sketch` | Draw rectangles, circles and polylines on XY, XZ or YZ. A contour inside another one becomes a hole. |
| `extrude` | Extrude a sketch: add to the part, or cut or intersect a body; forward, backward, both ways, or through everything. |
| `revolve` | Revolve a sketch about the sketch's x or y axis. |
| `primitive` | Add a box, cylinder, sphere, cone or torus. |
| `fillet`, `chamfer` | Round or bevel the listed edges, or every edge. |
| `shell` | Hollow a body, the listed faces opened; inward, outward or centred. |
| `hole` | Drill a plain hole into a planar face. |
| `boolean` | Cut, unite or intersect two bodies. |
| `set_parameter`, `set_dimension` | Create or change a parameter, or drive a dimension of a feature by an expression; everything after it rebuilds. |
| `export` | Write STEP (exact B-rep) or STL (mesh). |

Every change rebuilds the model and answers with what it made, which nodes failed, and the bodies now standing. A
model cannot see the screen, so this answer is the only way it learns that a node went red. A call that cannot be
done is refused with the reason (`isError`), and the document stays as it was.

Lengths are in millimetres and angles in degrees.

## Building

The same way as the program, against OCCT 7.9.3 (see [`packaging/README.md`](../../packaging/README.md)):

```bash
OCCT_INCLUDE_DIR=/opt/occt/include/opencascade OCCT_LIB_DIR=/opt/occt/lib cargo build --release -p qymcad-mcp
```

The binary is `target/release/qymcad-mcp`. On Windows the OCCT DLLs must be next to it or on `PATH`, as for
`qymcad.exe`.

## Connecting a client

The server speaks over stdio, one JSON message per line. Most clients take a block like this one:

```json
{
  "mcpServers": {
    "qymcad": {
      "command": "/path/to/qymcad-mcp"
    }
  }
}
```

A session from the inside, by hand:

```text
-> {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}
-> {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"sketch","arguments":{"shapes":[{"kind":"rectangle","from":[0,0],"to":[40,30]}]}}}
<- ... "sketch": <S>, "contours": [<C>] ...
-> {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"extrude","arguments":{"sketch":<S>,"distance":10}}}
<- ... "bodies": [{"id": <B>, "volume_mm3": 12000.0, ...}] ...
```

## Checks

`tests/a_model_builds_a_part.rs` drives the server only through the protocol, with the real kernel behind it:

- a block is rounded, drilled and driven by a parameter, then saved, opened again and exported to STEP and STL;
- a ring is revolved and cut through;
- a plate with a hole is extruded;
- calls that cannot be done are refused with a reason.

Every volume is checked against its analytic value.
