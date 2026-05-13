use gameboy_core::GameboyColor;

pub struct VramView {}

impl VramView {
    pub fn window(&mut self, gb: &GameboyColor, ui: &mut egui::Ui) {
        // show tile data and tile maps
        ui.collapsing("VRAM", |ui| {});
    }
}
