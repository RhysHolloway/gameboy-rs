use egui::Widget;
use gameboy_core::cpu::{DReg, Opcode};
use gameboy_core::{Address, Cartridge};

pub struct MemoryView {
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

impl MemoryView {
    pub fn window(
        &mut self,
        opcodes: &super::opcode::OpcodeTable,
        gb: &gameboy_core::GameboyColor,
        cart: &dyn gameboy_core::Cartridge,
        ui: &mut egui::Ui,
    ) {
        ui.columns(4, |cols| {
            cols[0].text_edit_singleline(&mut self.addr_text);

            if egui::Button::new("ADDR").ui(&mut cols[1]).clicked() {
                self.address = super::read_address(&self.addr_text).map(Address::new);
            }

            cols[2].text_edit_singleline(&mut self.size_text);

            if egui::Button::new("SIZE").ui(&mut cols[3]).clicked() {
                if let Ok(size) = self.size_text.parse::<usize>() {
                    self.size = size;
                }
            }
        });

        let mut address = self
            .address
            .unwrap_or_else(|| Address::new(gb.cpu.registers[DReg::PC]));

        for i in 0..self.size {
            let op = gb.bus.read::<true>(cart, address);
            let opcode = Opcode(op);
            let ptr = match i == 0 {
                true => "<-",
                false => "",
            };
            match opcodes.get(&opcode) {
                Some(desc) => {
                    egui::Label::new(format!(
                        "{address}\t{opcode},\t{}\t{ptr}",
                        desc.format(cart, &gb.bus, address)
                    ))
                    .wrap_mode(egui::TextWrapMode::Extend)
                    .ui(ui);
                    address += desc.length as u16;
                }
                None => {
                    egui::Label::new(format!("{address}\t{opcode},\tUnknown\t{ptr}"))
                        .wrap_mode(egui::TextWrapMode::Extend)
                        .ui(ui);
                    address += 1;
                }
            }
        }
    }

    pub(crate) fn new_cartridge(&self, _: &dyn Cartridge) {}
}
