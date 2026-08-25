//! The post-processor: a single dialect-driven emitter.
//!
//! One engine (`emit`) plus a table of controller differences (`Dialect`) covers Mach3, GRBL and LinuxCNC. The
//! differences — cycles or no cycles, tool length offset, tool change, preamble, comments — are expressed as
//! flags, which is the whole idea of treating posts as data.

use qymcad_core::ir::{
    ArcDir, CoolantMode, DrillKind, Move, Plane, Program, SpindleDir, ToolRef, Units,
};

use crate::PostOptions;

/// The description of a controller dialect.
#[derive(Clone, Copy)]
pub struct Dialect {
    pub name: &'static str,
    pub preamble: &'static [&'static str],
    pub postamble: &'static [&'static str],
    /// Emit `T.. M6`; otherwise a comment about a manual change.
    pub tool_change: bool,
    /// Emit `G43 H..`, the tool length compensation.
    pub tlo: bool,
    /// Support for the drilling cycles G81, G82 and G83; otherwise they expand into G0 and G1.
    pub canned_cycles: bool,
    /// Comments introduced by `;`; otherwise `( ... )`.
    pub semicolon_comments: bool,
    pub file_ext: &'static str,
}

/// Mach3 and Mach4, the reference dialect.
pub const MACH3: Dialect = Dialect {
    name: "Mach3",
    preamble: &["G17 G54 G40 G49 G80 G90"],
    postamble: &["M05", "G17 G54 G90 G80 G40", "M2"],
    tool_change: true,
    tlo: true,
    canned_cycles: true,
    semicolon_comments: false,
    file_ext: "tap",
};

/// GRBL: no cycles, no tool length offset, manual tool change.
pub const GRBL: Dialect = Dialect {
    name: "GRBL",
    preamble: &["G17 G90 G94 G54"],
    postamble: &["M5", "M2"],
    tool_change: false,
    tlo: false,
    canned_cycles: false,
    semicolon_comments: false,
    file_ext: "nc",
};

/// LinuxCNC: the full set.
pub const LINUXCNC: Dialect = Dialect {
    name: "LinuxCNC",
    preamble: &["G17 G40 G49 G54 G80 G90 G94"],
    postamble: &["M5", "M2"],
    tool_change: true,
    tlo: true,
    canned_cycles: true,
    semicolon_comments: false,
    file_ext: "ngc",
};

/// Mach3 output, kept for compatibility and used by the tests.
pub fn post(program: &Program, opts: &PostOptions) -> String {
    emit(program, &MACH3, opts)
}

/// Generate G-code for an arbitrary dialect.
pub fn emit(program: &Program, dialect: &Dialect, opts: &PostOptions) -> String {
    let mut w = Writer::new(opts.clone(), *dialect);

    if opts.output_header {
        // Comments inside the program stay in Latin script, and that is not an oversight. They are read by
        // the controller rather than by a window: Mach3 and GRBL display a program line through their own
        // character generator, where anything else comes out as rubbish or truncates the line. The language of
        // the interface does not affect the contents of the .nc file.
        w.comment(&program.name);
        w.comment(&format!("Controller: {}", dialect.name));
        w.tool_list(program);
    }

    for line in dialect.preamble {
        w.raw(line);
    }
    w.raw(match program.units {
        Units::Mm => "G21",
        Units::Inch => "G20",
    });

    for tp in &program.toolpaths {
        // the work coordinate system, emitted when the setup changes
        let wcs = tp.meta.wcs;
        if (54..=59).contains(&wcs) && w.last_wcs != Some(wcs) {
            w.raw(&format!("G{wcs}"));
            w.last_wcs = Some(wcs);
        }
        for m in &tp.moves {
            w.emit_move(m);
        }
    }

    for line in dialect.postamble {
        w.raw(line);
    }

    w.finish()
}

struct Writer {
    opts: PostOptions,
    dialect: Dialect,
    out: String,
    line_no: u32,
    last_motion: Option<&'static str>,
    last_feed: Option<String>,
    last_x: Option<String>,
    last_y: Option<String>,
    last_z: Option<String>,
    last_tool: Option<u32>,
    last_spindle: Option<String>,
    last_wcs: Option<u8>,
}

impl Writer {
    fn new(opts: PostOptions, dialect: Dialect) -> Self {
        Self {
            opts,
            dialect,
            out: String::new(),
            line_no: 10,
            last_motion: None,
            last_feed: None,
            last_x: None,
            last_y: None,
            last_z: None,
            last_tool: None,
            last_spindle: None,
            last_wcs: Some(54), // the preamble already contains G54
        }
    }

