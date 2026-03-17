use std::collections::{HashMap, VecDeque};
use egui::Widget;

use crate::gb::cpu::{CycleExecution, ExecutionType, Opcode};
use crate::gb::util::{Address, Width};

use self::opcode::OpcodeDescriptor;

use super::cpu::registers::DReg;
use super::{Cycles, GameboyColor};

mod opcode;

pub struct Debugger {
    opcodes: HashMap<Opcode, OpcodeDescriptor>,
    breakpoint_box: String,
    breakpoints: HashMap<Address, bool>,
    breakpoint: bool,
    delete_mode: bool,
    step: bool,
    run: bool,
    error: Option<String>,
    serial: Vec<u8>,
    history: VecDeque<ExecutionType>,
}

impl Debugger {

    pub fn new() -> Self {
        Self {
            opcodes: opcode::generate_table(),
            breakpoints: Default::default(),
            breakpoint_box: String::new(),
            delete_mode: false,
            step: false,
            run: false,
            breakpoint: false,
            error: None,
            serial: Vec::new(),
            history: VecDeque::new(),
        }
        
    }

    pub fn on_cycle(&mut self, result: CycleExecution) {
        if matches!(result.execution, ExecutionType::Halt) {
            if matches!(self.history.back(), Some(&ExecutionType::Halt)) {
                return;
            }
        }
        self.history.push_back(result.execution);
        if self.history.len() > 100 {
            self.history.pop_front();
        }
    }

