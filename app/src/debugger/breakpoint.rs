use egui::Widget;
use egui::ahash::HashMap;
use gameboy_core::{Address, GameboyColor};
use gameboy_core::cpu::{CycleResult, DReg, ExecutionType};

#[derive(Default)]
pub struct BreakpointView {
    string: String,
    list: HashMap<Address, bool>,
    delete_mode: bool,
    break_on_interrupt: bool,
    at: bool,
}

impl BreakpointView {
    pub fn window(&mut self, bpcol: &mut egui::Ui) {
        // breakpoints

        if egui::Button::new("Delete Mode")
            .selected(self.delete_mode)
            .ui(bpcol)
            .clicked()
        {
            self.delete_mode = !self.delete_mode;
        }

        let mut remove = None;
        for (addr, enabled) in self.list.iter_mut() {
            if egui::Button::new(format!("{addr}"))
                .selected(*enabled)
                .ui(bpcol)
                .clicked()
            {
                match self.delete_mode {
                    true => {
                        remove = Some(*addr);
                    }
                    false => {
                        *enabled = !*enabled;
                    }
                }
            }
        }
        if let Some(addr) = remove {
            self.list.remove(&addr);
        }

        bpcol.separator();

        bpcol.text_edit_singleline(&mut self.string);

        bpcol.columns(2, |cols| {
            if cols[0].button("Add Breakpoint").clicked() {
                if let Some(address) = super::read_address(&self.string).map(Address::new) {
                    self.list.insert(address, true);
                    self.string.clear();
                }
            };
            cols[1].checkbox(&mut self.break_on_interrupt, "Break on interrupt");
        });
    }

    pub fn on_cycle(&mut self, result: &CycleResult) -> bool {
        if let ExecutionType::Interrupt(..) = &result.kind
            && self.break_on_interrupt
        {
            self.at = true;
            return true;
        }
        false
    }

    pub fn should_step(&mut self, step: &mut bool, gb: &GameboyColor) -> bool {
        let pc = Address::new(gb.cpu.registers[DReg::PC]);
        if self.list.get(&pc).copied().unwrap_or_default() {
            if !self.at {
                *step = false;
                self.at = true;
                false
            } else if *step {
                self.at = false;
                true
            } else {
                false
            }
        } else {
            true
        }
    }
}
