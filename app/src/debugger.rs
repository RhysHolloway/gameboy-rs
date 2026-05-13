use egui::Widget;
use gameboy_core::Cartridge;
use pixels::winit::dpi::PhysicalSize;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Receiver, Sender};

use gameboy_core::cpu::{CycleResult, DReg, ExecutionType, Opcode, Reg};
use gameboy_core::util::{Address, Width};

use self::opcode::OpcodeDescriptor;

use super::GameboyColor;

mod breakpoint;
mod history;
mod memory;
mod opcode;
mod serial;
mod speed;
mod status;
mod vram;

pub fn read_address(string: &str) -> Option<Width> {
    if string.starts_with("0x") {
        Width::from_str_radix(&string[2..], 16).ok()
    } else {
        Width::from_str_radix(string, 10).ok()
    }
}

#[derive(Default)]
pub struct Debugger {
    opcodes: opcode::OpcodeTable,
    memory: memory::MemoryView,
    breakpoint: breakpoint::BreakpointView,
    history: history::HistoryView,
    speed: speed::Speed,
    serial: serial::SerialView,
    step: bool,
    run: bool,
}

impl Debugger {
    pub fn create_serial_callback(&mut self) -> Box<dyn FnMut(u8)> {
        self.serial.create_serial_callback()
    }

    pub fn log(&mut self, cart: &dyn Cartridge, gb: &GameboyColor) {
        let address = Address::new(gb.cpu.registers[DReg::PC]);
        // A:00 F:11 B:22 C:33 D:44 E:55 H:66 L:77 SP:8888 PC:9999 PCMEM:AA,BB,CC,DD
        println!(
            "A:{:02X} F:{:02X} B:{:02X} C:{:02X} D:{:02X} E:{:02X} H:{:02X} L:{:02X} SP:{:04X} PC:{:04X} PCMEM:{:02X},{:02X},{:02X},{:02X}",
            gb.cpu.registers[Reg::A],
            gb.cpu.registers[Reg::F],
            gb.cpu.registers[Reg::B],
            gb.cpu.registers[Reg::C],
            gb.cpu.registers[Reg::D],
            gb.cpu.registers[Reg::E],
            gb.cpu.registers[Reg::H],
            gb.cpu.registers[Reg::L],
            gb.cpu.registers[DReg::SP],
            address,
            gb.bus.read::<true>(cart, address),
            gb.bus.read::<true>(cart, address + 1),
            gb.bus.read::<true>(cart, address + 2),
            gb.bus.read::<true>(cart, address + 3)
        );
    }

    pub fn on_cycle(&mut self, result: CycleResult) {
        match result.kind {
            ExecutionType::Stop | ExecutionType::Halt => {
                return;
            }
            _ => (),
        }

        // if self.breakpoint.on_cycle(&result) {
        //     self.step = false;
        //     self.run = false;
        // }

        self.history.on_cycle(&result);
        self.serial.on_cycle();
    }

    pub fn window(
        &mut self,
        cart: &dyn Cartridge,
        gb: &mut GameboyColor,
        ctx: &egui::Context,
        window: PhysicalSize<u32>,
    ) {
        egui::Window::new(format!("Debug - {}", cart.title())).show(ctx, |ui| {
            ui.columns(3, |cols| {
                self.memory.window(&self.opcodes, gb, cart, &mut cols[0]);
                self.history
                    .window(&self.opcodes, gb, cart, window, &mut cols[1]);
                self.serial.window(&mut cols[2]);
                cols[2].separator();
                self.breakpoint.window(&mut cols[2]);
            });

            ui.separator();

            ui.columns(3, |buttons| {
                if buttons[0]
                    .add_enabled(true, egui::Button::new("Step").small())
                    .clicked()
                {
                    self.run = false;
                    self.step = true;
                }

                if buttons[1]
                    .add_enabled(true, egui::Button::new("Run").small().selected(self.run))
                    .clicked()
                {
                    self.run = !self.run;
                    self.step = self.run;
                }

                if buttons[2].add(egui::Button::new("Reset").small()).clicked() {
                    self.reset();
                    gb.reset(cart);
                }
            });

            ui.separator();
        });
    }

    pub const fn speed(&self) -> f64 {
        self.speed.multiplier()
    }

    pub fn should_step(&mut self, gb: &GameboyColor) -> bool {
        if self.run {
            self.breakpoint.should_step(&mut self.step, gb)
        } else if self.step {
            self.step = false;
            true
        } else {
            false
        }
    }

    pub fn new_cartridge(&mut self, cartridge: &(dyn Cartridge + 'static)) {
        self.memory.new_cartridge(cartridge);
        self.breakpoint.new_cartridge(cartridge);
        self.reset();
    }

    pub fn reset(&mut self) {
        self.run = false;
        self.step = false;
        self.serial.reset();
    }
}
