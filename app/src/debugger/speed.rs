use egui::Widget;

pub struct Speed {
    input: String,
    multiplier: f64,
}

impl Default for Speed {
    fn default() -> Self {
        Self {
            input: 1.0.to_string(),
            multiplier: 1.0,
        }
    }
}

impl Speed {
    pub(crate) fn window(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            cols[0].text_edit_singleline(&mut self.input);
            if egui::Button::new("Set speed").ui(&mut cols[1]).clicked() {
                if let Ok(speed) = self.input.parse::<f64>() {
                    self.multiplier = speed;
                }
            }
        });
    }

    pub const fn multiplier(&self) -> f64 {
        self.multiplier
    }
}
