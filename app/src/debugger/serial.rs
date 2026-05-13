use std::sync::mpsc::*;

use egui::Widget;
use gameboy_core::Cartridge;

struct SerialConnection {
    sender: Sender<u8>,
    receiver: Receiver<u8>,
    buffer: Vec<u8>,
}

#[derive(Default)]
pub struct SerialView {
    connection: Option<SerialConnection>,
}

impl SerialView {
    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    pub fn window(&mut self, ui: &mut egui::Ui) {
        ui.label("Serial I/O");
        ui.separator();

        match self.connection.as_ref() {
            Some(SerialConnection { buffer, .. }) => {
                if !buffer.is_empty() {
                    if buffer.len() < 128 {
                        egui::ScrollArea::vertical()
                            .id_salt("serbytes")
                            .show(ui, |ui| {
                                let bytes = buffer
                                    .iter()
                                    .fold(String::new(), |prev, next| format!("{prev}{next:02X}"));
                                egui::Label::new(format!("{}", bytes))
                                    .wrap_mode(egui::TextWrapMode::Wrap)
                                    .ui(ui);
                            });
                        ui.separator();
                    }

                    egui::ScrollArea::vertical()
                        .id_salt("sertext")
                        .show(ui, |ui| {
                            egui::Label::new(String::from_utf8_lossy(&buffer))
                                .wrap_mode(egui::TextWrapMode::Wrap)
                                .ui(ui);
                        });
                }
            }
            None => {
                ui.label("Not connected");
            }
        }
    }

    pub fn create_serial_callback(&mut self) -> Box<dyn FnMut(u8)> {
        let serial = self.connection.get_or_insert_with(|| {
            let (sender, receiver) = std::sync::mpsc::channel();
            SerialConnection {
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

    pub fn on_cycle(&mut self) {
        if let Some(SerialConnection {
            receiver, buffer, ..
        }) = self.connection.as_mut()
        {
            loop {
                match receiver.try_recv() {
                    Ok(byte) => buffer.push(byte),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.connection = None;
                        break;
                    }
                }
            }
        }
    }

    pub fn reset(&mut self) {
        if let Some(conn) = self.connection.as_mut() {
            conn.buffer.clear();
        }
    }
}
