//! A START AFTER A START THAT DIED DRAWS MORE SAFELY.
//!
//! Reported behaviour: on some cards the program dies in the graphics driver - at the start, or in the first seconds
//! of drawing - and every start after it dies the same way, with nothing a person can change from inside a window
//! that never opens. The journal of the run before (`crash::begin_journal`) tells how it ended; a run that stopped
//! without closing before it had drawn ten seconds of frames died opening the window or drawing, and the next start
//! takes one step down:
//!
//! 1. as chosen - the best card, the antialiasing of the settings;
//! 2. no antialiasing - the multisample pass is where several drivers failed (an RX 580 under DX12 drew nothing with it);
//! 3. another backend of the same card - the one the run that died drew with is avoided (Vulkan where DX12 died,
//!    DX12 or GL where Vulkan died), and stays avoided while the step holds;
//! 4. the processor - slow, and it draws.
//!
//! A run that closed as it should, or that died after the first ten seconds of drawing (more likely the kernel than
//! the driver), keeps the step it had. The step is written into the journal, so the start after reads it back.

use std::sync::atomic::{AtomicU8, Ordering};

/// How the window draws this run, safest last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    AsChosen,
    NoAntialiasing,
    OtherBackend,
    Processor,
}

impl Step {
    fn number(self) -> u8 {
        self as u8
    }

    fn from_number(n: u8) -> Step {
        match n {
            0 => Step::AsChosen,
            1 => Step::NoAntialiasing,
            2 => Step::OtherBackend,
            _ => Step::Processor,
        }
    }

    fn down(self) -> Step {
        Step::from_number((self.number() + 1).min(Step::Processor.number()))
    }
}

/// The line of the journal that carries the step.
const STEP_LINE: &str = "graphics step: ";
/// The line of the journal that names the backend avoided from the third step.
const AVOID_LINE: &str = "avoided backend: ";
/// The line of the journal that names the adapter drawing, as `diagnostics::note_gpu` writes it.
const DRAWING_LINE: &str = "drawing with: wgpu ";

/// THE STEP OF THIS RUN, from the journal the run before left (`None`: there was none).
pub fn step_after(previous: Option<&str>) -> Step {
    let Some(journal) = previous else { return Step::AsChosen };
    let had = journal.lines().rev().find_map(|l| l.strip_prefix(STEP_LINE)).and_then(|n| n.trim().parse::<u8>().ok()).map(Step::from_number).unwrap_or(Step::AsChosen);
    let closed = journal.lines().last().map(str::trim) == Some(crate::crash::ENDED);
    let drew_a_while = journal.lines().any(|l| l.trim() == crate::crash::DREW_A_WHILE);
    if closed || drew_a_while {
        had
    } else {
        had.down()
    }
}

/// THE BACKEND TO KEEP AWAY FROM at this step: from the third step down, the one the journal already avoids, or else
/// the one the run before drew with when it died.
pub fn avoided_after(previous: Option<&str>) -> Option<String> {
    if step_after(previous) < Step::OtherBackend {
        return None;
    }
    let journal = previous?;
    let named = |prefix: &str| journal.lines().rev().find_map(|l| l.strip_prefix(prefix)).map(|rest| rest.split(',').next().unwrap_or(rest).trim().to_string());
    named(AVOID_LINE).or_else(|| named(DRAWING_LINE))
}

static STEP: AtomicU8 = AtomicU8::new(0);
static AVOIDED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// TAKE THE STEP OF THIS RUN and write it into the journal; answers it.
pub fn take(previous: Option<&str>) -> Step {
    let step = step_after(previous);
    STEP.store(step.number(), Ordering::Relaxed);
    crate::crash::journal(&format!("{STEP_LINE}{}", step.number()));
    let avoided = avoided_after(previous);
    if let Some(backend) = &avoided {
        crate::crash::journal(&format!("{AVOID_LINE}{backend}"));
    }
    if let Ok(mut a) = AVOIDED.lock() {
        *a = avoided;
    }
    step
}

/// The backend this run keeps away from, by the name `{:?}` gives it (`Dx12`, `Vulkan`).
pub fn avoided() -> Option<String> {
    AVOIDED.lock().ok().and_then(|a| a.clone())
}

/// The step this run draws at.
pub fn step() -> Step {
    Step::from_number(STEP.load(Ordering::Relaxed))
}

/// THE SAMPLES A PIXEL IS DRAWN WITH at this step: the setting, or one from the second step down.
pub fn samples(setting: u32) -> u32 {
    if step() >= Step::NoAntialiasing {
        1
    } else {
        setting
    }
}

/// THE CATALOGUE KEY OF WHAT THE STATUS LINE SAYS at this step - nothing at the first.
pub fn said(step: Step) -> Option<&'static str> {
    match step {
        Step::AsChosen => None,
        Step::NoAntialiasing => Some("gpu-safe-no-antialiasing"),
        Step::OtherBackend => Some("gpu-safe-other-backend"),
        Step::Processor => Some("gpu-safe-processor"),
    }
}

/// One adapter as the ranking sees it: what kind of device, through which backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offered {
    pub kind: eframe::wgpu::DeviceType,
    pub backend: eframe::wgpu::Backend,
}

/// HOW GOOD AN ADAPTER IS AT A STEP, highest first: the kind of device, then the backend - DX12 and Metal, then
/// Vulkan, then GL. From the third step the backend `avoid` falls below every other one of the same card; from the
/// fourth the processor comes first of all.
pub fn rank(step: Step, offered: Offered, avoid: Option<&str>) -> u16 {
    use eframe::wgpu::{Backend, DeviceType};
    let device = crate::gui::rank_device_type(offered.kind) as u16;
    let device = if step == Step::Processor && offered.kind == DeviceType::Cpu { 100 } else { device };
    let api = match offered.backend {
        Backend::Dx12 | Backend::Metal => 4,
        Backend::Vulkan => 3,
        Backend::Gl => 2,
        _ => 1,
    };
    let avoided = step >= Step::OtherBackend && avoid.is_some_and(|a| a == format!("{:?}", offered.backend));
    device * 10 + if avoided { 0 } else { api }
}

