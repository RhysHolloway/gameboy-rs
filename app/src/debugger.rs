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
mod opcode;

pub fn read_address(string: &str) -> Option<Width> {
    if string.starts_with("0x") {
        Width::from_str_radix(&string[2..], 16).ok()
    } else {
        Width::from_str_radix(string, 10).ok()
    }
}

pub struct Debugger {
    opcodes: HashMap<Opcode, OpcodeDescriptor>,
    breakpoint: breakpoint::BreakpointView,
    step: bool,
    run: bool,
    error: Option<String>,
    serial: Option<Serial>,
    memory: MemoryView,
    speed_text: String,
    speed: f64,
    history: VecDeque<ExecutionHistory>,
}

struct ExecutionHistory {
    kind: ExecutionType,
    count: usize,
}
struct MemoryView {
    addr_text: String,
    size_text: String,
    address: Option<Address>,
    size: usize,
}

impl Default for MemoryView {
    fn default() -> Self {
        Self {
            addr_text: String::new(),
            size_text: String::new(),
            address: None,
            size: 16,
        }
    }
}

struct Serial {
    sender: Sender<u8>,
    receiver: Receiver<u8>,
    buffer: Vec<u8>,
}

impl Debugger {
    pub fn new() -> Self {
        Self {
            opcodes: opcode::generate_table(),
            breakpoint: Default::default(),
            step: false,
            run: false,
            speed_text: String::new(),
            speed: 1.0,
            error: None,
            serial: None,
            memory: MemoryView::default(),
            history: VecDeque::new(),
        }
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

    pub fn create_serial_callback(&mut self) -> Box<dyn FnMut(u8)> {
        let serial = self.serial.get_or_insert_with(|| {
            let (sender, receiver) = std::sync::mpsc::channel();
            Serial {
                sender,
                receiver,
                buffer: Vec::new(),
            }
        });
        let sender = serial.sender.clone();
        serial.buffer.clear();
        Box::new(move |byte| {
            sender.send(byte).unwrap();
        })
    }

    pub fn on_cycle(&mut self, result: CycleResult) {
        match result.kind {
            ExecutionType::Stop | ExecutionType::Halt => {
                return;
            }
            _ => (),
        }

        if self.breakpoint.on_cycle(&result) {
            self.step = false;
            self.run = false;
        }

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
                if self.history.len() > 200 {
                    self.history.pop_front();
                }
            }
        }