    fn finish(self) -> String {
        self.out
    }

    fn axis(&self, v: f64) -> String {
        fmt(v, self.opts.axis_precision)
    }

    fn raw(&mut self, line: &str) {
        if self.opts.output_line_numbers {
            self.out.push_str(&format!("N{} {}\n", self.line_no, line));
            self.line_no += 5;
        } else {
            self.out.push_str(line);
            self.out.push('\n');
        }
    }

    fn comment(&mut self, text: &str) {
        if !self.opts.output_comments {
            return;
        }
        if self.dialect.semicolon_comments {
            self.raw(&format!("; {text}"));
        } else {
            let safe = text.replace('(', "[").replace(')', "]");
            self.raw(&format!("({safe})"));
        }
    }

    fn tool_list(&mut self, program: &Program) {
        let mut seen: Vec<u32> = Vec::new();
        for tp in &program.toolpaths {
            if let Some(ToolRef { number, name }) = &tp.meta.tool {
                if !seen.contains(number) {
                    seen.push(*number);
                    self.comment(&format!("T{number}: {name}"));
                }
            }
        }
    }

    fn emit_move(&mut self, m: &Move) {
        match m {
            Move::Rapid { to } => self.motion("G0", Some(to.x), Some(to.y), Some(to.z), None),
            Move::Linear { to, feed } => self.motion("G1", Some(to.x), Some(to.y), Some(to.z), Some(*feed)),
            Move::Plunge { to, feed } => self.motion("G1", Some(to.x), Some(to.y), Some(to.z), Some(*feed)),
            Move::Arc { to, center, plane, dir, feed } => self.arc(to, center, *plane, *dir, *feed),
            Move::Helix { to, center, dir, feed, .. } => self.arc(to, center, Plane::XY, *dir, *feed),
            Move::Dwell { secs } => self.raw(&format!("G4 P{}", fmt(*secs, 3))),
            Move::SpindleOn { rpm, dir } => {
                let code = match dir {
                    SpindleDir::Cw => "M3",
                    SpindleDir::Ccw => "M4",
                };
                let s = format!("S{} {code}", fmt(*rpm, self.opts.spindle_decimals));
                if self.last_spindle.as_deref() != Some(&s) {
                    self.raw(&s);
                    self.last_spindle = Some(s);
                }
            }
            Move::SpindleOff => self.raw("M5"),
            Move::ToolChange { tool } => self.tool_change(tool),
            Move::Coolant { mode } => self.raw(match mode {
                CoolantMode::Mist => "M7",
                CoolantMode::Flood => "M8",
                CoolantMode::Off => "M9",
            }),
            Move::RotaryTo { a, feed } => {
                let mut line = format!("A{}", self.axis(*a));
                if let Some(f) = feed {
                    line.push_str(&format!(" F{}", fmt(*f, self.opts.feed_precision)));
                }
                self.raw(&line);
            }
            Move::DrillCycle { kind, points, retract, feed, peck, dwell } => {
                self.drill_cycle(*kind, points, *retract, *feed, *peck, *dwell)
            }
            Move::Comment { text } => self.comment(text),
        }
    }

    fn tool_change(&mut self, tool: &ToolRef) {
        if self.last_tool == Some(tool.number) {
            return;
        }
        self.last_tool = Some(tool.number);
        if self.dialect.tool_change {
            self.raw(&format!("T{} M6", tool.number));
            self.comment(&tool.name);
            if self.dialect.tlo && self.opts.output_tool_length_offset {
                self.raw(&format!("G43 H{}", tool.number));
            }
        } else {
            self.comment(&format!("Manual tool change: T{} {}", tool.number, tool.name));
        }
        self.last_x = None;
        self.last_y = None;
        self.last_z = None;
    }

    fn motion(&mut self, g: &'static str, x: Option<f64>, y: Option<f64>, z: Option<f64>, feed: Option<f64>) {
        let mut words = String::new();
        if self.last_motion != Some(g) {
            words.push_str(g);
            self.last_motion = Some(g);
        }

        let mut moved = false;
        for (val, letter, last) in [
            (x, 'X', &mut self.last_x),
            (y, 'Y', &mut self.last_y),
            (z, 'Z', &mut self.last_z),
        ] {
            if let Some(v) = val {
                let s = fmt(v, self.opts.axis_precision);
                if last.as_deref() != Some(&s) {
                    push_word(&mut words, &format!("{letter}{s}"));
                    *last = Some(s);
                    moved = true;
                }
            }
        }

        if let Some(f) = feed {
            let s = fmt(f, self.opts.feed_precision);
            if self.last_feed.as_deref() != Some(&s) {
                push_word(&mut words, &format!("F{s}"));
                self.last_feed = Some(s);
            }
        }

        if moved {
            self.raw(words.trim());
        }
    }

