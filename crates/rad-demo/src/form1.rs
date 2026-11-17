//! Hand-written half of Form1, same split WinForms uses: the designer owns
//! `form1.designer.rs` and regenerates it freely on every save; this file
//! is yours, and the designer never touches it. Event handlers and other
//! logic belong here, wired up against `Form1Controls`' fields.
//!
//! This form uses every control `rad-core` has. Controls are shared with
//! their own handlers through an `Rc` (they are not `Clone`); the cycle is
//! deliberate and harmless, since the form lives for the whole program.

#[path = "form1.designer.rs"]
mod designer;

pub use designer::Form1Controls;

use rad_core::{Color, Window};
use std::cell::Cell;
use std::rc::Rc;


pub fn build(window: &Window) -> Rc<Form1Controls> {
    let c = Rc::new(designer::build(window));
    let step = Rc::new(Cell::new(10u32));
    let ticks = Rc::new(Cell::new(0u32));

    // A PictureBox with no image would be invisible, so give it a color.
    c.picture1.set_back_color(Some(Color::rgb(70, 130, 180)));

    // Button + TextBox + Label: read the text box, write the status label.
    {
        let c2 = c.clone();
        c.button1.set_on_click(Box::new(move || {
            c2.label2.set_text(&format!("Hello, {}!", c2.textbox1.text()));
        }));
    }

    // CheckBox: reports its new state.
    {
        let c2 = c.clone();
        c.checkbox1.set_on_click(Box::new(move || {
            let state = if c2.checkbox1.checked() { "enabled" } else { "disabled" };
            c2.label2.set_text(&format!("Progress {state}"));
            // The timer follows the checkbox: it only advances the bar while enabled.
            c2.timer1.set_enabled(c2.checkbox1.checked());
        }));
    }

    // Timer: advance the progress bar by the chosen step and count ticks.
    {
        let (c2, step, ticks) = (c.clone(), step.clone(), ticks.clone());
        c.timer1.set_on_tick(Box::new(move || {
            ticks.set(ticks.get() + 1);
            let next = c2.progress1.value() + step.get();
            c2.progress1.set_value(if next > 100 { 0 } else { next });
            c2.label3.set_text(&format!("Ticks: {}", ticks.get()));
        }));
    }

    // RadioButtons: pick the step size.
    {
        let (c2, step) = (c.clone(), step.clone());
        c.radio1.set_on_click(Box::new(move || {
            step.set(10);
            c2.label2.set_text("Step size: small (10%)");
        }));
    }
    {
        let (c2, step) = (c.clone(), step.clone());
        c.radio2.set_on_click(Box::new(move || {
            step.set(25);
            c2.label2.set_text("Step size: large (25%)");
        }));
    }

    // Step button: advances the ProgressBar, wrapping past 100%.
    {
        let (c2, step) = (c.clone(), step.clone());
        c.button2.set_on_click(Box::new(move || {
            if !c2.checkbox1.checked() {
                c2.label2.set_text("Progress is disabled");
                return;
            }
            let next = c2.progress1.value() + step.get();
            let next = if next > 100 { 0 } else { next };
            c2.progress1.set_value(next);
            c2.label2.set_text(&format!("Progress: {next}%"));
        }));
    }

    c
}
