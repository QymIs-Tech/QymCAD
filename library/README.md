# The built-in libraries (compiled into the binary)

The root of the **shipped** libraries. Everything here is compiled into the binary by `build.rs` through
`include_dir`, which works on every target (a portable `.exe`, an AppImage, macOS) and needs no files lying
beside the application.

The namespace follows the *kind* of library, there being more than one in time:

```
library/
  parts/     <- the parts library (standard parts: profiles, fasteners and so on)
  ...        <- other kinds later (materials, templates, presets)
```

A user's own libraries do NOT come here: they live in the data directory of the operating system
(`<data_dir>/library/...`, read-write). What is here is the shipped set, compiled into the build.
