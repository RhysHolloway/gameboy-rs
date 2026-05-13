use gameboy_core::cpu::DReg;

pub struct StatusView;

impl StatusView {
    pub fn window(gb: &gameboy_core::GameboyColor, ui: &mut egui::Ui) {
        ui.label("Registers and I/O");
        ui.separator();

        ui.columns(2, |cols| {
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
    }
}