#[cfg(test)]
mod tests {
    use super::{avoided_after, rank, step_after, Offered, Step};
    use eframe::wgpu::{Backend, DeviceType};

    fn journal(lines: &[&str]) -> String {
        lines.iter().map(|l| format!("{l}\n")).collect()
    }

    /// A RUN THAT DIED BEFORE TEN SECONDS OF DRAWING sends the next one a step down, and the steps go down one at a
    /// time to the processor and stay there.
    #[test]
    fn a_run_that_died_early_sends_the_next_a_step_down() {
        assert_eq!(step_after(None), Step::AsChosen, "a first start");
        let died = |step: u8| journal(&["started", &format!("graphics step: {step}"), "drawing with: wgpu Dx12, Radeon (TM) RX 480 Series (DiscreteGpu)", "the first frame is drawn"]);
        assert_eq!(step_after(Some(&died(0))), Step::NoAntialiasing);
        assert_eq!(step_after(Some(&died(1))), Step::OtherBackend);
        assert_eq!(step_after(Some(&died(2))), Step::Processor);
        assert_eq!(step_after(Some(&died(3))), Step::Processor, "below the processor there is nothing");
        let before_a_frame = journal(&["started", "graphics step: 0", "adapter offered: Vulkan/DiscreteGpu Radeon"]);
        assert_eq!(step_after(Some(&before_a_frame)), Step::NoAntialiasing, "a run that died opening the window");
    }

    /// A RUN THAT CLOSED, OR DIED AFTER DRAWING A WHILE, keeps its step: a fault in the middle of work is more
    /// likely the kernel's than the driver's.
    #[test]
    fn a_run_that_closed_or_drew_a_while_keeps_its_step() {
        assert_eq!(step_after(Some(&journal(&["started", "graphics step: 1", "the first frame is drawn", "ended"]))), Step::NoAntialiasing);
        assert_eq!(step_after(Some(&journal(&["started", "graphics step: 2", "the first frame is drawn", "600 frames drawn", "operation: Fillet"]))), Step::OtherBackend);
        assert_eq!(step_after(Some(&journal(&["started", "the first frame is drawn", "ended"]))), Step::AsChosen, "an older journal without a step");
    }

    /// THE CARD IS TAKEN BEFORE THE PROCESSOR AND DX12 BEFORE VULKAN; from the third step the backend the run before
    /// died with is kept away from, and from the fourth the processor comes first.
    #[test]
    fn the_adapter_follows_the_step() {
        let o = |kind, backend| Offered { kind, backend };
        let best = |step: Step, avoid: Option<&str>, offered: &[Offered]| offered.iter().copied().max_by_key(|a| rank(step, *a, avoid));
        let rx480 = [o(DeviceType::DiscreteGpu, Backend::Vulkan), o(DeviceType::DiscreteGpu, Backend::Dx12), o(DeviceType::Cpu, Backend::Dx12), o(DeviceType::Other, Backend::Gl)];
        assert_eq!(best(Step::AsChosen, None, &rx480), Some(o(DeviceType::DiscreteGpu, Backend::Dx12)));
        assert_eq!(best(Step::NoAntialiasing, Some("Dx12"), &rx480), Some(o(DeviceType::DiscreteGpu, Backend::Dx12)), "the backend is changed from the third step only");
        assert_eq!(best(Step::OtherBackend, Some("Dx12"), &rx480), Some(o(DeviceType::DiscreteGpu, Backend::Vulkan)));
        assert_eq!(best(Step::OtherBackend, Some("Vulkan"), &rx480), Some(o(DeviceType::DiscreteGpu, Backend::Dx12)));
        assert_eq!(best(Step::Processor, Some("Dx12"), &rx480), Some(o(DeviceType::Cpu, Backend::Dx12)));
        let nvk = [o(DeviceType::DiscreteGpu, Backend::Vulkan), o(DeviceType::DiscreteGpu, Backend::Gl), o(DeviceType::Cpu, Backend::Vulkan)];
        assert_eq!(best(Step::OtherBackend, Some("Vulkan"), &nvk), Some(o(DeviceType::DiscreteGpu, Backend::Gl)), "Vulkan died: GL of the same card");
    }

    /// THE BACKEND AVOIDED IS THE ONE THE RUN BEFORE DIED WITH, and it stays avoided while the step holds - a run that
    /// closed on Vulkan does not send the next back to the DX12 that died.
    #[test]
    fn the_backend_that_died_stays_avoided() {
        let died_on_dx12 = journal(&["started", "graphics step: 1", "drawing with: wgpu Dx12, Radeon (TM) RX 480 Series (DiscreteGpu), driver 31.0", "the first frame is drawn"]);
        assert_eq!(avoided_after(Some(&died_on_dx12)).as_deref(), Some("Dx12"));
        let closed_on_vulkan = journal(&["started", "graphics step: 2", "avoided backend: Dx12", "drawing with: wgpu Vulkan, Radeon (TM) RX 480 Graphics (DiscreteGpu)", "600 frames drawn", "ended"]);
        assert_eq!(avoided_after(Some(&closed_on_vulkan)).as_deref(), Some("Dx12"));
        let died_without_antialiasing = journal(&["started", "graphics step: 0", "drawing with: wgpu Dx12, Radeon", "the first frame is drawn"]);
        assert_eq!(avoided_after(Some(&died_without_antialiasing)), None, "the second step keeps the backend");
    }
}
