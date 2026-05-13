use egui::Widget;
use egui::ahash::HashMap;
use gameboy_core::cpu::{CycleResult, DReg, ExecutionType};
use gameboy_core::{Address, Cartridge, GameboyColor};

#[derive(Default)]
pub struct BreakpointView {
    string: String,
    list: HashMap<Address, bool>,
    delete_mode: bool,
    // break_on_interrupt: bool,
    at: bool,
}

impl BreakpointView {
    pub fn window(&mut self, ui: &mut egui::Ui) {
        ui.label("Breakpoints");
        ui.separator();

        ui.columns(3, |cols| {
            cols[0].text_edit_singleline(&mut self.string);

            if cols[1].button("Add").clicked() {
                if let Some(address) = super::read_address(&self.string).map(Address::new) {
                    self.list.insert(address, true);
                    self.string.clear();
                }
            };

            if egui::Button::new("Delete Mode")
                .selected(self.delete_mode)
                .ui(&mut cols[2])
                .clicked()
            {
                self.delete_mode = !self.delete_mode;
            }
        });

        let mut remove = None;

        egui::ScrollArea::vertical()
            .id_salt("breakpoints")
            .show(ui, |ui| {
                for (addr, enabled) in self.list.iter_mut() {
                    if egui::Button::new(format!("{addr}"))
                        .selected(*enabled)
                        .ui(ui)
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
            });

        if let Some(addr) = remove {
            self.list.remove(&addr);
        }

        ui.separator();
    }

    // pub fn on_cycle(&mut self, result: &CycleResult) -> bool {
    //     if let ExecutionType::Interrupt(..) = &result.kind
    //         && self.break_on_interrupt
    //     {
    //         self.at = true;
    //         return true;
    //     }
    //     false
    // }

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

    pub(crate) fn new_cartridge(&mut self, _: &(dyn Cartridge + 'static)) {
        self.list.clear();
        self.delete_mode = false;
    }

    pub fn reset(&mut self) {
        self.at = false;
    }
}