    fn arc(&mut self, to: &qymcad_core::geom::Point3, center: &qymcad_core::geom::Point3, plane: Plane, dir: ArcDir, feed: f64) {
        let g = if dir == ArcDir::Cw { "G2" } else { "G3" };
        let sx: f64 = self.last_x.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let sy: f64 = self.last_y.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let sz: f64 = self.last_z.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let (i, j, k) = (center.x - sx, center.y - sy, center.z - sz);

        let mut words = String::from(g);
        self.last_motion = Some(g);
        push_word(&mut words, &format!("X{}", self.axis(to.x)));
        push_word(&mut words, &format!("Y{}", self.axis(to.y)));
        if (to.z - sz).abs() > 1e-9 {
            push_word(&mut words, &format!("Z{}", self.axis(to.z)));
        }
        match plane {
            Plane::XY => {
                push_word(&mut words, &format!("I{}", self.axis(i)));
                push_word(&mut words, &format!("J{}", self.axis(j)));
            }
            Plane::XZ => {
                push_word(&mut words, &format!("I{}", self.axis(i)));
                push_word(&mut words, &format!("K{}", self.axis(k)));
            }
            Plane::YZ => {
                push_word(&mut words, &format!("J{}", self.axis(j)));
                push_word(&mut words, &format!("K{}", self.axis(k)));
            }
        }
        let fs = fmt(feed, self.opts.feed_precision);
        if self.last_feed.as_deref() != Some(&fs) {
            push_word(&mut words, &format!("F{fs}"));
            self.last_feed = Some(fs);
        }
        self.raw(&words);
        self.last_x = Some(self.axis(to.x));
        self.last_y = Some(self.axis(to.y));
        self.last_z = Some(self.axis(to.z));
    }

    fn drill_cycle(&mut self, kind: DrillKind, points: &[qymcad_core::geom::Point3], retract: f64, feed: f64, peck: Option<f64>, dwell: Option<f64>) {
        if points.is_empty() {
            return;
        }
        let canned = self.dialect.canned_cycles && !self.opts.translate_drill_cycles;
        if !canned {
            self.drill_translate(points, retract, feed, peck);
            return;
        }
        let g = match kind {
            DrillKind::Drill => "G81",
            DrillKind::DwellDrill => "G82",
            DrillKind::Peck => "G83",
        };
        self.raw("G98");
        for (idx, p) in points.iter().enumerate() {
            let line = if idx == 0 {
                let mut s = format!("{g} X{} Y{} Z{} R{}", self.axis(p.x), self.axis(p.y), self.axis(p.z), self.axis(retract));
                if matches!(kind, DrillKind::Peck) {
                    if let Some(q) = peck {
                        s.push_str(&format!(" Q{}", self.axis(q)));
                    }
                }
                if matches!(kind, DrillKind::DwellDrill) {
                    if let Some(d) = dwell {
                        s.push_str(&format!(" P{}", fmt(d, 3)));
                    }
                }
                s.push_str(&format!(" F{}", fmt(feed, self.opts.feed_precision)));
                s
            } else {
                format!("X{} Y{}", self.axis(p.x), self.axis(p.y))
            };
            self.raw(line.trim());
        }
        self.raw("G80");
        self.last_motion = None;
        self.last_x = points.last().map(|p| self.axis(p.x));
        self.last_y = points.last().map(|p| self.axis(p.y));
        self.last_z = None;
    }

    fn drill_translate(&mut self, points: &[qymcad_core::geom::Point3], retract: f64, feed: f64, peck: Option<f64>) {
        for p in points {
            self.motion("G0", Some(p.x), Some(p.y), Some(retract), None);
            match peck {
                Some(q) if q > 0.0 => {
                    let mut z = retract;
                    while z > p.z + 1e-6 {
                        z = (z - q).max(p.z);
                        self.motion("G1", None, None, Some(z), Some(feed));
                        self.motion("G0", None, None, Some(retract), None);
                    }
                }
                _ => {
                    self.motion("G1", None, None, Some(p.z), Some(feed));
                    self.motion("G0", None, None, Some(retract), None);
                }
            }
        }
    }
}

fn push_word(line: &mut String, word: &str) {
    if !line.is_empty() {
        line.push(' ');
    }
    line.push_str(word);
}

fn fmt(v: f64, prec: u8) -> String {
    let v = if v == 0.0 { 0.0 } else { v };
    format!("{:.*}", prec as usize, v)
}
