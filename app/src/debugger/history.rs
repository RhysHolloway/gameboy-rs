use std::collections::VecDeque;

use gameboy_core::cpu::{CycleResult, ExecutionType, Opcode};
use pixels::winit::dpi::PhysicalSize;

use crate::debugger::opcode::OpcodeTable;

struct ExecutionHistory {
    kind: ExecutionType,
    count: usize,
}

#[derive(Default)]
pub struct HistoryView {
    history: VecDeque<ExecutionHistory>,
}

impl HistoryView {
    const MAXIMUM: usize = 1000;
    const SALT: &str = "opcode_history";

    pub fn window(
        &mut self,
        opcodes: &OpcodeTable,
        gb: &gameboy_core::GameboyColor,
        cart: &dyn gameboy_core::Cartridge,
        window: PhysicalSize<u32>,
        ui: &mut egui::Ui,
    ) {
        egui::ScrollArea::vertical()
            .id_salt(Self::SALT)
            .max_height(window.height as f32 / 2.0)
            .auto_shrink([false, true])
            // .max_height(window.height as f32 / 2.0)
            .show(ui, |bpcol| {
                for (i, prev) in self.history.iter().rev().enumerate() {
                    let i = -(i as isize);

                    let kind = match &prev.kind {
                        ExecutionType::Interrupt(address) => {
                            format!("interrupt jump to {address}")
                        }
                        ExecutionType::Halt => format!("halt"),
                        ExecutionType::Stop => format!("stop"),
                        ExecutionType::Opcode(address) => {
                            let opcode = Opcode(gb.bus.read::<true>(cart, *address));
                            format!(
                                "{address}: {opcode} {}",
                                opcodes
                                    .get(&opcode)
                                    .map(|desc| format!(
                                        "({})",
                                        desc.format(cart, &gb.bus, *address)
                                    ))
                                    .unwrap_or_else(|| format!("Unknown"))
                            )
                        }
                    };

                    let count = if prev.count > 1 {
                        format_args!(" (x{})", prev.count)
                    } else {
                        format_args!("")
                    };

                    bpcol.label(format!("{i} | {kind}{count}"));
                }
            });
    }

    pub fn on_cycle(&mut self, result: &CycleResult) {
        match self
            .history
            .back_mut()
            .filter(|prev| prev.kind == result.kind)
        {
            Some(prev) => {
                prev.count += 1;
            }
            None => {
                self.history.push_back(ExecutionHistory {
                    kind: result.kind,
                    count: 1,
                });
                if self.history.len() > Self::MAXIMUM {
                    self.history.pop_front();
                }
            }
        }
    }
}