    pub fn window(&mut self, gb: &mut GameboyColor, ctx: &egui::Context) {
        use crate::gb::cpu::registers::*;
        egui::Window::new(format!("Debug - {}", gb.title())).show(ctx, |ui| {

            ui.columns(4, |cols| {

                // address space / error 

                let mut address = Address(gb.cpu.registers[DReg::PC]);

                let opcol = &mut cols[0];

                for i in 0..5 {
                    match gb.bus.read(address) {
                        Ok(op) => {
                            let opcode = Opcode(op);
                            let ptr = match i == 0 {
                                true => "<-",
                                false => "",
                            };
                            match self.opcodes.get(&opcode) {
                                Some(desc) => {
                                    opcol.label(format!("{address}\t{opcode},\t{}\t{ptr}", desc.format(&gb.bus, address)));
                                    address.0 += desc.length as u16;
                                },
                                None => {
                                    opcol.label(format!("{address}\t{opcode},\tUnknown\t{ptr}"));
                                    address.0 += 1;
                                },
                            }
                        }
                        Err(err) => {
                            opcol.label(format!("{address} : Invalid memory location, {err}"));
                            break;
                        },
                    }
                }
                
                let regcol = &mut cols[1];

                regcol.label("Registers and I/O");
                regcol.separator();

                regcol.columns(2, |cols| {

                    cols[0].label(format!("AF=\t{:#04X}", gb.cpu.registers[DReg::AF]));
                    cols[1].label(format!("LCDC=\t{:#02X}", gb.bus.ppu.lcdc()));
        
                    cols[0].label(format!("BC=\t{:#04X}", gb.cpu.registers[DReg::BC]));
                    cols[1].label(format!("STAT=\t{:#06b}", gb.bus.ppu.stat()));
        
                    cols[0].label(format!("DE=\t{:#04X}", gb.cpu.registers[DReg::DE]));
                    cols[1].label(format!("LY=\t{:#02X}", gb.bus.ppu.ly()));
        
                    cols[0].label(format!("HL=\t{:#04X}", gb.cpu.registers[DReg::HL]));
                    cols[1].label(format!("PPU=\t{:#02X}", gb.bus.ppu.clock()));
        
                    cols[0].label(format!("SP=\t{:#04X}", gb.cpu.registers[DReg::SP]));
                    cols[1].label(format!("HALT=\t{}", gb.bus.interrupts.is_halting()));
        
                    cols[0].label(format!("PC=\t{:#04X}", gb.cpu.registers[DReg::PC]));
                    cols[1].label(format!("DMA=\t{}", gb.bus.dma.is_active()));

                    cols[0].label(format!("IE=\t{:#05b}", gb.bus.interrupts.ie()));
                    cols[1].label(format!("IME=\t{}", gb.bus.interrupts.ime()));

                    cols[0].label(format!("DIV=\t{:#02X}", gb.bus.timer.div()));
                    cols[1].label(format!("TAC=\t{:#02X}", gb.bus.timer.tac()));

                    cols[0].label(format!("TIMA=\t{:#02X}", gb.bus.timer.tima()));
                    cols[1].label(format!("TMA=\t{:#02X}", gb.bus.timer.tma()));

                    cols[0].label(format!("todo"));
                    cols[1].label(format!("ROM=\t0x{:02X}", gb.bus.rom_bank()));
                });

                regcol.separator();

                if regcol.button("PRINT FRAMEBUFFER").clicked() {
                    println!("{:?}", gb.bus.ppu.framebuffer());
                }

                let sercol = &mut cols[2];

                sercol.label("Serial I/O");
                sercol.separator();

                if !gb.bus.serial.output.is_empty() {
                    self.serial.extend(gb.bus.serial.output.drain(..));
                }

                if self.serial.len() < 128 {
                    egui::ScrollArea::vertical().id_salt("serbytes").show(sercol, |sercol| {
                        let bytes = self.serial.iter().fold(String::new(), |prev, next| format!("{prev}{next:02X}"));
                        egui::Label::new(format!("{}", bytes)).wrap_mode(egui::TextWrapMode::Wrap).ui(sercol);
                    });
                    sercol.separator();
                }
                
                egui::ScrollArea::vertical().id_salt("sertext").show(sercol, |sercol| {
                    egui::Label::new(String::from_utf8_lossy(&self.serial)).wrap_mode(egui::TextWrapMode::Wrap).ui(sercol);
                });

                let bpcol = &mut cols[3];

                // breakpoints

                if egui::Button::new("Delete Mode").selected(self.delete_mode).ui(bpcol).clicked() {
                    self.delete_mode = !self.delete_mode;
                }

                let mut remove = None;
                for (addr, enabled) in &mut self.breakpoints {
                    if egui::Button::new(format!("{addr}")).selected(*enabled).ui(bpcol).clicked() {
                        self.run = false;
                        match self.delete_mode {
                            true => {
                                remove = Some(*addr);
                            },
                            false => {
                                *enabled = !*enabled;
                            },
                        }
                    }
                }
                if let Some(addr) = remove {
                    self.breakpoints.remove(&addr);
                }

                bpcol.separator();

                bpcol.text_edit_singleline(&mut self.breakpoint_box);
                if bpcol.button("Add Breakpoint").clicked() {
                    self.run = false;
                    match Width::from_str_radix(&self.breakpoint_box, 16) {
                        Ok(addr) => {
                            self.breakpoints.insert(Address(addr), true);
                            self.breakpoint_box.clear();
                        },
                        Err(..) => (),
                    }
                };

                bpcol.separator();

                egui::ScrollArea::vertical().id_salt("ophistory").max_height(50.0).show(bpcol, |bpcol| {
                    for (i, addr) in self.history.iter().rev().enumerate() {
                        let i = -(i as isize);
                        bpcol.label(format!("{i}: {}", match addr {
                            ExecutionType::Interrupt(address) => format!("interrupt jump to {address}"),
                            ExecutionType::Halt => "halt".to_string(),
                            ExecutionType::Opcode(address) => {
                                format!("{address} {}", gb.bus.read_dma(*address)
                                    .and_then(|op| self.opcodes.get(&Opcode(op))
                                        .map(|desc| format!("({})", desc.format(&gb.bus, *address)))
                                    ).unwrap_or_else(|| "Unknown".to_string()))
                            },
                        }));
                    }
                });

            });

            ui.separator();

            ui.columns(3, |buttons| {

                let no_error = self.error.is_none();

                if buttons[0].add_enabled(no_error, egui::Button::new("Step").small()).clicked() {
                    self.run = false;
                    self.step = true;
                }

                if buttons[1].add_enabled(no_error, egui::Button::new("Run").small().selected(self.run)).clicked() {
                    self.run = !self.run;
                    self.step = self.run;
                }

                if buttons[2].add(egui::Button::new("Reset").small()).clicked() {
                    self.reset(gb);
                }

            });

            ui.separator(); 

            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::RED, error);
            }

        });
    }

    pub fn should_step(&mut self, gb: &GameboyColor) -> bool {
        if self.run {
            let pc = Address(gb.cpu.registers[DReg::PC]);
            if self.breakpoints.get(&pc).copied().unwrap_or_default() {
                if !self.breakpoint {
                    self.step = false;
                    self.breakpoint = true;
                    false
                } else if self.step {
                    self.breakpoint = false;
                    true
                } else {
                    false
                }
            } else {
                true
            }
        } else if self.step {
            self.step = false;
            true
        } else {
            false
        }
    }

    pub fn cycle_multiplier(&self) -> Cycles {
        Cycles(8)
    }

    pub fn pause(&mut self) {
        self.run = false;
    }

    pub fn error(&mut self, err: super::cpu::CycleError) {
        self.pause();
        self.error = Some(err.to_string());
    }
    
    pub fn reset(&mut self, gb: &mut GameboyColor) {
        self.error = None;
        self.run = false;
        self.step = false;
        self.breakpoint = false;
        self.delete_mode = false;
        self.breakpoint_box.clear();
        gb.reset();
    }

}