        self.read_serial();
    }

    pub fn read_serial(&mut self) {
        if let Some(Serial {
            receiver, buffer, ..
        }) = self.serial.as_mut()
        {
            loop {
                match receiver.try_recv() {
                    Ok(byte) => buffer.push(byte),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.serial = None;
                        break;
                    }
                }
            }
        }
    }

    pub fn window(
        &mut self,
        cart: &dyn Cartridge,
        gb: &mut GameboyColor,
        ctx: &egui::Context,
        window: PhysicalSize<u32>,
    ) {
        egui::Window::new(format!("Debug - {}", cart.title())).show(ctx, |ui| {
            ui.columns(4, |cols| {
                // address space / error

                let opcol = &mut cols[0];

                opcol.columns(4, |cols| {
                    cols[0].text_edit_singleline(&mut self.memory.addr_text);

                    if egui::Button::new("ADDR").ui(&mut cols[1]).clicked() {
                        self.memory.address =
                            read_address(&self.memory.addr_text).map(Address::new);
                    }

                    cols[2].text_edit_singleline(&mut self.memory.size_text);

                    if egui::Button::new("SIZE").ui(&mut cols[3]).clicked() {
                        if let Ok(size) = self.memory.size_text.parse::<usize>() {
                            self.memory.size = size;
                        }
                    }
                });

                let mut address = self
                    .memory
                    .address
                    .unwrap_or_else(|| Address::new(gb.cpu.registers[DReg::PC]));

                for i in 0..self.memory.size {
                    let op = gb.bus.read::<true>(cart, address);
                    let opcode = Opcode(op);
                    let ptr = match i == 0 {
                        true => "<-",
                        false => "",
                    };
                    match self.opcodes.get(&opcode) {
                        Some(desc) => {
                            egui::Label::new(format!(
                                "{address}\t{opcode},\t{}\t{ptr}",
                                desc.format(cart, &gb.bus, address)
                            ))
                            .wrap_mode(egui::TextWrapMode::Extend)
                            .ui(opcol);
                            address += desc.length as u16;
                        }
                        None => {
                            egui::Label::new(format!("{address}\t{opcode},\tUnknown\t{ptr}"))
                                .wrap_mode(egui::TextWrapMode::Extend)
                                .ui(opcol);
                            address += 1;
                        }
                    }
                }

                opcol.separator();

                opcol.columns(2, |cols| {
                    cols[0].text_edit_singleline(&mut self.speed_text);

                    if egui::Button::new("Set speed").ui(&mut cols[1]).clicked() {
                        if let Ok(speed) = self.speed_text.parse::<f64>() {
                            self.speed = speed;
                        }
                    }
                });

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
                    cols[1].label(format!("HALT=\t{}", gb.bus.interrupts.halted()));

                    cols[0].label(format!("PC=\t{:#04X}", gb.cpu.registers[DReg::PC]));
                    cols[1].label(format!("DMA=\t{}", gb.bus.dma.is_active()));

                    cols[0].label(format!("IE=\t{:#05b}", gb.bus.interrupts.ie()));
                    cols[1].label(format!("IME=\t{}", gb.bus.interrupts.ime()));

                    cols[0].label(format!("DIV=\t{:#02X}", gb.bus.timer.div()));
                    cols[1].label(format!("TAC=\t{:#02X}", gb.bus.timer.tac()));

                    cols[0].label(format!("TIMA=\t{:#02X}", gb.bus.timer.tima()));
                    cols[1].label(format!("TMA=\t{:#02X}", gb.bus.timer.tma()));

                    // cols[0].label(format!("CONTR"));
                    // cols[1].label(format!("ROM=\t0x{:02X}", cart.rom_bank()));
                });

                let sercol = &mut cols[2];

                sercol.label("Serial I/O");
                sercol.separator();

                match self.serial.as_ref() {
                    Some(Serial { buffer, .. }) => {
                        if !buffer.is_empty() {
                            if buffer.len() < 128 {
                                egui::ScrollArea::vertical().id_salt("serbytes").show(
                                    sercol,
                                    |sercol| {
                                        let bytes =
                                            buffer.iter().fold(String::new(), |prev, next| {
                                                format!("{prev}{next:02X}")
                                            });
                                        egui::Label::new(format!("{}", bytes))
                                            .wrap_mode(egui::TextWrapMode::Wrap)
                                            .ui(sercol);
                                    },
                                );
                                sercol.separator();
                            }

                            egui::ScrollArea::vertical().id_salt("sertext").show(
                                sercol,
                                |sercol| {
                                    egui::Label::new(String::from_utf8_lossy(&buffer))
                                        .wrap_mode(egui::TextWrapMode::Wrap)
                                        .ui(sercol);
                                },
                            );
                        }
                    }
                    None => {
                        sercol.label("Not connected");
                    }
                }

                let bpcol = &mut cols[3];

                self.breakpoint.window(bpcol);

                bpcol.separator();

                egui::ScrollArea::vertical()
                    .id_salt("ophistory")
                    .max_height(window.height as f32 / 2.0)
                    .show(bpcol, |bpcol| {
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
                                        self.opcodes
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
            });

            ui.separator();

            ui.columns(3, |buttons| {
                let no_error = self.error.is_none();

                if buttons[0]
                    .add_enabled(no_error, egui::Button::new("Step").small())
                    .clicked()
                {
                    self.run = false;
                    self.step = true;
                }

                if buttons[1]
                    .add_enabled(
                        no_error,
                        egui::Button::new("Run").small().selected(self.run),
                    )
                    .clicked()
                {
                    self.run = !self.run;
                    self.step = self.run;
                }

                if buttons[2].add(egui::Button::new("Reset").small()).clicked() {
                    self.reset(gb, cart);
                }
            });

            ui.separator();

            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::RED, error);
            }
        });
    }

    pub fn speed(&self) -> f64 {
        self.speed
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

    pub fn reset(&mut self, gb: &mut GameboyColor, cart: &dyn Cartridge) {
        self.error = None;
        self.run = false;
        self.step = false;
        self.breakpoint = Default::default();
        if let Some(serial) = self.serial.as_mut() {
            serial.buffer.clear();
        }
        gb.reset(cart);
    }
}
